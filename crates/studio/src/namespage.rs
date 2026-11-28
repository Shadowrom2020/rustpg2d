//! The Flags and variables page: every flag, variable and text variable of the game with how
//! often it is set and read, a note for each, and under the one picked every place it stands
//! (a click goes there) and its rename. What is only ever set, or only ever read, is marked:
//! mostly a name typed two ways.

use eframe::egui;
use editor_core::names::{How, NameKind, Named};
use editor_core::search::Target;
use editor_core::Workspace;

#[derive(Default)]
pub struct NamesPage {
    all: Vec<Named>,
    /// What the survey was made of: made again when it changes.
    key: String,
    filter: String,
    /// The kinds hidden.
    hidden: [bool; 3],
    only_marked: bool,
    /// The one opened, the name typed for it, and its note as it is being written.
    open: Option<(NameKind, String)>,
    rename: String,
    note: String,
}

impl NamesPage {
    /// `names=N flags=N variables=N texts=N marked=N open=…` for the script's `status`.
    pub fn status(&self) -> String {
        let n = |k: NameKind| self.all.iter().filter(|x| x.kind == k).count();
        format!("names={} flags={} variables={} texts={} marked={} open={}", self.all.len(), n(NameKind::Flag), n(NameKind::Variable), n(NameKind::Text), self.all.iter().filter(|x| x.warning().is_some()).count(), self.open.as_ref().map_or("-".to_string(), |(k, n)| format!("{}:{n}", k.key())))
    }

    /// Opens a name's row (the script's `names open KIND NAME`).
    pub fn pick(&mut self, ws: &Workspace, kind: NameKind, name: &str) {
        self.open = Some((kind, name.to_string()));
        self.rename = name.to_string();
        self.note = ws.name_note(kind, name).to_string();
    }

    fn refresh(&mut self, ws: &Workspace, extra: &str) {
        let key = format!("{}|{}|{}|{extra}", ws.timeline.status(), ws.scripts_rev, ws.is_dirty());
        if key == self.key {
            return;
        }
        self.key = key;
        self.all = ws.survey_names();
        // (Renamed or gone: its row closes.)
        if let Some((kind, name)) = &self.open {
            if !self.all.iter().any(|n| n.kind == *kind && n.name == *name) {
                self.open = None;
            }
        }
    }

    /// The page. `extra`: what else the survey depends on (scripts saved from their tabs).
    /// Returns where a click asked to go.
    pub fn show(&mut self, ui: &mut egui::Ui, ws: &mut Workspace, extra: &str) -> Option<Target> {
        self.refresh(ws, extra);
        let mut go = None;
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new("Flags and variables").strong());
            ui.add(egui::TextEdit::singleline(&mut self.filter).hint_text("Search").desired_width(180.0)).widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, "Search the names"));
            for (k, kind) in NameKind::ALL.into_iter().enumerate() {
                let n = self.all.iter().filter(|x| x.kind == kind).count();
                let mut shown = !self.hidden[k];
                let label = match kind {
                    NameKind::Flag => format!("Flags ({n})"),
                    NameKind::Variable => format!("Variables ({n})"),
                    NameKind::Text => format!("Text variables ({n})"),
                };
                if ui.toggle_value(&mut shown, label).changed() {
                    self.hidden[k] = !shown;
                }
            }
            let marked = self.all.iter().filter(|x| x.warning().is_some()).count();
            ui.toggle_value(&mut self.only_marked, format!("To look at ({marked})")).on_hover_text("Only those that are set and never read, or read and never set: mostly a name typed two ways");
        });
        ui.weak("What the game remembers: set by actions and scripts, read by conditions. Scripts are read as they are saved.");
        ui.separator();
        let f = self.filter.trim().to_lowercase();
        let rows: Vec<Named> = self
            .all
            .iter()
            .filter(|n| !self.hidden[NameKind::ALL.iter().position(|k| *k == n.kind).unwrap_or(0)])
            .filter(|n| !self.only_marked || n.warning().is_some())
            .filter(|n| f.is_empty() || n.name.to_lowercase().contains(&f) || ws.name_note(n.kind, &n.name).to_lowercase().contains(&f))
            .cloned()
            .collect();
        if rows.is_empty() {
            ui.weak(if self.all.is_empty() { "This game has no flags or variables yet: a Set flag action or set_flag() in a script makes the first." } else { "Nothing fits." });
            return None;
        }
        let mut renamed: Option<(NameKind, String, String)> = None;
        // (Columns of a fixed width: the opened row's places stand under it, as wide as the page.)
        const WIDTHS: [f32; 4] = [250.0, 110.0, 54.0, 54.0];
        let cell = |ui: &mut egui::Ui, k: usize, add: &mut dyn FnMut(&mut egui::Ui)| {
            ui.allocate_ui_with_layout(egui::vec2(WIDTHS[k], ui.spacing().interact_size.y), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.set_min_width(WIDTHS[k]);
                ui.set_max_width(WIDTHS[k]);
                add(ui)
            });
        };
        ui.horizontal(|ui| {
            for (k, head) in ["Name", "Kind", "Set", "Read"].into_iter().enumerate() {
                cell(ui, k, &mut |ui| {
                    ui.weak(head);
                });
            }
            ui.weak("Note");
        });
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for n in &rows {
                let open = self.open.as_ref().is_some_and(|(k, name)| *k == n.kind && *name == n.name);
                let mut clicked = false;
                ui.horizontal(|ui| {
                    cell(ui, 0, &mut |ui| clicked = ui.add(egui::Button::selectable(open, n.name.as_str()).truncate()).on_hover_text("Where it is set and read; its note; rename").clicked());
                    cell(ui, 1, &mut |ui| {
                        ui.weak(n.kind.label());
                    });
                    for (k, how) in [(2, How::Set), (3, How::Read)] {
                        cell(ui, k, &mut |ui| match n.count(how) {
                            0 => {
                                ui.colored_label(ui.visuals().warn_fg_color, "never");
                            }
                            c => {
                                ui.label(c.to_string());
                            }
                        });
                    }
                    let note = ws.name_note(n.kind, &n.name);
                    match (n.warning(), note.is_empty()) {
                        (Some(w), true) => ui.colored_label(ui.visuals().warn_fg_color, w),
                        _ => ui.add(egui::Label::new(note).truncate()),
                    };
                });
                if clicked {
                    if open {
                        self.open = None;
                    } else {
                        self.pick(ws, n.kind, &n.name);
                    }
                    continue;
                }
                if !open {
                    continue;
                }
                // The one picked: its note, its rename, every place it stands.
                ui.indent(("name", n.kind, &n.name), |ui| {
                    ui.horizontal(|ui| {
                        let label = ui.label("Note");
                        let r = ui.add(egui::TextEdit::singleline(&mut self.note).hint_text("what it is for").desired_width(420.0)).labelled_by(label.id);
                        if r.lost_focus() && self.note != ws.name_note(n.kind, &n.name) {
                            ws.set_name_note(n.kind, &n.name, &self.note);
                        }
                    });
                    ui.horizontal(|ui| {
                        let label = ui.label("Rename to");
                        ui.add(egui::TextEdit::singleline(&mut self.rename).desired_width(220.0)).labelled_by(label.id);
                        let ok = self.rename.trim() != n.name && editor_core::names::valid_name(self.rename.trim());
                        if ui.add_enabled(ok, egui::Button::new("Rename everywhere")).on_hover_text("In every definition, on every map and in the scripts; undone with Ctrl+Z").clicked() {
                            renamed = Some((n.kind, n.name.clone(), self.rename.trim().to_string()));
                        }
                    });
                    for u in &n.uses {
                        ui.horizontal(|ui| {
                            ui.weak(match u.how {
                                How::Set => "sets it",
                                How::Read => "reads it",
                                How::Used => "names it",
                            });
                            if ui.link(u.place.as_str()).on_hover_text("Go there").clicked() {
                                go = Some(u.target.clone());
                            }
                        });
                    }
                    ui.add_space(4.0);
                });
            }
        });
        if let Some((kind, old, new)) = renamed {
            match ws.rename_name(kind, &old, &new) {
                Ok(msg) => {
                    ws.set_status(msg);
                    self.open = Some((kind, new.clone()));
                    self.rename = new;
                    self.key.clear();
                }
                Err(e) => ws.set_status(format!("Not renamed: {e}")),
            }
        }
        go
    }
}

#[cfg(test)]
mod ui_tests {
    use super::*;
    use egui_kittest::kittest::Queryable;
    use egui_kittest::Harness;

    struct State {
        ws: Workspace,
        page: NamesPage,
        went: Vec<Target>,
        dir: std::path::PathBuf,
    }

    fn copy(from: &std::path::Path, to: &std::path::Path) {
        std::fs::create_dir_all(to).unwrap();
        for e in std::fs::read_dir(from).unwrap().flatten() {
            let (src, dst) = (e.path(), to.join(e.file_name()));
            if src.is_dir() {
                if e.file_name() != "saves" {
                    copy(&src, &dst);
                }
            } else {
                std::fs::copy(&src, &dst).unwrap();
            }
        }
    }

    #[test]
    fn a_flag_is_found_noted_followed_and_renamed_from_its_page() {
        let dir = std::env::temp_dir().join(format!("rustpg2d-names-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        copy(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../example_project"), &dir);
        let mut ws = Workspace::new(dir.clone(), editor_core::Gpu::No).unwrap();
        ws.refresh_refs();
        ws.history_frame(None);
        let state = State { ws, page: NamesPage::default(), went: Vec::new(), dir };
        let mut h = Harness::builder().with_size(egui::vec2(1100.0, 900.0)).build_ui_state(
            |ui, s: &mut State| {
                s.ws.history_frame(None);
                if let Some(t) = s.page.show(ui, &mut s.ws, "") {
                    s.went.push(t);
                }
            },
            state,
        );
        h.run();
        // Searched, the list is short; a name opened shows where it stands.
        let search = h.get_by_role_and_label(egui::accesskit::Role::TextInput, "Search the names");
        search.focus();
        search.type_text("quest_acc");
        h.run();
        assert!(h.query_all_by_label("shrine_lit").next().is_none());
        h.get_by_role_and_label(egui::accesskit::Role::Button, "quest_accepted").click();
        h.run();
        assert!(h.query_all_by_label("sets it").next().is_some() && h.query_all_by_label("reads it").next().is_some());
        // A place is a link to it.
        h.get_by_label("house: gate_key_chest").click();
        h.run();
        assert!(matches!(h.state().went.as_slice(), [Target::Entity(map, _)] if map == "house"), "{:?}", h.state().went);
        // A note is kept with the project.
        let note = h.get_by_role_and_label(egui::accesskit::Role::TextInput, "Note");
        note.focus();
        note.type_text("The elder's errand was taken.");
        h.run();
        h.get_by_role_and_label(egui::accesskit::Role::TextInput, "Rename to").focus();
        h.run();
        assert_eq!(h.state().ws.name_note(NameKind::Flag, "quest_accepted"), "The elder's errand was taken.");
        // Renamed everywhere from its row; the row stays open under the new name.
        let rename = h.get_by_role_and_label(egui::accesskit::Role::TextInput, "Rename to");
        rename.focus();
        rename.type_text("_by_hero");
        h.run();
        h.get_by_label("Rename everywhere").click();
        h.run();
        h.state_mut().page.filter.clear();
        h.run();
        let s = h.state();
        assert!(s.ws.status.0.starts_with("Renamed the flag 'quest_accepted' to 'quest_accepted_by_hero' in 4 places"), "{}", s.ws.status.0);
        assert!(s.page.all.iter().any(|n| n.name == "quest_accepted_by_hero") && !s.page.all.iter().any(|n| n.name == "quest_accepted"));
        assert_eq!(s.ws.name_note(NameKind::Flag, "quest_accepted_by_hero"), "The elder's errand was taken.");
        assert!(std::fs::read_to_string(s.dir.join("scripts/main.rhai")).unwrap().contains("flag(\"quest_accepted_by_hero\")"));
        let dir = h.state().dir.clone();
        let _ = std::fs::remove_dir_all(dir);
    }
}
