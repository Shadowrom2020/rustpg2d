//! "New asset" wizard: creates a prop, item, tileset, sprite sheet or structure in one step,
//! either drawing a fresh image (opened in the pixel editor) or reusing an existing one.
//! (Characters are assembled in the character builder, `builder.rs`.)

use anyhow::{bail, Result};
use egui_macroquad::egui;
use rustpg2d_engine::defs::*;
use rustpg2d_engine::imageio::Rgba;
use rustpg2d_engine::project::Project;

use crate::app::{Editor, Mode};
use crate::pixel::Layout;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WKind {
    Prop,
    Item,
    Tileset,
    Sheet,
    /// Multi-tile structure (assembled from tileset stamps in the structure editor).
    Structure,
}

impl WKind {
    pub fn parse(s: &str) -> Option<WKind> {
        Some(match s {
            "prop" => WKind::Prop,
            "item" => WKind::Item,
            "tileset" => WKind::Tileset,
            "sheet" => WKind::Sheet,
            "structure" => WKind::Structure,
            _ => return None,
        })
    }
}

/// Where the image comes from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Source {
    /// A new transparent image, opened in the pixel editor.
    Blank,
    /// A copy of an existing image (then edited in the pixel editor).
    CopyOf,
    /// A frame of an existing sprite sheet (no new image).
    Existing,
}

pub struct Wizard {
    pub kind: WKind,
    pub id: String,
    pub name: String,
    pub source: Source,
    /// Prop size in tiles.
    pub cells: (u32, u32),
    /// Sprite sheet frame size.
    pub frame: (u32, u32),
    /// Sprite sheet frames, tileset tiles.
    pub grid: (u32, u32),
    /// Sheet (or tileset) to copy / reuse, and the frame to use.
    pub other: String,
    pub other_frame: u32,
    /// Structures: palette category, and an existing structure to copy ("" = start empty).
    pub category: String,
    pub copy_of: String,
    pub error: Option<String>,
}

impl Wizard {
    pub fn new(project: &Project) -> Self {
        let ts = rustpg2d_engine::project::TILE_SIZE;
        Wizard {
            kind: WKind::Prop,
            id: String::new(),
            name: String::new(),
            source: Source::Blank,
            cells: (1, 1),
            frame: (ts, ts * 3 / 2),
            grid: (4, 4),
            other: project.db.sheets.first().map(|s| s.id.clone()).unwrap_or_default(),
            other_frame: 0,
            category: String::new(),
            copy_of: String::new(),
            error: None,
        }
    }
}

/// Lowercase letters, digits and underscores.
fn clean_id(s: &str) -> String {
    s.trim().to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect()
}

fn free_path(project: &Project, dir: &str, stem: &str) -> String {
    let mut rel = format!("{dir}/{stem}.png");
    let mut n = 2;
    while project.root.join(&rel).exists() {
        rel = format!("{dir}/{stem}_{n}.png");
        n += 1;
    }
    rel
}

impl Editor {
    /// Creates the asset described by the wizard. Returns a status message.
    pub fn create_asset(&mut self, w: &Wizard) -> Result<String> {
        let id = clean_id(&w.id);
        if id.is_empty() {
            bail!("Please enter an id");
        }
        let name = if w.name.trim().is_empty() { id.replace('_', " ") } else { w.name.trim().to_string() };
        let ts = rustpg2d_engine::project::TILE_SIZE;
        let u = ts as f32 / 16.0; // 2: values below are given for the classic 16 px grid
        let db = &self.project.db;
        let taken = match w.kind {
            WKind::Prop => db.prop(&id).is_some(),
            WKind::Item => db.item(&id).is_some(),
            WKind::Tileset => db.tileset(&id).is_some(),
            WKind::Sheet => db.sheet(&id).is_some(),
            WKind::Structure => db.structure(&id).is_some(),
        };
        if taken {
            bail!("The id '{id}' is already used");
        }
        if w.kind == WKind::Structure {
            // No image: an empty grid (or a copy) to fill in the structure editor.
            let mut d = match db.structure(&w.copy_of) {
                Some(src) => src.clone(),
                None => rustpg2d_engine::defs::StructureDef { w: w.cells.0.max(1), h: w.cells.1.max(1), ..Default::default() },
            };
            d.id = id.clone();
            d.name = name;
            d.category = w.category.trim().to_string();
            self.project.db.structures.push(d);
            self.defs_dirty = true;
            self.db.tab = crate::database::Tab::Structures;
            self.db.sel = self.project.db.structures.len() - 1;
            self.mode = Mode::Database;
            return Ok(format!("Created structure '{id}' — pick tiles from a tileset on the right and paint them on the grid, then save"));
        }
        // The image: a new one (blank or copied) or a frame of an existing sheet.
        let mut new_image: Option<(String, (u32, u32), Layout)> = None;
        let mut sprite = SpriteRef::new(&w.other, w.other_frame);
        if w.source == Source::Existing && !matches!(w.kind, WKind::Tileset | WKind::Sheet) {
            if db.sheet(&w.other).is_none() {
                bail!("Choose a sprite sheet");
            }
        } else {
            let (dir, frame, size, layout) = match w.kind {
                WKind::Prop => ("sprites", (ts, ts), (ts * w.cells.0, ts * w.cells.1), Layout::Plain),
                WKind::Item => ("sprites", (ts, ts), (ts, ts), Layout::Plain),
                WKind::Sheet => ("sprites", w.frame, (w.frame.0 * w.grid.0, w.frame.1 * w.grid.1), if w.frame.1 > w.frame.0 { Layout::Character } else { Layout::Plain }),
                WKind::Tileset | WKind::Structure => ("tilesets", (ts, ts), (ts * w.grid.0, ts * w.grid.1), Layout::Plain),
            };
            let rel = free_path(&self.project, dir, &id);
            let (bytes, frame) = if w.source == Source::CopyOf {
                let (src, f) = match w.kind {
                    WKind::Tileset => (db.tileset(&w.other).map(|t| t.image.clone()), (ts, ts)),
                    _ => (db.sheet(&w.other).map(|s| s.image.clone()), db.sheet(&w.other).map_or(frame, |s| (s.frame_w, s.frame_h))),
                };
                let Some(src) = src else { bail!("Choose an image to copy") };
                (std::fs::read(self.project.root.join(src))?, f)
            } else {
                (Rgba::new(size.0.max(1), size.1.max(1)).encode()?, frame)
            };
            let path = self.project.root.join(&rel);
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            std::fs::write(&path, bytes)?;
            if w.kind == WKind::Tileset {
                self.project.db.tilesets.push(TilesetDef { id: id.clone(), image: rel.clone(), solid: vec![], ladder: vec![], animations: vec![], lights: vec![], terrains: vec![] });
            } else {
                let mut sheet_id = id.clone();
                let mut n = 2;
                while self.project.db.sheet(&sheet_id).is_some() {
                    sheet_id = format!("{id}_{n}");
                    n += 1;
                }
                self.project.db.sheets.push(SheetDef::new(&sheet_id, &rel, frame.0, frame.1));
                sprite = if w.kind == WKind::Prop && w.source == Source::Blank { SpriteRef::block(&sheet_id, 0, w.cells.0, w.cells.1) } else { SpriteRef::new(&sheet_id, 0) };
            }
            new_image = Some((rel, frame, layout));
        }
        let db = &mut self.project.db;
        match w.kind {
            WKind::Prop => db.props.push(PropDef {
                id: id.clone(),
                name,
                hitbox: [(ts * sprite.cells().0) as f32 - 2.0 * u, 8.0 * u],
                sprite,
                solid: true,
                triggers: vec![],
                container: None,
                lock: None,
                default_state: default_state_name(),
                states: vec![],
                light: None,
                use_spot: None,
                gather: None,
                regrow: None,
                plot: false,
            }),
            WKind::Item => db.items.push(ItemDef {
                id: id.clone(),
                name,
                description: String::new(),
                icon: sprite,
                consumable: false,
                on_use: vec![],
                weapon: None,
                armor: None,
                max_stack: 99,
                value: 0,
        light: None,
        teaches: String::new(),
        held: String::new(),
        tool: String::new(),
        use_on: Default::default(),
        crop: None,
        food: false,
            }),
            WKind::Tileset | WKind::Sheet | WKind::Structure => {}
        }
        self.defs_dirty = true;
        self.reload_assets();
        let label = format!("{:?}", w.kind).to_lowercase();
        match new_image {
            Some((rel, frame, layout)) => {
                self.open_pixel(&rel, frame, layout);
                Ok(format!("Created {label} '{id}' — draw it now, then save the image (Ctrl+S) and the definitions"))
            }
            None => {
                self.mode = Mode::Database;
                Ok(format!("Created {label} '{id}' using sheet '{}'. Save the definitions to keep it.", w.other))
            }
        }
    }

    pub fn wizard_window(&mut self, ctx: &egui::Context) {
        let Some(mut w) = self.wizard.take() else { return };
        let mut open = true;
        let mut create = false;
        let ts = rustpg2d_engine::project::TILE_SIZE;
        egui::Window::new("New asset").open(&mut open).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            ui.horizontal(|ui| {
                for (k, l) in [(WKind::Prop, "Prop"), (WKind::Item, "Item"), (WKind::Tileset, "Tileset"), (WKind::Sheet, "Sprite sheet"), (WKind::Structure, "Structure")] {
                    if ui.selectable_value(&mut w.kind, k, l).clicked() && k == WKind::Structure && w.cells == (1, 1) {
                        w.cells = (6, 5);
                    }
                }
            });
            if w.kind == WKind::Structure {
                egui::Grid::new("wiz_st").num_columns(2).show(ui, |ui| {
                    ui.label("Id");
                    ui.add(egui::TextEdit::singleline(&mut w.id).hint_text("e.g. stone_house"));
                    ui.end_row();
                    ui.label("Name");
                    ui.add(egui::TextEdit::singleline(&mut w.name).hint_text("shown in the palette"));
                    ui.end_row();
                    ui.label("Category");
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut w.category).hint_text("e.g. Buildings").desired_width(140.0));
                        let mut cats: Vec<String> = self.project.db.structures.iter().map(|d| d.category.clone()).filter(|c| !c.is_empty()).collect();
                        cats.sort();
                        cats.dedup();
                        egui::ComboBox::from_id_salt("wiz_cat").selected_text("").width(24.0).show_ui(ui, |ui| {
                            for c in cats {
                                ui.selectable_value(&mut w.category, c.clone(), c);
                            }
                        });
                    });
                    ui.end_row();
                    ui.label("Start");
                    ui.vertical(|ui| {
                        let empty = w.copy_of.is_empty();
                        if ui.radio(empty, "Empty grid").clicked() {
                            w.copy_of.clear();
                        }
                        if ui.radio(!empty, "Copy of an existing structure").clicked() && empty {
                            w.copy_of = self.project.db.structures.first().map(|d| d.id.clone()).unwrap_or_default();
                        }
                    });
                    ui.end_row();
                    if w.copy_of.is_empty() {
                        ui.label("Size (tiles)");
                        ui.horizontal(|ui| {
                            ui.add(egui::DragValue::new(&mut w.cells.0).range(1..=64));
                            ui.label("×");
                            ui.add(egui::DragValue::new(&mut w.cells.1).range(1..=64));
                        });
                    } else {
                        ui.label("Structure");
                        egui::ComboBox::from_id_salt("wiz_copy").selected_text(&w.copy_of).show_ui(ui, |ui| {
                            for d in &self.project.db.structures {
                                ui.selectable_value(&mut w.copy_of, d.id.clone(), format!("{} ({})", d.name, d.id));
                            }
                        });
                    }
                    ui.end_row();
                });
                ui.weak("The structure editor opens next: pick tile stamps from a tileset and paint them on the grid.");
                if let Some(e) = &w.error {
                    ui.colored_label(egui::Color32::LIGHT_RED, e);
                }
                if ui.button("Create").clicked() {
                    create = true;
                }
                return;
            }
            if matches!(w.kind, WKind::Tileset | WKind::Sheet) && w.source == Source::Existing {
                w.source = Source::Blank;
            }
            egui::Grid::new("wiz").num_columns(2).show(ui, |ui| {
                ui.label("Id");
                ui.add(egui::TextEdit::singleline(&mut w.id).hint_text("e.g. treasure_chest"));
                ui.end_row();
                if !matches!(w.kind, WKind::Tileset | WKind::Sheet) {
                    ui.label("Name");
                    ui.add(egui::TextEdit::singleline(&mut w.name).hint_text("shown in game"));
                    ui.end_row();
                }
                ui.label("Image");
                ui.vertical(|ui| {
                    ui.radio_value(&mut w.source, Source::Blank, "Draw a new image");
                    ui.radio_value(&mut w.source, Source::CopyOf, "Start from a copy of an existing image");
                    if !matches!(w.kind, WKind::Tileset | WKind::Sheet) {
                        ui.radio_value(&mut w.source, Source::Existing, "Use a frame of an existing sprite sheet");
                    }
                });
                ui.end_row();
                match (w.source, w.kind) {
                    (Source::Blank, WKind::Prop) => {
                        ui.label("Size (tiles)");
                        ui.horizontal(|ui| {
                            ui.add(egui::DragValue::new(&mut w.cells.0).range(1..=8));
                            ui.label("×");
                            ui.add(egui::DragValue::new(&mut w.cells.1).range(1..=8));
                        });
                        ui.end_row();
                    }
                    (Source::Blank, WKind::Sheet) => {
                        ui.label("Frame (px)");
                        ui.horizontal(|ui| {
                            ui.add(egui::DragValue::new(&mut w.frame.0).range(4..=256));
                            ui.label("×");
                            ui.add(egui::DragValue::new(&mut w.frame.1).range(4..=256));
                        });
                        ui.end_row();
                        if w.kind == WKind::Sheet {
                            ui.label("Frames");
                            ui.horizontal(|ui| {
                                ui.add(egui::DragValue::new(&mut w.grid.0).range(1..=32));
                                ui.label("×");
                                ui.add(egui::DragValue::new(&mut w.grid.1).range(1..=32));
                            });
                            ui.end_row();
                        }
                    }
                    (Source::Blank, WKind::Tileset) => {
                        ui.label("Tiles");
                        ui.horizontal(|ui| {
                            ui.add(egui::DragValue::new(&mut w.grid.0).range(1..=32));
                            ui.label("×");
                            ui.add(egui::DragValue::new(&mut w.grid.1).range(1..=32));
                        });
                        ui.end_row();
                    }
                    (Source::CopyOf | Source::Existing, _) => {
                        let tileset = w.kind == WKind::Tileset;
                        let ids: Vec<String> = if tileset { self.project.db.tilesets.iter().map(|t| t.id.clone()).collect() } else { self.project.db.sheets.iter().map(|s| s.id.clone()).collect() };
                        if !ids.contains(&w.other) {
                            w.other = ids.first().cloned().unwrap_or_default();
                        }
                        ui.label(if tileset { "Tileset" } else { "Sprite sheet" });
                        egui::ComboBox::from_id_salt("wiz_other").selected_text(&w.other).show_ui(ui, |ui| {
                            for id in ids {
                                ui.selectable_value(&mut w.other, id.clone(), id);
                            }
                        });
                        ui.end_row();
                        if w.source == Source::Existing {
                            ui.label("Frame");
                            ui.horizontal(|ui| {
                                ui.add(egui::DragValue::new(&mut w.other_frame).range(0..=4095));
                                let r = ui.allocate_exact_size(egui::vec2(36.0, 36.0), egui::Sense::hover()).0;
                                crate::widgets::paint_frame(ui, &self.tex, &self.assets, &w.other, w.other_frame, r);
                            });
                            ui.end_row();
                        }
                    }
                    _ => {}
                }
            });
            let hint = match (w.kind, w.source) {
                (_, Source::Existing) => "The definition uses the chosen frame; nothing new is drawn.".to_string(),
                (WKind::Sheet, _) => format!("Frames taller than wide get the character layout (4 walk frames per row, rows down / left / right / up). Tiles are {ts}px."),
                (WKind::Tileset, _) => format!("A tileset image of {ts}px tiles; mark solid tiles in Database \u{E13A} Tilesets."),
                _ => format!("Tiles are {ts}px. After creating, the image opens in the pixel editor."),
            };
            ui.weak(hint);
            if let Some(e) = &w.error {
                ui.colored_label(egui::Color32::LIGHT_RED, e);
            }
            ui.horizontal(|ui| {
                if ui.button("Create").clicked() {
                    create = true;
                }
            });
        });
        if create {
            match self.create_asset(&w) {
                Ok(msg) => {
                    self.set_status(msg);
                    return;
                }
                Err(e) => w.error = Some(format!("{e:#}")),
            }
        }
        if open {
            self.wizard = Some(w);
        }
    }
}
