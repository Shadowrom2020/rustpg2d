//! *Test setup…*: what a test-play starts with, as a form: the hero's level and class, the
//! bag, flags, variables, quests, the hour and the day, the party. Every change is kept at once
//! (for the project, in the editor's settings: not a step of the history). *Play from here
//! with the test setup* on a map and `play MAP X Y setup` start with it.
//!
//! The setup itself is `editor_core::testsetup`.

use eframe::egui;
use editor_core::testsetup::TestSetup;
use editor_core::Workspace;
use rustpg2d_engine::refactor::RefKind;

use crate::form::{self, Cx};

/// One of the window's lists: what a row holds, how a new one starts.
fn list<T: Clone>(ui: &mut egui::Ui, title: &str, help: &str, rows: &mut Vec<T>, fresh: T, mut row: impl FnMut(&mut egui::Ui, usize, &mut T) -> bool) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.strong(title).on_hover_text(help);
        if ui.small_button("+ Add").on_hover_text(format!("{title}: one more")).clicked() {
            rows.push(fresh.clone());
            changed = true;
        }
    });
    let mut gone = None;
    for (k, r) in rows.iter_mut().enumerate() {
        // (Each row's controls have ids of their own: two rows' pickers would share one.)
        ui.push_id((title, k), |ui| {
            ui.horizontal(|ui| {
                changed |= row(ui, k, r);
                if ui.small_button("Remove").on_hover_text(format!("{title}: this one goes")).clicked() {
                    gone = Some(k);
                }
            });
        });
    }
    if let Some(k) = gone {
        rows.remove(k);
        changed = true;
    }
    changed
}

#[derive(Default)]
pub struct SetupWindow {
    pub setup: TestSetup,
}

impl SetupWindow {
    pub fn open(ws: &Workspace) -> SetupWindow {
        SetupWindow { setup: ws.test_setup() }
    }

    /// The window. Returns false once it is closed; `asks` gets what a picker asked for (a
    /// definition opened, a new one).
    pub fn show(&mut self, ctx: &egui::Context, ws: &mut Workspace, lists: &[(RefKind, Vec<(String, String)>)], asks: &mut Vec<form::Ask>) -> bool {
        let mut open = true;
        let mut changed = false;
        let mut close = false;
        let placed: Vec<String> = {
            let mut names: Vec<String> = ws.refs.placed_characters.iter().map(|(name, _)| name.clone()).filter(|n| !n.is_empty()).collect();
            names.sort();
            names.dedup();
            names
        };
        let (flags, vars) = (ws.refs.all_flags.clone(), ws.refs.vars.clone());
        let mut cx = Cx { lists, maps: &ws.maps, show_all: false, compact: false, pictures: None, refs: Some(&ws.refs), asks: Vec::new(), said: Default::default() };
        let s = &mut self.setup;
        egui::Window::new("Test setup").open(&mut open).collapsible(false).resizable(true).default_width(560.0).show(ctx, |ui| {
            ui.weak("What a test-play starts with: Play from here with the test setup (right-click a map) and Play's Restart. Every change is kept at once, for this project.");
            ui.add_space(4.0);
            egui::ScrollArea::vertical().max_height(520.0).show(ui, |ui| {
                egui::Grid::new("test setup").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                    let l = ui.label("Level").on_hover_text("The hero's level (1 = a new hero's)");
                    let mut level = s.level.max(1);
                    if ui.add(egui::DragValue::new(&mut level).range(1..=20)).labelled_by(l.id).changed() {
                        s.level = level;
                        changed = true;
                    }
                    ui.end_row();
                    let l = ui.label("Class").on_hover_text("None: as a new game without the character creator");
                    changed |= form::picker(ui, &mut cx, &[RefKind::Class], &mut s.class, l.id);
                    ui.end_row();
                    let l = ui.label("The hour").on_hover_text("When the game starts: unticked, the project's start hour");
                    ui.horizontal(|ui| {
                        let mut set = s.hour.is_some();
                        if ui.checkbox(&mut set, "set").labelled_by(l.id).changed() {
                            s.hour = set.then_some(8.0);
                            changed = true;
                        }
                        if let Some(h) = &mut s.hour {
                            changed |= ui.add(egui::DragValue::new(h).range(0.0..=23.99).speed(0.1).fixed_decimals(1).suffix(" h")).on_hover_text("The hour").changed();
                            let d = ui.label("day");
                            let mut day = s.day.max(1);
                            if ui.add(egui::DragValue::new(&mut day).range(1..=9999)).labelled_by(d.id).changed() {
                                s.day = day;
                                changed = true;
                            }
                        }
                    });
                    ui.end_row();
                });
                ui.add_space(6.0);
                changed |= list(ui, "Items", "In the hero's bag", &mut s.items, (String::new(), 1), |ui, k, (item, count)| {
                    let l = ui.label(format!("Item {}", k + 1));
                    let mut c = form::picker(ui, &mut cx, &[RefKind::Item], item, l.id);
                    c |= ui.add(egui::DragValue::new(count).range(1..=999).prefix("x ")).on_hover_text("How many").changed();
                    c
                });
                ui.add_space(4.0);
                changed |= list(ui, "Flags", "Flags that are set", &mut s.flags, String::new(), |ui, k, flag| {
                    let l = ui.label(format!("Flag {}", k + 1));
                    form::named(ui, &flags, flag, l.id)
                });
                ui.add_space(4.0);
                changed |= list(ui, "Variables", "Variables and their values", &mut s.vars, (String::new(), 0), |ui, k, (name, value)| {
                    let l = ui.label(format!("Variable {}", k + 1));
                    let mut c = form::named(ui, &vars, name, l.id);
                    c |= ui.add(egui::DragValue::new(value)).on_hover_text("Its value").changed();
                    c
                });
                ui.add_space(4.0);
                changed |= list(ui, "Quests", "Quests under way: their stage, or done", &mut s.quests, (String::new(), 0, false), |ui, k, (quest, stage, done)| {
                    let l = ui.label(format!("Quest {}", k + 1));
                    let mut c = form::picker(ui, &mut cx, &[RefKind::Quest], quest, l.id);
                    c |= ui.add(egui::DragValue::new(stage).range(0..=99).prefix("stage ")).on_hover_text("Its stage (0 = the first)").changed();
                    c |= ui.checkbox(done, "done").changed();
                    c
                });
                ui.add_space(4.0);
                changed |= list(ui, "Party", "Placed characters (on any map, by name) who join the hero", &mut s.party, String::new(), |ui, k, name| {
                    let l = ui.label(format!("Companion {}", k + 1));
                    form::named(ui, &placed, name, l.id)
                });
            });
            ui.separator();
            ui.horizontal(|ui| {
                ui.weak(if *s == TestSetup::default() { "Nothing is set: a new game's hero.".to_string() } else { s.summary() });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Close").clicked() {
                        close = true;
                    }
                    if ui.add_enabled(*s != TestSetup::default(), egui::Button::new("Forget it")).on_hover_text("Back to a new game's hero").clicked() {
                        *s = TestSetup::default();
                        changed = true;
                    }
                });
            });
        });
        asks.append(&mut cx.asks);
        if changed {
            ws.set_test_setup(self.setup.clone());
        }
        open && !close
    }
}

/// `testsetup …`: changes the project's test setup without the window: `level N`, `class
/// ID|-`, `item ID N` (0 = none of it), `flag NAME on|off`, `var NAME N|-`, `quest ID STAGE
/// [done]|-`, `hour H|-`, `day N`, `party NAME on|off`, `clear`. Returns its summary.
pub fn word(ws: &mut Workspace, args: &[String]) -> Result<String, String> {
    let arg = |i: usize| args.get(i).map(String::as_str).unwrap_or("");
    let num = |i: usize| arg(i).parse::<i64>().map_err(|_| format!("testsetup {}: a number after {}", arg(0), arg(i - 1)));
    let mut s = ws.test_setup();
    let set = |list: &mut Vec<String>, name: &str, on: bool| {
        list.retain(|n| n != name);
        if on {
            list.push(name.to_string());
        }
    };
    match arg(0) {
        "" => {}
        "level" => s.level = num(1)?.clamp(1, 20) as u32,
        "class" => s.class = if arg(1) == "-" { String::new() } else { arg(1).to_string() },
        "item" => {
            let n = num(2)?.max(0) as u32;
            s.items.retain(|(i, _)| i != arg(1));
            if n > 0 {
                s.items.push((arg(1).to_string(), n));
            }
        }
        "flag" => set(&mut s.flags, arg(1), arg(2) != "off"),
        "party" => set(&mut s.party, arg(1), arg(2) != "off"),
        "var" => {
            s.vars.retain(|(v, _)| v != arg(1));
            if arg(2) != "-" {
                s.vars.push((arg(1).to_string(), num(2)?));
            }
        }
        "quest" => {
            s.quests.retain(|(q, ..)| q != arg(1));
            if arg(2) != "-" {
                s.quests.push((arg(1).to_string(), num(2)?.max(0) as u32, arg(3) == "done"));
            }
        }
        "hour" => s.hour = if arg(1) == "-" { None } else { Some(arg(1).parse::<f32>().map_err(|_| "testsetup hour H|-")?.clamp(0.0, 23.99)) },
        "day" => s.day = num(1)?.max(1) as u32,
        "clear" => s = TestSetup::default(),
        other => return Err(format!("testsetup {other}: level, class, item, flag, var, quest, hour, day, party, clear")),
    }
    let summary = if s == TestSetup::default() { "none".to_string() } else { s.summary() };
    ws.set_test_setup(s);
    Ok(summary)
}

#[cfg(test)]
mod ui_tests {
    use super::*;
    use egui_kittest::kittest::Queryable;
    use egui_kittest::Harness;

    struct State {
        ws: Workspace,
        win: Option<SetupWindow>,
        lists: Vec<(RefKind, Vec<(String, String)>)>,
    }

    #[test]
    fn what_is_set_in_the_form_is_the_test_setup() {
        let base = std::env::temp_dir().join(format!("rustpg2d-setupwin-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("game")).unwrap();
        // (The setup is kept in the test's own settings.)
        editor_core::files::config_dir_here(Some(base.join("config")));
        editor_core::files::create_project(&base.join("game")).unwrap();
        let mut ws = Workspace::new(base.join("game"), editor_core::Gpu::No).unwrap();
        ws.project.db.items.push(rustpg2d_engine::defs::ItemDef { name: "Potion".into(), ..rustpg2d_engine::defs::ItemDef::fresh("potion") });
        ws.project.db.reindex();
        let lists = vec![(RefKind::Item, editor_core::defs::list(&ws.project.db, RefKind::Item))];
        let win = SetupWindow::open(&ws);
        let mut h = Harness::builder().with_size(egui::vec2(900.0, 800.0)).build_ui_state(
            |ui, s: &mut State| {
                let mut asks = Vec::new();
                if let Some(mut w) = s.win.take() {
                    if w.show(ui.ctx(), &mut s.ws, &s.lists, &mut asks) {
                        s.win = Some(w);
                    }
                }
            },
            State { ws, win: Some(win), lists },
        );
        h.run();
        h.get_by_label("Nothing is set: a new game's hero.");
        // Level 3.
        h.get_by_role_and_label(egui::accesskit::Role::SpinButton, "Level").set_value_accesskit(3.0);
        h.run();
        assert_eq!(h.state().ws.test_setup().level, 3);
        // A potion: a row added, the item picked from the list.
        h.query_all_by_label("+ Add").next().unwrap().click();
        h.run();
        assert_eq!(h.state().ws.test_setup().items, [(String::new(), 1)]);
        h.get_by_role_and_label(egui::accesskit::Role::ComboBox, "Item 1").click();
        h.run();
        h.get_by_label("Potion  (potion)").click();
        h.run();
        assert_eq!(h.state().ws.test_setup().items, [("potion".to_string(), 1)]);
        // A flag, typed.
        h.query_all_by_label("+ Add").nth(1).unwrap().click();
        h.run();
        h.get_by_role_and_label(egui::accesskit::Role::TextInput, "Flag 1").focus();
        h.run();
        h.get_by_role_and_label(egui::accesskit::Role::TextInput, "Flag 1").type_text("gate_open");
        h.run();
        let setup = h.state().ws.test_setup();
        assert_eq!((setup.level, setup.flags.clone()), (3, vec!["gate_open".to_string()]));
        h.get_by_label(setup.summary().as_str());
        // Forget it: back to nothing, and nothing kept.
        h.get_by_label("Forget it").click();
        h.run();
        assert_eq!(h.state().ws.test_setup(), TestSetup::default());
        editor_core::files::config_dir_here(None);
        let _ = std::fs::remove_dir_all(base);
    }
}
