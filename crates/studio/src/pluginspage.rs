//! Plugins, at the top of the *Scripts & plugins* page: the project's plugins, each with what
//! it is, on or off, its place in the order and its parameters by their names; one added from
//! the asset library or from a plugin file, one taken out.
//!
//! The logic is `editor_core::plugins`.

use eframe::egui::{self, RichText};
use editor_core::plugins::PluginRow;
use editor_core::Workspace;
use rustpg2d_engine::plugins::{ParamDef, ParamKind, ParamValue};
use rustpg2d_engine::refactor::RefKind;

#[derive(Default)]
pub struct PluginsPage {
    pub said: Option<Result<String, String>>,
}

/// What a click asked for.
enum Do {
    Enable(String, bool),
    Move(String, bool),
    Remove(String),
    Set(String, String, Option<ParamValue>),
    Add(String),
    File(std::path::PathBuf),
}

/// A value as a script writes it: `on` / `off`, a number, else a text.
fn value_of(kind: ParamKind, text: &str) -> ParamValue {
    match kind {
        ParamKind::Flag => ParamValue::Flag(matches!(text, "on" | "true" | "yes")),
        ParamKind::Number => ParamValue::Number(text.parse().unwrap_or(0.0)),
        _ => ParamValue::Text(text.to_string()),
    }
}

impl PluginsPage {
    fn act(&mut self, ws: &mut Workspace, what: Do) {
        let r = match what {
            Do::Enable(id, on) => ws.set_plugin_enabled(&id, on).map(|()| String::new()),
            Do::Move(id, up) => ws.move_plugin(&id, up).map(|()| String::new()),
            Do::Remove(id) => ws.remove_plugin(&id),
            Do::Set(id, name, value) => ws.set_plugin_param(&id, &name, value).map(|()| String::new()),
            Do::Add(id) => ws.add_plugin_from_library(&id),
            Do::File(file) => ws.add_plugin_file(&file),
        };
        match r {
            Ok(said) if said.is_empty() => {}
            other => self.said = Some(other),
        }
    }

    /// The script's line: `[calendar:on:1.0 …] said=…`.
    pub fn status(&self, ws: &Workspace) -> String {
        let said = match &self.said {
            Some(Ok(m)) => m.clone(),
            Some(Err(e)) => format!("failed: {e}"),
            None => "-".into(),
        };
        format!("{} said={said}", ws.plugins_status())
    }

    /// The script's words: `add ID` (from the library), `file PATH`, `remove ID`, `on ID`,
    /// `off ID`, `up ID`, `down ID`, `set ID NAME VALUE…|-` (`-`: back to its default).
    pub fn word(&mut self, ws: &mut Workspace, args: &[String]) -> Result<(), String> {
        let arg = |i: usize| args.get(i).map(String::as_str).unwrap_or("").to_string();
        let what = match arg(0).as_str() {
            "" => return Ok(()),
            "add" => Do::Add(arg(1)),
            "file" => Do::File(std::path::PathBuf::from(args[1..].join(" "))),
            "remove" => Do::Remove(arg(1)),
            "on" | "off" => Do::Enable(arg(1), arg(0) == "on"),
            "up" | "down" => Do::Move(arg(1), arg(0) == "up"),
            "set" => {
                let text = args[3.min(args.len())..].join(" ");
                let kind = ws.plugins().iter().find(|p| p.id == arg(1)).and_then(|p| p.def.as_ref().ok().and_then(|d| d.parameters.iter().find(|q| q.name == arg(2)).map(|q| q.kind))).unwrap_or(ParamKind::Text);
                Do::Set(arg(1), arg(2), (text != "-").then(|| value_of(kind, &text)))
            }
            other => return Err(format!("plugins {other}: add, file, remove, on, off, up, down, set")),
        };
        self.said = None;
        self.act(ws, what);
        match &self.said {
            Some(Err(e)) => Err(e.clone()),
            _ => Ok(()),
        }
    }

    /// `lists`: id and name of every definition by kind, `maps`: the maps (what a parameter
    /// that names one is picked from).
    pub fn show(&mut self, ui: &mut egui::Ui, ws: &mut Workspace, lists: &[(RefKind, Vec<(String, String)>)]) {
        ui.strong("Plugins");
        ui.weak("A plugin adds something to the game as a whole (a calendar under the clock, damage numbers, a harder mode): scripts of its own that the game calls at its moments, with parameters to set here.");
        let rows = ws.plugins();
        let mut what: Option<Do> = None;
        if rows.is_empty() {
            ui.weak("This game has no plugins. The asset library has some, below.");
        }
        let n = rows.len();
        for (k, p) in rows.iter().enumerate() {
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                plugin_ui(ui, p, k, n, lists, &ws.maps, &mut what);
            });
        }
        // What the asset library has.
        let offered = ws.library_plugins();
        let fresh: Vec<_> = offered.iter().filter(|p| p.in_project.is_none() || p.newer()).collect();
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.label("Add a plugin");
            if let Some(file) = crate::pick::browse(ui, crate::pick::Key::PluginFile, "From a file…", "A plugin file (.r2dplugin)", || crate::pick::Want::File("Plugins", &["r2dplugin"]), None) {
                what = Some(Do::File(file));
            }
            ui.weak(if offered.is_empty() { "The asset library has none (a pack brings them: the Library tab)." } else if fresh.is_empty() { "The asset library has no others." } else { "or from the asset library:" });
        });
        for p in fresh {
            ui.horizontal_wrapped(|ui| {
                let label = if p.newer() { format!("Update to {}", p.def.version) } else { "Add".to_string() };
                if ui.button(label).widget_info_named(format!("Add {}", p.id)).clicked() {
                    what = Some(Do::Add(p.id.clone()));
                }
                ui.label(RichText::new(&p.def.name).strong());
                ui.weak(format!("{}{}", p.def.version, if p.def.author.is_empty() { String::new() } else { format!(" · {}", p.def.author) }));
                ui.label(&p.def.description);
            });
        }
        match &self.said {
            Some(Ok(m)) => {
                ui.weak(m);
            }
            Some(Err(e)) => {
                ui.colored_label(ui.visuals().error_fg_color, e);
            }
            None => {}
        }
        if let Some(what) = what {
            self.said = None;
            self.act(ws, what);
        }
        ui.add_space(6.0);
        ui.separator();
    }
}

/// One plugin of the project: its row and its parameters.
fn plugin_ui(ui: &mut egui::Ui, p: &PluginRow, k: usize, n: usize, lists: &[(RefKind, Vec<(String, String)>)], maps: &[String], what: &mut Option<Do>) {
    ui.horizontal_wrapped(|ui| {
        let mut on = p.enabled;
        if ui.checkbox(&mut on, "").on_hover_text("On: the game uses it. Off: it stays in the project with its values, unused.").widget_info_named_as(egui::Role::CheckBox, format!("{} is on", p.id)).changed() {
            *what = Some(Do::Enable(p.id.clone(), on));
        }
        match &p.def {
            Ok(d) => {
                ui.label(RichText::new(if d.name.is_empty() { &p.id } else { &d.name }).strong());
                ui.weak(format!("{}{}", d.version, if d.author.is_empty() { String::new() } else { format!(" · {}", d.author) }));
            }
            Err(_) => {
                ui.label(RichText::new(&p.id).strong());
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("Remove").on_hover_text("Takes it out of the project. Its files are kept under editor/backup.").widget_info_named(format!("Remove {}", p.id)).clicked() {
                *what = Some(Do::Remove(p.id.clone()));
            }
            if ui.add_enabled(k + 1 < n, egui::Button::new("v").small()).on_hover_text("Later in the order: its hooks run after the others' of the same priority").widget_info_named(format!("Move {} down", p.id)).clicked() {
                *what = Some(Do::Move(p.id.clone(), false));
            }
            if ui.add_enabled(k > 0, egui::Button::new("^").small()).on_hover_text("Earlier in the order").widget_info_named(format!("Move {} up", p.id)).clicked() {
                *what = Some(Do::Move(p.id.clone(), true));
            }
        });
    });
    let def = match &p.def {
        Ok(d) => d,
        Err(e) => {
            ui.colored_label(ui.visuals().error_fg_color, format!("Its folder plugins/{} can't be read: {e}. Add it again, or remove it.", p.id));
            return;
        }
    };
    if !def.description.is_empty() {
        ui.label(&def.description);
    }
    if p.too_new {
        ui.colored_label(ui.visuals().warn_fg_color, format!("It needs the script functions of {}: this version of the game leaves it out.", def.api));
    }
    if def.parameters.is_empty() {
        return;
    }
    ui.add_enabled_ui(p.enabled, |ui| {
        for param in &def.parameters {
            let set = p.values.get(&param.name);
            ui.horizontal(|ui| {
                // (A column of labels, as a form has it: the values line up.)
                let by = ui
                    .allocate_ui_with_layout(egui::vec2(190.0, ui.spacing().interact_size.y), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.set_min_width(190.0);
                        let r = ui.add(egui::Label::new(param.shown_label()).truncate());
                        if param.help.is_empty() { r } else { r.on_hover_text(&param.help) }.id
                    })
                    .inner;
                let now = param.value(set);
                if let Some(new) = param_ui(ui, param, &now, by, lists, maps) {
                    *what = Some(Do::Set(p.id.clone(), param.name.clone(), Some(new)));
                }
                if set.is_some() {
                    if ui.small_button("Default").on_hover_text(format!("Back to the plugin's own: {}", param.default_value().show())).widget_info_named(format!("{} back to its default", param.shown_label())).clicked() {
                        *what = Some(Do::Set(p.id.clone(), param.name.clone(), None));
                    }
                } else {
                    ui.weak("its default");
                }
            });
        }
    });
}

/// The control for a parameter's value. Returns the new value when it was changed.
fn param_ui(ui: &mut egui::Ui, param: &ParamDef, now: &ParamValue, by: egui::Id, lists: &[(RefKind, Vec<(String, String)>)], maps: &[String]) -> Option<ParamValue> {
    let ids = |kind: RefKind| -> Vec<String> { lists.iter().find(|(k, _)| *k == kind).map_or(Vec::new(), |(_, l)| l.iter().map(|(id, _)| id.clone()).collect()) };
    match (param.kind, now) {
        (ParamKind::Flag, ParamValue::Flag(b)) => {
            let mut b = *b;
            ui.checkbox(&mut b, "").labelled_by(by).changed().then_some(ParamValue::Flag(b))
        }
        (ParamKind::Number, ParamValue::Number(n)) => {
            let mut n = *n;
            let mut drag = egui::DragValue::new(&mut n).speed(0.1).max_decimals(3);
            if param.min != 0.0 || param.max != 0.0 {
                drag = drag.range(param.min..=param.max);
            }
            ui.add(drag).labelled_by(by).changed().then_some(ParamValue::Number(n))
        }
        (kind, ParamValue::Text(s)) => {
            let known: Option<Vec<String>> = match kind {
                ParamKind::Sprite => Some(ids(RefKind::Sheet)),
                ParamKind::Sound => Some(ids(RefKind::Sound)),
                ParamKind::Item => Some(ids(RefKind::Item)),
                ParamKind::Map => Some(maps.to_vec()),
                _ => None,
            };
            match known {
                // One of the project's things: picked from them.
                Some(known) => {
                    let mut pick = None;
                    egui::ComboBox::from_id_salt(("plugin parameter", &param.name)).selected_text(if s.is_empty() { "none" } else { s.as_str() }).width(220.0).show_ui(ui, |ui| {
                        if ui.selectable_label(s.is_empty(), "none").clicked() {
                            pick = Some(String::new());
                        }
                        for id in &known {
                            if ui.selectable_label(s == id, id.as_str()).clicked() {
                                pick = Some(id.clone());
                            }
                        }
                    })
                    .response
                    .labelled_by(by);
                    pick.map(ParamValue::Text)
                }
                None => {
                    let mut text = s.clone();
                    let hint = if kind == ParamKind::Color { "a colour's name or #rrggbb" } else { "" };
                    ui.add(egui::TextEdit::singleline(&mut text).hint_text(hint).desired_width(360.0)).labelled_by(by).changed().then_some(ParamValue::Text(text))
                }
            }
        }
        _ => None,
    }
}

/// A control named for screen readers and tests when several share their text.
trait Named: Sized {
    fn widget_info_named_as(self, role: egui::Role, name: impl Into<String>) -> Self;
    fn widget_info_named(self, name: impl Into<String>) -> Self {
        self.widget_info_named_as(egui::Role::Button, name)
    }
}

impl Named for egui::Response {
    fn widget_info_named_as(self, role: egui::Role, name: impl Into<String>) -> Self {
        let name = name.into();
        self.widget_info(|| egui::WidgetInfo::labeled(role, true, name.as_str()));
        self
    }
}

#[cfg(test)]
mod ui_tests {
    use super::*;
    use egui_kittest::kittest::Queryable;
    use egui_kittest::Harness;

    struct State {
        ws: Workspace,
        page: PluginsPage,
    }

    #[test]
    fn a_plugin_is_added_from_the_library_and_its_parameters_are_set_by_name() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir = std::env::temp_dir().join(format!("rustpg2d-pluginspage-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        editor_core::files::create_project(&dir).unwrap();
        let mut ws = Workspace::new(dir.clone(), editor_core::Gpu::No).unwrap();
        ws.history_frame(None);
        ws.lib.lib = Some(rustpg2d_engine::project::Project::load(&root.join("crates/editor/assets/starter_library")).unwrap());
        let mut h = Harness::builder().with_size(egui::vec2(1100.0, 1100.0)).build_ui_state(
            |ui, s: &mut State| {
                egui::ScrollArea::vertical().show(ui, |ui| s.page.show(ui, &mut s.ws, &[]));
                s.ws.history_frame(None);
            },
            State { ws, page: PluginsPage::default() },
        );
        h.run();
        // None yet: the library's are offered with what they are.
        h.get_by_label("This game has no plugins. The asset library has some, below.");
        h.get_by_label("Calendar");
        h.get_by_label("Add calendar").click();
        h.run_steps(3);
        assert_eq!(h.state().ws.plugins_status().split(':').next(), Some("[calendar"));
        assert!(h.query_all_by_label("Add calendar").next().is_none(), "what the project has isn't offered again");
        // Its parameters by their labels: a switch flipped, and back to its default.
        h.get_by_label("Days per season");
        h.get_by_role_and_label(egui::accesskit::Role::CheckBox, "Show the year").click();
        h.run_steps(3);
        assert_eq!(h.state().ws.plugins()[0].values.get("show_year"), Some(&ParamValue::Flag(true)));
        h.get_by_label("Show the year back to its default").click();
        h.run_steps(3);
        assert!(h.state().ws.plugins()[0].values.is_empty());
        // Off, then taken out: its files are kept aside, and the library offers it again.
        h.get_by_role_and_label(egui::accesskit::Role::CheckBox, "calendar is on").click();
        h.run_steps(3);
        assert!(!h.state().ws.plugins()[0].enabled);
        h.get_by_label("Remove calendar").click();
        h.run_steps(3);
        assert!(h.state().ws.plugins().is_empty());
        assert!(h.query_all_by_label_contains("its files are kept in editor/backup/").next().is_some());
        h.get_by_label("Add calendar");
        let _ = std::fs::remove_dir_all(dir);
    }
}
