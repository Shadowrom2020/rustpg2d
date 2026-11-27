//! The event sheet: what happens in the game as rows to read. A trigger is *When* (its event),
//! *If* (its conditions) and *Do* (its actions); every condition and action is one line that
//! says what it does (`editor_core::events`), opened in place to change it, dragged to another
//! place in its list, copied to another list. New actions are picked from a searched palette
//! grouped as the manual groups them.
//!
//! The sheet is how the forms show lists of triggers, conditions and actions wherever they
//! are (a placed character, a zone, a definition, the branches of an *If*): `form.rs` hands
//! such a list here. The values stay the files' own (JSON); nothing is stored differently.

use eframe::egui;
use editor_core::events;
use editor_core::schema::{Node, Shape};
use rustpg2d_engine::refactor::RefKind;
use serde_json::Value;

use crate::form::{self, Cx, Row};

/// What the rows of a list are.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Kind {
    Trigger,
    Condition,
    Action,
}

impl Kind {
    fn of(node: Node) -> Option<Kind> {
        match node.type_name() {
            "Trigger" => Some(Kind::Trigger),
            "Condition" => Some(Kind::Condition),
            "Action" => Some(Kind::Action),
            _ => None,
        }
    }

    /// (one of them, several of them)
    fn words(self) -> (&'static str, &'static str) {
        match self {
            Kind::Trigger => ("event", "events"),
            Kind::Condition => ("condition", "conditions"),
            Kind::Action => ("action", "actions"),
        }
    }
}

/// A row being dragged to another place of its list.
#[derive(Clone, Copy)]
struct RowDrag {
    list: egui::Id,
    from: usize,
}

/// What a row's menu or a drag asked for.
enum Op {
    Remove(usize),
    /// Takes the row out and puts it where row `to` is.
    Move { from: usize, to: usize },
    Duplicate(usize),
    /// Puts a copied row in before row `k` (or at the end).
    Paste(usize, Value),
}

fn clip_id(kind: Kind) -> egui::Id {
    egui::Id::unique(("event sheet clipboard", kind))
}

fn copied(ui: &egui::Ui, kind: Kind) -> Option<Value> {
    ui.data(|d| d.get_temp::<Value>(clip_id(kind)))
}

/// A definition's name, for the lines.
fn name_of(cx: &Cx, kind: RefKind, id: &str) -> Option<String> {
    cx.lists.iter().find(|(k, _)| *k == kind).and_then(|(_, l)| l.iter().find(|(i, _)| i == id)).map(|(_, n)| n.clone()).filter(|n| !n.is_empty())
}

fn line(cx: &Cx, kind: Kind, v: &Value) -> String {
    let names = |k: RefKind, id: &str| name_of(cx, k, id);
    match kind {
        Kind::Action => events::action_summary(v, &names),
        Kind::Condition => events::condition_summary(v, &names),
        Kind::Trigger => {
            let n = v.get("actions").and_then(Value::as_array).map_or(0, Vec::len);
            // (Its first condition tells two events of the same kind apart.)
            let conds = v.get("conditions").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
            let when = match conds {
                [] => String::new(),
                [one] => format!(", if {}", events::condition_summary(one, &names)),
                [first, more @ ..] => format!(", if {} and {} more", events::condition_summary(first, &names), more.len()),
            };
            format!("When {}{when} · {n} action{}", events::trigger_summary(v.get("on").unwrap_or(&Value::Null), &names), if n == 1 { "" } else { "s" })
        }
    }
}

/// A list of triggers, conditions or actions as rows of the sheet. `None`: the list holds
/// something else (the form shows it its own way).
pub fn list(ui: &mut egui::Ui, cx: &mut Cx, inner: Node, site: &str, list: &mut Vec<Value>, depth: usize) -> Option<bool> {
    let kind = Kind::of(inner)?;
    Some(match kind {
        Kind::Trigger => triggers(ui, cx, inner, site, list, depth),
        _ => rows(ui, cx, inner, site, list, depth, kind),
    })
}

/// Applies what a row asked for; `open`: the row that is open, which stays the same row.
fn apply(list: &mut Vec<Value>, op: Op, open: &mut Option<usize>) {
    match op {
        Op::Remove(k) if k < list.len() => {
            list.remove(k);
            *open = match *open {
                Some(o) if o == k => None,
                Some(o) if o > k => Some(o - 1),
                o => o,
            };
        }
        Op::Move { from, to } if from < list.len() && to < list.len() && from != to => {
            let item = list.remove(from);
            list.insert(to, item);
            *open = open.map(|o| if o == from { to } else if from < o && o <= to { o - 1 } else if to <= o && o < from { o + 1 } else { o });
        }
        Op::Duplicate(k) if k < list.len() => {
            list.insert(k + 1, list[k].clone());
            *open = open.map(|o| if o > k { o + 1 } else { o });
        }
        Op::Paste(k, v) => {
            let k = k.min(list.len());
            list.insert(k, v);
            *open = open.map(|o| if o >= k { o + 1 } else { o });
        }
        _ => {}
    }
}

/// A row's menu. Returns what was picked.
fn row_menu(ui: &mut egui::Ui, kind: Kind, k: usize, n: usize, item: &Value) -> Option<Op> {
    let mut op = None;
    if ui.add_enabled(k > 0, egui::Button::new("Move up")).clicked() {
        op = Some(Op::Move { from: k, to: k - 1 });
    }
    if ui.add_enabled(k + 1 < n, egui::Button::new("Move down")).clicked() {
        op = Some(Op::Move { from: k, to: k + 1 });
    }
    ui.separator();
    if ui.button("Duplicate").clicked() {
        op = Some(Op::Duplicate(k));
    }
    if ui.button("Copy").on_hover_text(format!("Pasted into any list of {}: another event, another character, another map", kind.words().1)).clicked() {
        ui.data_mut(|d| d.insert_temp(clip_id(kind), item.clone()));
        ui.close();
    }
    if ui.button("Cut").clicked() {
        ui.data_mut(|d| d.insert_temp(clip_id(kind), item.clone()));
        op = Some(Op::Remove(k));
    }
    if let Some(v) = copied(ui, kind) {
        if ui.button("Paste above").clicked() {
            op = Some(Op::Paste(k, v));
        }
    }
    ui.separator();
    if ui.button("Delete").clicked() {
        op = Some(Op::Remove(k));
    }
    if op.is_some() {
        ui.close();
    }
    op
}

/// Conditions or actions: a line each, the open one with its fields under it.
fn rows(ui: &mut egui::Ui, cx: &mut Cx, inner: Node, site: &str, list: &mut Vec<Value>, depth: usize, kind: Kind) -> bool {
    let list_id = ui.make_persistent_id(("event sheet", site, kind));
    let mut open: Option<usize> = ui.data(|d| d.get_temp::<Option<usize>>(list_id)).flatten().filter(|k| *k < list.len());
    let lines: Vec<String> = list.iter().map(|v| line(cx, kind, v)).collect();
    let n = list.len();
    let (mut changed, mut op) = (false, None);
    for (k, item) in list.iter_mut().enumerate() {
        ui.push_id(k, |ui| {
            let is_open = open == Some(k);
            let r = ui
                .horizontal(|ui| {
                    ui.weak(format!("{}.", k + 1));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("x").on_hover_text(format!("Remove this {}", kind.words().0)).clicked() {
                            op = Some(Op::Remove(k));
                        }
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| ui.add(egui::Button::selectable(is_open, lines[k].as_str()).truncate().sense(egui::Sense::click_and_drag()))).inner
                    })
                    .inner
                })
                .inner;
            if r.clicked() {
                open = (!is_open).then_some(k);
            }
            // Dragged onto another row of its list, it takes that row's place.
            r.dnd_set_drag_payload(RowDrag { list: list_id, from: k });
            if let Some(d) = r.dnd_hover_payload::<RowDrag>().filter(|d| d.list == list_id && d.from != k) {
                let y = if d.from < k { r.rect.bottom() } else { r.rect.top() };
                ui.painter().hline(r.rect.x_range(), y, egui::Stroke::new(2.0, ui.visuals().selection.stroke.color));
            }
            if let Some(d) = r.dnd_release_payload::<RowDrag>().filter(|d| d.list == list_id && d.from != k) {
                op = Some(Op::Move { from: d.from, to: k });
            }
            r.context_menu(|ui| {
                if let Some(picked) = row_menu(ui, kind, k, n, item) {
                    op = Some(picked);
                }
            });
            if is_open {
                ui.indent("open row", |ui| changed |= form::body(ui, cx, inner, site, item, depth + 1));
            }
        });
    }
    if let Some(op) = op {
        apply(list, op, &mut open);
        changed = true;
    }
    if let Some(new) = add_menu(ui, inner, site, kind) {
        list.push(new);
        open = Some(list.len() - 1);
        changed = true;
    }
    ui.data_mut(|d| d.insert_temp(list_id, open));
    changed
}

/// *+ Add…*: what a new row starts as, picked from a searched list (actions: by their groups).
fn add_menu(ui: &mut egui::Ui, inner: Node, site: &str, kind: Kind) -> Option<Value> {
    let (one, _) = kind.words();
    let mut pick = None;
    ui.horizontal(|ui| {
        ui.menu_button(format!("+ Add {} {one}…", if kind == Kind::Condition { "a" } else { "an" }), |ui| {
            ui.set_min_width(260.0);
            let fid = ui.scope_id().with("search");
            let mut filter: String = ui.data_mut(|d| d.get_temp(fid).unwrap_or_default());
            if kind != Kind::Trigger {
                let r = ui.add(egui::TextEdit::singleline(&mut filter).hint_text("Search").desired_width(f32::INFINITY));
                r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, format!("Search the {one}s")));
                if ui.memory(|m| m.focused().is_none()) {
                    r.request_focus();
                }
            }
            let f = filter.trim().to_lowercase();
            // What each kind is for, from the engine's own comments.
            let help = |variant: &str| match inner.shape() {
                Shape::Choice(variants) => variants.iter().find(|v| v.name == variant).map(|v| v.help()).unwrap_or_default(),
                _ => String::new(),
            };
            egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| match kind {
                Kind::Action => {
                    let all = events::action_templates();
                    for (group, _) in events::ACTION_GROUPS {
                        let rows: Vec<&(String, String, Value)> = all.iter().filter(|(label, variant, _)| events::action_group(variant) == *group && (f.is_empty() || label.to_lowercase().contains(&f))).collect();
                        if rows.is_empty() {
                            continue;
                        }
                        ui.weak(*group);
                        for (label, variant, value) in rows {
                            let r = ui.button(label.as_str());
                            let tip = help(variant);
                            let r = if tip.is_empty() { r } else { r.on_hover_text(tip) };
                            if r.clicked() {
                                pick = Some(value.clone());
                            }
                        }
                    }
                }
                Kind::Condition => {
                    for (label, value) in events::condition_templates().iter().filter(|(label, _)| f.is_empty() || label.to_lowercase().contains(&f)) {
                        if ui.button(label.as_str()).clicked() {
                            pick = Some(value.clone());
                        }
                    }
                }
                Kind::Trigger => {
                    let none = |_: RefKind, _: &str| None;
                    for value in events::trigger_templates(events::owner_of(site)) {
                        let when = events::trigger_summary(value.get("on").unwrap_or(&Value::Null), &none);
                        if ui.button(format!("When {when}")).clicked() {
                            pick = Some(value);
                        }
                    }
                }
            });
            if pick.is_some() {
                filter.clear();
                ui.close();
            }
            ui.data_mut(|d| d.insert_temp(fid, filter));
        });
        if let Some(v) = copied(ui, kind) {
            if ui.small_button("Paste").on_hover_text(format!("The {one} copied last, at the end")).clicked() {
                pick = Some(v);
            }
        }
    });
    pick
}

/// Triggers: each *When … · n actions*, opened as When / If / Do.
fn triggers(ui: &mut egui::Ui, cx: &mut Cx, inner: Node, site: &str, list: &mut Vec<Value>, depth: usize) -> bool {
    let Shape::Struct(fields) = inner.shape() else { return false };
    let list_id = ui.make_persistent_id(("event sheet", site, Kind::Trigger));
    let lines: Vec<String> = list.iter().map(|v| line(cx, Kind::Trigger, v)).collect();
    let n = list.len();
    let (mut changed, mut op) = (false, None);
    for (k, item) in list.iter_mut().enumerate() {
        let once = item.get("once").and_then(Value::as_bool).unwrap_or(false);
        let id = list_id.with(k);
        let state = egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, n == 1);
        let header = state.show_header(ui, |ui| {
            let r = ui.add(egui::Label::new(egui::RichText::new(lines[k].as_str()).strong()).truncate().sense(egui::Sense::click()));
            if once {
                ui.weak("once");
            }
            r.context_menu(|ui| {
                if let Some(picked) = row_menu(ui, Kind::Trigger, k, n, item) {
                    op = Some(picked);
                }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("x").on_hover_text("Remove this event with all it does").clicked() {
                    op = Some(Op::Remove(k));
                }
            });
        });
        header.body(|ui| {
            ui.push_id(k, |ui| {
                let Value::Object(map) = item else { return };
                for field in &fields {
                    let fsite = field.site();
                    let part = |ui: &mut egui::Ui, word: &str, tip: &str| {
                        ui.allocate_ui_with_layout(egui::vec2(34.0, ui.spacing().interact_size.y), egui::Layout::left_to_right(egui::Align::Center), |ui| ui.label(egui::RichText::new(word).strong()).on_hover_text(tip));
                    };
                    match field.key {
                        "on" => {
                            let v = map.entry("on").or_insert(Value::Null);
                            changed |= form::node_ui(ui, cx, field.node, Row { site: &fsite, label: "When", help: "The event this waits for", reset: false }, v, depth + 1);
                        }
                        "conditions" | "actions" => {
                            let Shape::List(of) = field.node.shape() else { continue };
                            let v = map.entry(field.key).or_insert_with(|| Value::Array(Vec::new()));
                            let Value::Array(items) = v else { continue };
                            let conditions = field.key == "conditions";
                            ui.horizontal_top(|ui| {
                                if conditions {
                                    part(ui, "If", "All of these must hold when the event comes; none: always");
                                } else {
                                    part(ui, "Do", "What happens then, from the first to the last");
                                }
                                ui.vertical(|ui| {
                                    if conditions && items.is_empty() {
                                        ui.weak("always");
                                    }
                                    changed |= rows(ui, cx, of, &fsite, items, depth + 1, if conditions { Kind::Condition } else { Kind::Action });
                                });
                            });
                        }
                        _ => {
                            let v = map.entry(field.key).or_insert_with(|| field.node.blank());
                            let (label, help) = (field.label(), field.node.help());
                            changed |= form::node_ui(ui, cx, field.node, Row { site: &fsite, label: &label, help: &help, reset: false }, v, depth + 1);
                        }
                    }
                }
            });
        });
        if k + 1 < n {
            ui.add_space(2.0);
        }
    }
    if let Some(op) = op {
        apply(list, op, &mut None);
        changed = true;
    }
    if let Some(new) = add_menu(ui, inner, site, Kind::Trigger) {
        list.push(new);
        // (A new event is open: its actions are what comes next.)
        let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), list_id.with(list.len() - 1), true);
        state.set_open(true);
        state.store(ui.ctx());
        changed = true;
    }
    changed
}

#[cfg(test)]
mod ui_tests {
    use super::*;
    use editor_core::schema;
    use egui_kittest::kittest::Queryable;
    use egui_kittest::Harness;

    struct State {
        /// Two owners' triggers: a character's and a prop's.
        elder: Vec<Value>,
        well: Vec<Value>,
        lists: Vec<(RefKind, Vec<(String, String)>)>,
    }

    fn sheet() -> Harness<'static, State> {
        let lists = vec![(RefKind::Item, vec![("potion".to_string(), "Healing Potion".to_string())])];
        Harness::builder().with_size(egui::vec2(900.0, 1400.0)).build_ui_state(
            |ui, s: &mut State| {
                let mut cx = Cx { lists: &s.lists, maps: &[], show_all: false, compact: false, pictures: None, refs: None, asks: Vec::new() };
                ui.push_id("elder", |ui| list(ui, &mut cx, schema::trigger(), "CharacterDef.triggers", &mut s.elder, 1));
                ui.separator();
                ui.push_id("well", |ui| list(ui, &mut cx, schema::trigger(), "PropDef.triggers", &mut s.well, 1));
            },
            State { elder: Vec::new(), well: Vec::new(), lists },
        )
    }

    fn none(_: RefKind, _: &str) -> Option<String> {
        None
    }

    /// The palette's name of an action kind.
    fn label_of(variant: &str) -> &'static str {
        events::action_templates().iter().find(|(_, v, _)| v == variant).map(|(l, _, _)| l.as_str()).unwrap()
    }

    fn kinds(actions: &Value) -> Vec<String> {
        actions.as_array().unwrap().iter().map(|a| events::variant(a).0.to_string()).collect()
    }

    #[test]
    fn an_event_is_built_reordered_and_copied_to_another_owner() {
        let mut h = sheet();
        h.run();
        // A new event of the character: what a character's events can be, the likeliest first.
        h.query_all_by_label("+ Add an event…").next().unwrap().click();
        h.run();
        assert!(h.query_all_by_label("When the player steps in").next().is_none(), "a character never gets a zone's event");
        assert!(h.query_all_by_label("When it is defeated").next().is_some());
        h.get_by_label("When the player uses it or talks to it").click();
        h.run();
        assert_eq!(h.state().elder.len(), 1);
        assert_eq!(h.state().elder[0]["on"], "Interact");
        // It is open: a condition, then three actions from the palette.
        assert!(h.query_all_by_label("always").next().is_some());
        h.get_by_label("+ Add a condition…").click();
        h.run();
        let first = events::condition_templates()[0].0.clone();
        h.get_by_label(first.as_str()).click();
        h.run();
        assert_eq!(h.state().elder[0]["conditions"].as_array().unwrap().len(), 1);
        for variant in ["Dialog", "GiveItem", "SetFlag"] {
            h.get_by_label("+ Add an action…").click();
            h.run();
            // (The palette is grouped as the manual groups the actions.)
            assert!(h.query_all_by_label("Talking").next().is_some() && h.query_all_by_label("Game state").next().is_some());
            h.get_by_role_and_label(egui::accesskit::Role::Button, label_of(variant)).click();
            h.run();
        }
        assert_eq!(kinds(&h.state().elder[0]["actions"]), ["Dialog", "GiveItem", "SetFlag"]);
        // The last row is open with its fields; closed, each row is one line of what it does.
        assert!(h.query_all_by_label("Kind").next().is_some());
        let flag_line = events::action_summary(&h.state().elder[0]["actions"][2], &none);
        h.get_by_role_and_label(egui::accesskit::Role::Button, flag_line.as_str()).click();
        h.run();
        assert!(h.query_all_by_label("Kind").next().is_none());
        // Reordered from its menu.
        h.get_by_role_and_label(egui::accesskit::Role::Button, flag_line.as_str()).click_secondary();
        h.run();
        h.get_by_label("Move up").click();
        h.run();
        assert_eq!(kinds(&h.state().elder[0]["actions"]), ["Dialog", "SetFlag", "GiveItem"]);
        // An action copied: pasted at the end of the same list.
        h.get_by_role_and_label(egui::accesskit::Role::Button, flag_line.as_str()).click_secondary();
        h.run();
        h.get_by_label("Duplicate").click();
        h.run();
        assert_eq!(kinds(&h.state().elder[0]["actions"]), ["Dialog", "SetFlag", "SetFlag", "GiveItem"]);
        // The whole event copied to the prop.
        let header = line(&Cx { lists: &h.state().lists, maps: &[], show_all: false, compact: false, pictures: None, refs: None, asks: Vec::new() }, Kind::Trigger, &h.state().elder[0]);
        assert!(header.starts_with("When the player uses it or talks to it, if ") && header.ends_with(" · 4 actions"), "{header}");
        h.get_by_label(header.as_str()).click_secondary();
        h.run();
        h.get_by_label("Copy").click();
        h.run();
        // (Both lists offer Paste now; the second is the prop's.)
        h.query_all_by_label("Paste").nth(1).unwrap().click();
        h.run();
        assert_eq!(h.state().well, h.state().elder);
        // Removed with its x.
        h.query_all_by_label("x").next().unwrap().click();
        h.run();
        assert!(h.state().elder.is_empty() && h.state().well.len() == 1);
    }

    #[test]
    fn rows_keep_their_place_when_one_is_moved_or_removed() {
        let mut list: Vec<Value> = (0..5).map(Value::from).collect();
        let mut open = Some(3);
        apply(&mut list, Op::Move { from: 0, to: 4 }, &mut open);
        assert_eq!((list.clone(), open), (vec![1.into(), 2.into(), 3.into(), 4.into(), 0.into()], Some(2)));
        apply(&mut list, Op::Remove(0), &mut open);
        assert_eq!(open, Some(1));
        apply(&mut list, Op::Paste(0, 9.into()), &mut open);
        assert_eq!((list[0].clone(), open), (9.into(), Some(2)));
        apply(&mut list, Op::Duplicate(2), &mut open);
        assert_eq!((list.len(), list[2].clone(), list[3].clone(), open), (6, 3.into(), 3.into(), Some(2)));
        apply(&mut list, Op::Remove(2), &mut open);
        assert_eq!(open, None);
        apply(&mut list, Op::Move { from: 9, to: 0 }, &mut open);
        assert_eq!(list.len(), 5);
    }
}
