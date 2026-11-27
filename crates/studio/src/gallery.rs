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
}

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
    Cancel,
}

/// The gallery's window. `groups`: the kinds as the Project browser groups them; `lists`: id
/// and name of every definition, by kind (what a copy can be taken of).
pub fn show(ctx: &egui::Context, state: &mut NewAsset, groups: &[(&str, &[RefKind])], lists: &[(RefKind, Vec<(String, String)>)], library: &dyn Fn(RefKind) -> Vec<(String, String)>, mut pictures: Option<&mut dyn Pictures>) -> Option<Outcome> {
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
                        let kinds: Vec<RefKind> = kinds.iter().copied().filter(|k| k.is_def()).collect();
                        if kinds.is_empty() {
                            continue;
                        }
                        ui.weak(*group);
                        for kind in kinds {
                            if ui.selectable_label(state.kind == Some(kind), kind.label()).on_hover_text(editor_core::defs::about(kind)).clicked() && state.kind != Some(kind) {
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
                let takes_files = [RefKind::Prop, RefKind::Sheet, RefKind::Sound].contains(&kind);
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
                        RefKind::Sheet => "from a file: a picture (.png)",
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
                        ui.add(egui::TextEdit::singleline(&mut state.file).hint_text("Drop a file on the window, or type where it is").desired_width(340.0)).labelled_by(by);
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
                            ui.colored_label(ui.visuals().warn_fg_color, can.unwrap_or_else(|| "That is no 3D model (.glb), picture (.png) or sound (.ogg, .wav).".to_string()));
                        }
                    }
                }
                if state.library {
                    ui.add(egui::TextEdit::singleline(&mut state.filter).hint_text("Search the library").desired_width(300.0)).widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, "Search the library"));
                    let f = state.filter.trim().to_lowercase();
                    egui::ScrollArea::vertical().id_salt("library").max_height(210.0).show(ui, |ui| {
                        ui.set_min_width(400.0);
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
