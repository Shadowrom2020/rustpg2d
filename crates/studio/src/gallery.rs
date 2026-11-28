//! *New…* (Ctrl+N): what to make, its name, and what it starts as: blank, a copy of something
//! there is, an asset of the library (which comes with what it needs: the library's assets
//! are the templates), or a file: a 3D model, a picture, a sound, typed or dropped on the
//! window. (From Meshy: part 8 adds it here.)

use eframe::egui;
use rustpg2d_engine::refactor::RefKind;

use crate::form::Pictures;

/// The gallery while it is open.
#[derive(Default)]
pub struct NewAsset {
    pub kind: Option<RefKind>,
    pub name: String,
    /// It starts as a copy (of `from`, once one is picked).
    pub copy: bool,
    /// It starts as an asset of the library (`from`, once one is picked).
    pub library: bool,
    pub from: Option<String>,
    filter: String,
    /// What the library has of the kind chosen (read when the kind is picked).
    in_library: Option<(RefKind, Vec<(String, String)>)>,
    /// It starts from a file: where that is, as typed or dropped.
    pub from_file: bool,
    pub file: String,
    /// Why the last *Create* didn't work.
    pub error: Option<String>,
    focused: bool,
    /// A new map: how many tiles wide and high (0 = not looked at yet: the usual size), and
    /// the material its ground is filled with (`None` = not picked yet: the first there is).
    pub map_size: (u32, u32),
    pub map_ground: Option<String>,
}

/// The size a new map starts with.
pub const MAP_SIZE: (u32, u32) = (40, 30);

impl NewAsset {
    pub fn of(kind: Option<RefKind>) -> NewAsset {
        NewAsset { kind, ..Default::default() }
    }

    /// The gallery for a file dropped on the window: what it most likely becomes, named after
    /// the file. None for a file that is no art the editor takes.
    pub fn of_file(file: &std::path::Path) -> Option<NewAsset> {
        let what = editor_core::fromfile::FileKind::of(file)?;
        Some(NewAsset { kind: what.makes().first().copied(), name: editor_core::fromfile::name_from_file(file), from_file: true, file: file.display().to_string(), ..Default::default() })
    }
}

/// What the gallery asks for.
pub enum Outcome {
    Create { kind: RefKind, name: String, from: Option<String> },
    /// A new one from the library's `from` (no name: the asset as it is).
    FromLibrary { kind: RefKind, name: String, from: String },
    /// A new one from a file (a 3D model, a picture, a sound).
    FromFile { kind: RefKind, name: String, file: std::path::PathBuf },
    /// A new map: its name, its size in tiles, the material its ground is filled with ("" = bare).
    Map { name: String, width: u32, height: u32, ground: String },
    Cancel,
}

/// A new map: its name, its size, its ground. Returns what was asked for.
fn new_map(ui: &mut egui::Ui, state: &mut NewAsset, lists: &[(RefKind, Vec<(String, String)>)]) -> Option<Outcome> {
    use rustpg2d_engine::map::MAX_MAP_SIZE;
    if state.map_size.0 == 0 {
        state.map_size = MAP_SIZE;
    }
    let materials: &[(String, String)] = lists.iter().find(|(k, _)| *k == RefKind::Material).map_or(&[], |(_, l)| l.as_slice());
    let maps: &[(String, String)] = lists.iter().find(|(k, _)| *k == RefKind::Map).map_or(&[], |(_, l)| l.as_slice());
    // (Grass when the project has it, else the first material there is.)
    let ground = state.map_ground.get_or_insert_with(|| materials.iter().find(|(id, _)| id == "grass").or(materials.first()).map_or(String::new(), |(id, _)| id.clone()));
    let entered = ui
        .horizontal(|ui| {
            let by = ui.label("Name").id;
            let r = ui.add(egui::TextEdit::singleline(&mut state.name).hint_text("forest, old_mill, cellar-2").desired_width(300.0)).labelled_by(by);
            let entered = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if !state.focused {
                r.request_focus();
                state.focused = true;
            }
            entered
        })
        .inner;
    ui.weak("Its name is also its file's: letters, digits, _ and -. Rename changes it later, everywhere it is named.");
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        let by = ui.label("Width").id;
        ui.add(egui::DragValue::new(&mut state.map_size.0).range(1..=MAX_MAP_SIZE)).labelled_by(by);
        let by = ui.label("Height").id;
        ui.add(egui::DragValue::new(&mut state.map_size.1).range(1..=MAX_MAP_SIZE)).labelled_by(by);
        ui.weak("tiles").on_hover_text("A tile is one step of a character. A room: 8 x 6; a village: 40 x 30. The size can be changed later (the map's Inspector).");
    });
    ui.horizontal(|ui| {
        let by = ui.label("Ground").id;
        let shown = |id: &str| materials.iter().find(|(i, _)| i == id).map_or_else(|| "bare (nothing painted)".to_string(), |(i, n)| if n == i || n.is_empty() { i.clone() } else { format!("{n}  ({i})") });
        egui::ComboBox::from_id_salt("new map ground").selected_text(shown(ground)).width(240.0).show_ui(ui, |ui| {
            ui.selectable_value(ground, String::new(), "bare (nothing painted)");
            for (id, _) in materials {
                ui.selectable_value(ground, id.clone(), shown(id));
            }
        })
        .response
        .labelled_by(by)
        .on_hover_text("What the whole map is painted with to begin with. The Terrain tool paints over it.");
    });
    ui.add_space(8.0);
    let name = state.name.trim().to_string();
    let why = if name.is_empty() {
        Some("")
    } else if !editor_core::rename::valid_name(&name) {
        Some("A map's name has letters, digits, _ and - only (no spaces).")
    } else if maps.iter().any(|(id, _)| *id == name) {
        Some("A map of this name exists.")
    } else {
        None
    };
    if let Some(why) = why.filter(|w| !w.is_empty()) {
        ui.colored_label(ui.visuals().warn_fg_color, why);
    }
    if let Some(e) = &state.error {
        ui.colored_label(ui.visuals().error_fg_color, e);
    }
    let mut outcome = None;
    ui.horizontal(|ui| {
        let create = ui.add_enabled(why.is_none(), egui::Button::new("Create")).clicked();
        if (create || entered) && why.is_none() {
            outcome = Some(Outcome::Map { name, width: state.map_size.0, height: state.map_size.1, ground: ground.clone() });
        }
        if ui.button("Cancel").clicked() {
            outcome = Some(Outcome::Cancel);
        }
    });
    outcome
}

/// The gallery's window. `groups`: the kinds as the Project browser groups them; `lists`: id
/// and name of every definition, by kind (what a copy can be taken of).
pub fn show(ctx: &egui::Context, state: &mut NewAsset, groups: &[(&str, &[RefKind])], lists: &[(RefKind, Vec<(String, String)>)], library: &dyn Fn(RefKind) -> Vec<(String, String)>, mut pictures: Option<&mut dyn Pictures>, library_picture: &mut dyn FnMut(RefKind, &str) -> Option<crate::thumbs::Thumb>) -> Option<Outcome> {
    let mut outcome = None;
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        return Some(Outcome::Cancel);
    }
    egui::Window::new("New…").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, -40.0]).order(egui::Order::Foreground).show(ctx, |ui| {
        ui.horizontal_top(|ui| {
            // What to make.
            ui.vertical(|ui| {
                ui.set_width(200.0);
                egui::ScrollArea::vertical().id_salt("kinds").max_height(430.0).show(ui, |ui| {
                    for (group, kinds) in groups {
                        let kinds: Vec<RefKind> = kinds.iter().copied().filter(|k| k.is_def() || *k == RefKind::Map).collect();
                        if kinds.is_empty() {
                            continue;
                        }
                        ui.weak(*group);
                        for kind in kinds {
                            let r = ui.selectable_label(state.kind == Some(kind), kind.label()).on_hover_text(editor_core::defs::about(kind));
                            // (A tour points at the kind to pick, until it is picked.)
                            // (Scrolled out of view: the list it is in, to scroll.)
                            if state.kind != Some(kind) {
                                crate::tourpanel::mark(ui, &format!("new:{}", kind.name()), if ui.is_rect_visible(r.rect) { r.rect } else { egui::Rect::from_x_y_ranges(r.rect.x_range(), ui.clip_rect().y_range()) });
                            }
                            if r.clicked() && state.kind != Some(kind) {
                                state.kind = Some(kind);
                                state.from = None;
                                state.error = None;
                                state.focused = false;
                            }
                        }
                        ui.add_space(4.0);
                    }
                });
            });
            ui.separator();
            // Its name and what it starts as.
            ui.vertical(|ui| {
                ui.set_width(430.0);
                let Some(kind) = state.kind else {
                    ui.add_space(8.0);
                    ui.label("What would you like to make?");
                    ui.weak("Pick a kind on the left. Point at one to read what it is.");
                    return;
                };
                ui.heading(format!("New {}", kind.label().to_lowercase()));
                ui.weak(editor_core::defs::about(kind));
                ui.add_space(6.0);
                if kind == RefKind::Map {
                    if let Some(o) = new_map(ui, state, lists) {
                        outcome = Some(o);
                    }
                    return;
                }
                let entered = ui
                    .horizontal(|ui| {
                        let by = ui.label("Name").id;
                        let r = ui.add(egui::TextEdit::singleline(&mut state.name).hint_text("What it is called").desired_width(300.0)).labelled_by(by);
                        let entered = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                        if !state.focused {
                            r.request_focus();
                            state.focused = true;
                        }
                        entered
                    })
                    .inner;
                if !state.name.trim().is_empty() {
                    ui.weak(format!("Its id: {} (a number is added when that is taken; Rename changes it later)", editor_core::defs::id_from_name(&state.name)));
                }
                ui.add_space(6.0);
                ui.label("It starts");
                if !state.in_library.as_ref().is_some_and(|(k, _)| *k == kind) {
                    state.in_library = Some((kind, if rustpg2d_engine::library::KINDS.contains(&kind) { library(kind) } else { Vec::new() }));
                }
                let offered: Vec<(String, String)> = state.in_library.as_ref().map(|(_, l)| l.clone()).unwrap_or_default();
                let takes_files = [RefKind::Prop, RefKind::Sheet, RefKind::Sound, RefKind::Model, RefKind::Material, RefKind::Font].contains(&kind);
                if !takes_files {
                    state.from_file = false;
                }
                let mut start = if state.from_file { 3 } else if state.library { 2 } else if state.copy { 1 } else { 0 };
                let was = start;
                ui.radio_value(&mut start, 0, "blank");
                let all: &[(String, String)] = lists.iter().find(|(k, _)| *k == kind).map_or(&[], |(_, l)| l.as_slice());
                ui.add_enabled_ui(!all.is_empty(), |ui| ui.radio_value(&mut start, 1, format!("as a copy of another {}", kind.label().to_lowercase())));
                ui.add_enabled_ui(!offered.is_empty(), |ui| ui.radio_value(&mut start, 2, format!("as one of the asset library ({})", offered.len())).on_hover_text("It comes with what it needs: its pictures, sounds and models, the script functions its events call. Without a name it is added as it is.").on_disabled_hover_text("The asset library has none of this kind"));
                if takes_files {
                    ui.radio_value(&mut start, 3, match kind {
                        RefKind::Prop => "from a file: a 3D model (.glb) or a picture (.png)",
                        RefKind::Model => "from a file: a 3D model (.glb)",
                        RefKind::Sheet => "from a file: a picture (.png)",
                        RefKind::Material => "from a file: a picture that joins itself at its edges (.png)",
                        RefKind::Font => "from a file: a font (.ttf, .otf)",
                        _ => "from a file: a sound (.ogg, .wav)",
                    })
                    .on_hover_text("The file is copied into the project. Drop a file on the editor's window to start here with it.");
                }
                if start != was {
                    (state.copy, state.library, state.from_file, state.from) = (start == 1, start == 2, start == 3, None);
                    state.filter.clear();
                }
                let mut file_fits = false;
                if state.from_file {
                    ui.horizontal(|ui| {
                        let by = ui.label("File").id;
                        ui.add(egui::TextEdit::singleline(&mut state.file).hint_text("Browse, drop a file on the window, or type where it is").desired_width(270.0)).labelled_by(by);
                        let want = || match kind {
                            RefKind::Prop => crate::pick::Want::File("3D models and pictures", &["glb", "png"]),
                            RefKind::Model => crate::pick::Want::File("3D models", &["glb"]),
                            RefKind::Sheet | RefKind::Material => crate::pick::Want::File("Pictures", &["png"]),
                            RefKind::Font => crate::pick::Want::File("Fonts", &["ttf", "otf"]),
                            _ => crate::pick::Want::File("Sounds", &["ogg", "wav"]),
                        };
                        let start = Some(std::path::PathBuf::from(state.file.trim())).filter(|p| !p.as_os_str().is_empty());
                        if let Some(picked) = crate::pick::browse(ui, crate::pick::Key::NewFromFile, "Browse…", "The file the new one starts from", want, start) {
                            // (Named after the file while no name was typed.)
                            if state.name.trim().is_empty() {
                                state.name = editor_core::fromfile::name_from_file(&picked);
                            }
                            state.file = picked.display().to_string();
                        }
                    });
                    let path = std::path::Path::new(state.file.trim());
                    match editor_core::fromfile::what_it_makes(kind, path) {
                        _ if state.file.trim().is_empty() => {}
                        Some(what) if path.is_file() => {
                            file_fits = true;
                            ui.weak(what);
                        }
                        Some(_) => {
                            ui.colored_label(ui.visuals().warn_fg_color, "There is no such file.");
                        }
                        None => {
                            let can = editor_core::fromfile::FileKind::of(path).map(|k| format!("{} can become {}", k.label(), k.makes().iter().map(|k| format!("a {}", k.label().to_lowercase())).collect::<Vec<_>>().join(" or ")));
                            ui.colored_label(ui.visuals().warn_fg_color, can.unwrap_or_else(|| "That is no 3D model (.glb), picture (.png), sound (.ogg, .wav) or font (.ttf, .otf).".to_string()));
                        }
                    }
                }
                if state.library {
                    ui.horizontal_top(|ui| {
                        ui.vertical(|ui| {
                            ui.set_width(300.0);
                            ui.add(egui::TextEdit::singleline(&mut state.filter).hint_text("Search the library").desired_width(f32::INFINITY)).widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, "Search the library"));
                            let f = state.filter.trim().to_lowercase();
                            egui::ScrollArea::vertical().id_salt("library").max_height(210.0).show(ui, |ui| {
                                ui.set_min_width(290.0);
                                for (id, name) in offered.iter().filter(|(id, name)| f.is_empty() || id.to_lowercase().contains(&f) || name.to_lowercase().contains(&f)).take(200) {
                                    let mut label = if name == id || name.is_empty() { id.clone() } else { format!("{name}  ({id})") };
                                    if all.iter().any(|(i, _)| i == id) {
                                        label += "  · in the project";
                                    }
                                    if ui.selectable_label(state.from.as_deref() == Some(id.as_str()), label).clicked() {
                                        state.from = Some(id.clone());
                                    }
                                }
                            });
                        });
                        // The one picked, as the library has it.
                        if let Some(from) = state.from.as_ref().filter(|_| crate::thumbs::has_pictures(kind)) {
                            crate::thumbs::boxed(ui, 96.0, || library_picture(kind, from));
                        }
                    });
                }
                if state.copy {
                    ui.horizontal_top(|ui| {
                        ui.vertical(|ui| {
                            ui.set_width(300.0);
                            ui.add(egui::TextEdit::singleline(&mut state.filter).hint_text("Search").desired_width(f32::INFINITY)).widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, "Search"));
                            let f = state.filter.trim().to_lowercase();
                            let has_pictures = crate::thumbs::has_pictures(kind) && pictures.is_some();
                            egui::ScrollArea::vertical().id_salt("copies").max_height(210.0).show(ui, |ui| {
                                ui.set_min_width(290.0);
                                for (id, name) in all.iter().filter(|(id, name)| f.is_empty() || id.to_lowercase().contains(&f) || name.to_lowercase().contains(&f)).take(150) {
                                    ui.horizontal(|ui| {
                                        if has_pictures {
                                            crate::thumbs::boxed(ui, 20.0, || pictures.as_mut().and_then(|p| p.of(kind, id)));
                                        }
                                        let label = if name == id { id.clone() } else { format!("{name}  ({id})") };
                                        if ui.selectable_label(state.from.as_deref() == Some(id.as_str()), label).clicked() {
                                            state.from = Some(id.clone());
                                        }
                                    });
                                }
                            });
                        });
                        // What the copy is taken of.
                        if let Some(from) = &state.from {
                            crate::thumbs::boxed(ui, 96.0, || pictures.as_mut().and_then(|p| p.of(kind, from)));
                        }
                    });
                }
                ui.add_space(8.0);
                if let Some(e) = &state.error {
                    ui.colored_label(ui.visuals().error_fg_color, e);
                }
                let named = !state.name.trim().is_empty();
                let ready = if state.from_file { named && file_fits } else if state.library { state.from.is_some() } else { named && (!state.copy || state.from.is_some()) };
                ui.horizontal(|ui| {
                    let create = ui.add_enabled(ready, egui::Button::new("Create")).clicked();
                    if (create || entered) && ready {
                        outcome = Some(match (&state.from, state.library) {
                            _ if state.from_file => Outcome::FromFile { kind, name: state.name.trim().to_string(), file: std::path::PathBuf::from(state.file.trim()) },
                            (Some(from), true) => Outcome::FromLibrary { kind, name: state.name.trim().to_string(), from: from.clone() },
                            _ => Outcome::Create { kind, name: state.name.trim().to_string(), from: state.from.clone().filter(|_| state.copy) },
                        });
                    }
                    if ui.button("Cancel").clicked() {
                        outcome = Some(Outcome::Cancel);
                    }
                    if state.copy && state.from.is_none() {
                        ui.weak("Pick what to copy.");
                    }
                    if state.library {
                        ui.weak(match (&state.from, named) {
                            (None, _) => "Pick one of the library.",
                            (Some(_), false) => "No name: it is added as it is.",
                            (Some(_), true) => "It is added under your name.",
                        });
                    }
                });
            });
        });
    });
    outcome
}

#[cfg(test)]
mod ui_tests {
    use super::*;
    use egui_kittest::kittest::Queryable;
    use egui_kittest::Harness;

    struct G {
        state: NewAsset,
        asked: Vec<(String, u32, u32, String)>,
    }

    #[test]
    fn a_new_map_gets_its_name_its_size_and_its_ground() {
        let lists = vec![
            (RefKind::Material, vec![("dirt".to_string(), "Dirt".to_string()), ("grass".to_string(), "Grass".to_string())]),
            (RefKind::Map, vec![("village".to_string(), "village".to_string())]),
        ];
        let mut h = Harness::builder().with_size(egui::vec2(1000.0, 700.0)).build_ui_state(
            move |ui, g: &mut G| {
                let library = |_: RefKind| Vec::new();
                if let Some(Outcome::Map { name, width, height, ground }) = show(ui.ctx(), &mut g.state, &[("World", &[RefKind::Map, RefKind::Material])], &lists, &library, None, &mut |_, _| None) {
                    g.asked.push((name, width, height, ground));
                }
            },
            G { state: NewAsset::of(None), asked: Vec::new() },
        );
        h.run();
        // A map is one of the things to make; it starts with the usual size and grass.
        h.get_by_label("Map").click();
        h.run();
        h.get_by_label("New map");
        assert_eq!(h.state().state.map_size, MAP_SIZE);
        assert_eq!(h.state().state.map_ground.as_deref(), Some("grass"));
        // A name that is no file's, or taken, says why; nothing is made.
        let name = h.get_by_role_and_label(egui::accesskit::Role::TextInput, "Name");
        name.focus();
        name.type_text("old mill");
        h.run();
        assert!(h.query_all_by_label_contains("letters, digits, _ and - only").next().is_some());
        h.get_by_label("Create").click();
        h.run();
        assert!(h.state().asked.is_empty());
        h.state_mut().state.name = "village".into();
        h.run();
        h.get_by_label("A map of this name exists.");
        // Named, sized and with its ground picked, it is asked for.
        h.state_mut().state.name = "old_mill".into();
        h.state_mut().state.map_size = (24, 18);
        h.run();
        h.get_by_role_and_label(egui::accesskit::Role::ComboBox, "Ground").click();
        h.run();
        h.get_by_label("Dirt  (dirt)").click();
        h.run();
        h.get_by_label("Create").click();
        h.run();
        assert_eq!(h.state().asked, [("old_mill".to_string(), 24, 18, "dirt".to_string())]);
    }
}
