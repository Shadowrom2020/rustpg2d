//! The form of a definition, made from the field schema (`editor_core::schema`): sections, a
//! label and help for every field, the control that fits what the field holds (a choice for an
//! enum, a picker for an id, a fixed row for a size's two numbers, lists with add / remove /
//! move, groups that fold). It edits the definition's JSON form; the workspace turns that back
//! into the definition and refuses what isn't one.
//!
//! Fields a file leaves out (they hold their default) are shown with that default, so a form
//! shows everything a definition can have. Parts with an editor of their own (triggers, stat
//! blocks, buildings) come as hand-made controls plugged in here; until then they are forms too.

use eframe::egui;
use editor_core::schema::{self, Field, Node, Shape, Variant};
use egui::collapsing_header::CollapsingState;
use rustpg2d_engine::refactor::RefKind;
use serde_json::Value;

/// Lists longer than this aren't laid out (a structure's cells): they stay as they are.
const MAX_LIST: usize = 200;
/// A picker shows this many matches (type to narrow them down).
const MAX_PICKS: usize = 150;
const LABEL_WIDTH: f32 = 190.0;

/// What a form asks the window for.
pub enum Ask {
    /// Open this definition (or map) in a tab.
    Open(RefKind, String),
    /// Make a new definition of this kind.
    New(RefKind),
}

/// The pictures a form can show: a sprite, a definition's thumbnail, a sheet's frames to pick
/// from. (A form without them, in tests, shows the fields alone.)
pub trait Pictures {
    fn sprite(&mut self, sprite: &rustpg2d_engine::defs::SpriteRef) -> Option<crate::thumbs::Thumb>;
    fn of(&mut self, kind: RefKind, id: &str) -> Option<crate::thumbs::Thumb>;
    fn frames(&self, sheet: &str) -> Option<crate::thumbs::SheetFrames>;
}

/// What a form needs from around it.
pub struct Cx<'a> {
    /// Id and name of every definition, by kind (the pickers' lists).
    pub lists: &'a [(RefKind, Vec<(String, String)>)],
    pub maps: &'a [String],
    /// Also the fields for advanced use.
    pub show_all: bool,
    /// A table's cells: every text on one line.
    pub compact: bool,
    pub pictures: Option<&'a mut dyn Pictures>,
    /// The names the project uses (flags, variables, script functions…), offered where one is typed.
    pub refs: Option<&'a editor_core::refs::Refs>,
    pub asks: Vec<Ask>,
}

impl Cx<'_> {
    /// Id and name of everything of `kind` a field can name.
    fn ids(&self, kind: RefKind) -> Vec<(String, String)> {
        match kind {
            RefKind::Map => self.maps.iter().map(|m| (m.clone(), m.clone())).collect(),
            // Proficiencies have no definitions: the SRD's skills and tools.
            RefKind::Skill => {
                use rustpg2d_engine::srd::abilities::{SKILLS, TOOLS};
                SKILLS.iter().chain(TOOLS.iter()).map(|(id, name, _)| (id.to_string(), name.to_string())).collect()
            }
            _ => self.lists.iter().find(|(k, _)| *k == kind).map(|(_, l)| l.clone()).unwrap_or_default(),
        }
    }

    /// The names in use of a kind a text field takes.
    fn names(&self, names: schema::Names) -> Vec<String> {
        use schema::Names;
        let Some(r) = self.refs else { return Vec::new() };
        let mut list = match names {
            Names::Function => r.functions.clone(),
            Names::Flag => r.all_flags.clone(),
            Names::Variable => r.vars.clone(),
            Names::Faction => r.factions.clone(),
            Names::State => r.states.clone(),
            Names::Target => r.targets.iter().chain(&r.entity_names).cloned().collect(),
            Names::Station => r.stations.clone(),
            Names::Emotion => r.emotions.clone(),
        };
        let mut seen = std::collections::HashSet::new();
        list.retain(|n| !n.is_empty() && seen.insert(n.clone()));
        list
    }

    /// The kind and name of `id` among `kinds`.
    fn find(&self, kinds: &[RefKind], id: &str) -> Option<(RefKind, String)> {
        kinds.iter().find_map(|k| self.ids(*k).into_iter().find(|(i, _)| i == id).map(|(_, name)| (*k, name)))
    }
}

/// A few words on what a list entry or group is: its variant, its id, name or first text.
fn summary(v: &Value) -> String {
    let short = |s: &str| -> String {
        let line = s.lines().next().unwrap_or("");
        if line.chars().count() > 48 {
            format!("{}…", line.chars().take(48).collect::<String>())
        } else {
            line.to_string()
        }
    };
    match v {
        // A trigger: when it runs and how much it does.
        Value::Object(m) if m.contains_key("on") && m.contains_key("actions") => {
            let n = m["actions"].as_array().map_or(0, Vec::len);
            format!("When: {} · {n} action{}", summary(&m["on"]), if n == 1 { "" } else { "s" })
        }
        Value::Object(m) => {
            let named = ["name", "id", "title", "label", "item", "def", "text", "spell", "sound", "function"].iter().find_map(|k| m.get(*k).and_then(|x| x.as_str()).filter(|s| !s.is_empty()));
            match named {
                Some(s) => short(s),
                // An enum with data: its variant, and what that says of itself.
                None if m.len() == 1 && m.keys().next().is_some_and(|k| k.starts_with(char::is_uppercase)) => {
                    let (name, data) = m.iter().next().map(|(k, v)| (schema::variant_label(k), summary(v))).unwrap_or_default();
                    if data.is_empty() || data == "none" {
                        name
                    } else {
                        format!("{name}: {data}")
                    }
                }
                None => m.values().find_map(|x| x.as_str()).map(short).unwrap_or_default(),
            }
        }
        Value::String(s) => short(s),
        Value::Null => "none".into(),
        Value::Array(a) => format!("{} entries", a.len()),
        other => other.to_string(),
    }
}

/// Forms shown from here on are in a narrow panel (a map's Inspector): shorter labels and
/// fields. Switch it off again after the form.
pub fn set_narrow(ctx: &egui::Context, narrow: bool) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::unique("narrow forms"), narrow));
}

fn narrow(ui: &egui::Ui) -> bool {
    ui.data(|d| d.get_temp(egui::Id::unique("narrow forms"))).unwrap_or(false)
}

fn label_width(ui: &egui::Ui) -> f32 {
    if narrow(ui) {
        112.0
    } else {
        LABEL_WIDTH
    }
}

/// The width of a text field: `wide` in a form with room, less in a narrow one.
fn text_width(ui: &egui::Ui, wide: f32) -> f32 {
    if narrow(ui) {
        (wide * 0.6).min(190.0)
    } else {
        wide
    }
}

/// The label of a row, with the field's help when the pointer rests on it. Returns the label's
/// id: the row's controls are named by it (screen readers, and the tests, find "Stack size").
fn label_cell(ui: &mut egui::Ui, label: &str, help: &str) -> egui::Id {
    let width = label_width(ui);
    let size = egui::vec2(width, ui.spacing().interact_size.y);
    ui.allocate_ui_with_layout(size, egui::Layout::left_to_right(egui::Align::Center), |ui| {
        ui.set_min_width(width);
        let r = ui.add(egui::Label::new(label).truncate());
        let id = r.id;
        if !help.is_empty() {
            r.on_hover_text(help);
        }
        id
    })
    .inner
}

/// The label of a field's row; true when *Back to the default* was picked from its menu.
fn field_label(ui: &mut egui::Ui, row: Row) -> (egui::Id, bool) {
    let width = label_width(ui);
    let size = egui::vec2(width, ui.spacing().interact_size.y);
    ui.allocate_ui_with_layout(size, egui::Layout::left_to_right(egui::Align::Center), |ui| {
        ui.set_min_width(width);
        let mut r = ui.add(egui::Label::new(row.label).truncate().sense(if row.reset { egui::Sense::click() } else { egui::Sense::hover() }));
        let id = r.id;
        if !row.help.is_empty() {
            r = r.on_hover_text(row.help);
        }
        let mut reset = false;
        if row.reset {
            r.context_menu(|ui| {
                if ui.button("Back to the default").clicked() {
                    reset = true;
                    ui.close();
                }
            });
        }
        (id, reset)
    })
    .inner
}

/// A group's header: folds, shows the label with its help and a few words on what is inside
/// (`[label, help, about]`), and what `extra` adds at its end. Returns (changed in the body, what `extra` returned).
fn group<R>(ui: &mut egui::Ui, salt: impl std::hash::Hash + std::fmt::Debug, [label, help, about]: [&str; 3], open: bool, extra: impl FnOnce(&mut egui::Ui) -> R, body: impl FnOnce(&mut egui::Ui) -> bool) -> (bool, R) {
    let id = ui.make_persistent_id(salt);
    let (_, header, body) = CollapsingState::load_with_default_open(ui.ctx(), id, open)
        .show_header(ui, |ui| {
            let r = ui.label(label);
            if !help.is_empty() {
                r.on_hover_text(help);
            }
            if !about.is_empty() {
                ui.weak(about);
            }
            extra(ui)
        })
        .body(body);
    (body.is_some_and(|b| b.inner), header.inner)
}

/// A text as a colour and back ("#rrggbb", "#rrggbbaa").
fn color(ui: &mut egui::Ui, s: &mut String, by: egui::Id) -> bool {
    let hex = s.trim_start_matches('#');
    let part = |k: usize| u8::from_str_radix(hex.get(k * 2..k * 2 + 2).unwrap_or("ff"), 16).unwrap_or(255);
    let mut rgba = [part(0), part(1), part(2), if hex.len() >= 8 { part(3) } else { 255 }];
    let changed = ui.color_edit_button_srgba_unmultiplied(&mut rgba).labelled_by(by).changed();
    if changed {
        *s = format!("#{:02x}{:02x}{:02x}", rgba[0], rgba[1], rgba[2]);
        if rgba[3] != 255 {
            s.push_str(&format!("{:02x}", rgba[3]));
        }
    }
    ui.weak(s.as_str());
    changed
}

/// The picker of an id of `kinds`: what it names now (missing ones in red), a searchable list,
/// *none*, a new one, and a link to what it names.
fn picker(ui: &mut egui::Ui, cx: &mut Cx, kinds: &[RefKind], s: &mut String, by: egui::Id) -> bool {
    let known = cx.find(kinds, s);
    let text: egui::WidgetText = match (&known, s.is_empty()) {
        (_, true) => egui::RichText::new("none").weak().into(),
        (Some((_, name)), _) if name != s => format!("{name}  ({s})").into(),
        (Some(_), _) => s.clone().into(),
        (None, _) => egui::RichText::new(format!("{s}  (missing)")).color(ui.visuals().error_fg_color).into(),
    };
    let mut changed = false;
    let combo = egui::ComboBox::from_id_salt("pick").selected_text(text).width(280.0).height(340.0).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show_ui(ui, |ui| {
        let fid = ui.scope_id().with("filter");
        let mut filter: String = ui.data_mut(|d| d.get_temp(fid).unwrap_or_default());
        let r = ui.add(egui::TextEdit::singleline(&mut filter).hint_text("Search").desired_width(f32::INFINITY));
        r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, "Search"));
        if ui.memory(|m| m.focused().is_none()) {
            r.request_focus();
        }
        let f = filter.trim().to_lowercase();
        let mut pick = None;
        if ui.selectable_label(s.is_empty(), egui::RichText::new("none").weak()).clicked() {
            pick = Some(String::new());
        }
        for kind in kinds {
            let all = cx.ids(*kind);
            let hits: Vec<&(String, String)> = all.iter().filter(|(id, name)| f.is_empty() || id.to_lowercase().contains(&f) || name.to_lowercase().contains(&f)).collect();
            if kinds.len() > 1 {
                ui.weak(crate::shell::plural(kind.label()));
            }
            let pictures = crate::thumbs::has_pictures(*kind) && cx.pictures.is_some();
            for (id, name) in hits.iter().take(MAX_PICKS) {
                let label = if name == id { id.clone() } else { format!("{name}  ({id})") };
                let clicked = if pictures {
                    ui.horizontal(|ui| {
                        crate::thumbs::boxed(ui, 20.0, || cx.pictures.as_mut().and_then(|p| p.of(*kind, id)));
                        ui.selectable_label(id == s, label).clicked()
                    })
                    .inner
                } else {
                    ui.selectable_label(id == s, label).clicked()
                };
                if clicked {
                    pick = Some(id.clone());
                }
            }
            if hits.len() > MAX_PICKS {
                ui.weak(format!("… {} more: type to find them", hits.len() - MAX_PICKS));
            }
        }
        ui.separator();
        for kind in kinds.iter().filter(|k| k.is_def()) {
            if ui.button(format!("+ New {}…", kind.label().to_lowercase())).clicked() {
                cx.asks.push(Ask::New(*kind));
                ui.close();
            }
        }
        if let Some(id) = pick {
            *s = id;
            changed = true;
            filter.clear();
            ui.close();
        }
        ui.data_mut(|d| d.insert_temp(fid, filter));
    });
    combo.response.labelled_by(by);
    if let Some((kind, _)) = known.filter(|(k, _)| *k != RefKind::Skill) {
        if ui.small_button("Open").on_hover_text(format!("Open this {} in a tab", kind.label().to_lowercase())).clicked() {
            cx.asks.push(Ask::Open(kind, s.clone()));
        }
    }
    changed
}

/// A name the project makes up (a flag, a script function): typed, or taken from the ones in
/// use, which the button beside the field lists (those that contain what is typed first).
fn named(ui: &mut egui::Ui, known: &[String], s: &mut String, by: egui::Id) -> bool {
    let mut changed = ui.add(egui::TextEdit::singleline(s).desired_width(text_width(ui, 230.0))).labelled_by(by).changed();
    if known.is_empty() {
        return changed;
    }
    let typed = s.trim().to_lowercase();
    let pick = ui
        .menu_button(format!("{} in use", known.len()), |ui| {
            let mut pick = None;
            egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                let (first, rest): (Vec<&String>, Vec<&String>) = known.iter().partition(|n| !typed.is_empty() && n.to_lowercase().contains(&typed));
                for name in first.into_iter().chain(rest).take(300) {
                    if ui.selectable_label(name == s, name.as_str()).clicked() {
                        pick = Some(name.clone());
                    }
                }
            });
            if pick.is_some() {
                ui.close();
            }
            pick
        })
        .inner
        .flatten();
    if let Some(name) = pick {
        *s = name;
        changed = true;
    }
    changed
}

/// The choice among an enum's variants. Returns the one picked now.
fn variant_combo(ui: &mut egui::Ui, variants: &[Variant], now: Option<usize>, by: egui::Id) -> Option<usize> {
    let text = now.map_or_else(|| "?".to_string(), |k| schema::variant_label(variants[k].name));
    let mut pick = None;
    let many = variants.len() > 12;
    let behavior = if many { egui::PopupCloseBehavior::CloseOnClickOutside } else { egui::PopupCloseBehavior::CloseOnClick };
    let r = egui::ComboBox::from_id_salt("variant").selected_text(text).width(200.0).height(340.0).close_behavior(behavior).show_ui(ui, |ui| {
        pick = variant_list(ui, variants, now, many);
        if pick.is_some() {
            ui.close();
        }
    });
    let r = r.response.labelled_by(by);
    if let Some(k) = now.filter(|k| !variants[*k].help().is_empty()) {
        r.on_hover_text(variants[k].help());
    }
    pick
}

/// The variants as a list to click, with a search field when there are many.
fn variant_list(ui: &mut egui::Ui, variants: &[Variant], now: Option<usize>, search: bool) -> Option<usize> {
    let fid = ui.scope_id().with("filter");
    let mut filter: String = ui.data_mut(|d| d.get_temp(fid).unwrap_or_default());
    if search {
        let r = ui.add(egui::TextEdit::singleline(&mut filter).hint_text("Search").desired_width(f32::INFINITY));
        r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, "Search"));
        if ui.memory(|m| m.focused().is_none()) {
            r.request_focus();
        }
    }
    let f = filter.trim().to_lowercase();
    let mut pick = None;
    for (k, v) in variants.iter().enumerate() {
        let label = schema::variant_label(v.name);
        if !f.is_empty() && !label.to_lowercase().contains(&f) {
            continue;
        }
        let r = ui.selectable_label(now == Some(k), label);
        let help = v.help();
        if r.clicked() {
            pick = Some(k);
        }
        if !help.is_empty() {
            r.on_hover_text(help);
        }
    }
    if pick.is_some() {
        filter.clear();
    }
    ui.data_mut(|d| d.insert_temp(fid, filter));
    pick
}

/// The fields of an enum value with data, when it has them by name.
fn data_of(v: &Value) -> Option<&serde_json::Map<String, Value>> {
    v.as_object().filter(|m| m.len() == 1).and_then(|m| m.values().next()).and_then(Value::as_object)
}

/// `variant` with the values it starts with, keeping what the old variant had under the same
/// names (the target of an action stays when its kind changes).
fn switched(old: &Value, variant: &Variant) -> Value {
    let mut new = variant.blank();
    if let Some(had) = data_of(old).cloned() {
        if let Some(fields) = new.as_object_mut().and_then(|m| m.values_mut().next()).and_then(Value::as_object_mut) {
            for (key, v) in fields.iter_mut() {
                if let Some(o) = had.get(key).filter(|o| std::mem::discriminant(*o) == std::mem::discriminant(v)) {
                    *v = o.clone();
                }
            }
        }
    }
    new
}

/// Whether a value of this shape fits on a row beside its label.
fn is_inline(shape: &Shape) -> bool {
    match shape {
        Shape::Text | Shape::Color | Shape::Int { .. } | Shape::Float | Shape::Bool => true,
        Shape::Choice(variants) => variants.iter().all(|v| v.data.is_none()),
        Shape::Fixed(nodes) => nodes.len() <= 4 && nodes.iter().all(|n| matches!(n.shape(), Shape::Text | Shape::Int { .. } | Shape::Float | Shape::Bool)),
        Shape::Optional(inner) => is_inline(&inner.shape()),
        _ => false,
    }
}

/// The control of a value that fits on a row. True when it changed.
fn inline(ui: &mut egui::Ui, cx: &mut Cx, node: Node, site: &str, v: &mut Value, by: egui::Id) -> bool {
    match node.shape() {
        Shape::Text => {
            let kinds = schema::reference(site);
            if !v.is_string() {
                *v = Value::String(String::new());
            }
            let Value::String(s) = v else { return false };
            let key = site.rsplit('.').next().unwrap_or("");
            if !kinds.is_empty() {
                picker(ui, cx, kinds, s, by)
            } else if let Some(names) = schema::names(site, key) {
                named(ui, &cx.names(names), s, by)
            } else if !cx.compact && (s.contains('\n') || s.len() > 60) {
                ui.add(egui::TextEdit::multiline(s).desired_rows(2).desired_width(text_width(ui, 460.0))).labelled_by(by).changed()
            } else {
                ui.add(egui::TextEdit::singleline(s).desired_width(text_width(ui, 280.0))).labelled_by(by).changed()
            }
        }
        Shape::Color => {
            if !v.is_string() {
                *v = Value::String("#ffffff".into());
            }
            let Value::String(s) = v else { return false };
            color(ui, s, by)
        }
        Shape::Int { min, max } => {
            let mut i = v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)).unwrap_or(0);
            let range = min.unwrap_or(i64::MIN)..=max.unwrap_or(i64::MAX);
            let changed = ui.add(egui::DragValue::new(&mut i).range(range)).labelled_by(by).changed();
            if changed {
                *v = Value::from(i);
            }
            changed
        }
        Shape::Float => {
            let mut f = v.as_f64().unwrap_or(0.0);
            let changed = ui.add(egui::DragValue::new(&mut f).speed(0.05).max_decimals(3)).labelled_by(by).changed();
            if changed {
                // (As the engine's f32 writes it: 0.4, not 0.4000000059604645.)
                *v = serde_json::Number::from_f64(f).map_or(Value::Null, Value::Number);
            }
            changed
        }
        Shape::Bool => {
            let mut b = v.as_bool().unwrap_or(false);
            let changed = ui.checkbox(&mut b, "").labelled_by(by).changed();
            if changed {
                *v = Value::Bool(b);
            }
            changed
        }
        Shape::Choice(variants) => match variant_combo(ui, &variants, schema::variant_of(&variants, v), by) {
            Some(k) => {
                *v = variants[k].blank();
                true
            }
            None => false,
        },
        Shape::Fixed(nodes) => {
            if v.as_array().is_none_or(|a| a.len() != nodes.len()) {
                *v = node.blank();
            }
            let mut changed = false;
            for (k, (n, item)) in nodes.iter().zip(v.as_array_mut().into_iter().flatten()).enumerate() {
                ui.push_id(k, |ui| changed |= inline(ui, cx, *n, &format!("{site}.{k}"), item, by));
            }
            changed
        }
        Shape::Optional(inner) => {
            if v.is_null() {
                ui.weak("none");
                if ui.small_button("Set").clicked() {
                    *v = inner.blank();
                    // (A default of "nothing": the plainest value instead.)
                    if v.is_null() {
                        *v = match inner.shape() {
                            Shape::Text => Value::String(String::new()),
                            Shape::Bool => Value::Bool(false),
                            Shape::Float => Value::from(0.0),
                            _ => Value::from(0),
                        };
                    }
                    return true;
                }
                false
            } else {
                let mut changed = inline(ui, cx, inner, site, v, by);
                if ui.small_button("x").on_hover_text("Back to none").clicked() {
                    *v = Value::Null;
                    changed = true;
                }
                changed
            }
        }
        _ => false,
    }
}

/// Where a value is and what it is called: its site in the schema (`ItemDef.max_stack`), its
/// label and its help.
#[derive(Clone, Copy)]
pub struct Row<'a> {
    pub site: &'a str,
    pub label: &'a str,
    pub help: &'a str,
    /// A field that holds something else than its default: its label's menu (right-click)
    /// puts it back.
    pub reset: bool,
}

/// Whether a field's value fits into a table's cell (a text, a number, a switch, a choice,
/// an id).
pub fn fits_cell(field: &Field) -> bool {
    is_inline(&field.node.shape())
}

/// How wide a table's column for the field is, in points.
pub fn cell_width(field: &Field) -> f32 {
    fn width(shape: &Shape, picker: bool) -> f32 {
        match shape {
            Shape::Text if picker => 360.0,
            Shape::Text | Shape::Color => 300.0,
            Shape::Int { .. } | Shape::Float => 76.0,
            Shape::Bool => 48.0,
            Shape::Choice(_) => 224.0,
            Shape::Fixed(nodes) => nodes.len() as f32 * 60.0 + 16.0,
            Shape::Optional(inner) => width(&inner.shape(), picker) + 34.0,
            _ => 120.0,
        }
    }
    width(&field.node.shape(), !field.reference().is_empty() || schema::names(&field.site(), field.key).is_some())
}

/// A field's control alone, as a table's cell shows it (the default while the object leaves
/// the field out). `by`: the column's header, which names the control. True when it changed.
pub fn cell(ui: &mut egui::Ui, cx: &mut Cx, field: &Field, map: &mut serde_json::Map<String, Value>, by: egui::Id) -> bool {
    let site = field.site();
    match map.get_mut(field.key) {
        Some(v) => inline(ui, cx, field.node, &site, v, by),
        None => {
            let mut v = field.node.blank();
            let changed = inline(ui, cx, field.node, &site, &mut v, by);
            if changed {
                map.insert(field.key.to_string(), v);
            }
            changed
        }
    }
}

/// A value under its label: on a row when it fits there, else as a group that folds. True when
/// it changed.
pub fn node_ui(ui: &mut egui::Ui, cx: &mut Cx, node: Node, row: Row, v: &mut Value, depth: usize) -> bool {
    let Row { site, label, help, .. } = row;
    let shape = node.shape();
    if is_inline(&shape) {
        return ui
            .horizontal(|ui| {
                let (by, reset) = field_label(ui, row);
                if reset {
                    *v = node.blank();
                    return true;
                }
                inline(ui, cx, node, site, v, by)
            })
            .inner;
    }
    if depth > 24 {
        ui.weak(format!("{label}: nested too deep to show"));
        return false;
    }
    match shape {
        // Nothing or a group: *Add* when there is none, *Remove* in its header.
        Shape::Optional(inner) => {
            if v.is_null() {
                ui.horizontal(|ui| {
                    label_cell(ui, label, help);
                    ui.weak("none");
                    if ui.small_button("+ Add").clicked() {
                        *v = inner.blank();
                        if v.is_null() {
                            *v = blank_past_default(inner);
                        }
                        true
                    } else {
                        false
                    }
                })
                .inner
            } else {
                let about = summary(v);
                let (mut changed, remove) = group(ui, (label, "opt"), [label, help, &about], depth <= 1, |ui| ui.small_button("Remove").clicked(), |ui| body(ui, cx, inner, site, v, depth + 1));
                if remove {
                    *v = Value::Null;
                    changed = true;
                }
                changed
            }
        }
        // One of several kinds, some with fields: the kind on the row, its fields under it.
        Shape::Choice(variants) => {
            let now = schema::variant_of(&variants, v);
            let pick = ui
                .horizontal(|ui| {
                    let (by, reset) = field_label(ui, row);
                    (variant_combo(ui, &variants, now, by), reset)
                })
                .inner;
            let (pick, reset) = pick;
            if reset {
                *v = node.blank();
                return true;
            }
            let mut changed = false;
            if let Some(k) = pick {
                *v = switched(v, &variants[k]);
                changed = true;
            }
            if let Some(n) = schema::variant_of(&variants, v).and_then(|k| variants[k].data) {
                ui.indent("variant", |ui| changed |= variant_data(ui, cx, n, v, depth + 1));
            }
            changed
        }
        Shape::List(_) | Shape::Map(_) | Shape::Fixed(_) => {
            let n = v.as_array().map(Vec::len).or_else(|| v.as_object().map(serde_json::Map::len)).unwrap_or(0);
            let about = if n == 0 { "none".to_string() } else { format!("{n}") };
            // (Events are open, also while there are none: what happens is what one comes for.)
            let events = matches!(&shape, Shape::List(inner) if inner.type_name() == "Trigger");
            group(ui, (label, "list"), [label, help, &about], depth <= 1 && (events || (n > 0 && n <= 6)), |_| (), |ui| body(ui, cx, node, site, v, depth + 1)).0
        }
        _ => {
            let about = summary(v);
            group(ui, (label, "group"), [label, help, &about], depth <= 1, |_| (), |ui| body(ui, cx, node, site, v, depth + 1)).0
        }
    }
}

/// A value to start with where the default is "nothing".
fn blank_past_default(node: Node) -> Value {
    match node.shape() {
        Shape::Struct(fields) => Value::Object(fields.iter().map(|f| (f.key.to_string(), f.node.blank())).collect()),
        Shape::Choice(variants) => variants.first().map_or(Value::Null, Variant::blank),
        Shape::List(_) | Shape::Fixed(_) => Value::Array(Vec::new()),
        Shape::Map(_) => Value::Object(Default::default()),
        _ => Value::Null,
    }
}

/// One field of an object: shown with its default while the object leaves it out.
fn field_ui(ui: &mut egui::Ui, cx: &mut Cx, field: &Field, map: &mut serde_json::Map<String, Value>, depth: usize) -> bool {
    let (site, label, help) = (field.site(), field.label(), field.node.help());
    // What still holds its default is shown dimmed: what was set stands out.
    let at_default = map.get(field.key).is_none_or(|v| *v == field.node.blank());
    let row = Row { site: &site, label: &label, help: &help, reset: !at_default };
    ui.push_id(field.key, |ui| {
        if at_default {
            ui.visuals_mut().widgets.noninteractive.fg_stroke.color = ui.visuals().weak_text_color();
        }
        field_value_ui(ui, cx, field, row, map, depth)
    })
    .inner
}

/// The field's value, or its default while the object leaves it out.
fn field_value_ui(ui: &mut egui::Ui, cx: &mut Cx, field: &Field, row: Row, map: &mut serde_json::Map<String, Value>, depth: usize) -> bool {
    match map.get_mut(field.key) {
        Some(v) => node_ui(ui, cx, field.node, row, v, depth),
        None => {
            let mut v = field.node.blank();
            let changed = node_ui(ui, cx, field.node, row, &mut v, depth);
            if changed {
                map.insert(field.key.to_string(), v);
            }
            changed
        }
    }
}

/// What an enum value's variant holds (`n`): its fields, or its one value.
fn variant_data(ui: &mut egui::Ui, cx: &mut Cx, n: Node, v: &mut Value, depth: usize) -> bool {
    let Some(data) = v.as_object_mut().and_then(|m| m.values_mut().next()) else { return false };
    // A variant with one value (not fields): its site is the variant itself.
    let site = format!("{}.{}", n.ty, n.variant);
    if matches!(n.shape(), Shape::Struct(_)) {
        body(ui, cx, n, &site, data, depth)
    } else {
        node_ui(ui, cx, n, Row { site: &site, label: "Value", help: "", reset: false }, data, depth)
    }
}

/// A sprite: its picture beside its fields, and the sheet's frames to pick one from.
fn sprite_ui(ui: &mut egui::Ui, cx: &mut Cx, fields: &[Field], v: &mut Value, depth: usize) -> bool {
    let now: Option<rustpg2d_engine::defs::SpriteRef> = serde_json::from_value(v.clone()).ok();
    let Value::Object(map) = v else { return false };
    let mut changed = false;
    ui.horizontal(|ui| {
        crate::thumbs::boxed(ui, 64.0, || cx.pictures.as_mut().zip(now.as_ref()).and_then(|(p, s)| p.sprite(s)));
        ui.vertical(|ui| {
            for field in fields {
                changed |= field_ui(ui, cx, field, map, depth);
            }
            let sheet = now.as_ref().map(|s| s.sheet.clone()).unwrap_or_default();
            let Some(frames) = cx.pictures.as_ref().and_then(|p| p.frames(&sheet)).filter(|f| f.count > 1) else { return };
            let pick = ui
                .menu_button("Pick the frame…", |ui| {
                    let mut pick = None;
                    egui::ScrollArea::both().max_height(380.0).max_width(560.0).show(ui, |ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(2.0, 2.0);
                        for row in 0..frames.count.div_ceil(frames.columns) {
                            ui.horizontal(|ui| {
                                for frame in (row * frames.columns..((row + 1) * frames.columns).min(frames.count)).take(64) {
                                    let (rect, r) = ui.allocate_exact_size(egui::vec2(36.0, 36.0), egui::Sense::click());
                                    r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, format!("Frame {frame}")));
                                    if !ui.is_rect_visible(rect) {
                                        continue;
                                    }
                                    let chosen = now.as_ref().is_some_and(|s| s.frame == frame);
                                    if chosen || r.hovered() {
                                        ui.painter().rect_filled(rect, 3.0, if chosen { ui.visuals().selection.bg_fill } else { ui.visuals().widgets.hovered.bg_fill });
                                    }
                                    if let Some(t) = cx.pictures.as_mut().and_then(|p| p.sprite(&rustpg2d_engine::defs::SpriteRef::new(&sheet, frame))) {
                                        t.paint(ui, rect.shrink(2.0));
                                    }
                                    if r.on_hover_text(format!("Frame {frame}")).clicked() {
                                        pick = Some(frame);
                                    }
                                }
                            });
                        }
                    });
                    if pick.is_some() {
                        ui.close();
                    }
                    pick
                })
                .inner
                .flatten();
            if let Some(frame) = pick {
                map.insert("frame".into(), Value::from(frame));
                changed = true;
            }
        });
    });
    changed
}

/// The inside of a group (no header of its own).
pub(crate) fn body(ui: &mut egui::Ui, cx: &mut Cx, node: Node, site: &str, v: &mut Value, depth: usize) -> bool {
    let mut changed = false;
    match node.shape() {
        Shape::Optional(inner) => changed |= body(ui, cx, inner, site, v, depth),
        Shape::Struct(fields) => {
            if !v.is_object() {
                *v = blank_past_default(node);
            }
            if node.type_name() == "SpriteRef" && cx.pictures.is_some() {
                return sprite_ui(ui, cx, &fields, v, depth);
            }
            let Value::Object(map) = v else { return false };
            for field in &fields {
                changed |= field_ui(ui, cx, field, map, depth);
            }
            // (What the schema doesn't know: shown as it is.)
            let extra: Vec<String> = map.keys().filter(|k| !fields.iter().any(|f| f.key == k.as_str())).cloned().collect();
            for key in extra {
                if let Some(x) = map.get_mut(&key) {
                    ui.push_id(&key, |ui| changed |= value_ui(ui, &schema::plain_label(&key), x));
                }
            }
        }
        Shape::Choice(variants) => {
            let now = schema::variant_of(&variants, v);
            let pick = ui
                .horizontal(|ui| {
                    let by = label_cell(ui, "Kind", &now.map(|k| variants[k].help()).unwrap_or_default());
                    variant_combo(ui, &variants, now, by)
                })
                .inner;
            if let Some(k) = pick {
                *v = switched(v, &variants[k]);
                changed = true;
            }
            if let Some(n) = schema::variant_of(&variants, v).and_then(|k| variants[k].data) {
                changed |= variant_data(ui, cx, n, v, depth);
            }
        }
        Shape::List(inner) => {
            if !v.is_array() {
                *v = Value::Array(Vec::new());
            }
            let Value::Array(list) = v else { return false };
            if list.len() > MAX_LIST {
                ui.weak(format!("{} entries (too many to lay out here)", list.len()));
                return false;
            }
            // Triggers, conditions and actions are rows of the event sheet.
            if let Some(changed) = crate::sheet::list(ui, cx, inner, site, list, depth) {
                return changed;
            }
            let inner_shape = inner.shape();
            let rows = is_inline(&inner_shape);
            let (mut remove, mut up, mut down) = (None, None, None);
            let n = list.len();
            for (k, item) in list.iter_mut().enumerate() {
                ui.push_id(k, |ui| {
                    let mut buttons = |ui: &mut egui::Ui| {
                        if k > 0 && ui.small_button("^").on_hover_text("Move up").clicked() {
                            up = Some(k);
                        }
                        if k + 1 < n && ui.small_button("v").on_hover_text("Move down").clicked() {
                            down = Some(k);
                        }
                        if ui.small_button("x").on_hover_text("Remove").clicked() {
                            remove = Some(k);
                        }
                    };
                    if rows {
                        ui.horizontal(|ui| {
                            let by = ui.weak(format!("{}.", k + 1)).id;
                            changed |= inline(ui, cx, inner, site, item, by);
                            buttons(ui);
                        });
                    } else {
                        let about = summary(item);
                        changed |= group(ui, "entry", [&format!("{}.", k + 1), "", &about], false, buttons, |ui| body(ui, cx, inner, site, item, depth + 1)).0;
                    }
                });
            }
            if let Some(k) = remove {
                list.remove(k);
                changed = true;
            }
            if let Some(k) = up {
                list.swap(k, k - 1);
                changed = true;
            }
            if let Some(k) = down {
                list.swap(k, k + 1);
                changed = true;
            }
            // A new entry: of an enum, the variant picked from a list; else the plainest value.
            match inner_shape {
                Shape::Choice(variants) if variants.iter().any(|v| v.data.is_some()) => {
                    let pick = ui.menu_button("+ Add…", |ui| egui::ScrollArea::vertical().max_height(340.0).show(ui, |ui| variant_list(ui, &variants, None, variants.len() > 12)).inner).inner.flatten();
                    if let Some(k) = pick {
                        list.push(variants[k].blank());
                        changed = true;
                    }
                }
                _ => {
                    if ui.button("+ Add").clicked() {
                        let mut new = inner.blank();
                        if new.is_null() {
                            new = blank_past_default(inner);
                        }
                        list.push(new);
                        changed = true;
                    }
                }
            }
        }
        Shape::Fixed(nodes) => {
            if v.as_array().is_none_or(|a| a.len() != nodes.len()) {
                *v = node.blank();
            }
            for (k, (n, item)) in nodes.iter().zip(v.as_array_mut().into_iter().flatten()).enumerate() {
                ui.push_id(k, |ui| changed |= node_ui(ui, cx, *n, Row { site: &format!("{site}.{k}"), label: &format!("{}.", k + 1), help: "", reset: false }, item, depth + 1));
            }
        }
        Shape::Map(inner) => {
            if !v.is_object() {
                *v = Value::Object(Default::default());
            }
            let Value::Object(map) = v else { return false };
            if map.len() > MAX_LIST {
                ui.weak(format!("{} entries (too many to lay out here)", map.len()));
                return false;
            }
            let mut remove = None;
            for (key, item) in map.iter_mut() {
                ui.push_id(key.as_str(), |ui| {
                    ui.horizontal(|ui| {
                        if ui.small_button("x").on_hover_text("Remove").clicked() {
                            remove = Some(key.clone());
                        }
                        ui.vertical(|ui| changed |= node_ui(ui, cx, inner, Row { site, label: key, help: "", reset: false }, item, depth + 1));
                    });
                });
            }
            if let Some(key) = remove {
                map.shift_remove(&key);
                changed = true;
            }
            ui.horizontal(|ui| {
                let id = ui.scope_id().with("new key");
                let mut name: String = ui.data_mut(|d| d.get_temp(id).unwrap_or_default());
                ui.add(egui::TextEdit::singleline(&mut name).hint_text("name").desired_width(140.0)).widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, "Name of the new entry"));
                if ui.add_enabled(!name.trim().is_empty() && !map.contains_key(name.trim()), egui::Button::new("+ Add")).clicked() {
                    let mut new = inner.blank();
                    if new.is_null() {
                        new = blank_past_default(inner);
                    }
                    map.insert(name.trim().to_string(), new);
                    name.clear();
                    changed = true;
                }
                ui.data_mut(|d| d.insert_temp(id, name));
            });
        }
        // What fits on a row, standing alone in a group (a list entry's only value).
        shape if is_inline(&shape) => {
            changed |= ui
                .horizontal(|ui| {
                    let by = label_cell(ui, "Value", "");
                    inline(ui, cx, node, site, v, by)
                })
                .inner
        }
        _ => changed |= value_ui(ui, "Value", v),
    }
    changed
}

/// The form of a definition: its sections with their fields (the id is in the tab's header).
/// True when something changed.
pub fn definition_ui(ui: &mut egui::Ui, cx: &mut Cx, node: Node, value: &mut Value) -> bool {
    sections_ui(ui, cx, node, value, schema::sections(node.ty))
}

/// [`definition_ui`] with the sections given: (title, the fields' keys; `~` before a key: for
/// advanced use). A section without a title is not shown (what the form's owner shows itself).
pub fn sections_ui(ui: &mut egui::Ui, cx: &mut Cx, node: Node, value: &mut Value, sections: &[(&str, &str)]) -> bool {
    let (Shape::Struct(fields), Value::Object(map)) = (node.shape(), value) else { return false };
    let mut changed = false;
    let mut placed: Vec<&str> = vec!["id"];
    for (title, list) in sections {
        let keys: Vec<(&str, bool)> = list.split_whitespace().map(|k| (k.trim_start_matches('~'), k.starts_with('~'))).collect();
        placed.extend(keys.iter().map(|(k, _)| *k));
        if title.is_empty() {
            continue;
        }
        let shown: Vec<&Field> = keys.iter().filter(|(k, advanced)| *k != "id" && (cx.show_all || !advanced)).filter_map(|(k, _)| fields.iter().find(|f| f.key == *k)).collect();
        if shown.is_empty() {
            continue;
        }
        egui::CollapsingHeader::new(egui::RichText::new(*title).strong()).id_salt(("section", *title)).default_open(true).show(ui, |ui| {
            for field in shown {
                changed |= field_ui(ui, cx, field, map, 1);
            }
        });
    }
    // Fields without a section (a kind the tables don't know yet).
    let rest: Vec<&Field> = fields.iter().filter(|f| !placed.contains(&f.key)).collect();
    if !rest.is_empty() {
        egui::CollapsingHeader::new(egui::RichText::new(if sections.is_empty() { "Fields" } else { "More" }).strong()).default_open(true).show(ui, |ui| {
            for field in rest {
                changed |= field_ui(ui, cx, field, map, 1);
            }
        });
    }
    changed
}

/// A page of fields of an object (the settings' pages): the ones `list` names, in its order
/// (`~` before a name: for advanced use). True when something changed.
pub fn page_ui(ui: &mut egui::Ui, cx: &mut Cx, node: Node, value: &mut Value, list: &str) -> bool {
    let (Shape::Struct(fields), Value::Object(map)) = (node.shape(), value) else { return false };
    let mut changed = false;
    for key in list.split_whitespace() {
        let advanced = key.starts_with('~');
        if advanced && !cx.show_all {
            continue;
        }
        if let Some(field) = fields.iter().find(|f| f.key == key.trim_start_matches('~')) {
            changed |= field_ui(ui, cx, field, map, 1);
        }
    }
    changed
}

/// The control of a plain value the schema says nothing about. True when it changed.
fn plain(ui: &mut egui::Ui, v: &mut Value, by: egui::Id) -> bool {
    match v {
        Value::Bool(b) => ui.checkbox(b, "").labelled_by(by).changed(),
        Value::Number(n) => {
            if n.is_f64() {
                let mut f = n.as_f64().unwrap_or(0.0);
                let changed = ui.add(egui::DragValue::new(&mut f).speed(0.05).max_decimals(3)).labelled_by(by).changed();
                if changed {
                    *v = serde_json::Number::from_f64(f).map_or(Value::Null, Value::Number);
                }
                changed
            } else {
                let mut i = n.as_i64().unwrap_or(0);
                let changed = ui.add(egui::DragValue::new(&mut i)).labelled_by(by).changed();
                if changed {
                    *v = Value::from(i);
                }
                changed
            }
        }
        Value::String(s) => {
            if s.contains('\n') || s.len() > 60 {
                ui.add(egui::TextEdit::multiline(s).desired_rows(2).desired_width(text_width(ui, 460.0))).labelled_by(by).changed()
            } else {
                ui.add(egui::TextEdit::singleline(s).desired_width(text_width(ui, 280.0))).labelled_by(by).changed()
            }
        }
        // Nothing set: what is typed is read as data (a number, "a text", true, {…}).
        Value::Null => {
            let id = ui.next_auto_id();
            let mut text: String = ui.data_mut(|d| d.get_temp(id).unwrap_or_default());
            let r = ui.add(egui::TextEdit::singleline(&mut text).hint_text("none").desired_width(160.0)).labelled_by(by).on_hover_text("Not set. Type a value and press Enter: a number, \"a text\", true, or data in braces.");
            let mut changed = false;
            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                if let Ok(parsed) = serde_json::from_str::<Value>(&text).or_else(|_| serde_json::from_str::<Value>(&format!("{text:?}"))) {
                    *v = parsed;
                    text.clear();
                    changed = true;
                }
            }
            ui.data_mut(|d| d.insert_temp(id, text));
            changed
        }
        _ => false,
    }
}

/// The form of a value the schema says nothing about, by what it holds. True when it changed.
pub fn value_ui(ui: &mut egui::Ui, label: &str, v: &mut Value) -> bool {
    let mut changed = false;
    match v {
        Value::Object(map) => {
            let title = format!("{label}  ({})", summary(&Value::Object(map.clone())));
            changed |= egui::CollapsingHeader::new(title)
                .id_salt(label)
                .default_open(map.len() <= 6)
                .show(ui, |ui| {
                    let mut changed = false;
                    for (key, val) in map.iter_mut() {
                        ui.push_id(key.as_str(), |ui| changed |= value_ui(ui, &schema::plain_label(key), val));
                    }
                    changed
                })
                .body_returned
                .unwrap_or(false);
        }
        Value::Array(list) => {
            let n = list.len();
            if n > MAX_LIST {
                ui.horizontal(|ui| {
                    ui.label(label);
                    ui.weak(format!("{n} entries (too many to lay out here)"));
                });
                return false;
            }
            egui::CollapsingHeader::new(format!("{label}  ({n})")).id_salt(label).default_open(n > 0 && n <= 4).show(ui, |ui| {
                let mut remove = None;
                for (k, item) in list.iter_mut().enumerate() {
                    ui.push_id(k, |ui| {
                        ui.horizontal(|ui| {
                            if ui.small_button("x").on_hover_text("Remove").clicked() {
                                remove = Some(k);
                            }
                            ui.vertical(|ui| changed |= value_ui(ui, &format!("{}.", k + 1), item));
                        });
                    });
                }
                if let Some(k) = remove {
                    list.remove(k);
                    changed = true;
                }
                if ui.button("+ Add").clicked() {
                    list.push(list.last().cloned().unwrap_or(Value::Null));
                    changed = true;
                }
            });
        }
        _ => {
            ui.horizontal(|ui| {
                let by = label_cell(ui, label, "");
                changed |= plain(ui, v, by);
            });
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summaries() {
        assert_eq!(summary(&serde_json::json!({"item": "potion", "count": 2})), "potion");
        assert_eq!(summary(&serde_json::json!({"Dialog": {"speaker": "", "text": "Hello"}})), "Dialog: Hello");
        assert_eq!(summary(&serde_json::json!({"SetUiVisible": {"visible": true}})), "Set ui visible");
        assert_eq!(summary(&Value::Null), "none");
        assert_eq!(summary(&serde_json::json!({"on": {"UseItem": {"item": "potion"}}, "conditions": [], "actions": [1, 2], "once": false})), "When: Use item: potion · 2 actions");
        assert_eq!(summary(&serde_json::json!({"on": "Interact", "conditions": [], "actions": [1], "once": false})), "When: Interact · 1 action");
    }

    #[test]
    fn a_changed_kind_keeps_what_fits() {
        let Shape::Struct(fields) = schema::trigger().shape() else { panic!() };
        let Shape::List(action) = fields.iter().find(|f| f.key == "actions").unwrap().node.shape() else { panic!() };
        let Shape::Choice(variants) = action.shape() else { panic!() };
        let take = variants.iter().find(|v| v.name == "TakeItem").unwrap();
        let new = switched(&serde_json::json!({"GiveItem": {"item": "potion", "count": 3}}), take);
        assert_eq!(new, serde_json::json!({"TakeItem": {"item": "potion", "count": 3}}));
    }
}

/// The form driven as a creator would: clicks and typing through egui's AccessKit tree (no window).
#[cfg(test)]
mod ui_tests {
    use super::*;
    use egui_kittest::kittest::Queryable;
    use egui_kittest::Harness;

    struct State {
        kind: RefKind,
        value: Value,
        show_all: bool,
        lists: Vec<(RefKind, Vec<(String, String)>)>,
        asks: Vec<String>,
    }

    /// A form on a definition of the example project.
    fn form(kind: RefKind, id: &str) -> Harness<'static, State> {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../example_project");
        let db = rustpg2d_engine::project::Project::load(&dir).unwrap().db;
        let lists: Vec<(RefKind, Vec<(String, String)>)> = RefKind::ALL.iter().map(|k| (*k, editor_core::defs::list(&db, *k))).collect();
        let value = {
            use rustpg2d_engine::defs::Def;
            rustpg2d_engine::with_def_kind!(kind, |T| T::list(&db).iter().find(|d| d.id() == id).map(|d| serde_json::to_value(d).unwrap()), else => None).unwrap_or_else(|| panic!("the example has no {id}"))
        };
        let state = State { kind, value, show_all: false, lists, asks: Vec::new() };
        Harness::builder().with_size(egui::vec2(900.0, 4000.0)).build_ui_state(
            |ui, state: &mut State| {
                let mut cx = Cx { lists: &state.lists, maps: &[], show_all: state.show_all, compact: false, pictures: None, refs: None, asks: Vec::new() };
                definition_ui(ui, &mut cx, schema::of_kind(state.kind).unwrap(), &mut state.value);
                for ask in cx.asks {
                    state.asks.push(match ask {
                        Ask::Open(k, id) => format!("open {} {id}", k.name()),
                        Ask::New(k) => format!("new {}", k.name()),
                    });
                }
            },
            state,
        )
    }

    #[test]
    fn a_form_has_sections_labels_and_hides_advanced_fields() {
        let mut h = form(RefKind::Character, "elder");
        h.run();
        for label in ["Basics", "Look", "Rules", "Gear", "Name", "Race", "Stat block"] {
            assert!(h.query_all_by_label(label).next().is_some(), "no \"{label}\"");
        }
        assert!(h.query_all_by_label("Hit points (old)").next().is_none(), "advanced fields stay out of the way");
        h.state_mut().show_all = true;
        h.run();
        assert!(h.query_all_by_label("Hit points (old)").next().is_some());
    }

    #[test]
    fn a_choice_is_picked_from_its_list() {
        let mut h = form(RefKind::Item, "iron_sword");
        h.run();
        assert_eq!(h.state().value.get("use_on"), None, "left out of the file while it is the default");
        h.get_by_role_and_label(egui::accesskit::Role::ComboBox, "Used on").click();
        h.run();
        h.get_by_label("Ally").click();
        h.run();
        assert_eq!(h.state().value["use_on"], "Ally");
    }

    #[test]
    fn a_field_goes_back_to_its_default_from_its_label() {
        let mut h = form(RefKind::Item, "iron_sword");
        h.run();
        assert_eq!(h.state().value["max_stack"], 1);
        h.get_by_role_and_label(egui::accesskit::Role::Label, "Stack size").click_secondary();
        h.run();
        h.get_by_label("Back to the default").click();
        h.run();
        assert_eq!(h.state().value["max_stack"], 99);
    }

    #[test]
    fn an_id_is_picked_by_searching_and_opens_what_it_names() {
        let mut h = form(RefKind::Item, "iron_sword");
        h.run();
        // The icon's sheet is named by a picker with a link to it.
        h.get_all_by_role_and_label(egui::accesskit::Role::ComboBox, "Sheet").next().unwrap().click();
        h.run();
        h.get_by_role_and_label(egui::accesskit::Role::TextInput, "Search").type_text("food");
        h.run();
        h.get_by_label("food").click();
        h.run();
        assert_eq!(h.state().value["icon"]["sheet"], "food");
        h.get_all_by_label("Open").next().unwrap().click();
        h.run();
        assert_eq!(h.state().asks, ["open sheet food"]);
    }
}
