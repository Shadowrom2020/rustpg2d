//! The Database's table view (the *Table* toggle of the Items, Characters, Spells and Recipes
//! tabs): one row per definition with columns for the main fields, sorting by any column,
//! editing in place and bulk edits of the selected rows (set a value, add a percentage, a
//! formula of `x`). A bulk edit changes the definitions in one frame, so the Database's history
//! makes it one undo step.

use std::collections::BTreeSet;

use egui_macroquad::egui::{self, Color32};
use rustpg2d_engine::crafting::RecipeDef;
use rustpg2d_engine::defs::{CharacterDef, ItemDef};
use rustpg2d_engine::skills::SpellDef;

use crate::database::Tab;
use crate::ui::{self as eui, ph};

/// A cell's value.
#[derive(Clone, Debug, PartialEq)]
pub enum Val {
    Num(f64),
    Text(String),
    Flag(bool),
    /// The definition doesn't have this field (an item that isn't a weapon has no damage dice).
    Blank,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Int,
    Float,
    Text,
    Flag,
}

/// A column: how it reads and writes a definition's field. `set` returns false where the
/// definition doesn't have the field.
pub struct Col<T> {
    pub name: &'static str,
    pub kind: Kind,
    pub width: f32,
    pub tip: &'static str,
    pub get: fn(&T) -> Val,
    pub set: fn(&mut T, &Val) -> bool,
}

fn num(v: &Val) -> Option<f64> {
    match v {
        Val::Num(n) => Some(*n),
        _ => None,
    }
}
fn int(v: &Val) -> Option<i64> {
    num(v).map(|n| n.round() as i64)
}
fn text(v: &Val) -> Option<String> {
    match v {
        Val::Text(t) => Some(t.clone()),
        _ => None,
    }
}
fn flag(v: &Val) -> Option<bool> {
    match v {
        Val::Flag(b) => Some(*b),
        _ => None,
    }
}

/// Writes an unsigned whole number (negative values become 0).
fn put_u32(f: &mut u32, v: &Val) -> bool {
    int(v).map(|n| *f = n.clamp(0, u32::MAX as i64) as u32).is_some()
}
fn put_i32(f: &mut i32, v: &Val) -> bool {
    int(v).map(|n| *f = n.clamp(i32::MIN as i64, i32::MAX as i64) as i32).is_some()
}
fn put_f32(f: &mut f32, v: &Val) -> bool {
    num(v).map(|n| *f = n.max(0.0) as f32).is_some()
}
fn put_text(f: &mut String, v: &Val) -> bool {
    text(v).map(|t| *f = t).is_some()
}
fn put_flag(f: &mut bool, v: &Val) -> bool {
    flag(v).map(|b| *f = b).is_some()
}

pub fn item_cols() -> Vec<Col<ItemDef>> {
    vec![
        Col { name: "Name", kind: Kind::Text, width: 170.0, tip: "The name players see", get: |d| Val::Text(d.name.clone()), set: |d, v| put_text(&mut d.name, v) },
        Col { name: "Cost (cp)", kind: Kind::Int, width: 80.0, tip: "The SRD's cost in copper pieces (0 = its value in the shop's currency)", get: |d| Val::Num(d.cost_cp as f64), set: |d, v| put_u32(&mut d.cost_cp, v) },
        Col { name: "Value", kind: Kind::Int, width: 70.0, tip: "Base trade value (merchants without their own price use it)", get: |d| Val::Num(d.value as f64), set: |d, v| put_u32(&mut d.value, v) },
        Col { name: "Weight (lb)", kind: Kind::Float, width: 80.0, tip: "Its weight in pounds", get: |d| Val::Num(d.weight as f64), set: |d, v| put_f32(&mut d.weight, v) },
        Col { name: "Stack", kind: Kind::Int, width: 60.0, tip: "How many fit in one inventory slot", get: |d| Val::Num(d.max_stack as f64), set: |d, v| int(v).map(|n| d.max_stack = n.clamp(1, u32::MAX as i64) as u32).is_some() },
        Col {
            name: "Damage",
            kind: Kind::Text,
            width: 90.0,
            tip: "A weapon's damage dice (2d6, 1d8+1); empty = by its damage number",
            get: |d| d.weapon.as_ref().map_or(Val::Blank, |w| Val::Text(w.dice.clone())),
            set: |d, v| d.weapon.as_mut().is_some_and(|w| put_text(&mut w.dice, v)),
        },
        Col {
            name: "Base AC",
            kind: Kind::Int,
            width: 70.0,
            tip: "Armor's base armor class (a shield's bonus)",
            get: |d| d.armor.as_ref().map_or(Val::Blank, |a| Val::Num(a.base_ac as f64)),
            set: |d, v| d.armor.as_mut().is_some_and(|a| put_i32(&mut a.base_ac, v)),
        },
    ]
}

pub fn character_cols() -> Vec<Col<CharacterDef>> {
    vec![
        Col { name: "Name", kind: Kind::Text, width: 160.0, tip: "The name shown in play", get: |d| Val::Text(d.name.clone()), set: |d, v| put_text(&mut d.name, v) },
        Col { name: "Max HP", kind: Kind::Int, width: 70.0, tip: "0 = can't be fought; the stat block's hit points replace it in fights", get: |d| Val::Num(d.max_hp as f64), set: |d, v| put_i32(&mut d.max_hp, v) },
        Col {
            name: "Block HP",
            kind: Kind::Int,
            width: 70.0,
            tip: "The stat block's hit points",
            get: |d| d.stats.as_ref().map_or(Val::Blank, |s| Val::Num(s.hp as f64)),
            set: |d, v| d.stats.as_mut().is_some_and(|s| put_i32(&mut s.hp, v)),
        },
        Col {
            name: "AC",
            kind: Kind::Int,
            width: 55.0,
            tip: "The stat block's armor class",
            get: |d| d.stats.as_ref().map_or(Val::Blank, |s| Val::Num(s.ac as f64)),
            set: |d, v| d.stats.as_mut().is_some_and(|s| put_i32(&mut s.ac, v)),
        },
        Col {
            name: "CR",
            kind: Kind::Text,
            width: 55.0,
            tip: "The stat block's challenge rating (1/4, 2…)",
            get: |d| d.stats.as_ref().map_or(Val::Blank, |s| Val::Text(s.cr.clone())),
            set: |d, v| d.stats.as_mut().is_some_and(|s| put_text(&mut s.cr, v)),
        },
        Col { name: "XP", kind: Kind::Int, width: 70.0, tip: "Experience for defeating it (0 = the stat block's)", get: |d| Val::Num(d.xp as f64), set: |d, v| put_u32(&mut d.xp, v) },
        Col { name: "Speed", kind: Kind::Float, width: 65.0, tip: "Walking speed (pixels a second; 0 = the default)", get: |d| Val::Num(d.speed as f64), set: |d, v| put_f32(&mut d.speed, v) },
        Col { name: "Hostile", kind: Kind::Flag, width: 60.0, tip: "Attacks the player", get: |d| Val::Flag(d.hostile), set: |d, v| put_flag(&mut d.hostile, v) },
        Col { name: "Faction", kind: Kind::Text, width: 100.0, tip: "Its faction (empty = none)", get: |d| Val::Text(d.faction.clone()), set: |d, v| put_text(&mut d.faction, v) },
    ]
}

pub fn spell_cols() -> Vec<Col<SpellDef>> {
    vec![
        Col { name: "Name", kind: Kind::Text, width: 170.0, tip: "The name players see", get: |d| Val::Text(d.name.clone()), set: |d, v| put_text(&mut d.name, v) },
        Col { name: "Level", kind: Kind::Int, width: 55.0, tip: "Spell level (0 = cantrip)", get: |d| Val::Num(d.level as f64), set: |d, v| int(v).map(|n| d.level = n.clamp(0, 9) as u32).is_some() },
        Col { name: "School", kind: Kind::Text, width: 100.0, tip: "School of magic", get: |d| Val::Text(d.school.clone()), set: |d, v| put_text(&mut d.school, v) },
        Col { name: "Mana", kind: Kind::Int, width: 60.0, tip: "Mana it costs (projects without spell slots)", get: |d| Val::Num(d.mana as f64), set: |d, v| put_u32(&mut d.mana, v) },
        Col { name: "Cooldown", kind: Kind::Float, width: 70.0, tip: "Seconds before it can be cast again outside fights", get: |d| Val::Num(d.cooldown as f64), set: |d, v| put_f32(&mut d.cooldown, v) },
        Col { name: "Concentration", kind: Kind::Flag, width: 95.0, tip: "Needs concentration", get: |d| Val::Flag(d.concentration), set: |d, v| put_flag(&mut d.concentration, v) },
        Col { name: "Ritual", kind: Kind::Flag, width: 55.0, tip: "Can be cast as a ritual", get: |d| Val::Flag(d.ritual), set: |d, v| put_flag(&mut d.ritual, v) },
    ]
}

pub fn recipe_cols() -> Vec<Col<RecipeDef>> {
    vec![
        Col { name: "Result", kind: Kind::Text, width: 140.0, tip: "The item it makes", get: |d| Val::Text(d.result.clone()), set: |d, v| put_text(&mut d.result, v) },
        Col { name: "Count", kind: Kind::Int, width: 60.0, tip: "How many it makes", get: |d| Val::Num(d.count as f64), set: |d, v| int(v).map(|n| d.count = n.clamp(1, u32::MAX as i64) as u32).is_some() },
        Col { name: "Station", kind: Kind::Text, width: 120.0, tip: "The prop to craft it at (empty = anywhere)", get: |d| Val::Text(d.station.clone()), set: |d, v| put_text(&mut d.station, v) },
        Col { name: "Level", kind: Kind::Int, width: 55.0, tip: "The skill level it needs", get: |d| Val::Num(d.level as f64), set: |d, v| put_u32(&mut d.level, v) },
    ]
}

/// Whether `tab` has a table view.
pub fn has_table(tab: Tab) -> bool {
    matches!(tab, Tab::Items | Tab::Characters | Tab::Spells | Tab::Recipes)
}

/// The column names of `tab`'s table.
pub fn column_names(tab: Tab) -> Vec<&'static str> {
    fn names<T>(c: Vec<Col<T>>) -> Vec<&'static str> {
        c.iter().map(|c| c.name).collect()
    }
    match tab {
        Tab::Items => names(item_cols()),
        Tab::Characters => names(character_cols()),
        Tab::Spells => names(spell_cols()),
        Tab::Recipes => names(recipe_cols()),
        _ => Vec::new(),
    }
}

/// The column of `tab` called `name` (case and spaces ignored; the start of its name will do:
/// `cost` = *Cost (cp)*).
pub fn column_index(tab: Tab, name: &str) -> Option<usize> {
    let norm = |s: &str| s.to_lowercase().replace([' ', '_'], "");
    let n = norm(name);
    let names = column_names(tab);
    names.iter().position(|c| norm(c) == n).or_else(|| names.iter().position(|c| norm(c).starts_with(&n)))
}

/// The bulk edit of `st` on `tab`'s definitions (the editor script's `dbbulk`).
pub fn bulk_tab(db: &mut rustpg2d_engine::defs::Database, tab: Tab, st: &TableState) -> Result<(usize, usize), String> {
    match tab {
        Tab::Items => bulk(&mut db.items, &item_cols(), |d| &d.id, st),
        Tab::Characters => bulk(&mut db.characters, &character_cols(), |d| &d.id, st),
        Tab::Spells => bulk(&mut db.spells, &spell_cols(), |d| &d.id, st),
        Tab::Recipes => bulk(&mut db.recipes, &recipe_cols(), |d| &d.id, st),
        _ => Err("This tab has no table".into()),
    }
}

impl TableState {
    /// `status` field `dbtable=`: off, or on:COLUMN^|v|-:rows/selected[the first rows shown].
    pub fn status(&self, tab: Tab) -> String {
        if !self.on || !has_table(tab) {
            return "off".into();
        }
        let sort = match self.sort {
            Some((k, asc)) => format!("{}{}", column_names(tab).get(k).copied().unwrap_or("?").replace(' ', "_"), if asc { "^" } else { "v" }),
            None => "-".into(),
        };
        let first: Vec<&str> = self.shown.iter().take(5).map(|s| s.as_str()).collect();
        format!("on:{sort}:{}/{}[{}]", self.shown.len(), self.sel.len(), first.join(","))
    }
}

/// What a bulk edit does to each selected cell.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BulkOp {
    #[default]
    Set,
    AddPercent,
    Formula,
}

impl BulkOp {
    pub fn label(self) -> &'static str {
        match self {
            BulkOp::Set => "Set to",
            BulkOp::AddPercent => "Add %",
            BulkOp::Formula => "Formula",
        }
    }
    pub fn parse(s: &str) -> Option<BulkOp> {
        match s {
            "set" => Some(BulkOp::Set),
            "add%" | "percent" | "add" => Some(BulkOp::AddPercent),
            "formula" => Some(BulkOp::Formula),
            _ => None,
        }
    }
}

/// The table's state (kept while the editor runs; the selection is cleared with another tab).
pub struct TableState {
    pub on: bool,
    /// Sorted by this column (index into the columns; None = the file's order), ascending.
    pub sort: Option<(usize, bool)>,
    /// Selected rows by id (ids stay the same when the order changes).
    pub sel: BTreeSet<String>,
    /// The row a Shift+click selects from.
    pub anchor: Option<String>,
    pub bulk_col: usize,
    pub bulk_op: BulkOp,
    pub bulk_value: String,
    /// What the last bulk edit did (or why it didn't).
    pub msg: String,
    /// The rows in the order shown last (ids), for Shift+click and the editor script.
    pub shown: Vec<String>,
}

impl Default for TableState {
    fn default() -> Self {
        // (Column 1 is every table's first number: cost, max HP, level, count.)
        TableState { on: false, sort: None, sel: BTreeSet::new(), anchor: None, bulk_col: 1, bulk_op: BulkOp::Set, bulk_value: String::new(), msg: String::new(), shown: Vec::new() }
    }
}

/// `old` changed by `op` with `input`: the new value, or why not.
pub fn apply_op(op: BulkOp, kind: Kind, old: &Val, input: &str) -> Result<Val, String> {
    let input = input.trim();
    match (kind, op) {
        (Kind::Flag, BulkOp::Set) => match input.to_lowercase().as_str() {
            "yes" | "true" | "on" | "1" | "x" => Ok(Val::Flag(true)),
            "no" | "false" | "off" | "0" | "" => Ok(Val::Flag(false)),
            _ => Err(format!("'{input}' isn't yes or no")),
        },
        (Kind::Flag, _) => Err("A tick column can only be set (yes / no)".into()),
        (Kind::Text, BulkOp::Set) => Ok(Val::Text(input.to_string())),
        (Kind::Text, _) => Err("A text column can only be set".into()),
        (_, op) => {
            let x = num(old).ok_or("no value")?;
            let y = match op {
                BulkOp::Set => input.parse::<f64>().map_err(|_| format!("'{input}' isn't a number"))?,
                BulkOp::AddPercent => x * (1.0 + input.trim_end_matches('%').trim().parse::<f64>().map_err(|_| format!("'{input}' isn't a percentage"))? / 100.0),
                BulkOp::Formula => formula(input, x)?,
            };
            if !y.is_finite() {
                return Err("The result isn't a number".into());
            }
            Ok(Val::Num(if kind == Kind::Int { y.round() } else { (y * 100.0).round() / 100.0 }))
        }
    }
}

/// A Rhai expression of `x` (the cell's value): `x * 1.1 + 5`, `(x / 10.0).round() * 10.0`.
pub fn formula(expr: &str, x: f64) -> Result<f64, String> {
    let engine = rhai::Engine::new();
    let mut scope = rhai::Scope::new();
    scope.push("x", x);
    let v: rhai::Dynamic = engine.eval_expression_with_scope(&mut scope, expr).map_err(|e| format!("Formula: {e}"))?;
    if let Ok(f) = v.as_float() {
        Ok(f)
    } else if let Ok(i) = v.as_int() {
        Ok(i as f64)
    } else {
        Err(format!("The formula gave {} instead of a number", v.type_name()))
    }
}

/// Applies the bulk edit to the selected rows: how many cells changed and how many were left
/// alone (no such field, or an error).
pub fn bulk<T>(items: &mut [T], cols: &[Col<T>], id_of: fn(&T) -> &str, st: &TableState) -> Result<(usize, usize), String> {
    let c = cols.get(st.bulk_col).ok_or("No column chosen")?;
    let (mut done, mut skipped) = (0, 0);
    for d in items.iter_mut().filter(|d| st.sel.contains(id_of(d))) {
        let old = (c.get)(d);
        if old == Val::Blank {
            skipped += 1;
            continue;
        }
        match apply_op(st.bulk_op, c.kind, &old, &st.bulk_value) {
            Ok(v) => {
                if v != old && (c.set)(d, &v) {
                    done += 1;
                }
            }
            // (The input is wrong for every row alike: say why before anything changed.)
            Err(e) if done == 0 => return Err(e),
            Err(_) => skipped += 1,
        }
    }
    Ok((done, skipped))
}

/// The rows shown in order: `shown` (the list's search and filter), sorted by the column.
pub fn order<T>(items: &[T], cols: &[Col<T>], shown: &[usize], sort: Option<(usize, bool)>) -> Vec<usize> {
    let mut rows = shown.to_vec();
    if let Some((k, asc)) = sort {
        if let Some(c) = cols.get(k) {
            let key = |i: usize| (c.get)(&items[i]);
            rows.sort_by(|a, b| {
                let o = match (key(*a), key(*b)) {
                    (Val::Num(x), Val::Num(y)) => x.total_cmp(&y),
                    (Val::Text(x), Val::Text(y)) => natural(&x, &y),
                    (Val::Flag(x), Val::Flag(y)) => x.cmp(&y),
                    // (Blank cells last either way.)
                    (Val::Blank, Val::Blank) => std::cmp::Ordering::Equal,
                    (Val::Blank, _) => return std::cmp::Ordering::Greater,
                    (_, Val::Blank) => return std::cmp::Ordering::Less,
                    _ => std::cmp::Ordering::Equal,
                };
                if asc {
                    o
                } else {
                    o.reverse()
                }
            });
        }
    }
    rows
}

/// Text order that puts "1d6" before "1d10" and "Potion 2" before "Potion 10".
fn natural(a: &str, b: &str) -> std::cmp::Ordering {
    let split = |s: &str| -> Vec<(String, u64)> {
        let mut out = Vec::new();
        let mut word = String::new();
        let mut n = String::new();
        for ch in s.to_lowercase().chars() {
            if ch.is_ascii_digit() {
                n.push(ch);
            } else {
                if !n.is_empty() {
                    out.push((std::mem::take(&mut word), n.parse().unwrap_or(0)));
                    n.clear();
                }
                word.push(ch);
            }
        }
        out.push((word, n.parse().unwrap_or(0)));
        out
    };
    split(a).cmp(&split(b))
}

/// The table of one tab. Returns the row whose form should open (a double-click on its id).
pub fn table_ui<T>(ui: &mut egui::Ui, items: &mut [T], cols: &[Col<T>], id_of: fn(&T) -> &str, shown: &[usize], st: &mut TableState) -> Option<usize> {
    let mut open = None;
    let rows = order(items, cols, shown, st.sort);
    st.shown = rows.iter().map(|&i| id_of(&items[i]).to_string()).collect();
    st.sel.retain(|id| st.shown.contains(id));
    // The bulk edit bar.
    ui.horizontal_wrapped(|ui| {
        ui.label(format!("{} of {} selected", st.sel.len(), rows.len()));
        if ui.small_button("All").on_hover_text("Selects every row shown").clicked() {
            st.sel = st.shown.iter().cloned().collect();
        }
        if ui.small_button("None").clicked() {
            st.sel.clear();
        }
        ui.separator();
        st.bulk_col = st.bulk_col.min(cols.len().saturating_sub(1));
        egui::ComboBox::from_id_salt("tbl_bulk_col").selected_text(cols[st.bulk_col].name).show_ui(ui, |ui| {
            for (k, c) in cols.iter().enumerate() {
                ui.selectable_value(&mut st.bulk_col, k, c.name);
            }
        });
        egui::ComboBox::from_id_salt("tbl_bulk_op").selected_text(st.bulk_op.label()).show_ui(ui, |ui| {
            for op in [BulkOp::Set, BulkOp::AddPercent, BulkOp::Formula] {
                ui.selectable_value(&mut st.bulk_op, op, op.label());
            }
        });
        let hint = match st.bulk_op {
            BulkOp::Set => "value",
            BulkOp::AddPercent => "10 or -25",
            BulkOp::Formula => "x * 1.5 + 2",
        };
        ui.add(egui::TextEdit::singleline(&mut st.bulk_value).hint_text(hint).desired_width(130.0))
            .on_hover_text("Set to: the new value. Add %: a percentage to add (negative takes away). Formula: a Rhai expression of x, the cell's value, e.g. (x * 1.1).round(). Whole-number columns are rounded.");
        if ui.add_enabled(!st.sel.is_empty(), egui::Button::new(eui::t(ph::CHECK, "Apply"))).on_hover_text("Changes this column of the selected rows (one undo step; rows without the field are left alone)").clicked() {
            st.msg = match bulk(items, cols, id_of, st) {
                Ok((done, 0)) => format!("{done} changed"),
                Ok((done, skipped)) => format!("{done} changed, {skipped} without this field left alone"),
                Err(e) => e,
            };
        }
        if !st.msg.is_empty() {
            ui.weak(&st.msg);
        }
    });
    ui.separator();
    let row_h = 22.0;
    let id_w = 170.0;
    egui::ScrollArea::horizontal().id_salt("tbl_h").show(ui, |ui| {
        // The header: click to sort, again to reverse.
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            let sort = st.sort;
            let arrow = |k: usize| match sort {
                Some((s, true)) if s == k => format!(" {}", ph::CARET_UP),
                Some((s, false)) if s == k => format!(" {}", ph::CARET_DOWN),
                _ => String::new(),
            };
            ui.add_sized([id_w, row_h], egui::Label::new(egui::RichText::new("Id").strong()));
            for (k, c) in cols.iter().enumerate() {
                let b = ui.add_sized([c.width, row_h], egui::Button::new(egui::RichText::new(format!("{}{}", c.name, arrow(k))).strong()).frame(false)).on_hover_text(format!("{}\n\nClick to sort by it, again to reverse.", c.tip));
                if b.clicked() {
                    st.sort = match st.sort {
                        Some((s, true)) if s == k => Some((k, false)),
                        Some((s, false)) if s == k => None,
                        _ => Some((k, true)),
                    };
                }
            }
        });
        ui.separator();
        egui::ScrollArea::vertical().id_salt("tbl_v").auto_shrink([false, false]).show_rows(ui, row_h, rows.len(), |ui, range| {
            for &i in &rows[range] {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    let id = id_of(&items[i]).to_string();
                    let selected = st.sel.contains(&id);
                    let r = ui.add_sized([id_w, row_h], egui::SelectableLabel::new(selected, &id)).on_hover_text("Click to select, Ctrl+click to add or remove, Shift+click for a range; double-click opens its form");
                    if r.double_clicked() {
                        open = Some(i);
                    } else if r.clicked() {
                        let m = ui.input(|i| i.modifiers);
                        if m.shift {
                            let a = st.anchor.as_ref().and_then(|a| st.shown.iter().position(|s| s == a)).unwrap_or(0);
                            let b = st.shown.iter().position(|s| *s == id).unwrap_or(0);
                            st.sel.extend(st.shown[a.min(b)..=a.max(b)].iter().cloned());
                        } else if m.command || m.ctrl {
                            if !st.sel.remove(&id) {
                                st.sel.insert(id.clone());
                            }
                            st.anchor = Some(id.clone());
                        } else {
                            st.sel = BTreeSet::from([id.clone()]);
                            st.anchor = Some(id.clone());
                        }
                    }
                    for c in cols {
                        let d = &mut items[i];
                        match ((c.get)(d), c.kind) {
                            (Val::Blank, _) => {
                                ui.add_sized([c.width, row_h], egui::Label::new(egui::RichText::new("-").color(Color32::GRAY)));
                            }
                            (Val::Num(n), Kind::Int) => {
                                let mut v = n as i64;
                                if ui.add_sized([c.width, row_h], egui::DragValue::new(&mut v).speed(0.2)).changed() {
                                    (c.set)(d, &Val::Num(v as f64));
                                }
                            }
                            (Val::Num(n), _) => {
                                let mut v = n;
                                if ui.add_sized([c.width, row_h], egui::DragValue::new(&mut v).speed(0.05).max_decimals(2)).changed() {
                                    (c.set)(d, &Val::Num(v));
                                }
                            }
                            (Val::Text(t), _) => {
                                let mut v = t;
                                if ui.add_sized([c.width, row_h], egui::TextEdit::singleline(&mut v)).changed() {
                                    (c.set)(d, &Val::Text(v));
                                }
                            }
                            (Val::Flag(b), _) => {
                                let mut v = b;
                                // (A tick in a cell of the column's width.)
                                let (rect, _) = ui.allocate_exact_size(egui::vec2(c.width, row_h), egui::Sense::hover());
                                let mut cell = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::left_to_right(egui::Align::Center)));
                                if cell.checkbox(&mut v, "").on_hover_text(c.name).changed() {
                                    (c.set)(d, &Val::Flag(v));
                                }
                            }
                        }
                    }
                });
            }
        });
    });
    open
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items() -> Vec<ItemDef> {
        let mut a = ItemDef::fresh("dagger");
        a.cost_cp = 200;
        a.weapon = Some(rustpg2d_engine::defs::Weapon::melee(3, 0.35));
        let mut b = ItemDef::fresh("rope");
        b.cost_cp = 100;
        let mut c = ItemDef::fresh("lantern");
        c.cost_cp = 500;
        vec![a, b, c]
    }

    #[test]
    fn sorting_by_a_column_and_reversing() {
        let it = items();
        let cols = item_cols();
        let cost = cols.iter().position(|c| c.name == "Cost (cp)").unwrap();
        assert_eq!(order(&it, &cols, &[0, 1, 2], Some((cost, true))), vec![1, 0, 2]);
        assert_eq!(order(&it, &cols, &[0, 1, 2], Some((cost, false))), vec![2, 0, 1]);
        // Blank cells (no weapon) last both ways.
        let dmg = cols.iter().position(|c| c.name == "Damage").unwrap();
        assert_eq!(order(&it, &cols, &[1, 0, 2], Some((dmg, false)))[0], 0);
        assert_eq!(natural("1d10", "1d6"), std::cmp::Ordering::Greater);
    }

    #[test]
    fn bulk_edits_set_add_percent_and_formula() {
        let mut it = items();
        let cols = item_cols();
        let mut st = TableState { bulk_col: 1, sel: BTreeSet::from(["dagger".to_string(), "lantern".to_string()]), bulk_value: "300".into(), ..Default::default() };
        assert_eq!(bulk(&mut it, &cols, |d| &d.id, &st), Ok((2, 0)));
        assert_eq!(it.iter().map(|d| d.cost_cp).collect::<Vec<_>>(), vec![300, 100, 300]);
        st.bulk_op = BulkOp::AddPercent;
        st.bulk_value = "-10".into();
        bulk(&mut it, &cols, |d| &d.id, &st).unwrap();
        assert_eq!(it[0].cost_cp, 270);
        st.bulk_op = BulkOp::Formula;
        st.bulk_value = "(x / 100.0).round() * 100.0".into();
        bulk(&mut it, &cols, |d| &d.id, &st).unwrap();
        assert_eq!(it[2].cost_cp, 300);
        // A broken formula changes nothing and says why.
        st.bulk_value = "x *".into();
        assert!(bulk(&mut it, &cols, |d| &d.id, &st).is_err());
        // Damage: only the weapon has the field.
        st.bulk_col = cols.iter().position(|c| c.name == "Damage").unwrap();
        st.bulk_op = BulkOp::Set;
        st.bulk_value = "1d6".into();
        st.sel.insert("rope".into());
        assert_eq!(bulk(&mut it, &cols, |d| &d.id, &st), Ok((1, 2)));
        assert_eq!(it[0].weapon.as_ref().unwrap().dice, "1d6");
    }
}
