//! The shared asset library: the Asset Library window's Browse view (`rustpg2d-editor --library`,
//! see `library_window.rs`: import / export packages, remove, jump to editing) and the main
//! editor's "Add from library" dialog. Both use the same thumbnail grid.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use egui_macroquad::egui::{self, pos2, vec2, Color32, Rect, Sense, Stroke, StrokeKind, TextureHandle};
use rustpg2d_engine::defs::{Database, SpriteRef};
use rustpg2d_engine::imageio::Rgba;
use rustpg2d_engine::library::{self, kind_label, AssetRef, Bundle, KINDS};
use rustpg2d_engine::project::Project;
use rustpg2d_engine::refactor::RefKind;
use rustpg2d_engine::script_refs::ScriptFn;

use crate::app::Editor;
use crate::files::Purpose;
use crate::ui::{self, ph};

include!(concat!(env!("OUT_DIR"), "/starter_files.rs"));

/// Assets the library starts with: the files of `crates/editor/assets/starter_library` (the
/// example project's assets plus library-only extras), embedded by `build.rs`.
pub const STARTER: &rustpg2d_engine::library::LibraryFiles<'static> = STARTER_FILES;

struct Tex {
    handle: TextureHandle,
    w: f32,
    h: f32,
}

/// The "Add from library" dialog of the main editor.
#[derive(Default)]
pub struct AddDialog {
    /// Opened for one kind of asset (e.g. from the Tilesets tab); `None` = all kinds.
    pub kind: Option<RefKind>,
    /// Kind tab inside the dialog when it was opened for all kinds.
    pub filter: Option<RefKind>,
    pub picked: BTreeSet<AssetRef>,
    pub search: String,
}

/// Where project assets go once their script functions are picked.
#[derive(Clone, Debug, PartialEq)]
pub enum SendTo {
    Library,
    /// A `.r2dpack` file.
    Package(PathBuf),
}

/// The "Script functions" dialog: project assets that use scripts are about to be copied into
/// the library or a package, and the user picks which functions go along.
pub struct ScriptPick {
    pub assets: Vec<AssetRef>,
    pub to: SendTo,
    /// Every function of the project's scripts.
    pub fns: Vec<ScriptFn>,
    pub picked: BTreeSet<String>,
    /// Functions the assets need, with what needs them ("Spell 'Fireball'", "cast_fireball()").
    pub needed: BTreeMap<String, String>,
    /// Functions the assets call that no script defines: (function, who calls it).
    pub missing: Vec<(String, String)>,
    /// Functions the library has already: name -> identical?
    pub in_library: BTreeMap<String, bool>,
}

#[derive(Default)]
pub struct LibState {
    /// The library as a project-like store (main editor: loaded for the add dialog and the
    /// model chooser; the Asset Library window uses `Editor::project` instead).
    pub lib: Option<Project>,
    pub error: Option<String>,
    tex: HashMap<String, Tex>,
    /// The library's 3D models (loaded on first use: character thumbnails).
    models: std::cell::RefCell<Option<rustpg2d_engine::render::model::Models>>,
    /// The folder they are read from: the shown store's (the library, or in the Asset Library
    /// window the project it edits).
    models_root: Option<std::path::PathBuf>,
    pub tex_dirty: bool,
    pub filter: Option<RefKind>,
    pub search: String,
    pub selected: BTreeSet<AssetRef>,
    pub report: Option<String>,
    /// Assets waiting for the user to confirm their removal (the confirmation dialog is open).
    pub confirm_remove: Option<Vec<AssetRef>>,
    pub add: Option<AddDialog>,
    pub script_pick: Option<ScriptPick>,
}

impl LibState {
    pub fn ensure_loaded(&mut self) {
        if self.lib.is_some() || self.error.is_some() {
            return;
        }
        match library::open_library(&library::library_dir(), Some(STARTER)) {
            Ok(p) => {
                self.lib = Some(p);
                self.tex_dirty = true;
            }
            Err(e) => self.error = Some(format!("Cannot open the asset library at {}: {e:#}", library::library_dir().display())),
        }
    }

    /// Reads the library from disk again (it may have been changed in the Asset Library window).
    pub fn reload(&mut self) {
        self.lib = None;
        self.error = None;
        self.ensure_loaded();
        if let Some(lib) = &self.lib {
            self.selected.retain(|a| library::exists(&lib.db, a));
        }
    }

    pub(crate) fn textures(&mut self, ctx: &egui::Context) {
        if let Some(lib) = self.lib.take() {
            self.textures_of(ctx, &lib);
            self.lib = Some(lib);
        }
    }

    /// Thumbnail textures of a store's tilesets and sprite sheets (rebuilt when `tex_dirty`).
    /// Called every frame with the store on show: loads its pictures when they changed, and starts
    /// the frame of its models (they render a few thumbnails a frame, more in the next ones).
    pub(crate) fn textures_of(&mut self, ctx: &egui::Context, store: &Project) {
        if self.models_root.as_deref() != Some(store.root.as_path()) {
            self.models_root = Some(store.root.clone());
            *self.models.borrow_mut() = None;
        }
        if let Some(m) = self.models.borrow().as_ref() {
            m.begin_frame();
        }
        if !self.tex_dirty {
            return;
        }
        self.tex_dirty = false;
        self.tex.clear();
        *self.models.borrow_mut() = None;
        let images = store.db.tilesets.iter().map(|t| (format!("ts:{}", t.id), &t.image)).chain(store.db.sheets.iter().map(|s| (format!("sh:{}", s.id), &s.image)));
        for (key, rel) in images {
            let Ok(img) = std::fs::read(store.root.join(rel)).map_err(anyhow::Error::from).and_then(|b| Rgba::decode(&b)) else { continue };
            let ci = crate::widgets::color_image(ctx, img.w as usize, img.h as usize, &img.px);
            let handle = ctx.load_texture(format!("lib:{key}"), ci, egui::TextureOptions::NEAREST);
            self.tex.insert(key.clone(), Tex { handle, w: img.w as f32, h: img.h as f32 });
        }
    }

    /// Paints a library character facing `facing` into `rect` (idle frame).
    pub(crate) fn paint_character(&self, ui: &egui::Ui, id: &str, facing: rustpg2d_engine::entity::Facing, rect: Rect) -> bool {
        self.lib.as_ref().is_some_and(|lib| self.paint_character_in(ui, &lib.db, id, facing, rect))
    }

    /// Paints a character of `db` (its model, rendered from the shown store's files) into `rect`,
    /// standing facing `facing`.
    fn paint_character_in(&self, ui: &egui::Ui, db: &Database, id: &str, facing: rustpg2d_engine::entity::Facing, rect: Rect) -> bool {
        use rustpg2d_engine::render::model::{Gear, Models, PoseReq};
        let Some(d) = db.character(id) else { return false };
        let mut m = self.models.borrow_mut();
        if m.is_none() {
            let Some(root) = self.models_root.as_ref() else { return false };
            *m = Some(Models::load(db, &|rel| std::fs::read(root.join(rel))));
        }
        let Some(models) = m.as_ref() else { return false };
        let a = models.look_of(&d.appearance);
        let gear = Gear::default();
        let req = PoseReq { look: &a, gear: &gear, anim: "idle", time: 0.0, once: true, facing, bust: false };
        crate::widgets::paint_pose(ui, models, db, &HashMap::new(), &req, rect)
    }

    fn paint_sprite_in(&self, ui: &egui::Ui, db: &Database, sprite: &SpriteRef, rect: Rect) -> bool {
        let (Some(t), Some(s)) = (self.tex.get(&format!("sh:{}", sprite.sheet)), db.sheet(&sprite.sheet)) else { return false };
        let (fw, fh) = (s.frame_w.max(1) as f32, s.frame_h.max(1) as f32);
        let cols = (t.w / fw).floor().max(1.0) as u32;
        let (cw, ch) = sprite.cells();
        let (x, y) = ((sprite.frame % cols) as f32 * fw, (sprite.frame / cols) as f32 * fh);
        let (w, h) = (fw * cw as f32, fh * ch as f32);
        let uv = Rect::from_min_max(pos2(x / t.w, y / t.h), pos2((x + w) / t.w, (y + h) / t.h));
        let scale = (rect.width() / w).min(rect.height() / h);
        let size = vec2(w * scale, h * scale);
        let r = Rect::from_min_size(pos2(rect.center().x - size.x / 2.0, rect.max.y - size.y), size);
        ui.painter().image(t.handle.id(), r, uv, Color32::WHITE);
        true
    }

    fn paint_thumb(&self, ui: &egui::Ui, db: &Database, a: &AssetRef, rect: Rect) {
        if a.0 == RefKind::Structure {
            if let Some(d) = db.structure(&a.1) {
                let tileset = |id: &str| self.tex.get(&format!("ts:{id}")).map(|t| (t.handle.id(), vec2(t.w, t.h)));
                let sprite = |s: &SpriteRef, r: Rect| self.paint_sprite_in(ui, db, s, r);
                crate::structures::paint_structure(ui, rect, d, db, &tileset, &sprite);
            }
            return;
        }
        let sprite = match a.0 {
            RefKind::Item => db.item(&a.1).map(|d| d.icon.clone()),
            RefKind::Prop => db.prop(&a.1).map(|d| d.sprite.clone()),
            RefKind::Character => None,
            RefKind::Sheet => Some(SpriteRef::new(&a.1, 0)),
            RefKind::Skill => db.skill(&a.1).map(|d| d.icon.clone()).filter(|i| !i.sheet.is_empty()),
            RefKind::Spell => db.spell(&a.1).map(|d| d.icon.clone()).filter(|i| !i.sheet.is_empty()),
            // A recipe shows what it makes.
            RefKind::Recipe => db.recipe(&a.1).and_then(|r| db.item(&r.result)).map(|d| d.icon.clone()),
            // A race shows its first body.
            RefKind::Race => db.race(&a.1).and_then(|r| r.bodies_male.first().or(r.bodies_female.first())).map(|b| SpriteRef::new(b, 0)),
            _ => None,
        };
        let drawn = match (a.0, sprite) {
            (RefKind::Tileset, _) => self.tex.get(&format!("ts:{}", a.1)).is_some_and(|t| {
                let scale = (rect.width() / t.w).min(rect.height() / t.h);
                let r = Rect::from_center_size(rect.center(), vec2(t.w, t.h) * scale);
                ui.painter().image(t.handle.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                true
            }),
            (RefKind::Character, _) => self.paint_character_in(ui, db, &a.1, rustpg2d_engine::entity::Facing::Down, rect),
            (_, Some(s)) => self.paint_sprite_in(ui, db, &s, rect),
            _ => false,
        };
        if !drawn {
            let glyph = if a.0 == RefKind::Sound { "\u{E33C}" } else { "?" };
            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, glyph, egui::FontId::proportional(28.0), Color32::GRAY);
        }
    }

    /// A wrapping grid of asset cards (thumbnail, name, kind; `note` adds a small tag like
    /// "in project"). Returns the clicked and the double-clicked card.
    fn grid(&self, ui: &mut egui::Ui, db: &Database, shown: &[AssetRef], on: &BTreeSet<AssetRef>, note: &dyn Fn(&AssetRef) -> Option<String>) -> (Option<AssetRef>, Option<AssetRef>) {
        let (mut clicked, mut double) = (None, None);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
            for a in shown {
                let (rect, resp) = ui.allocate_exact_size(vec2(118.0, 112.0), Sense::click());
                let sel = on.contains(a);
                let bg = if sel { Color32::from_rgb(52, 64, 90) } else if resp.hovered() { Color32::from_gray(48) } else { Color32::from_gray(38) };
                ui.painter().rect_filled(rect, 4.0, bg);
                ui.painter().rect_stroke(rect, 4.0, Stroke::new(if sel { 2.0_f32 } else { 1.0 }, if sel { Color32::YELLOW } else { Color32::from_gray(60) }), StrokeKind::Inside);
                let thumb = Rect::from_min_size(rect.min + vec2(23.0, 6.0), vec2(72.0, 64.0));
                self.paint_thumb(ui, db, a, thumb);
                let name = name_of(db, a);
                let short: String = if name.chars().count() > 16 { name.chars().take(15).collect::<String>() + "…" } else { name.clone() };
                ui.painter().text(pos2(rect.center().x, rect.min.y + 84.0), egui::Align2::CENTER_CENTER, short, egui::FontId::proportional(13.0), Color32::WHITE);
                let sub = match note(a) {
                    Some(n) => format!("{} · {n}", kind_label(a.0)),
                    None => kind_label(a.0).to_string(),
                };
                ui.painter().text(pos2(rect.center().x, rect.min.y + 100.0), egui::Align2::CENTER_CENTER, sub, egui::FontId::proportional(11.0), Color32::GRAY);
                let deps: Vec<String> = library::deps(db, a).into_iter().map(|d| format!("{} {}", kind_label(d.0).to_lowercase(), d.1)).collect();
                let tip = if deps.is_empty() { format!("{name}\nid: {}", a.1) } else { format!("{name}\nid: {}\nalso brings: {}", a.1, deps.join(", ")) };
                let resp = resp.on_hover_text(tip);
                if resp.double_clicked() {
                    double = Some(a.clone());
                } else if resp.clicked() {
                    clicked = Some(a.clone());
                }
            }
        });
        (clicked, double)
    }
}

/// Display name of an asset (items, props and characters have one; others show their id).
fn name_of(db: &Database, a: &AssetRef) -> String {
    match a.0 {
        RefKind::Item => db.item(&a.1).map(|d| d.name.clone()),
        RefKind::Prop => db.prop(&a.1).map(|d| d.name.clone()),
        RefKind::Character => db.character(&a.1).map(|d| d.name.clone()),
        RefKind::Skill => db.skill(&a.1).map(|d| d.name.clone()),
        RefKind::Spell => db.spell(&a.1).map(|d| d.name.clone()),
        RefKind::Recipe => db.recipe(&a.1).and_then(|r| db.item(&r.result)).map(|d| format!("{} (recipe)", d.name)),
        RefKind::Race => db.race(&a.1).map(|d| format!("{} (race)", d.name)),
        _ => None,
    }
    .unwrap_or_else(|| a.1.clone())
}

/// Assets of `db` of the kind `k` (all with `None`) matching a search text (id or name).
fn filtered(db: &Database, k: Option<RefKind>, search: &str) -> Vec<AssetRef> {
    let q = search.trim().to_lowercase();
    library::all_assets(db)
        .into_iter()
        .filter(|a| k.is_none_or(|k| k == a.0))
        .filter(|a| q.is_empty() || a.1.to_lowercase().contains(&q) || name_of(db, a).to_lowercase().contains(&q))
        .collect()
}

/// "tilesets", "sprite sheets", …
fn plural(k: RefKind) -> String {
    format!("{}s", kind_label(k).to_lowercase())
}

/// Parses "item", "prop", "character", "sheet", "tileset", "sound", "structure", "skill",
/// "spell" or "recipe": the kinds a library holds.
pub fn parse_kind(s: &str) -> Option<RefKind> {
    RefKind::parse(s).filter(|k| library::KINDS.contains(k))
}

fn with_ext(path: &Path) -> std::path::PathBuf {
    if path.extension().is_some_and(|e| e == library::PACK_EXT) {
        path.to_path_buf()
    } else {
        let mut p = path.as_os_str().to_owned();
        p.push(format!(".{}", library::PACK_EXT));
        p.into()
    }
}

impl Editor {
    /// The library as a store: the open project in the Asset Library window, else the
    /// library loaded next to the project.
    fn lib_store_mut(&mut self) -> Option<&mut Project> {
        if self.library_mode {
            Some(&mut self.project)
        } else {
            self.lib.ensure_loaded();
            self.lib.lib.as_mut()
        }
    }

    fn lib_store(&mut self) -> Option<&Project> {
        self.lib_store_mut().map(|p| &*p)
    }

    /// Adds library assets (with their dependencies) to the project.
    pub fn library_to_project(&mut self, assets: &[AssetRef]) -> Result<String> {
        self.lib.ensure_loaded();
        let lib = self.lib.lib.as_ref().context("library not available")?;
        let mut b = Bundle::collect_with_needed_scripts(&lib.db, &lib.root, rustpg2d_engine::project::TILE_SIZE, "library", assets)?;
        b.rescale(rustpg2d_engine::project::TILE_SIZE)?;
        let root = self.project.root.clone();
        let report = b.merge_into(&mut self.project.db, &root)?;
        self.defs_dirty = true;
        self.after_scripts_merged();
        self.reload_assets();
        Ok(format!("Added to the project: {}. Save the definitions to keep them.", report.summary()))
    }

    /// Sends project assets (with their dependencies and the script functions they need) to the
    /// asset library without asking: the AI agent server's `library_send`. `replace` overwrites
    /// the library's version of assets it already has (updating a library from the project its
    /// assets come from, e.g. the starter library from the example project) instead of adding
    /// them under new ids; `drop_script_triggers` leaves triggers that run the project's
    /// script functions behind.
    pub fn project_to_library(&mut self, assets: &[AssetRef], replace: bool, drop_script_triggers: bool) -> Result<String> {
        let b = Bundle::collect_for_library(&self.project.db, &self.project.root, rustpg2d_engine::project::TILE_SIZE, &self.project.config.name, assets, drop_script_triggers)?;
        self.lib.ensure_loaded();
        let lib = self.lib.lib.as_mut().with_context(|| self.lib.error.clone().unwrap_or_else(|| "library not available".into()))?;
        let report = if replace {
            let root = lib.root.clone();
            let r = b.replace_into(&mut lib.db, &root)?;
            lib.save_defs()?;
            r
        } else {
            library::add_to_library(lib, b)?
        };
        self.lib.tex_dirty = true;
        Ok(format!("Library ({}): {}", lib.root.display(), report.summary()))
    }

    /// Copies project assets (with their dependencies) into the library or a package. When
    /// they use scripts, the "Script functions" dialog asks which functions go along first.
    pub fn send_project_assets(&mut self, assets: Vec<AssetRef>, to: SendTo) {
        match self.script_pick_for(&assets, &to) {
            Ok(Some(pick)) => self.lib.script_pick = Some(pick),
            Ok(None) => {
                let r = self.finish_send(&assets, &to, &[]);
                self.show_result(r);
            }
            Err(e) => self.show_result(Err(e)),
        }
    }

    /// The dialog's contents, or `None` if the assets (and what they bring) use no scripts.
    fn script_pick_for(&mut self, assets: &[AssetRef], to: &SendTo) -> Result<Option<ScriptPick>> {
        let b = Bundle::collect(&self.project.db, &self.project.root, rustpg2d_engine::project::TILE_SIZE, "", assets)?;
        let fns = library::read_scripts(&self.project.root);
        let names: BTreeSet<String> = fns.iter().map(|f| f.name.clone()).collect();
        let mut needed: BTreeMap<String, String> = BTreeMap::new();
        let mut missing = Vec::new();
        let mut stack = Vec::new();
        for a in library::all_assets(&b.db) {
            for root in library::script_roots(&b.db, &a) {
                let who = format!("{} '{}'", kind_label(a.0), name_of(&b.db, &a));
                if !names.contains(&root) {
                    missing.push((root, who));
                } else if !needed.contains_key(&root) {
                    needed.insert(root.clone(), who);
                    stack.push(root);
                }
            }
        }
        // What those functions call or pass on (aim, fly, then_call), transitively.
        while let Some(n) = stack.pop() {
            let Some(f) = fns.iter().find(|f| f.name == n) else { continue };
            for u in rustpg2d_engine::script_refs::used_functions(f, &names) {
                if !needed.contains_key(&u) {
                    needed.insert(u.clone(), format!("{n}()"));
                    stack.push(u);
                }
            }
        }
        if needed.is_empty() && missing.is_empty() {
            return Ok(None);
        }
        let mut in_library = BTreeMap::new();
        if *to == SendTo::Library {
            self.lib.ensure_loaded();
            if let Some(lib) = &self.lib.lib {
                for f in library::read_scripts(&lib.root) {
                    if let Some(mine) = fns.iter().find(|m| m.name == f.name) {
                        in_library.insert(f.name.clone(), mine.text.trim() == f.text.trim());
                    }
                }
            }
        }
        let picked = needed.keys().cloned().collect();
        Ok(Some(ScriptPick { assets: assets.to_vec(), to: to.clone(), fns, picked, needed, missing, in_library }))
    }

    /// Copies the assets plus the picked script functions (and the assets those refer to).
    fn finish_send(&mut self, assets: &[AssetRef], to: &SendTo, fns: &[ScriptFn]) -> Result<String> {
        let b = Bundle::collect_with_scripts(&self.project.db, &self.project.root, rustpg2d_engine::project::TILE_SIZE, &self.project.config.name, assets, fns)?;
        let n_fns = b.scripts.len();
        match to {
            SendTo::Library => {
                self.lib.ensure_loaded();
                let lib = self.lib.lib.as_mut().context("library not available")?;
                let report = library::add_to_library(lib, b)?;
                self.lib.tex_dirty = true;
                Ok(format!("Library: {}", report.summary()))
            }
            SendTo::Package(path) => {
                let path = with_ext(path);
                std::fs::write(&path, b.to_pack()?).with_context(|| format!("writing {}", path.display()))?;
                Ok(format!("Exported {} asset(s) and {n_fns} script function(s) to {}", library::all_assets(&b.db).len(), path.display()))
            }
        }
    }

    /// Ends the "Script functions" dialog: `ok` copies with the picked functions.
    pub fn close_script_pick(&mut self, ok: bool) {
        let Some(p) = self.lib.script_pick.take() else { return };
        if ok {
            let chosen: Vec<ScriptFn> = p.fns.iter().filter(|f| p.picked.contains(&f.name)).cloned().collect();
            let r = self.finish_send(&p.assets, &p.to, &chosen);
            self.show_result(r);
        }
    }

    /// Exports library assets (with the script functions they need) to a package.
    pub fn export_package(&mut self, assets: &[AssetRef], path: &Path) -> Result<String> {
        let lib = self.lib_store().context("library not available")?;
        let b = Bundle::collect_with_needed_scripts(&lib.db, &lib.root, rustpg2d_engine::project::TILE_SIZE, "R(ust)PG2D assets", assets)?;
        let path = with_ext(path);
        std::fs::write(&path, b.to_pack()?).with_context(|| format!("writing {}", path.display()))?;
        Ok(format!("Exported {} asset(s) and {} script function(s) to {}", library::all_assets(&b.db).len(), b.scripts.len(), path.display()))
    }

    /// The "Script functions" dialog (see [`ScriptPick`]).
    pub(crate) fn script_pick_dialog(&mut self, ctx: &egui::Context) {
        let Some(p) = &mut self.lib.script_pick else { return };
        let mut choice: Option<bool> = None;
        let to_lib = p.to == SendTo::Library;
        let modal = egui::Modal::new(egui::Id::new("lib_script_pick")).show(ctx, |ui| {
            ui.set_width(560.0);
            ui.heading(ui::t(ph::CODE, if to_lib { "Script functions for the library" } else { "Script functions for the package" }));
            ui.label("These assets use scripts. Only the functions ticked here are copied with them; the rest of your scripts stay in the project.");
            ui.add_space(6.0);
            let row = |ui: &mut egui::Ui, f: &ScriptFn, picked: &mut BTreeSet<String>, why: Option<&String>, lib: Option<&bool>| {
                ui.horizontal(|ui| {
                    let mut on = picked.contains(&f.name);
                    let label = egui::RichText::new(format!("{}({})", f.name, f.params.join(", "))).monospace();
                    let resp = ui.checkbox(&mut on, label);
                    resp.on_hover_ui(|ui| {
                        ui.label(egui::RichText::new(f.text.trim()).monospace().size(11.0));
                    });
                    if on {
                        picked.insert(f.name.clone());
                    } else {
                        picked.remove(&f.name);
                    }
                    if let Some(w) = why {
                        ui.weak(format!("used by {w}"));
                    }
                    match lib {
                        Some(true) => {
                            ui.weak("· already in the library");
                        }
                        Some(false) => {
                            ui.colored_label(Color32::YELLOW, "· the library has a different version (kept)");
                        }
                        None => {}
                    }
                });
            };
            egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                ui.strong("Needed by these assets");
                for f in p.fns.iter().filter(|f| p.needed.contains_key(&f.name)) {
                    row(ui, f, &mut p.picked, p.needed.get(&f.name), p.in_library.get(&f.name));
                }
                for (m, who) in &p.missing {
                    ui.colored_label(Color32::LIGHT_RED, format!("{who} calls {m}(), which no script of the project defines."));
                }
                let others: Vec<&ScriptFn> = p.fns.iter().filter(|f| !p.needed.contains_key(&f.name)).collect();
                if !others.is_empty() {
                    ui.add_space(4.0);
                    egui::CollapsingHeader::new(format!("Other functions of the project ({})", others.len())).id_salt("lib_pick_others").show(ui, |ui| {
                        ui.weak("Not used by these assets. Tick one only if you want to share it too.");
                        for f in others {
                            row(ui, f, &mut p.picked, None, p.in_library.get(&f.name));
                        }
                    });
                }
            });
            let left_out: Vec<&String> = p.needed.keys().filter(|n| !p.picked.contains(*n)).collect();
            if !left_out.is_empty() {
                let list: Vec<String> = left_out.iter().map(|n| format!("{n}()")).collect();
                ui.colored_label(Color32::YELLOW, format!("Not copied although needed: {}. The assets only work where those functions exist.", list.join(", ")));
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let n = p.picked.len();
                let go = if to_lib { format!("Add to library ({n} function{})", if n == 1 { "" } else { "s" }) } else { format!("Export ({n} function{})", if n == 1 { "" } else { "s" }) };
                if ui.button(ui::t(if to_lib { ph::UPLOAD_SIMPLE } else { ph::PACKAGE }, &go)).clicked() {
                    choice = Some(true);
                }
                if ui.button("Cancel").clicked() {
                    choice = Some(false);
                }
            });
        });
        if modal.should_close() && choice.is_none() {
            choice = Some(false);
        }
        if let Some(ok) = choice {
            self.close_script_pick(ok);
        }
    }

    pub fn import_package(&mut self, into_library: bool, path: &Path) -> Result<String> {
        let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
        let mut b = Bundle::from_pack(&bytes)?;
        if into_library {
            let lib = self.lib_store_mut().context("library not available")?;
            let report = library::add_to_library(lib, b)?;
            self.after_library_write();
            Ok(format!("Imported into the library: {}", report.summary()))
        } else {
            b.rescale(rustpg2d_engine::project::TILE_SIZE)?;
            let root = self.project.root.clone();
            let report = b.merge_into(&mut self.project.db, &root)?;
            self.defs_dirty = true;
            self.after_scripts_merged();
            self.reload_assets();
            Ok(format!("Imported into the project: {}. Save the definitions to keep them.", report.summary()))
        }
    }

    /// A merge may have appended functions to the project's scripts/library.rhai: an unchanged
    /// open copy of it in the script editor is reloaded.
    fn after_scripts_merged(&mut self) {
        let path = self.project.scripts_dir().join(library::LIBRARY_SCRIPT);
        if self.db.script_file.as_deref() == Some(path.as_path()) && !self.db.script_dirty {
            self.db.script_text = std::fs::read_to_string(&path).unwrap_or_default();
            self.db.script_saved = self.db.script_text.clone();
        }
    }

    /// The library's definitions were just written to disk (import / remove save them).
    fn after_library_write(&mut self) {
        self.lib.tex_dirty = true;
        if self.library_mode {
            self.mark_library_saved();
            self.reload_assets();
        }
    }

    /// Removes assets from the library right away (after the confirmation dialog).
    pub fn remove_from_library(&mut self, assets: &[AssetRef]) {
        let r = match self.lib_store_mut() {
            Some(lib) => library::remove_assets(lib, assets).map(|_| format!("Removed {} asset(s) from the library", assets.len())),
            None => Ok(String::new()),
        };
        if r.is_ok() {
            self.lib.selected.clear();
            self.after_library_write();
            self.db.sel = 0;
        }
        self.show_result(r);
    }

    /// The "Remove from library?" dialog: lists the assets and what still uses them.
    pub(crate) fn confirm_remove_dialog(&mut self, ctx: &egui::Context) {
        let Some(list) = self.lib.confirm_remove.clone() else { return };
        let unsaved = self.library_mode && self.defs_dirty;
        let Some(lib) = self.lib_store() else {
            self.lib.confirm_remove = None;
            return;
        };
        // Remaining library assets that depend on the ones to remove (removal would be refused).
        let mut users: Vec<String> = list
            .iter()
            .flat_map(|a| library::dependents(&lib.db, a))
            .filter(|u| !list.contains(u))
            .map(|u| format!("{} '{}'", kind_label(u.0), u.1))
            .collect();
        users.sort();
        users.dedup();
        let mut choice: Option<bool> = None;
        let modal = egui::Modal::new(egui::Id::new("lib_confirm_remove")).show(ctx, |ui| {
            ui.set_max_width(460.0);
            ui.heading(format!("Remove {} asset(s) from the library?", list.len()));
            ui.add_space(4.0);
            egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                for a in &list {
                    ui.label(format!("• {} '{}'", kind_label(a.0), a.1));
                }
            });
            ui.add_space(4.0);
            if users.is_empty() {
                ui.label("Their definitions and image / sound files are deleted from the library folder. This can't be undone. Projects that already use them keep their own copies.");
                if unsaved {
                    ui.colored_label(Color32::YELLOW, "Your other unsaved library edits are saved as well.");
                }
            } else {
                ui.colored_label(Color32::LIGHT_RED, format!("Can't remove: still used by {}. Select those too, or remove them first.", users.join(", ")));
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let remove = egui::Button::new(egui::RichText::new(ui::t(ph::TRASH, "Remove")).color(Color32::WHITE)).fill(Color32::from_rgb(150, 40, 40));
                if ui.add_enabled(users.is_empty(), remove).clicked() {
                    choice = Some(true);
                }
                if ui.button("Cancel").clicked() {
                    choice = Some(false);
                }
            });
        });
        if modal.should_close() && choice.is_none() {
            choice = Some(false); // Esc or a click outside cancels
        }
        match choice {
            Some(true) => {
                self.lib.confirm_remove = None;
                self.remove_from_library(&list);
            }
            Some(false) => self.lib.confirm_remove = None,
            None => {}
        }
    }

    /// Browse view of the Asset Library window: all library assets, import / export / remove,
    /// and a jump to editing an asset.
    pub fn library_ui(&mut self, ctx: &egui::Context) {
        self.lib.textures_of(ctx, &self.project);
        let mut edit: Option<AssetRef> = None;
        let mut new_structure = false;
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Asset library");
                ui.weak(self.project.root.display().to_string());
            });
            let sel: Vec<AssetRef> = self.lib.selected.iter().cloned().collect();
            let any = !sel.is_empty();
            ui.horizontal_wrapped(|ui| {
                if ui.add_enabled(sel.len() == 1, egui::Button::new(ui::t(ph::PENCIL_SIMPLE, "Edit"))).on_hover_text("Edit the selected asset (or double-click it)").clicked() {
                    edit = sel.first().cloned();
                }
                if ui.button(ui::t(ph::SPARKLE, "New asset…")).on_hover_text("Create a prop, item, character, tileset, sprite sheet or structure in the library").clicked() {
                    self.wizard = Some(crate::wizard::Wizard::new(&self.project));
                }
                if ui.button(ui::t(ph::HOUSE, "New structure")).on_hover_text("A multi-tile structure (house, wall, room…), assembled from tileset stamps").clicked() {
                    new_structure = true;
                }
                ui.separator();
                if ui.button(ui::t(ph::DOWNLOAD_SIMPLE, "Import package…")).on_hover_text("Adds a .r2dpack file to the library").clicked() {
                    self.open_file_dialog(Purpose::ImportPackage { into_library: true });
                }
                if ui.add_enabled(any, egui::Button::new(ui::t(ph::PACKAGE, "Export package…"))).on_hover_text("Saves the selected assets and everything they need as a .r2dpack file").clicked() {
                    self.open_file_dialog(Purpose::ExportPackage { from_library: true, assets: sel.clone() });
                }
                if ui.add_enabled(any, egui::Button::new(ui::t(ph::TRASH, "Remove from library…"))).on_hover_text("Deletes the selected assets and their files from the library (asks first)").clicked() {
                    self.lib.confirm_remove = Some(sel.clone());
                }
            });
            ui.horizontal_wrapped(|ui| {
                if ui.selectable_label(self.lib.filter.is_none(), "All").clicked() {
                    self.lib.filter = None;
                }
                for k in KINDS {
                    if ui.selectable_label(self.lib.filter == Some(k), ui::t(ui::kind_icon(k), &format!("{}s", kind_label(k)))).clicked() {
                        self.lib.filter = Some(k);
                    }
                }
                ui.separator();
                ui.label(ph::MAGNIFYING_GLASS);
                ui.add(egui::TextEdit::singleline(&mut self.lib.search).hint_text("search").desired_width(160.0));
            });
            let db = &self.project.db;
            let shown = filtered(db, self.lib.filter, &self.lib.search);
            ui.horizontal(|ui| {
                ui.label(format!("{} shown · {} selected", shown.len(), self.lib.selected.len()));
                if ui.small_button("Select shown").clicked() {
                    self.lib.selected.extend(shown.iter().cloned());
                }
                if ui.small_button("Clear selection").clicked() {
                    self.lib.selected.clear();
                }
            });
            if let Some(r) = &self.lib.report {
                ui.colored_label(if r.starts_with("Error") { Color32::LIGHT_RED } else { Color32::LIGHT_GREEN }, r);
            }
            ui.separator();
            let (clicked, double) = egui::ScrollArea::vertical().id_salt("lib_grid").auto_shrink(false).show(ui, |ui| self.lib.grid(ui, db, &shown, &self.lib.selected, &|_| None)).inner;
            if let Some(a) = clicked {
                if !self.lib.selected.remove(&a) {
                    self.lib.selected.insert(a);
                }
            }
            if double.is_some() {
                edit = double;
            }
        });
        if let Some(a) = edit {
            self.edit_library_asset(&a);
        }
        if new_structure {
            self.new_structure();
        }
    }

    /// Opens the "Add from library" dialog (re-reading the library, which the Asset Library
    /// window may have changed). `kind` limits it to one kind of asset.
    pub fn open_add_dialog(&mut self, kind: Option<RefKind>) {
        self.lib.reload();
        self.lib.tex_dirty = true;
        self.lib.add = Some(AddDialog { kind, ..Default::default() });
    }

    /// Adds the assets picked in the "Add from library" dialog to the project and closes it.
    pub fn add_picked_from_library(&mut self) {
        let Some(d) = self.lib.add.take() else { return };
        let picked: Vec<AssetRef> = d.picked.into_iter().collect();
        if picked.is_empty() {
            return;
        }
        let r = self.library_to_project(&picked);
        self.show_result(r);
    }

    /// The main editor's "Add from library" dialog.
    pub fn add_from_library_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut d) = self.lib.add.take() else { return };
        self.lib.textures(ctx);
        let (mut close, mut add) = (false, false);
        let width = (ctx.screen_rect().width() * 0.8).clamp(400.0, 900.0);
        let height = ctx.screen_rect().height() * 0.5;
        let modal = egui::Modal::new(egui::Id::new("lib_add")).show(ctx, |ui| {
            ui.set_width(width);
            ui.heading(match d.kind {
                Some(k) => format!("Add {} from the library", plural(k)),
                None => "Add from the library".to_string(),
            });
            let Some(lib) = &self.lib.lib else {
                ui.colored_label(Color32::LIGHT_RED, self.lib.error.clone().unwrap_or_else(|| "The asset library is not available.".into()));
                close = ui.button("Close").clicked();
                return;
            };
            ui.horizontal_wrapped(|ui| {
                if d.kind.is_none() {
                    if ui.selectable_label(d.filter.is_none(), "All").clicked() {
                        d.filter = None;
                    }
                    for k in KINDS {
                        if ui.selectable_label(d.filter == Some(k), ui::t(ui::kind_icon(k), &format!("{}s", kind_label(k)))).clicked() {
                            d.filter = Some(k);
                        }
                    }
                    ui.separator();
                }
                ui.label(ph::MAGNIFYING_GLASS);
                ui.add(egui::TextEdit::singleline(&mut d.search).hint_text("search").desired_width(160.0));
            });
            let shown = filtered(&lib.db, d.kind.or(d.filter), &d.search);
            ui.horizontal(|ui| {
                ui.label(format!("{} in the library · {} picked", shown.len(), d.picked.len()));
                if ui.small_button("Pick all shown").clicked() {
                    d.picked.extend(shown.iter().cloned());
                }
                if ui.small_button("Clear").clicked() {
                    d.picked.clear();
                }
            });
            ui.separator();
            let project = &self.project.db;
            let note = |a: &AssetRef| library::exists(project, a).then(|| "in project".to_string());
            let (clicked, double) = egui::ScrollArea::vertical()
                .id_salt("lib_add_grid")
                .max_height(height)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    if shown.is_empty() {
                        ui.weak("Nothing in the library matches.");
                    }
                    self.lib.grid(ui, &lib.db, &shown, &d.picked, &note)
                })
                .inner;
            if let Some(a) = clicked.or(double.clone()) {
                if !d.picked.remove(&a) {
                    d.picked.insert(a);
                }
            }
            if let Some(a) = double {
                // Double-click: add just this one right away.
                d.picked = BTreeSet::from([a]);
                add = true;
            }
            ui.separator();
            let mut brings: Vec<String> = d
                .picked
                .iter()
                .flat_map(|a| library::deps(&lib.db, a))
                .filter(|a| !d.picked.contains(a) && !library::exists(project, a))
                .map(|a| format!("{} {}", kind_label(a.0).to_lowercase(), a.1))
                .collect();
            brings.sort();
            brings.dedup();
            if !brings.is_empty() {
                ui.weak(format!("Also brings what they need: {}", brings.join(", ")));
            }
            if d.picked.iter().any(|a| library::exists(project, a)) {
                ui.weak("Assets marked 'in project' are skipped if identical; a different one with the same id is added under a new id.");
            }
            ui.horizontal(|ui| {
                let label = ui::t(ph::DOWNLOAD_SIMPLE, &format!("Add {} to project", d.picked.len()));
                if ui.add_enabled(!d.picked.is_empty(), egui::Button::new(label)).clicked() {
                    add = true;
                }
                if ui.button("Cancel").clicked() {
                    close = true;
                }
            });
        });
        if modal.should_close() && !add {
            close = true;
        }
        if add {
            self.lib.add = Some(d);
            self.add_picked_from_library();
        } else if !close {
            self.lib.add = Some(d);
        }
    }

    /// "Add from library…" button for `kind` (`None`: all kinds).
    pub fn add_from_library_button(&mut self, ui: &mut egui::Ui, kind: Option<RefKind>) {
        let what = kind.map_or("assets".to_string(), plural);
        if ui.button(ui::t(ph::DOWNLOAD_SIMPLE, "Add from library…")).on_hover_text(format!("Choose {what} from the shared asset library to copy into this project")).clicked() {
            self.open_add_dialog(kind);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn starter() -> Bundle {
        Bundle::from_files(STARTER).unwrap()
    }

    /// The starter library loads, has every id once per kind, and everything its assets refer
    /// to (other assets, files, script functions) is in it.
    #[test]
    fn starter_library_is_consistent() {
        let b = starter();
        let assets = library::all_assets(&b.db);
        let mut seen = BTreeSet::new();
        for a in &assets {
            assert!(seen.insert(a.clone()), "the starter library has the {} {} twice", kind_label(a.0), a.1);
        }
        for a in &assets {
            for d in library::deps(&b.db, a) {
                assert!(library::exists(&b.db, &d), "{} {} needs the {} {}, which the starter library lacks", kind_label(a.0), a.1, kind_label(d.0), d.1);
            }
            for f in library::script_roots(&b.db, a) {
                assert!(b.scripts.contains_key(&f), "{} {} runs {f}(), which scripts/library.rhai lacks", kind_label(a.0), a.1);
            }
        }
        for a in library::script_asset_refs(b.scripts.values().map(String::as_str)) {
            assert!(library::exists(&b.db, &a), "a starter script uses the {} {}, which the library lacks", kind_label(a.0), a.1);
        }
        assert!(b.db.tilesets.len() >= 5 && b.db.sheets.len() > 15 && b.db.models.len() > 60, "{} tilesets, {} sheets, {} models", b.db.tilesets.len(), b.db.sheets.len(), b.db.models.len());
        // Every character is a model (none falls back to the mannequin).
        for c in &b.db.characters {
            assert!(b.db.model(&c.appearance.model).is_some(), "character {} has no model", c.id);
        }
    }

    /// Everything the example project has (but its quests, achievements and story scripts) is in
    /// the starter library, identical: send changed example assets with the AI agent server's
    /// `library_send` (replace) and `RUSTPG2D_LIBRARY` set to `crates/editor/assets/starter_library`.
    /// A definition matches with its scripted triggers (library features such as the fisher's
    /// restock, whose functions travel along) or without them (`drop_script_triggers`: triggers
    /// that run the example's story scripts stay with the example).
    #[test]
    fn starter_library_has_every_example_asset() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../example_project");
        let example = Project::load(&root).unwrap();
        let full = Bundle::collect(&example.db, &root, rustpg2d_engine::project::TILE_SIZE, "", &library::all_assets(&example.db)).unwrap();
        let mut ex = full.clone();
        ex.drop_script_triggers();
        let st = starter();
        let mut wrong = Vec::new();
        for a in library::all_assets(&ex.db) {
            if !library::exists(&st.db, &a) {
                wrong.push(format!("{} {} is missing", kind_label(a.0), a.1));
            } else if !library::same_def(&ex.db, &st.db, &a) && !library::same_def(&full.db, &st.db, &a) {
                wrong.push(format!("{} {} differs", kind_label(a.0), a.1));
            }
        }
        for (rel, bytes) in &ex.files {
            if st.files.get(rel) != Some(bytes) {
                wrong.push(format!("file {rel} differs or is missing"));
            }
        }
        assert!(wrong.is_empty(), "the starter library is out of date with the example project:\n{}", wrong.join("\n"));
    }
}
