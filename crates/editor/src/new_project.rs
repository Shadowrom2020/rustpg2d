//! New project wizard: game name, location and basic settings (player character's sex,
//! character creator, starter assets from the library, first map, scale, inventory).

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use egui_macroquad::egui::{self, Color32};
use rustpg2d_engine::library::{AssetRef, Bundle};
use rustpg2d_engine::map::{Map, Tile, Zone, ZoneKind};
use rustpg2d_engine::project::{CharacterCreation, Project, SexChoice, TILE_SIZE};
use rustpg2d_engine::refactor::RefKind;

use crate::app::Editor;

#[derive(Clone)]
pub struct NewProjectWizard {
    pub name: String,
    /// Folder the project file is created in.
    pub parent: String,
    /// Project file name without `.r2dproject` (follows the game name until edited).
    pub folder: String,
    pub folder_edited: bool,
    pub sex: SexChoice,
    pub creator: bool,
    pub default_name: String,
    pub starter: bool,
    pub map_name: String,
    pub width: u32,
    pub height: u32,
    pub scale: f32,
    pub inventory: u32,
    pub error: Option<String>,
}

impl NewProjectWizard {
    pub fn new(parent: PathBuf) -> Self {
        let name = "My Adventure".to_string();
        NewProjectWizard {
            folder: rustpg2d_engine::release::slug(&name),
            name,
            parent: parent.display().to_string(),
            folder_edited: false,
            sex: SexChoice::Both,
            creator: true,
            default_name: "Alex".into(),
            starter: true,
            map_name: "start".into(),
            width: 40,
            height: 30,
            scale: 1.5,
            inventory: 24,
            error: None,
        }
    }

    /// The project file to create.
    pub fn file(&self) -> PathBuf {
        PathBuf::from(self.parent.trim()).join(format!("{}.{}", self.folder.trim(), crate::project_file::EXT))
    }

    /// Why the project can't be created yet (None = ready).
    pub fn problem(&self) -> Option<String> {
        if self.name.trim().is_empty() {
            return Some("Enter a game name.".into());
        }
        if self.parent.trim().is_empty() || self.folder.trim().is_empty() {
            return Some("Choose where to create the project.".into());
        }
        if self.folder.contains(['/', '\\']) {
            return Some("The file name can't contain slashes.".into());
        }
        let map_ok = !self.map_name.is_empty() && self.map_name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-');
        if !map_ok {
            return Some("Map names may use letters, digits, '_' and '-'.".into());
        }
        let file = self.file();
        if file.exists() {
            return Some(format!("{} already exists.", file.display()));
        }
        None
    }
}

/// Library assets a new project starts with (those the library has).
const STARTER: [(RefKind, &str); 12] = [
    (RefKind::Tileset, "terrain"),
    (RefKind::Character, "hero"),
    (RefKind::Item, "coin"),
    (RefKind::Item, "apple"),
    (RefKind::Item, "potion"),
    (RefKind::Item, "sword"),
    (RefKind::Item, "key"),
    (RefKind::Prop, "chest"),
    (RefKind::Prop, "sign"),
    (RefKind::Prop, "barrel"),
    (RefKind::Prop, "well"),
    (RefKind::Prop, "boulder"),
];

/// What the character creator offers with the starter assets: the hero's and heroine's 3D models
/// (with their colour features: skin, hair, clothing).
const CREATOR_MODELS: [&str; 2] = ["hero", "heroine"];

impl Editor {
    /// Opens the wizard (after the current project's unsaved work was settled).
    pub fn open_new_project_wizard(&mut self) {
        let parent = match self.project_parent() {
            Some(p) if self.project_open => p,
            _ => rustpg2d_engine::paths::home().unwrap_or_default(),
        };
        self.new_project = Some(NewProjectWizard::new(parent));
    }

    /// Creates the project described by the wizard and opens it.
    pub fn create_from_wizard(&mut self) -> Result<PathBuf> {
        let w = self.new_project.clone().context("no wizard open")?;
        if let Some(p) = w.problem() {
            bail!(p);
        }
        let mut pf = crate::project_file::ProjectFile::create(&w.file())?;
        let dir = pf.work.clone();
        crate::files::create_project(&dir)?;
        let mut project = Project::load(&dir)?;
        let mut notes = Vec::new();
        // Starter assets from the library.
        let mut have_terrain = false;
        if w.starter {
            self.lib.ensure_loaded();
            match &self.lib.lib {
                Some(lib) => {
                    let mut want: Vec<AssetRef> = STARTER.iter().map(|(k, id)| (*k, id.to_string())).collect();
                    want.extend(CREATOR_MODELS.iter().map(|s| (RefKind::Model, s.to_string())));
                    want.retain(|a| rustpg2d_engine::library::exists(&lib.db, a));
                    let mut b = Bundle::collect(&lib.db, &lib.root, TILE_SIZE, "starter", &want)?;
                    b.rescale(TILE_SIZE)?;
                    let report = b.merge_into(&mut project.db, &dir)?;
                    notes.push(format!("{} starter assets", report.added.len()));
                    have_terrain = project.db.tileset("terrain").is_some();
                }
                None => notes.push("asset library not available: no starter assets".into()),
            }
        }
        let c = &mut project.config;
        c.name = w.name.trim().to_string();
        c.scale = w.scale;
        c.inventory_size = w.inventory;
        if project.db.character("hero").is_some() {
            c.player = "hero".into();
        }
        // Player character, the creator's choices and the (possibly fixed) sex.
        let have: Vec<String> = project.db.models.iter().map(|m| m.id.clone()).collect();
        let has = |id: &str| have.iter().any(|s| s == id);
        c.creation = CharacterCreation {
            enabled: w.creator,
            sex: w.sex,
            default_name: w.default_name.trim().to_string(),
            body_male: if has("hero") { "hero".into() } else { String::new() },
            body_female: if has("heroine") { "heroine".into() } else { String::new() },
        };
        if w.sex == SexChoice::Female {
            if let Some(h) = project.db.characters.iter_mut().find(|d| d.id == "hero") {
                if has("heroine") {
                    h.appearance.model = "heroine".into();
                    h.appearance.body.clear();
                    h.appearance.colors.clear();
                }
                h.name = "Heroine".into();
            }
        }
        // First map: grass when the terrain tileset came with the starter assets.
        let mut map = Map::new(&w.map_name, w.width.clamp(8, 1024), w.height.clamp(8, 1024));
        if have_terrain {
            let slot = map.tileset_slot("terrain");
            for y in 0..map.height {
                for x in 0..map.width {
                    map.layers[0].set(x, y, Tile::new(slot, 0));
                }
            }
        }
        let uid = map.alloc_uid();
        map.zones.push(Zone { uid, name: "start".into(), x: map.width / 2, y: map.height / 2, w: 1, h: 1, kind: ZoneKind::Spawn, triggers: vec![], loot: None });
        if w.map_name != "start" {
            let _ = std::fs::remove_file(project.map_path("start"));
        }
        project.config.start_map = w.map_name.clone();
        project.config.start_spawn = "start".into();
        project.save_map(&map)?;
        project.save_config()?;
        project.save_defs()?;
        pf.sync_now()?;
        let file = pf.path.clone();
        self.new_project = None;
        // Opening replaces the whole editor state, closing the old project.
        self.close_project();
        self.switch_project(file.clone());
        let extra = if notes.is_empty() { String::new() } else { format!(" ({})", notes.join(", ")) };
        self.set_status(format!("Created \"{}\"{extra}. Paint the map, then F5 to play.", self.project.config.name));
        Ok(file)
    }

    pub fn new_project_window(&mut self, ctx: &egui::Context) {
        let Some(mut w) = self.new_project.take() else { return };
        let mut open = true;
        let mut create = false;
        let mut cancel = false;
        let mut browse = false;
        egui::Window::new("New project").open(&mut open).collapsible(false).resizable(false).default_width(560.0).show(ctx, |ui| {
            egui::Grid::new("np").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                ui.label("Game name");
                if ui.add(egui::TextEdit::singleline(&mut w.name).desired_width(300.0)).changed() && !w.folder_edited {
                    w.folder = rustpg2d_engine::release::slug(&w.name);
                }
                ui.end_row();
                ui.label("Create in");
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut w.parent).desired_width(300.0));
                    browse = ui.button("Browse…").clicked();
                });
                ui.end_row();
                ui.label("File name");
                if ui.add(egui::TextEdit::singleline(&mut w.folder).desired_width(200.0)).changed() {
                    w.folder_edited = true;
                }
                ui.end_row();
                ui.label("");
                ui.weak(w.file().display().to_string());
                ui.end_row();
            });
            ui.separator();
            ui.heading("Player character");
            egui::Grid::new("np_player").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                ui.label("Sex");
                ui.horizontal(|ui| {
                    for v in SexChoice::ALL {
                        ui.radio_value(&mut w.sex, v, v.label());
                    }
                });
                ui.end_row();
                ui.label("Character creator");
                ui.checkbox(&mut w.creator, "The player chooses a name (and look) when starting a new game");
                ui.end_row();
                ui.label("Default name");
                ui.add(egui::TextEdit::singleline(&mut w.default_name).desired_width(160.0));
                ui.end_row();
            });
            ui.separator();
            ui.heading("Setup");
            egui::Grid::new("np_setup").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                ui.label("Starter assets");
                ui.checkbox(&mut w.starter, "Copy basics from the asset library: grass tileset, hero looks, a few items and props")
                    .on_hover_text("Everything else can be added later with Assets \u{E13A} Add from library");
                ui.end_row();
                ui.label("First map");
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut w.map_name).desired_width(110.0));
                    ui.add(egui::DragValue::new(&mut w.width).range(8..=1024).suffix(" w"));
                    ui.label("×");
                    ui.add(egui::DragValue::new(&mut w.height).range(8..=1024).suffix(" h"));
                    ui.weak("tiles");
                });
                ui.end_row();
                ui.label("Pixel scale");
                ui.add(egui::Slider::new(&mut w.scale, 1.0..=4.0).step_by(0.5)).on_hover_text("Zoom of the game window (tiles are 32 px)");
                ui.end_row();
                ui.label("Inventory slots");
                ui.add(egui::DragValue::new(&mut w.inventory).range(4..=120));
                ui.end_row();
            });
            ui.separator();
            let problem = w.problem();
            if let Some(p) = &problem {
                ui.colored_label(Color32::YELLOW, p);
            }
            if let Some(e) = &w.error {
                ui.colored_label(Color32::LIGHT_RED, e);
            }
            ui.horizontal(|ui| {
                create = ui.add_enabled(problem.is_none(), egui::Button::new("\u{E182} Create project")).clicked();
                cancel = ui.button("Cancel").clicked();
            });
        });
        if browse {
            self.new_project = Some(w);
            self.open_file_dialog(crate::files::Purpose::NewProjectParent);
            return;
        }
        if cancel || !open {
            return;
        }
        self.new_project = Some(w);
        if create {
            if let Err(e) = self.create_from_wizard() {
                if let Some(w) = &mut self.new_project {
                    w.error = Some(format!("{e:#}"));
                }
            }
        }
    }
}
