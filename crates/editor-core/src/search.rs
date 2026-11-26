//! Finding things: the search index (maps, definitions, script functions, characters, zones,
//! dialog lines) and the usages of an id.

use std::path::PathBuf;

use rustpg2d_engine::defs::Def;
use rustpg2d_engine::map::Map;
use rustpg2d_engine::refactor::RefKind;

use crate::workspace::Workspace;

/// Search hits shown at most.
const MAX_HITS: usize = 50;

/// Where a search hit or a usage leads.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    Map(String),
    /// A map's own triggers (the inspector with nothing selected).
    MapTriggers(String),
    Def(RefKind, String),
    /// A script file, the cursor at this character.
    Script(PathBuf, usize),
    Entity(String, u32),
    Zone(String, u32),
    /// Database › Project.
    Project,
}

impl Target {
    /// Where it is, for the agent server (`find_usages`).
    pub fn describe(&self) -> String {
        match self {
            Target::Map(m) => format!("map {m}"),
            Target::MapTriggers(m) => format!("map {m}, map triggers"),
            Target::Def(k, id) => format!("{} {id}", k.name()),
            Target::Script(f, at) => {
                let src = std::fs::read_to_string(f).unwrap_or_default();
                let line = src.chars().take(*at).filter(|c| *c == '\n').count() + 1;
                format!("script {}:{line}", f.file_name().unwrap_or_default().to_string_lossy())
            }
            Target::Entity(m, uid) => format!("map {m}, entity uid {uid}"),
            Target::Zone(m, uid) => format!("map {m}, zone uid {uid}"),
            Target::Project => "project settings".into(),
        }
    }
}

/// What a hit is (its icon).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum What {
    Map,
    Def(RefKind),
    Function,
    Entity,
    Zone,
    Line,
    Project,
}

impl What {
    /// For `status` and the script's printout.
    pub fn name(self) -> &'static str {
        match self {
            What::Map => "map",
            What::Def(k) => k.name(),
            What::Function => "function",
            What::Entity => "entity",
            What::Zone => "zone",
            What::Line => "line",
            What::Project => "project",
        }
    }
}

/// A search entry or a usage.
#[derive(Clone, Debug)]
pub struct Entry {
    pub what: What,
    pub label: String,
    pub detail: String,
    /// The lower-case text the query is matched against.
    key: String,
    pub target: Target,
}

impl Entry {
    fn new(what: What, label: impl Into<String>, detail: impl Into<String>, target: Target) -> Self {
        let label = label.into();
        let detail = detail.into();
        let key = format!("{label} {detail}").to_lowercase();
        Entry { what, label, detail, key, target }
    }
}

/// The quick search box.
#[derive(Default)]
pub struct Search {
    pub open: bool,
    pub query: String,
    index: Vec<Entry>,
    /// Indices into the index, best first.
    pub hits: Vec<usize>,
    pub sel: usize,
    searched: Option<String>,
}

impl Search {
    pub fn hit(&self, k: usize) -> Option<&Entry> {
        self.hits.get(k).and_then(|&i| self.index.get(i))
    }

    /// Matches the query again when it changed.
    fn refresh(&mut self) {
        if self.searched.as_deref() == Some(self.query.as_str()) {
            return;
        }
        self.searched = Some(self.query.clone());
        let q = self.query.trim().to_lowercase();
        let mut scored: Vec<(i32, usize)> = if q.is_empty() {
            Vec::new()
        } else {
            self.index.iter().enumerate().filter_map(|(i, e)| score(&q, e).map(|s| (s, i))).collect()
        };
        scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        self.hits = scored.into_iter().take(MAX_HITS).map(|(_, i)| i).collect();
        self.sel = 0;
    }

    /// `search=` for `status`.
    pub fn status(&self) -> String {
        if self.open {
            format!("open:{}:{}", self.query, self.hits.len())
        } else {
            "closed".into()
        }
    }
}

/// The *Usages* window.
#[derive(Default)]
pub struct Usages {
    pub of: Option<(RefKind, String)>,
    pub list: Vec<Entry>,
}

impl Usages {
    /// `usages=` for `status`.
    pub fn status(&self) -> String {
        match &self.of {
            Some((k, id)) => format!("{}:{id}:{}", k.name(), self.list.len()),
            None => "closed".into(),
        }
    }
}

/// How well `q` (lower case) matches an entry; None = not at all. Names and ids match fuzzily
/// (the letters in order; word starts, runs and the start of the text score higher), dialog
/// lines only by their words (each word of the query somewhere in the line).
fn score(q: &str, e: &Entry) -> Option<i32> {
    if e.what == What::Line {
        return q.split_whitespace().all(|w| e.key.contains(w)).then(|| 10 - (e.key.len() as i32 / 40));
    }
    let label = e.label.to_lowercase();
    let mut best = fuzzy(q, &label).map(|s| s + 20);
    if let Some(s) = fuzzy(q, &e.key) {
        best = Some(best.map_or(s, |b: i32| b.max(s)));
    }
    // The query is the whole id or name, or a whole word of it: that one first.
    let whole = label == q || label.ends_with(&format!("({q})")) || label.split(|c: char| !c.is_alphanumeric() && c != '_').any(|w| w == q);
    best.map(|s| s - label.len() as i32 / 4 + if label == q || label.ends_with(&format!("({q})")) { 80 } else if whole { 30 } else { 0 })
}

/// Subsequence score of `q` in `text` (both lower case); a space in the query stands for any
/// word gap (`shrine awa` finds `shrine_awakening`).
fn fuzzy(q: &str, text: &str) -> Option<i32> {
    let found = text.find(q).or_else(|| q.contains(' ').then(|| text.find(&q.replace(' ', "_"))).flatten());
    let q: String = q.chars().filter(|c| !c.is_whitespace()).collect();
    let q = q.as_str();
    if let Some(at) = found {
        // A plain substring: best at the start or at a word start.
        let word_start = at == 0 || !text[..at].chars().last().is_some_and(|c| c.is_alphanumeric());
        return Some(100 + if at == 0 { 40 } else if word_start { 25 } else { 0 } + q.len() as i32 * 3);
    }
    let chars: Vec<char> = text.chars().collect();
    let mut s = 0;
    let mut pos = 0;
    let mut prev: Option<usize> = None;
    for qc in q.chars() {
        let found = (pos..chars.len()).find(|&i| chars[i] == qc)?;
        s += 1;
        if prev == Some(found.wrapping_sub(1)) {
            s += 5;
        }
        if found == 0 || !chars[found - 1].is_alphanumeric() {
            s += 8;
        }
        prev = Some(found);
        pos = found + 1;
    }
    Some(s)
}

/// The `name` (or `title`) of a definition, from its JSON.
fn def_name(v: &serde_json::Value) -> String {
    ["name", "title"].iter().find_map(|k| v.get(*k).and_then(|n| n.as_str())).unwrap_or_default().to_string()
}

/// Dialog and choice lines inside a JSON value (triggers, a definition).
fn dialog_lines(v: &serde_json::Value, out: &mut Vec<String>) {
    match v {
        serde_json::Value::Object(m) => {
            for (k, x) in m {
                if k == "Dialog" || k == "Choice" {
                    if let Some(t) = x.get("text").and_then(|t| t.as_str()) {
                        if !t.trim().is_empty() {
                            out.push(t.to_string());
                        }
                    }
                    for o in x.get("options").and_then(|o| o.as_array()).into_iter().flatten() {
                        if let Some(o) = o.as_str() {
                            out.push(o.to_string());
                        }
                    }
                }
                dialog_lines(x, out);
            }
        }
        serde_json::Value::Array(a) => a.iter().for_each(|x| dialog_lines(x, out)),
        _ => {}
    }
}

/// A line shortened for a list row.
fn short(text: &str) -> String {
    let one: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() > 70 {
        format!("\u{201C}{}\u{2026}\u{201D}", one.chars().take(68).collect::<String>())
    } else {
        format!("\u{201C}{one}\u{201D}")
    }
}

/// How often a JSON value names map `id`: only where a map is meant (`map`, `target_map`), as
/// spawn points are often named like the map they lead to.
fn names_map(v: &serde_json::Value, id: &str) -> usize {
    match v {
        serde_json::Value::Object(m) => m.iter().map(|(k, x)| usize::from((k == "map" || k == "target_map") && x.as_str() == Some(id)) + names_map(x, id)).sum(),
        serde_json::Value::Array(a) => a.iter().map(|x| names_map(x, id)).sum(),
        _ => 0,
    }
}

/// [`names_id`], or [`names_map`] for maps.
fn names(kind: RefKind, v: &serde_json::Value, id: &str) -> usize {
    if kind == RefKind::Map {
        names_map(v, id)
    } else {
        names_id(v, id)
    }
}

/// Whether a JSON value names `id` (as a whole string), leaving the object's own `name` and
/// `uid` out (a placed guard named "guard" doesn't use the character "guard").
fn names_id(v: &serde_json::Value, id: &str) -> usize {
    let mut v = v.clone();
    if let Some(m) = v.as_object_mut() {
        m.remove("name");
        m.remove("uid");
    }
    serde_json::to_string(&v).unwrap_or_default().matches(&format!("\"{}\"", id.replace('"', "\\\""))).count()
}

/// `fn name(` definitions in a script, with the character index where each starts.
pub fn script_functions(text: &str) -> Vec<(String, usize)> {
    rustpg2d_engine::script_refs::functions(text).into_iter().map(|f| (f.name, text[..f.at].chars().count())).collect()
}

impl Workspace {
    /// A map: the open one as edited, others from disk.
    fn search_map(&self, name: &str) -> Option<Map> {
        match &self.doc {
            Some(d) if d.map.name == name => Some(d.map.clone()),
            _ => self.project.load_map(name).ok(),
        }
    }

    /// Everything the search box can find.
    fn build_search_index(&self) -> Vec<Entry> {
        let mut out = Vec::new();
        let db = &self.project.db;
        for m in &self.maps {
            out.push(Entry::new(What::Map, m.clone(), "map", Target::Map(m.clone())));
        }
        let kinds = [
            RefKind::VisualScript,
            RefKind::Item,
            RefKind::Prop,
            RefKind::Character,
            RefKind::Quest,
            RefKind::Skill,
            RefKind::Spell,
            RefKind::Recipe,
            RefKind::Achievement,
            RefKind::Race,
            RefKind::Structure,
            RefKind::Sheet,
            RefKind::Material,
            RefKind::Scatter,
            RefKind::Sound,
            RefKind::Font,
            RefKind::Skin,
        ];
        for k in kinds {
            let defs: Vec<serde_json::Value> = rustpg2d_engine::with_def_kind!(k, |T| T::list(db).iter().filter_map(|d| serde_json::to_value(d).ok()).collect(), else => Vec::new());
            for v in defs {
                let id = v.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string();
                let name = def_name(&v);
                let label = if name.is_empty() || name == id { id.clone() } else { format!("{name} ({id})") };
                out.push(Entry::new(What::Def(k), label, k.label(), Target::Def(k, id.clone())));
                let mut lines = Vec::new();
                dialog_lines(&v, &mut lines);
                for l in lines {
                    out.push(Entry::new(What::Line, short(&l), format!("{} {id}", k.name()), Target::Def(k, id.clone())));
                }
            }
        }
        for f in self.project.script_files() {
            let Ok(src) = std::fs::read_to_string(&f) else { continue };
            let file = f.file_name().unwrap_or_default().to_string_lossy().to_string();
            for (name, at) in script_functions(&src) {
                out.push(Entry::new(What::Function, format!("{name}()"), file.clone(), Target::Script(f.clone(), at)));
            }
        }
        for name in &self.maps {
            let Some(map) = self.search_map(name) else { continue };
            for e in map.entities.iter().filter(|e| !e.name.is_empty()) {
                out.push(Entry::new(What::Entity, e.name.clone(), format!("{} on {name}", e.def), Target::Entity(name.clone(), e.uid)));
            }
            for z in &map.zones {
                out.push(Entry::new(What::Zone, z.name.clone(), format!("{} zone on {name}", z.kind.label()), Target::Zone(name.clone(), z.uid)));
            }
            let mut lines = Vec::new();
            dialog_lines(&serde_json::to_value(&map.triggers).unwrap_or_default(), &mut lines);
            out.extend(lines.into_iter().map(|l| Entry::new(What::Line, short(&l), format!("map {name}"), Target::MapTriggers(name.clone()))));
            for z in &map.zones {
                let mut lines = Vec::new();
                dialog_lines(&serde_json::to_value(&z.triggers).unwrap_or_default(), &mut lines);
                out.extend(lines.into_iter().map(|l| Entry::new(What::Line, short(&l), format!("zone {} on {name}", z.name), Target::Zone(name.clone(), z.uid))));
            }
            for e in &map.entities {
                let mut lines = Vec::new();
                dialog_lines(&serde_json::to_value(&e.triggers).unwrap_or_default(), &mut lines);
                let who = if e.name.is_empty() { e.def.clone() } else { e.name.clone() };
                out.extend(lines.into_iter().map(|l| Entry::new(What::Line, short(&l), format!("{who} on {name}"), Target::Entity(name.clone(), e.uid))));
            }
        }
        out
    }

    /// Searches the project: the hits for `query`, best first (the index is rebuilt when the
    /// project changed since the last search).
    pub fn search(&mut self, query: &str) -> Vec<Entry> {
        if self.search.index.is_empty() || self.search_stale {
            self.search.index = self.build_search_index();
            self.search_stale = false;
        }
        self.search.query = query.to_string();
        self.search.searched = None;
        self.search.refresh();
        self.search.hits.iter().filter_map(|&i| self.search.index.get(i).cloned()).collect()
    }

    /// Every place `id` (a definition or map of `kind`) is used.
    pub fn find_usages(&self, kind: RefKind, id: &str) -> Vec<Entry> {
        let mut out = Vec::new();
        let db = &self.project.db;
        if kind != RefKind::Map {
            for (k, dep) in rustpg2d_engine::library::dependents(db, &(kind, id.to_string())) {
                out.push(Entry::new(What::Def(k), dep.clone(), format!("{} uses it", k.label()), Target::Def(k, dep)));
            }
            // Quests and achievements aren't library assets: their triggers / texts can still name it.
            for k in [RefKind::Quest, RefKind::Achievement] {
                let defs: Vec<serde_json::Value> = rustpg2d_engine::with_def_kind!(k, |T| T::list(db).iter().filter_map(|d| serde_json::to_value(d).ok()).collect(), else => Vec::new());
                for v in defs {
                    let did = v.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string();
                    let listed = out.iter().any(|e: &Entry| e.target == Target::Def(k, did.clone()));
                    if !(k == kind && did == id) && !listed && names(kind, &v, id) > 0 {
                        out.push(Entry::new(What::Def(k), did.clone(), format!("{} names it", k.label()), Target::Def(k, did)));
                    }
                }
            }
        }
        let ek = match kind {
            RefKind::Item => Some(rustpg2d_engine::entity::EntityKind::Item),
            RefKind::Prop => Some(rustpg2d_engine::entity::EntityKind::Prop),
            RefKind::Character => Some(rustpg2d_engine::entity::EntityKind::Character),
            _ => None,
        };
        for name in &self.maps {
            let Some(map) = self.search_map(name) else { continue };
            let mut settings = Vec::new();
            match kind {
                RefKind::Material if map.materials.iter().any(|t| t == id) => settings.push("painted ground"),
                RefKind::Prop if map.decor_defs.iter().any(|t| t == id) => settings.push("decorations"),
                RefKind::Sound if map.music == id => settings.push("music"),
                RefKind::Sound if map.battle_music.as_deref() == Some(id) => settings.push("battle music"),
                RefKind::Sheet if names_id(&serde_json::to_value(&map.parallax).unwrap_or_default(), id) > 0 => settings.push("parallax layer"),
                RefKind::Map if names_map(&serde_json::to_value(&map.edges).unwrap_or_default(), id) > 0 => settings.push("map edge"),
                _ => {}
            }
            if names(kind, &serde_json::to_value(&map.triggers).unwrap_or_default(), id) > 0 {
                settings.push("map triggers");
            }
            for s in settings {
                out.push(Entry::new(What::Map, name.clone(), s, if s == "map triggers" { Target::MapTriggers(name.clone()) } else { Target::Map(name.clone()) }));
            }
            for e in &map.entities {
                let placed = ek == Some(e.kind) && e.def == id;
                let named = names(kind, &serde_json::to_value(e).unwrap_or_default(), id).saturating_sub(usize::from(placed && names_id(&serde_json::json!({ "def": e.def }), id) > 0));
                if placed || named > 0 {
                    let who = if e.name.is_empty() { e.def.clone() } else { format!("{} ({})", e.name, e.def) };
                    let ts = self.project.config.tile_size as f32;
                    let how = if placed { format!("placed on {name} at {},{}", (e.x / ts).floor(), ((e.y - 1.0) / ts).floor()) } else { format!("on {name}: its triggers / items name it") };
                    out.push(Entry::new(What::Entity, who, how, Target::Entity(name.clone(), e.uid)));
                }
            }
            for z in &map.zones {
                if names(kind, &serde_json::to_value(z).unwrap_or_default(), id) > 0 {
                    out.push(Entry::new(What::Zone, z.name.clone(), format!("{} zone on {name}", z.kind.label()), Target::Zone(name.clone(), z.uid)));
                }
            }
        }
        // Scripts: string arguments of API calls that take this kind of id.
        let id_kind = match kind {
            RefKind::Item => Some(rustpg2d_engine::script_refs::IdKind::Item),
            RefKind::Sound => Some(rustpg2d_engine::script_refs::IdKind::Sound),
            RefKind::Quest => Some(rustpg2d_engine::script_refs::IdKind::Quest),
            RefKind::Skill => Some(rustpg2d_engine::script_refs::IdKind::Skill),
            RefKind::Spell => Some(rustpg2d_engine::script_refs::IdKind::Spell),
            RefKind::Sheet => Some(rustpg2d_engine::script_refs::IdKind::Sheet),
            RefKind::Character => Some(rustpg2d_engine::script_refs::IdKind::Character),
            RefKind::Recipe => Some(rustpg2d_engine::script_refs::IdKind::Recipe),
            RefKind::Achievement => Some(rustpg2d_engine::script_refs::IdKind::Achievement),
            RefKind::Prop => Some(rustpg2d_engine::script_refs::IdKind::Prop),
            RefKind::CommonEvent => Some(rustpg2d_engine::script_refs::IdKind::CommonEvent),
            RefKind::Class => Some(rustpg2d_engine::script_refs::IdKind::Class),
            RefKind::Disease => Some(rustpg2d_engine::script_refs::IdKind::Disease),
            RefKind::Map => Some(rustpg2d_engine::script_refs::IdKind::Map),
            _ => None,
        };
        if let Some(ik) = id_kind {
            for f in self.project.script_files() {
                let Ok(src) = std::fs::read_to_string(&f) else { continue };
                let file = f.file_name().unwrap_or_default().to_string_lossy().to_string();
                for (a, _) in rustpg2d_engine::script_refs::id_refs(&src).into_iter().filter(|(a, k)| *k == ik && a.value == id) {
                    let line = src.lines().nth(a.line.saturating_sub(1)).unwrap_or_default().trim().to_string();
                    let at = src[..a.span.0.min(src.len())].chars().count();
                    out.push(Entry::new(What::Function, format!("{file}:{}", a.line), short(&line), Target::Script(f.clone(), at)));
                }
            }
        }
        if kind == RefKind::Character && self.project.config.player == id {
            out.push(Entry::new(What::Project, "Project", "the player character", Target::Project));
        }
        if kind == RefKind::Map && self.project.config.start_map == id {
            out.push(Entry::new(What::Project, "Project", "the start map", Target::Project));
        }
        if kind == RefKind::Sound && self.project.config.battle_music == id {
            out.push(Entry::new(What::Project, "Project", "the battle music", Target::Project));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(what: What, label: &str) -> Entry {
        Entry::new(what, label, "", Target::Project)
    }

    #[test]
    fn fuzzy_ranks_starts_and_runs_first() {
        let q = "pot";
        let a = score(q, &entry(What::Def(RefKind::Item), "potion")).unwrap();
        let b = score(q, &entry(What::Def(RefKind::Item), "spotted_cap")).unwrap();
        let c = score(q, &entry(What::Def(RefKind::Item), "pick_of_toil")).unwrap();
        assert!(a > b && b > c, "{a} {b} {c}");
        // The exact id beats a longer name that starts with it.
        let exact = score(q, &entry(What::Def(RefKind::Item), "Red Pot (pot)")).unwrap();
        let shelf = score(q, &entry(What::Def(RefKind::Prop), "Pot Shelf (pot_shelf)")).unwrap();
        assert!(exact > shelf, "{exact} {shelf}");
        assert!(score("xyz", &entry(What::Map, "village")).is_none());
        // Letters in order across words.
        assert!(score("hpot", &entry(What::Def(RefKind::Item), "health_potion")).is_some());
        // A space stands for a word gap.
        assert!(score("shrine awa", &entry(What::Function, "shrine_awakening()")).is_some());
    }

    #[test]
    fn lines_match_by_words() {
        let e = Entry::new(What::Line, "\u{201C}Welcome to the forge\u{201D}", "map smithy", Target::Project);
        assert!(score("forge welcome", &e).is_some());
        assert!(score("fwg", &e).is_none());
    }

    #[test]
    fn dialog_lines_are_found_in_nested_actions() {
        let v = serde_json::json!([{ "actions": [{ "If": { "then": [{ "Dialog": { "speaker": "Tilda", "text": "Hello" } }], "otherwise": [{ "Choice": { "text": "Buy?", "options": ["Yes", "No"] } }] } }] }]);
        let mut out = Vec::new();
        dialog_lines(&v, &mut out);
        out.sort();
        assert_eq!(out, ["Buy?", "Hello", "No", "Yes"]);
    }

    #[test]
    fn an_objects_own_name_is_no_usage() {
        let v = serde_json::json!({ "uid": 3, "name": "guard", "def": "villager", "triggers": [] });
        assert_eq!(names_id(&v, "guard"), 0);
        let v = serde_json::json!({ "name": "x", "loot": [{ "item": "potion" }] });
        assert_eq!(names_id(&v, "potion"), 1);
        // A spawn named like a map isn't the map.
        let z = serde_json::json!({ "kind": { "Portal": { "target_map": "forest", "target_spawn": "mine" } } });
        assert_eq!(names(RefKind::Map, &z, "mine"), 0);
        assert_eq!(names(RefKind::Map, &z, "forest"), 1);
    }
}
