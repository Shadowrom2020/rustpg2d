//! *Problems*: one list of everything that is broken in the project, collected from the checks
//! that exist elsewhere (the definitions' references, map objects and zones, the script check
//! and compiler, translation templates, the export's project check, objects a map file had to
//! leave out). Each entry is an error (something is missing or can't work) or a warning
//! (probably unintended) and leads to its place like a search hit. The list is rebuilt when
//! something is saved (and on *Refresh*); maps that didn't change aren't read again.

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

use egui_macroquad::egui;
use rustpg2d_engine::defs::{CharacterDef, PropDef};
use rustpg2d_engine::entity::EntityKind;
use rustpg2d_engine::events::Trigger;
use rustpg2d_engine::map::{Map, ZoneKind};
use rustpg2d_engine::refactor::RefKind;

use crate::app::Editor;
use crate::search::Target;
use crate::ui::{self, ph};

/// How bad a problem is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Error,
    Warning,
}

impl Severity {
    pub fn name(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        }
    }

    pub fn parse(s: &str) -> Option<Severity> {
        match s.trim().to_lowercase().as_str() {
            "error" | "errors" | "e" => Some(Severity::Error),
            "warning" | "warnings" | "w" => Some(Severity::Warning),
            _ => None,
        }
    }
}

/// Where a problem is (the panel's filter).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Area {
    Project,
    Definitions,
    Maps,
    Scripts,
    Translations,
}

impl Area {
    pub const ALL: [Area; 5] = [Area::Project, Area::Definitions, Area::Maps, Area::Scripts, Area::Translations];

    pub fn name(self) -> &'static str {
        match self {
            Area::Project => "project",
            Area::Definitions => "definitions",
            Area::Maps => "maps",
            Area::Scripts => "scripts",
            Area::Translations => "translations",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Area::Project => "Project",
            Area::Definitions => "Definitions",
            Area::Maps => "Maps",
            Area::Scripts => "Scripts",
            Area::Translations => "Translations",
        }
    }

    pub fn parse(s: &str) -> Option<Area> {
        let s = s.trim().to_lowercase();
        Area::ALL.into_iter().find(|a| a.name() == s || a.name().trim_end_matches('s') == s)
    }
}

/// One entry of the list.
#[derive(Clone, Debug)]
pub struct Problem {
    pub severity: Severity,
    pub area: Area,
    /// Whose problem it is ("item potion", "map village: entity 12 (guard)").
    pub place: String,
    pub text: String,
    pub target: Target,
}

impl Problem {
    /// One line for scripts and the agent server.
    pub fn line(&self) -> String {
        format!("{} [{}] {}: {}", self.severity.name(), self.area.name(), self.place, self.text)
    }
}

/// A map as read from its file (cached until the file changes).
struct MapFile {
    stamp: (u64, u64),
    map: Option<Map>,
    /// What loading left out, or why it failed.
    warnings: Vec<String>,
}

/// The *Problems* panel.
#[derive(Default)]
pub struct Problems {
    pub open: bool,
    pub list: Vec<Problem>,
    /// Rebuild the list (something was saved).
    pub stale: bool,
    /// Has the list been built since the project opened?
    pub built: bool,
    pub show_errors: bool,
    pub show_warnings: bool,
    /// Only this area (None = all).
    pub area: Option<Area>,
    /// What loading the definitions left out (kept until they are saved again).
    pub defs_warnings: Vec<String>,
    maps: HashMap<String, MapFile>,
}

impl Problems {
    pub fn new() -> Self {
        Problems { show_errors: true, show_warnings: true, ..Default::default() }
    }

    pub fn counts(&self) -> (usize, usize) {
        let e = self.list.iter().filter(|p| p.severity == Severity::Error).count();
        (e, self.list.len() - e)
    }

    /// `problems=errors/warnings` for `status` (`-` before the first check).
    pub fn status(&self) -> String {
        if !self.built {
            return "-".into();
        }
        let (e, w) = self.counts();
        format!("{e}/{w}")
    }

    /// The entries the filters let through.
    pub fn shown(&self) -> Vec<&Problem> {
        self.list.iter().filter(|p| (p.severity == Severity::Error && self.show_errors || p.severity == Severity::Warning && self.show_warnings) && self.area.is_none_or(|a| a == p.area)).collect()
    }
}

/// `text` cut to `n` characters.
fn shorten(text: &str, n: usize) -> String {
    if text.chars().count() <= n {
        text.to_string()
    } else {
        format!("{}…", text.chars().take(n).collect::<String>())
    }
}

/// The line of a compiler message ("… (line 12, position 4)").
fn message_line(msg: &str) -> Option<usize> {
    let i = msg.rfind("line ")?;
    msg[i + 5..].chars().take_while(char::is_ascii_digit).collect::<String>().parse().ok()
}

/// The character offset of the start of `line` (1-based) in `src`.
fn line_offset(src: &str, line: usize) -> usize {
    src.split_inclusive('\n').take(line.saturating_sub(1)).map(|l| l.chars().count()).sum()
}

/// What an entity names beyond its definition: its own triggers, loot, shop, look, gear,
/// portrait, container and lock, as the definitions that hold the same fields see them.
fn entity_refs(e: &rustpg2d_engine::entity::EntityInstance) -> Vec<(RefKind, String)> {
    let mut c = CharacterDef { triggers: e.triggers.clone(), shop: e.shop.clone(), portrait: e.portrait.clone().unwrap_or_default(), ..Default::default() };
    if let Some(l) = &e.loot {
        c.loot = l.clone();
    }
    if let Some(a) = &e.appearance {
        c.appearance = a.clone();
    } else {
        c.appearance.body.clear();
        c.appearance.hair.clear();
        c.appearance.beard.clear();
    }
    if let Some(eq) = &e.equipment {
        c.equipment = eq.clone();
    }
    if let Some(inv) = &e.inventory {
        c.inventory = inv.clone();
    }
    let mut out = rustpg2d_engine::refactor::refs_of_character(&c);
    let p = PropDef { container: e.container.clone(), lock: e.lock.clone(), ..Default::default() };
    out.extend(rustpg2d_engine::refactor::refs_of_prop(&p));
    for s in &e.schedule {
        out.push((RefKind::Map, s.map.clone()));
    }
    out.retain(|(_, id)| !id.is_empty());
    out.sort();
    out.dedup();
    out
}

impl Editor {
    /// Does `id` of `kind` exist (maps included; an empty id counts as "none chosen")?
    fn problem_id_exists(&self, kind: RefKind, id: &str) -> bool {
        id.is_empty() || if kind == RefKind::Map { self.maps.iter().any(|m| m == id) } else { self.project.db.has(kind, id) }
    }

    /// Reads the maps whose files changed since the last check.
    fn problem_maps(&mut self) {
        let names: BTreeSet<String> = self.maps.iter().cloned().collect();
        self.problems.maps.retain(|n, _| names.contains(n));
        for name in names {
            let path = self.project.map_path(&name);
            let stamp = crate::app::file_stamp(&path);
            if self.problems.maps.get(&name).is_some_and(|f| f.stamp == stamp && stamp != (0, 0)) {
                continue;
            }
            let mut warnings = Vec::new();
            let map = match rustpg2d_engine::map::io::load_with(&path, &mut |w| warnings.push(w)) {
                Ok(m) => Some(m),
                Err(e) => {
                    warnings.push(format!("{e:#}"));
                    None
                }
            };
            self.problems.maps.insert(name, MapFile { stamp, map, warnings });
        }
    }

    /// Checks the whole project and replaces the list.
    pub fn refresh_problems(&mut self) {
        self.problem_maps();
        let mut out = Vec::new();
        self.definition_problems(&mut out);
        self.map_problems(&mut out);
        self.script_problems(&mut out);
        self.translation_problems(&mut out);
        self.project_problems(&mut out);
        out.sort_by(|a, b| a.severity.cmp(&b.severity).then(a.area.cmp(&b.area)).then(a.place.cmp(&b.place)));
        self.problems.list = out;
        self.problems.stale = false;
        self.problems.built = true;
    }

    /// Unknown ids a trigger list names, and the script functions it runs that don't exist.
    fn trigger_problems(&self, ts: &[Trigger], area: Area, place: &str, target: &Target, out: &mut Vec<Problem>) {
        for f in crate::mcp::exec_maps::functions_in(ts) {
            if !self.refs.functions.contains(&f) {
                out.push(Problem { severity: Severity::Error, area, place: place.to_string(), text: format!("runs the script function {f}(), which isn't defined in scripts/"), target: target.clone() });
            }
        }
    }

    fn definition_problems(&self, out: &mut Vec<Problem>) {
        let db = &self.project.db;
        for k in RefKind::DEFS {
            for id in db.ids_of(k) {
                let place = format!("{} {id}", k.name());
                let target = Target::Def(k, id.clone());
                let mut missing: Vec<(RefKind, String)> = db.refs_of(k, &id).into_iter().filter(|(rk, rid)| !self.problem_id_exists(*rk, rid)).collect();
                missing.sort();
                missing.dedup();
                for (rk, rid) in missing {
                    out.push(Problem { severity: Severity::Error, area: Area::Definitions, place: place.clone(), text: format!("refers to {} \"{rid}\", which doesn't exist", rk.name()), target: target.clone() });
                }
                let triggers = match k {
                    RefKind::Item => db.item(&id).map(|d| d.on_use.clone()),
                    RefKind::Prop => db.prop(&id).map(|d| d.triggers.clone()),
                    RefKind::Character => db.character(&id).map(|d| d.triggers.clone()),
                    _ => None,
                };
                self.trigger_problems(&triggers.unwrap_or_default(), Area::Definitions, &place, &target, out);
                let file = match k {
                    RefKind::Tileset => db.tileset(&id).map(|d| d.image.clone()),
                    RefKind::Sheet => db.sheet(&id).map(|d| d.image.clone()),
                    RefKind::Sound => db.sound(&id).map(|d| d.file.clone()),
                    _ => None,
                };
                if let Some(f) = file {
                    if f.is_empty() || !self.project.root.join(&f).is_file() {
                        out.push(Problem { severity: Severity::Error, area: Area::Definitions, place: place.clone(), text: if f.is_empty() { "has no file".into() } else { format!("its file {f} is missing") }, target: target.clone() });
                    }
                }
                if k == RefKind::Character && db.character(&id).is_some_and(|c| c.appearance.body.is_empty()) {
                    out.push(Problem { severity: Severity::Error, area: Area::Definitions, place: place.clone(), text: "has no body (it would be invisible)".into(), target: target.clone() });
                }
                let faction = match k {
                    RefKind::Character => db.character(&id).map(|d| d.faction.clone()),
                    RefKind::Race => db.race(&id).map(|d| d.faction.clone()),
                    _ => None,
                };
                // (A project without factions leaves the fields dormant: library characters name theirs.)
                let factions = &self.project.config.factions;
                if let Some(f) = faction.filter(|f| !f.is_empty() && !factions.is_empty() && factions.get(f).is_none()) {
                    out.push(Problem { severity: Severity::Warning, area: Area::Definitions, place: place.clone(), text: format!("belongs to the faction \"{f}\", which isn't in Database › Project › Factions (it has no faction until it is added there)"), target: target.clone() });
                }
                if k == RefKind::Quest && db.quest(&id).is_some_and(|q| q.stages.is_empty()) {
                    out.push(Problem { severity: Severity::Warning, area: Area::Definitions, place: place.clone(), text: "has no stages: the journal shows nothing while it is active".into(), target: target.clone() });
                }
            }
        }
        for w in &self.problems.defs_warnings {
            out.push(Problem { severity: Severity::Error, area: Area::Definitions, place: w.split_whitespace().next().unwrap_or("defs").to_string(), text: format!("{w} (it was left out and is lost when the definitions are saved)"), target: Target::Project });
        }
    }

    fn map_problems(&self, out: &mut Vec<Problem>) {
        let db = &self.project.db;
        let ts = self.project.config.tile_size as f32;
        // Spawn and transition zone names per map (portals and transitions name them).
        let mut spawns: HashMap<&str, Vec<&str>> = HashMap::new();
        let mut transitions: HashMap<&str, Vec<&str>> = HashMap::new();
        for (name, f) in &self.problems.maps {
            let Some(m) = &f.map else { continue };
            spawns.insert(name, m.zones.iter().filter(|z| z.kind == ZoneKind::Spawn).map(|z| z.name.as_str()).collect());
            transitions.insert(name, m.zones.iter().filter(|z| matches!(z.kind, ZoneKind::Transition { .. })).map(|z| z.name.as_str()).collect());
        }
        // Which maps lead where (for the maps nothing leads to).
        let mut links: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
        let mut from_anywhere: BTreeSet<String> = BTreeSet::new();
        for (name, f) in &self.problems.maps {
            let map_target = Target::Map(name.clone());
            for w in &f.warnings {
                out.push(Problem { severity: Severity::Error, area: Area::Maps, place: format!("map {name}"), text: format!("{w} (left out: saving the map again loses it)"), target: map_target.clone() });
            }
            let Some(m) = &f.map else { continue };
            let to = links.entry(name).or_default();
            let unknown = |what: &str, target: &Target, refs: Vec<(RefKind, String)>, out: &mut Vec<Problem>, to: &mut BTreeSet<String>| {
                let mut seen = BTreeSet::new();
                for (rk, rid) in refs {
                    if rk == RefKind::Map {
                        to.insert(rid.clone());
                    }
                    if !self.problem_id_exists(rk, &rid) && seen.insert((rk, rid.clone())) {
                        out.push(Problem { severity: Severity::Error, area: Area::Maps, place: format!("map {name}: {what}"), text: format!("refers to {} \"{rid}\", which doesn't exist", rk.name()), target: target.clone() });
                    }
                }
            };
            // The map's own settings and triggers.
            let mut refs: Vec<(RefKind, String)> = m.tilesets.iter().map(|t| (RefKind::Tileset, t.clone())).collect();
            refs.push((RefKind::Sound, m.music.clone()));
            refs.extend(m.parallax.iter().map(|l| (RefKind::Sheet, l.sheet.clone())));
            refs.extend([&m.edges.north, &m.edges.south, &m.edges.east, &m.edges.west].into_iter().flatten().map(|l| (RefKind::Map, l.target_map.clone())));
            unknown("settings", &map_target, refs.into_iter().filter(|(_, id)| !id.is_empty()).collect(), out, to);
            unknown("map triggers", &Target::MapTriggers(name.clone()), rustpg2d_engine::refactor::refs_of_triggers(&m.triggers), out, to);
            self.trigger_problems(&m.triggers, Area::Maps, &format!("map {name}: map triggers"), &Target::MapTriggers(name.clone()), out);
            for e in &m.entities {
                let who = if e.name.is_empty() { format!("{} {} at {},{}", e.kind.label().to_lowercase(), e.def, (e.x / ts).floor(), ((e.y - 1.0) / ts).floor()) } else { format!("{} ({})", e.name, e.def) };
                let target = Target::Entity(name.clone(), e.uid);
                let def_kind = match e.kind {
                    EntityKind::Item => RefKind::Item,
                    EntityKind::Prop => RefKind::Prop,
                    EntityKind::Character => RefKind::Character,
                };
                if !db.has(def_kind, &e.def) {
                    out.push(Problem { severity: Severity::Error, area: Area::Maps, place: format!("map {name}: {who}"), text: format!("its {} \"{}\" doesn't exist", def_kind.name(), e.def), target: target.clone() });
                }
                unknown(&who, &target, entity_refs(e), out, to);
                self.trigger_problems(&e.triggers, Area::Maps, &format!("map {name}: {who}"), &target, out);
            }
            for z in &m.zones {
                let who = format!("zone {}", z.name);
                let target = Target::Zone(name.clone(), z.uid);
                let place = format!("map {name}: {who}");
                match &z.kind {
                    ZoneKind::Portal { target_map, target_spawn } => {
                        to.insert(target_map.clone());
                        if target_map.is_empty() {
                            out.push(Problem { severity: Severity::Error, area: Area::Maps, place: place.clone(), text: "the portal leads nowhere (no target map)".into(), target: target.clone() });
                        } else if !self.problem_id_exists(RefKind::Map, target_map) {
                            out.push(Problem { severity: Severity::Error, area: Area::Maps, place: place.clone(), text: format!("the portal leads to map \"{target_map}\", which doesn't exist"), target: target.clone() });
                        } else if !target_spawn.is_empty() && spawns.get(target_map.as_str()).is_some_and(|s| !s.contains(&target_spawn.as_str())) {
                            out.push(Problem { severity: Severity::Error, area: Area::Maps, place: place.clone(), text: format!("the portal leads to spawn \"{target_spawn}\", which map {target_map} doesn't have"), target: target.clone() });
                        }
                    }
                    ZoneKind::Transition { target_map, target_zone } => {
                        to.insert(target_map.clone());
                        if !self.problem_id_exists(RefKind::Map, target_map) || target_map.is_empty() {
                            out.push(Problem { severity: Severity::Error, area: Area::Maps, place: place.clone(), text: format!("the transition leads to map \"{target_map}\", which doesn't exist"), target: target.clone() });
                        } else if transitions.get(target_map.as_str()).is_some_and(|t| !t.contains(&target_zone.as_str())) {
                            out.push(Problem { severity: Severity::Error, area: Area::Maps, place: place.clone(), text: format!("the transition leads to zone \"{target_zone}\", which map {target_map} doesn't have"), target: target.clone() });
                        }
                    }
                    ZoneKind::Spawner { character, .. } => {
                        if !db.has(RefKind::Character, character) {
                            out.push(Problem { severity: Severity::Error, area: Area::Maps, place: place.clone(), text: format!("spawns character \"{character}\", which doesn't exist"), target: target.clone() });
                        }
                    }
                    ZoneKind::FishingSpot { rod } => {
                        if !self.problem_id_exists(RefKind::Item, rod) {
                            out.push(Problem { severity: Severity::Error, area: Area::Maps, place: place.clone(), text: format!("needs the rod \"{rod}\", which isn't an item"), target: target.clone() });
                        }
                    }
                    ZoneKind::Spawn | ZoneKind::Trigger => {}
                }
                let mut refs = rustpg2d_engine::refactor::refs_of_triggers(&z.triggers);
                if let Some(l) = &z.loot {
                    refs.extend(l.iter().map(|e| (RefKind::Item, e.item.clone())));
                }
                unknown(&who, &target, refs, out, to);
                self.trigger_problems(&z.triggers, Area::Maps, &place, &target, out);
            }
        }
        // Maps the definitions' triggers and the scripts name can be reached from anywhere.
        for k in RefKind::DEFS {
            for id in self.project.db.ids_of(k) {
                from_anywhere.extend(self.project.db.refs_of(k, &id).into_iter().filter(|(rk, _)| *rk == RefKind::Map).map(|(_, m)| m));
            }
        }
        for f in self.project.script_files() {
            let Ok(src) = std::fs::read_to_string(&f) else { continue };
            from_anywhere.extend(rustpg2d_engine::script_refs::id_refs(&src).into_iter().filter(|(_, k)| *k == rustpg2d_engine::script_refs::IdKind::Map).map(|(a, _)| a.value));
        }
        let start = &self.project.config.start_map;
        let mut reached: BTreeSet<String> = from_anywhere.clone();
        reached.insert(start.clone());
        let mut queue: VecDeque<String> = reached.iter().cloned().collect();
        while let Some(m) = queue.pop_front() {
            for next in links.get(m.as_str()).into_iter().flatten() {
                if reached.insert(next.clone()) {
                    queue.push_back(next.clone());
                }
            }
        }
        if self.maps.contains(start) {
            for m in self.maps.iter().filter(|m| !reached.contains(*m)) {
                out.push(Problem { severity: Severity::Warning, area: Area::Maps, place: format!("map {m}"), text: format!("can't be reached: no portal, transition, map edge, trigger or script leads here from the start map {start}"), target: Target::Map(m.clone()) });
            }
        }
    }

    fn script_problems(&self, out: &mut Vec<Problem>) {
        let mut seen: BTreeMap<String, String> = BTreeMap::new();
        for f in self.project.script_files() {
            let Ok(src) = std::fs::read_to_string(&f) else { continue };
            let file = f.file_name().unwrap_or_default().to_string_lossy().to_string();
            for func in rustpg2d_engine::script_refs::functions(&src) {
                if let Some(other) = seen.insert(func.name.clone(), file.clone()) {
                    let at = src.find(&format!("fn {}", func.name)).map_or(0, |b| src[..b].chars().count());
                    out.push(Problem { severity: Severity::Warning, area: Area::Scripts, place: file.clone(), text: format!("function {}() is also defined in {other} (one of them is ignored)", func.name), target: Target::Script(f.clone(), at) });
                }
            }
            for issue in rustpg2d_engine::script_refs::check_script(&src, &self.project.db, &self.refs.maps, &self.refs.spawns) {
                let at = src[..issue.span.0.min(src.len())].chars().count();
                out.push(Problem { severity: Severity::Error, area: Area::Scripts, place: format!("{file}:{}", issue.line), text: issue.message, target: Target::Script(f.clone(), at) });
            }
        }
        let mut host = rustpg2d_engine::script::ScriptHost::new();
        host.load_dir(&self.project.scripts_dir());
        for e in &host.errors {
            let (file, msg) = e.split_once(": ").unwrap_or(("scripts", e.as_str()));
            let path = self.project.scripts_dir().join(file);
            let line = message_line(msg).unwrap_or(1);
            let at = std::fs::read_to_string(&path).map_or(0, |src| line_offset(&src, line));
            out.push(Problem { severity: Severity::Error, area: Area::Scripts, place: format!("{file}:{line}"), text: msg.to_string(), target: Target::Script(path, at) });
        }
    }

    fn translation_problems(&self, out: &mut Vec<Problem>) {
        let dir = self.project.root.join("lang");
        let Ok(entries) = std::fs::read_dir(&dir) else { return };
        let mut codes: Vec<String> = entries.flatten().filter_map(|e| e.path().file_name()?.to_str()?.strip_suffix(".ron").map(str::to_string)).collect();
        codes.sort();
        for code in codes {
            let existing = rustpg2d_engine::lang::load_file(&self.project.root, &code);
            if existing.is_none() {
                out.push(Problem { severity: Severity::Error, area: Area::Translations, place: format!("lang/{code}.ron"), text: "can't be read".into(), target: Target::Project });
                continue;
            }
            let (file, unused) = rustpg2d_engine::lang::template(&self.project, &code, existing);
            let missing: Vec<&String> = file.texts.iter().filter(|(k, v)| v.is_empty() && !unused.contains(k)).map(|(k, _)| k).collect();
            if !missing.is_empty() {
                let sample: Vec<String> = missing.iter().take(3).map(|t| format!("\"{}\"", shorten(t, 40))).collect();
                let more = if missing.len() > 3 { format!(" and {} more", missing.len() - 3) } else { String::new() };
                out.push(Problem { severity: Severity::Warning, area: Area::Translations, place: format!("lang/{code}.ron"), text: format!("{} text(s) not translated yet: {}{more} (Database › Project › Languages: export the template)", missing.len(), sample.join(", ")), target: Target::Project });
            }
            if !unused.is_empty() {
                out.push(Problem { severity: Severity::Warning, area: Area::Translations, place: format!("lang/{code}.ron"), text: format!("{} translation(s) of texts the project no longer has (delete them from the file)", unused.len()), target: Target::Project });
            }
        }
    }

    fn project_problems(&self, out: &mut Vec<Problem>) {
        let (_, check) = rustpg2d_engine::release::check(&self.project);
        // (Missing art and sound files are listed per definition.)
        let skip = |m: &String| m.contains("' has no file (");
        for e in check.errors.iter().filter(|m| !skip(m)) {
            out.push(Problem { severity: Severity::Error, area: Area::Project, place: "project".into(), text: e.clone(), target: Target::Project });
        }
        for w in check.warnings.iter().filter(|m| !skip(m)) {
            out.push(Problem { severity: Severity::Warning, area: Area::Project, place: "project".into(), text: w.clone(), target: Target::Project });
        }
    }

    /// Rebuilds the list when something was saved (called every frame).
    pub fn problems_tick(&mut self) {
        if self.project_open && !self.library_mode && (self.problems.stale || !self.problems.built) {
            self.refresh_problems();
        }
    }

    /// The *Problems* window.
    pub fn problems_window(&mut self, ctx: &egui::Context) {
        if !self.problems.open {
            return;
        }
        let mut open = true;
        let mut jump: Option<Target> = None;
        let (e, w) = self.problems.counts();
        egui::Window::new(format!("{} Problems", ph::WARNING_CIRCLE)).id(egui::Id::new("problems")).open(&mut open).default_width(620.0).default_height(420.0).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.toggle_value(&mut self.problems.show_errors, egui::RichText::new(format!("{} {e} error(s)", ph::X_CIRCLE)).color(egui::Color32::from_rgb(235, 110, 100))).on_hover_text("Show errors: something is missing or can't work");
                ui.toggle_value(&mut self.problems.show_warnings, egui::RichText::new(format!("{} {w} warning(s)", ph::WARNING)).color(egui::Color32::from_rgb(230, 190, 90))).on_hover_text("Show warnings: probably not what you meant");
                ui.separator();
                let label = self.problems.area.map_or("Everything", Area::label);
                egui::ComboBox::from_id_salt("problems_area").selected_text(label).show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.problems.area, None, "Everything");
                    for a in Area::ALL {
                        let n = self.problems.list.iter().filter(|p| p.area == a).count();
                        ui.selectable_value(&mut self.problems.area, Some(a), format!("{} ({n})", a.label()));
                    }
                });
                ui.separator();
                if ui.button(ui::t(ph::ARROW_CLOCKWISE, "Refresh")).on_hover_text("Check the project again (it is checked whenever something is saved)").clicked() {
                    self.problems.stale = true;
                }
            });
            ui.separator();
            let shown: Vec<Problem> = self.problems.shown().into_iter().cloned().collect();
            if shown.is_empty() {
                ui.label(if self.problems.list.is_empty() { "No problems found." } else { "Nothing with these filters." });
            }
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                for p in &shown {
                    ui.horizontal(|ui| {
                        let (icon, color) = match p.severity {
                            Severity::Error => (ph::X_CIRCLE, egui::Color32::from_rgb(235, 110, 100)),
                            Severity::Warning => (ph::WARNING, egui::Color32::from_rgb(230, 190, 90)),
                        };
                        ui.colored_label(color, icon);
                        if ui.selectable_label(false, egui::RichText::new(&p.place).strong()).on_hover_text("Go there").clicked() {
                            jump = Some(p.target.clone());
                        }
                        ui.add(egui::Label::new(&p.text).wrap());
                    });
                }
            });
        });
        if !open {
            self.problems.open = false;
        }
        if let Some(t) = jump {
            self.jump_to(&t);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiler_lines_are_found() {
        assert_eq!(message_line("Unknown operator: '+' (line 12, position 4)"), Some(12));
        assert_eq!(message_line("no line here"), None);
        let src = "a\nbc\nd";
        assert_eq!(line_offset(src, 1), 0);
        assert_eq!(line_offset(src, 3), 5);
    }

    #[test]
    fn areas_and_severities_parse() {
        assert_eq!(Area::parse("Maps"), Some(Area::Maps));
        assert_eq!(Area::parse("map"), Some(Area::Maps));
        assert_eq!(Area::parse("script"), Some(Area::Scripts));
        assert_eq!(Severity::parse("warnings"), Some(Severity::Warning));
        assert_eq!(Severity::parse("x"), None);
    }

    #[test]
    fn an_entitys_own_fields_are_checked() {
        let mut e: rustpg2d_engine::entity::EntityInstance = serde_json::from_value(serde_json::json!({ "uid": 1, "kind": "Character", "def": "guard", "x": 16.0, "y": 32.0, "facing": "Down" })).unwrap();
        e.loot = Some(vec![rustpg2d_engine::defs::LootEntry { item: "ghost_coin".into(), chance: 100.0, min: 1, max: 1 }]);
        e.schedule.push(ron::from_str("(hour: 9.0, x: 1, y: 1, map: \"nowhere\")").unwrap());
        let refs = entity_refs(&e);
        assert!(refs.contains(&(RefKind::Item, "ghost_coin".into())), "{refs:?}");
        assert!(refs.contains(&(RefKind::Map, "nowhere".into())), "{refs:?}");
        // A character without its own look names no body of its own.
        assert!(!refs.iter().any(|(k, _)| *k == RefKind::Sheet), "{refs:?}");
    }
}
