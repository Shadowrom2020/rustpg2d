//! A form for any definition, made from its data: every field gets a control by what it holds
//! (a text, a number, a switch, a list, a group of fields). It edits the definition's JSON form;
//! the workspace turns that back into the definition and refuses what isn't one.
//!
//! This is the ground the field schema of part 4 stands on: where the schema knows more about a
//! field (its label and help, its choices, the kind of id it names), a better control takes the
//! place of the plain one.

use eframe::egui;
use serde_json::Value;

/// Lists longer than this aren't laid out (a structure's cells): they stay as they are.
const MAX_LIST: usize = 200;

/// `frame_w` as "Frame w".
pub fn label_of(key: &str) -> String {
    let mut s = key.replace('_', " ");
    if let Some(f) = s.get(..1) {
        s = format!("{}{}", f.to_uppercase(), &s[1..]);
    }
    s
}

/// A few words on what a list entry or group is: its id, name or first text.
fn summary(v: &Value) -> String {
    match v {
        Value::Object(m) => {
            let named = ["name", "id", "title", "label", "item", "def", "text"].iter().find_map(|k| m.get(*k).and_then(|x| x.as_str()).filter(|s| !s.is_empty()));
            match named {
                Some(s) => s.chars().take(40).collect(),
                // An enum with data: its variant's name.
                None if m.len() == 1 => m.keys().next().cloned().unwrap_or_default(),
                None => m.values().find_map(|x| x.as_str()).unwrap_or_default().chars().take(40).collect(),
            }
        }
        Value::String(s) => s.chars().take(40).collect(),
        Value::Null => "none".into(),
        other => other.to_string().chars().take(40).collect(),
    }
}

fn is_plain(v: &Value) -> bool {
    !matches!(v, Value::Object(_) | Value::Array(_))
}

/// The control of a plain value. True when it changed.
fn plain(ui: &mut egui::Ui, v: &mut Value) -> bool {
    match v {
        Value::Bool(b) => ui.checkbox(b, "").changed(),
        Value::Number(n) => {
            if n.is_f64() {
                let mut f = n.as_f64().unwrap_or(0.0);
                let changed = ui.add(egui::DragValue::new(&mut f).speed(0.05).max_decimals(3)).changed();
                if changed {
                    *v = serde_json::Number::from_f64(f).map_or(Value::Null, Value::Number);
                }
                changed
            } else {
                let mut i = n.as_i64().unwrap_or(0);
                let changed = ui.add(egui::DragValue::new(&mut i)).changed();
                if changed {
                    *v = Value::from(i);
                }
                changed
            }
        }
        Value::String(s) => {
            if s.contains('\n') || s.len() > 70 {
                ui.add(egui::TextEdit::multiline(s).desired_rows(2).desired_width(520.0)).changed()
            } else {
                ui.add(egui::TextEdit::singleline(s).desired_width(280.0)).changed()
            }
        }
        // Nothing set: what is typed is read as data (a number, "a text", true, {…}).
        Value::Null => {
            let id = ui.next_auto_id();
            let mut text: String = ui.data_mut(|d| d.get_temp(id).unwrap_or_default());
            let r = ui.add(egui::TextEdit::singleline(&mut text).hint_text("none").desired_width(160.0)).on_hover_text("Not set. Type a value and press Enter: a number, \"a text\", true, or data in braces.");
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

/// The form of `v` under `label`. True when something changed.
pub fn value_ui(ui: &mut egui::Ui, label: &str, v: &mut Value, depth: usize) -> bool {
    let mut changed = false;
    match v {
        Value::Object(map) => {
            let open = depth == 0 || map.len() <= 6;
            let title = if depth == 0 { label.to_string() } else { format!("{label}  ({})", summary(&Value::Object(map.clone()))) };
            let mut body = |ui: &mut egui::Ui| {
                let mut changed = false;
                for (key, val) in map.iter_mut() {
                    ui.push_id(key.as_str(), |ui| {
                        changed |= value_ui(ui, &label_of(key), val, depth + 1);
                    });
                }
                changed
            };
            if depth == 0 {
                changed |= body(ui);
            } else {
                changed |= egui::CollapsingHeader::new(title).id_salt(label).default_open(open).show(ui, body).body_returned.unwrap_or(false);
            }
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
            egui::CollapsingHeader::new(format!("{label}  ({n})")).id_salt(label).default_open(n > 0 && n <= 4 && list.iter().all(is_plain)).show(ui, |ui| {
                let (mut remove, mut up) = (None, None);
                for (k, item) in list.iter_mut().enumerate() {
                    ui.push_id(k, |ui| {
                        if is_plain(item) {
                            ui.horizontal(|ui| {
                                ui.weak(format!("{}.", k + 1));
                                changed |= plain(ui, item);
                                if ui.small_button("x").on_hover_text("Remove").clicked() {
                                    remove = Some(k);
                                }
                            });
                        } else {
                            ui.horizontal(|ui| {
                                if ui.small_button("x").on_hover_text("Remove").clicked() {
                                    remove = Some(k);
                                }
                                if k > 0 && ui.small_button("^").on_hover_text("Move up").clicked() {
                                    up = Some(k);
                                }
                                ui.vertical(|ui| changed |= value_ui(ui, &format!("{}.", k + 1), item, depth + 1));
                            });
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
                // A new entry starts as a copy of the last (the shape is right), else unset.
                if ui.button("+ Add").clicked() {
                    list.push(list.last().cloned().unwrap_or(Value::Null));
                    changed = true;
                }
            });
        }
        _ => {
            ui.horizontal(|ui| {
                ui.add_sized([170.0, ui.spacing().interact_size.y], egui::Label::new(label).halign(egui::Align::LEFT));
                changed |= plain(ui, v);
            });
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_and_summaries() {
        assert_eq!(label_of("frame_w"), "Frame w");
        assert_eq!(label_of("id"), "Id");
        assert_eq!(summary(&serde_json::json!({"item": "potion", "count": 2})), "potion");
        assert_eq!(summary(&serde_json::json!({"Say": {"text": "Hello"}})), "Say");
        assert_eq!(summary(&Value::Null), "none");
    }
}
