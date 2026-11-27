//! The names a game makes up as it goes: flags, variables and text variables. Where each is
//! set and where it is read, in every definition, on every map and in the scripts, and a
//! rename that follows one through all of them.
//!
//! They are found by what the field schema says of a field (`schema::names`: the *Set flag*
//! action's name, a condition's variable, a common event's flag…), so a new field that holds
//! one is found once the schema's table names it; in scripts by the script API's argument
//! kinds.

use std::path::PathBuf;

use rustpg2d_engine::map::Map;
use rustpg2d_engine::refactor::RefKind;
use serde_json::Value;

use crate::history::Extra;
use crate::search::Target;
use crate::schema::{self, Node};
use crate::workspace::Workspace;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NameKind {
    Flag,
    /// A number.
    Variable,
    /// A text variable (a name the player typed, shown by `{name}` in texts).
    Text,
}

impl NameKind {
    pub const ALL: [NameKind; 3] = [NameKind::Flag, NameKind::Variable, NameKind::Text];

    pub fn label(self) -> &'static str {
        match self {
            NameKind::Flag => "flag",
            NameKind::Variable => "variable",
            NameKind::Text => "text variable",
        }
    }

    /// The word it is noted under in `editor/meta.ron` (`flag:met_elder`).
    pub fn key(self) -> &'static str {
        match self {
            NameKind::Flag => "flag",
            NameKind::Variable => "variable",
            NameKind::Text => "text",
        }
    }

    pub fn parse(s: &str) -> Option<NameKind> {
        NameKind::ALL.into_iter().find(|k| k.key() == s || k.label() == s || (s == "var" && *k == NameKind::Variable))
    }
}

/// What a place does with a name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum How {
    Set,
    Read,
    /// Named without saying which (a field the schema knows as a flag or variable outside
    /// actions and conditions).
    Used,
}

/// One place a name stands.
#[derive(Clone, Debug, PartialEq)]
pub struct NameUse {
    pub how: How,
    /// The place in words: "Character Elder", "village: guard", "main.rhai:12".
    pub place: String,
    /// Where *Go there* leads.
    pub target: Target,
}

/// A flag, a variable or a text variable with every place it stands.
#[derive(Clone, Debug, PartialEq)]
pub struct Named {
    pub kind: NameKind,
    pub name: String,
    pub uses: Vec<NameUse>,
}

impl Named {
    pub fn count(&self, how: How) -> usize {
        self.uses.iter().filter(|u| u.how == how).count()
    }

    /// Set somewhere and never read, or read and never set: likely a slip of the pen.
    pub fn warning(&self) -> Option<&'static str> {
        match (self.count(How::Set), self.count(How::Read), self.count(How::Used)) {
            (_, _, used) if used > 0 => None,
            (0, _, _) => Some("read, but nothing sets it"),
            (_, 0, _) => Some("set, but nothing reads it"),
            _ => None,
        }
    }
}

/// What the field at `site` holds, if it is one of these names, and what it does with it.
fn of_site(site: &str) -> Option<(NameKind, How)> {
    let key = site.rsplit('.').next().unwrap_or("");
    let kind = match schema::names(site, key)? {
        schema::Names::Flag => NameKind::Flag,
        schema::Names::Variable if matches!(site, "Action.InputText.var" | "Cond.TextVar.name") => NameKind::Text,
        schema::Names::Variable => NameKind::Variable,
        _ => return None,
    };
    let how = if site.starts_with("Action.") {
        How::Set
    } else if site.starts_with("Cond.") {
        How::Read
    } else {
        // A common event runs while its flag is set.
        How::Read
    };
    Some((kind, how))
}

/// The names in `value` (of the shape `node`): (kind, how, name, JSON pointer).
fn names_in(node: Node, value: &Value) -> Vec<(NameKind, How, String, String)> {
    let mut out = Vec::new();
    schema::texts(node, value, &mut |site, pointer, text| {
        if let (Some((kind, how)), false) = (of_site(site), text.is_empty()) {
            out.push((kind, how, text.to_string(), pointer.to_string()));
        }
    });
    out
}

/// Renames in a JSON value. Returns how many places changed.
fn rename_in(node: Node, value: &mut Value, kind: NameKind, old: &str, new: &str) -> usize {
    let places: Vec<String> = names_in(node, value).into_iter().filter(|(k, _, name, _)| *k == kind && name == old).map(|(.., p)| p).collect();
    for p in &places {
        if let Some(v) = value.pointer_mut(p) {
            *v = Value::String(new.to_string());
        }
    }
    places.len()
}

/// What a script function's argument `arg` does with which kind of name.
fn of_call(func: &str, arg: usize) -> Option<(NameKind, How)> {
    use rustpg2d_engine::script_api::{arg_kinds, Arg};
    match (func, arg) {
        ("get_text", 0) => return Some((NameKind::Text, How::Read)),
        ("set_text", 0) | ("input_text", 1) => return Some((NameKind::Text, How::Set)),
        _ => {}
    }
    let kind = arg_kinds(func, arg).find_map(|a| match a {
        Arg::Flag => Some(NameKind::Flag),
        Arg::Var => Some(NameKind::Variable),
        _ => None,
    })?;
    let how = if matches!(func, "flag" | "get_var") { How::Read } else { How::Set };
    Some((kind, how))
}

/// A name in a script: (kind, how, name, the literal's bytes with its quotes, line).
type InScript = (NameKind, How, String, (usize, usize), usize);

/// The names in a script.
fn names_in_script(src: &str) -> Vec<InScript> {
    rustpg2d_engine::script_refs::string_args(src).into_iter().filter(|a| !a.value.is_empty()).filter_map(|a| of_call(&a.func, a.arg).map(|(k, h)| (k, h, a.value, a.span, a.line))).collect()
}

/// Renames in a script's text. Returns the new text and the lines changed.
fn rename_in_script(src: &str, kind: NameKind, old: &str, new: &str) -> (String, Vec<usize>) {
    let mut hits: Vec<((usize, usize), usize)> = names_in_script(src).into_iter().filter(|(k, _, name, ..)| *k == kind && name == old).map(|(.., span, line)| (span, line)).collect();
    hits.sort_by_key(|h| std::cmp::Reverse(h.0 .0));
    let mut text = src.to_string();
    for ((a, b), _) in &hits {
        text.replace_range(*a..*b, &format!("\"{new}\""));
    }
    let mut lines: Vec<usize> = hits.iter().map(|h| h.1).collect();
    lines.sort_unstable();
    lines.dedup();
    (text, lines)
}

/// The names on a map: (kind, how, name, the place in words, where it is).
fn names_on_map(map: &Map) -> Vec<(NameKind, How, String, String, Target)> {
    let mut out = Vec::new();
    let name = &map.name;
    if let Ok(v) = serde_json::to_value(rustpg2d_engine::map::MapSettings::of(map)) {
        out.extend(names_in(schema::map_settings(), &v).into_iter().map(|(k, h, n, _)| (k, h, n, format!("{name}: the map's own events"), Target::MapTriggers(name.clone()))));
    }
    for e in &map.entities {
        let Ok(v) = serde_json::to_value(e) else { continue };
        let who = if e.name.is_empty() { e.def.clone() } else { e.name.clone() };
        out.extend(names_in(schema::entity(), &v).into_iter().map(|(k, h, n, _)| (k, h, n, format!("{name}: {who}"), Target::Entity(name.clone(), e.uid))));
    }
    for z in &map.zones {
        let Ok(v) = serde_json::to_value(z) else { continue };
        out.extend(names_in(schema::zone(), &v).into_iter().map(|(k, h, n, _)| (k, h, n, format!("{name}: zone {}", z.name), Target::Zone(name.clone(), z.uid))));
    }
    out
}

/// Renames on a map. Returns how many places changed.
fn rename_on_map(map: &mut Map, kind: NameKind, old: &str, new: &str) -> usize {
    let mut n = 0;
    if let Ok(mut v) = serde_json::to_value(rustpg2d_engine::map::MapSettings::of(map)) {
        let k = rename_in(schema::map_settings(), &mut v, kind, old, new);
        if let (true, Ok(settings)) = (k > 0, serde_json::from_value::<rustpg2d_engine::map::MapSettings>(v)) {
            settings.apply(map);
            n += k;
        }
    }
    for e in &mut map.entities {
        let Ok(mut v) = serde_json::to_value(&*e) else { continue };
        let k = rename_in(schema::entity(), &mut v, kind, old, new);
        if let (true, Ok(new)) = (k > 0, serde_json::from_value(v)) {
            *e = new;
            n += k;
        }
    }
    for z in &mut map.zones {
        let Ok(mut v) = serde_json::to_value(&*z) else { continue };
        let k = rename_in(schema::zone(), &mut v, kind, old, new);
        if let (true, Ok(new)) = (k > 0, serde_json::from_value(v)) {
            *z = new;
            n += k;
        }
    }
    n
}

/// A name may hold what an id may, and dots (`quest.stage`).
pub fn valid_name(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

impl Workspace {
    /// The definitions that can hold names, as (kind, id, JSON).
    fn defs_with_names(&self) -> Vec<(RefKind, String, Value)> {
        let mut out = Vec::new();
        for kind in RefKind::ALL.iter().copied().filter(|k| schema::of_kind(*k).is_some()) {
            for (id, _) in crate::defs::list(&self.project.db, kind) {
                if let Some(v) = self.def_value(kind, &id) {
                    out.push((kind, id, v));
                }
            }
        }
        out
    }

    /// Every map as it is now: the open ones as they are being worked on, the others from
    /// their files.
    fn maps_now(&self) -> Vec<Map> {
        self.maps
            .iter()
            .filter_map(|name| match self.open_maps().into_iter().find(|d| d.map.name == *name) {
                Some(doc) => Some(doc.map.clone()),
                None => self.project.load_map(name).ok(),
            })
            .collect()
    }

    /// Every flag, variable and text variable of the project with the places it stands:
    /// flags first, each kind by name.
    pub fn survey_names(&self) -> Vec<Named> {
        let mut all: std::collections::BTreeMap<(NameKind, String), Vec<NameUse>> = std::collections::BTreeMap::new();
        let mut add = |kind: NameKind, name: String, how: How, place: String, target: Target| all.entry((kind, name)).or_default().push(NameUse { how, place, target });
        for (kind, id, value) in self.defs_with_names() {
            let Some(node) = schema::of_kind(kind) else { continue };
            let label = value.get("name").or_else(|| value.get("title")).and_then(Value::as_str).filter(|n| !n.is_empty()).unwrap_or(&id).to_string();
            for (k, how, name, _) in names_in(node, &value) {
                add(k, name, how, format!("{} {label}", kind.label()), Target::Def(kind, id.clone()));
            }
        }
        for map in self.maps_now() {
            for (k, how, name, place, target) in names_on_map(&map) {
                add(k, name, how, place, target);
            }
        }
        for f in self.project.script_files() {
            let Ok(src) = std::fs::read_to_string(&f) else { continue };
            let file = f.file_name().unwrap_or_default().to_string_lossy().to_string();
            for (k, how, name, span, line) in names_in_script(&src) {
                add(k, name, how, format!("{file}:{line}"), Target::Script(f.clone(), src[..span.0.min(src.len())].chars().count()));
            }
        }
        all.into_iter().map(|((kind, name), uses)| Named { kind, name, uses }).collect()
    }

    /// What the creator noted about a name ("" = nothing).
    pub fn name_note(&self, kind: NameKind, name: &str) -> &str {
        self.meta.assets.get(&format!("{}:{name}", kind.key())).map_or("", |m| m.notes.as_str())
    }

    /// Notes something about a name: a step of the history, written at once.
    pub fn set_name_note(&mut self, kind: NameKind, name: &str, note: &str) -> bool {
        let before = self.meta.clone();
        let key = format!("{}:{name}", kind.key());
        if note.trim().is_empty() {
            self.meta.assets.remove(&key);
        } else {
            self.meta.assets.entry(key).or_default().notes = note.to_string();
        }
        if self.meta == before {
            return false;
        }
        self.db_hist.record(Extra::Meta(Box::new(before)));
        self.begin_unnamed(&format!("Note on the {} {name}", kind.label()));
        self.save_meta();
        true
    }

    /// Renames a flag, a variable or a text variable everywhere it stands: in the definitions
    /// (saved at once, as the maps on disk are), on every map, in the scripts (each file's old
    /// text kept as `<file>.rhai.bak`). Says what changed. One step of the history, undone by
    /// renaming back.
    pub fn rename_name(&mut self, kind: NameKind, old: &str, new: &str) -> Result<String, String> {
        let msg = self.rename_name_everywhere(kind, old, new)?;
        self.db_hist.record(Extra::Name { kind, old: old.to_string(), new: new.trim().to_string() });
        self.begin_unnamed(&format!("Rename the {} {old}", kind.label()));
        Ok(msg)
    }

    /// [`Self::rename_name`] without its step (undo and redo call it too).
    pub(crate) fn rename_name_everywhere(&mut self, kind: NameKind, old: &str, new: &str) -> Result<String, String> {
        let new = new.trim();
        if new == old || new.is_empty() {
            return Err("the new name is empty or the same as the old one".into());
        }
        if !valid_name(new) {
            return Err("names may only contain letters, digits, '_', '-' and '.'".into());
        }
        let all = self.survey_names();
        if !all.iter().any(|n| n.kind == kind && n.name == old) {
            return Err(format!("there is no {} \"{old}\"", kind.label()));
        }
        if all.iter().any(|n| n.kind == kind && n.name == new) {
            return Err(format!("there is a {} \"{new}\" already: the two would become one", kind.label()));
        }
        let (mut places, mut errors) = (0, Vec::new());
        // The definitions.
        for (k, id, mut value) in self.defs_with_names() {
            let Some(node) = schema::of_kind(k) else { continue };
            let n = rename_in(node, &mut value, kind, old, new);
            if n > 0 {
                match self.set_def_value(k, &id, value) {
                    Ok(()) => places += n,
                    Err(e) => errors.push(format!("{} {id}: {e}", k.name())),
                }
            }
        }
        // Every map on disk, then the open ones (which may have changes that aren't saved).
        let mut maps_changed = 0;
        for name in self.maps.clone() {
            match self.project.load_map(&name) {
                Ok(mut map) => {
                    let n = rename_on_map(&mut map, kind, old, new);
                    if n > 0 {
                        maps_changed += 1;
                        if let Err(e) = self.project.save_map(&map) {
                            errors.push(format!("{name}: {e:#}"));
                        }
                        if !self.open_maps().iter().any(|d| d.map.name == name) {
                            places += n;
                        }
                    }
                }
                Err(e) => errors.push(format!("{name}: {e:#}")),
            }
        }
        for doc in self.doc.iter_mut().chain(self.parked.values_mut()) {
            let n = rename_on_map(&mut doc.map, kind, old, new);
            if n > 0 {
                places += n;
                // (Its older steps still hold the old name.)
                doc.mark_changed();
                doc.clear_history();
            }
        }
        self.save_defs();
        // The scripts.
        let mut scripts: Vec<String> = Vec::new();
        for f in self.project.script_files() {
            let Ok(disk) = std::fs::read_to_string(&f) else { continue };
            let (text, lines) = rename_in_script(&disk, kind, old, new);
            if lines.is_empty() {
                continue;
            }
            let mut bak = f.clone().into_os_string();
            bak.push(".bak");
            if let Err(e) = std::fs::write(PathBuf::from(&bak), &disk).and_then(|_| std::fs::write(&f, &text)) {
                errors.push(format!("{}: {e}", f.display()));
                continue;
            }
            places += lines.len();
            scripts.push(format!("{}:{}", f.file_name().unwrap_or_default().to_string_lossy(), lines.iter().map(|l| l.to_string()).collect::<Vec<_>>().join(",")));
        }
        if !scripts.is_empty() {
            self.scripts_rev += 1;
        }
        // What was noted about it follows.
        if let Some(m) = self.meta.assets.remove(&format!("{}:{old}", kind.key())) {
            self.meta.assets.insert(format!("{}:{new}", kind.key()), m);
            self.save_meta();
        }
        self.refresh_refs();
        self.stale = true;
        let mut msg = format!("Renamed the {} '{old}' to '{new}' in {places} place{}", kind.label(), if places == 1 { "" } else { "s" });
        if maps_changed > 0 {
            msg.push_str(&format!(", {maps_changed} map file{}", if maps_changed == 1 { "" } else { "s" }));
        }
        if !scripts.is_empty() {
            msg.push_str(&format!(", scripts: {}", scripts.join(", ")));
        }
        if !errors.is_empty() {
            msg = format!("{msg}; errors: {}", errors.join("; "));
        }
        Ok(msg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_found_in_scripts_and_renamed_there() {
        let src = "fn talk() {\n    if flag(\"met\") { add_var(\"visits\", 1); }\n    set_flag(\"met\", true);\n    say(\"met\");\n    set_text(\"hero\", \"Ann\");\n    let n = get_var(\"visits\") + get_text(\"hero\").len();\n}\n";
        let found: Vec<(NameKind, How, String, usize)> = names_in_script(src).into_iter().map(|(k, h, n, _, line)| (k, h, n, line)).collect();
        assert_eq!(
            found,
            [
                (NameKind::Flag, How::Read, "met".to_string(), 2),
                (NameKind::Variable, How::Set, "visits".to_string(), 2),
                (NameKind::Flag, How::Set, "met".to_string(), 3),
                (NameKind::Text, How::Set, "hero".to_string(), 5),
                (NameKind::Variable, How::Read, "visits".to_string(), 6),
                (NameKind::Text, How::Read, "hero".to_string(), 6),
            ]
        );
        // The flag is renamed where it is a flag: the line that only says "met" stays.
        let (text, lines) = rename_in_script(src, NameKind::Flag, "met", "met_elder");
        assert_eq!(lines, [2, 3]);
        assert!(text.contains("flag(\"met_elder\")") && text.contains("set_flag(\"met_elder\", true)") && text.contains("say(\"met\")"));
        assert_eq!(rename_in_script(src, NameKind::Variable, "met", "x").1, Vec::<usize>::new());
    }

    #[test]
    fn a_field_is_a_name_by_what_the_schema_says() {
        assert_eq!(of_site("Action.SetFlag.name"), Some((NameKind::Flag, How::Set)));
        assert_eq!(of_site("Cond.Flag.name"), Some((NameKind::Flag, How::Read)));
        assert_eq!(of_site("Action.Choice.var"), Some((NameKind::Variable, How::Set)));
        assert_eq!(of_site("Cond.Var.name"), Some((NameKind::Variable, How::Read)));
        assert_eq!(of_site("Action.InputText.var"), Some((NameKind::Text, How::Set)));
        assert_eq!(of_site("Cond.TextVar.name"), Some((NameKind::Text, How::Read)));
        assert_eq!(of_site("Action.Dialog.text"), None);
        assert!(valid_name("quest.stage_2") && !valid_name("two words") && !valid_name(""));
    }
}
