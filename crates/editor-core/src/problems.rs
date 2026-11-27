//! *Problems*: one list of everything that is broken in the project, collected from the checks
//! that exist elsewhere (the definitions' references, map objects and zones, the script check
//! and compiler, translation templates, the export's project check, objects a map file had to
//! leave out). Each entry is an error (something is missing or can't work) or a warning
//! (probably unintended) and leads to its place like a search hit. The list is rebuilt when
//! something is saved (and on *Refresh*); maps that didn't change aren't read again.

use rustpg2d_engine::dialog_codes as dc;
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

use rustpg2d_engine::defs::{CharacterDef, PropDef};
use rustpg2d_engine::entity::EntityKind;
use rustpg2d_engine::events::Trigger;
use rustpg2d_engine::map::{Map, ZoneKind};
use rustpg2d_engine::refactor::RefKind;

use crate::workspace::Workspace;
use crate::search::Target;

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

    pub fn label(self) -> &'static str {
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
    /// When the maps were last read for Database › Factions (`ui::now()`).
    factions_read: f64,
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
pub(crate) fn message_line(msg: &str) -> Option<usize> {
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

impl Workspace {
    /// Does `id` of `kind` exist (maps included; an empty id counts as "none chosen")?
    fn problem_id_exists(&self, kind: RefKind, id: &str) -> bool {
        id.is_empty()
            || match kind {
                RefKind::Map => self.maps.iter().any(|m| m == id),
                // A proficiency: an SRD skill, tool, ability or saving throw (or ability:tool).
                RefKind::Skill => rustpg2d_engine::srd::abilities::is_known_proficiency(id) || rustpg2d_engine::srd::Spec::parse(id, false).is_some(),
                // (The *Has class* condition also takes a subclass.)
                RefKind::Class => self.project.db.class(id).is_some() || self.project.db.classes.iter().any(|c| c.subclass(id).is_some()),
                // (A call node calls a visual script or a text script's function.)
                RefKind::VisualScript => self.project.db.has(kind, id) || self.refs.functions.iter().any(|f| f == id),
                _ => self.project.db.has(kind, id),
            }
    }

    /// Placed characters with a faction of their own, on every map (the open one as edited):
    /// (map, uid, label, faction). Reads changed map files at most every two seconds.
    pub fn placed_factions(&mut self) -> Vec<(String, u32, String, String)> {
        let now = crate::now();
        if now - self.problems.factions_read > 2.0 || self.problems.maps.is_empty() {
            self.problems.factions_read = now;
            self.problem_maps();
        }
        let open = self.doc.as_ref().map(|d| &d.map);
        let mut maps: Vec<&Map> = self.problems.maps.values().filter_map(|f| f.map.as_ref()).filter(|m| open.is_none_or(|o| o.name != m.name)).collect();
        maps.extend(open);
        let mut out: Vec<(String, u32, String, String)> = maps
            .into_iter()
            .flat_map(|m| m.entities.iter().filter(|e| e.kind == EntityKind::Character && !e.faction.is_empty()).map(|e| (m.name.clone(), e.uid, e.label(), e.faction.clone())))
            .collect();
        out.sort();
        out
    }

    /// Reads the maps whose files changed since the last check.
    fn problem_maps(&mut self) {
        let names: BTreeSet<String> = self.maps.iter().cloned().collect();
        self.problems.maps.retain(|n, _| names.contains(n));
        for name in names {
            let path = self.project.map_path(&name);
            let stamp = crate::refs::file_stamp(&path);
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
        self.visual_script_problems(&mut out);
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
            // (Play cutscene may name a cut-scene asset instead.)
            if !self.refs.functions.contains(&f) && self.project.db.cutscene(&f).is_none() {
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
                    // A quest's action lists (steps, rewards), checked like triggers.
                    RefKind::Quest => db.quest(&id).map(|q| {
                        let lists = q.steps.iter().flat_map(|s| [&s.on_start, &s.on_done]).chain([&q.rewards.actions]);
                        lists.map(|a| Trigger { actions: a.clone(), ..Trigger::new(rustpg2d_engine::events::TriggerOn::Interact) }).collect()
                    }),
                    _ => None,
                };
                self.trigger_problems(&triggers.unwrap_or_default(), Area::Definitions, &place, &target, out);
                let file = match k {
                    RefKind::Material => db.material(&id).map(|d| d.texture.clone()),
                    RefKind::Sheet => db.sheet(&id).map(|d| d.image.clone()),
                    RefKind::Sound => db.sound(&id).map(|d| d.file.clone()),
                    RefKind::Font => db.font(&id).map(|d| d.file.clone()),
                    _ => None,
                };
                if let Some(l) = db.font(&id).filter(|_| k == RefKind::Font).map(|d| &d.license).filter(|l| !l.is_empty() && !self.project.root.join(l).is_file()) {
                    out.push(Problem { severity: Severity::Warning, area: Area::Definitions, place: place.clone(), text: format!("its licence file {l} is missing (exports carry it with the font)"), target: target.clone() });
                }
                if let Some(skin) = db.skin(&id).filter(|_| k == RefKind::Skin) {
                    for (part, p) in skin.pictures() {
                        let text = match db.sheet(&p.sheet) {
                            _ if p.sheet.is_empty() => Some(format!("its {part} picture has no sprite sheet")),
                            // (A missing sheet is reported with the other missing references.)
                            None => None,
                            Some(sh) if p.corner * 2 > sh.frame_w.min(sh.frame_h) => Some(format!("its {part} picture's corner ({} px) is more than half its frame ({}x{} px)", p.corner, sh.frame_w, sh.frame_h)),
                            Some(_) => None,
                        };
                        if let Some(text) = text {
                            out.push(Problem { severity: Severity::Warning, area: Area::Definitions, place: place.clone(), text, target: target.clone() });
                        }
                    }
                }
                if let Some(f) = file {
                    if f.is_empty() || !self.project.root.join(&f).is_file() {
                        out.push(Problem { severity: Severity::Error, area: Area::Definitions, place: place.clone(), text: if f.is_empty() { "has no file".into() } else { format!("its file {f} is missing") }, target: target.clone() });
                    }
                }
                if let Some(a) = db.character(&id).filter(|_| k == RefKind::Character).map(|c| &c.appearance) {
                    // Characters are 3D models; without one (or with an old figure's name) the mannequin stands in.
                    if !a.model.is_empty() && db.model(&a.model).is_none() {
                        out.push(Problem { severity: Severity::Error, area: Area::Definitions, place: place.clone(), text: format!("its 3D model {} doesn't exist", a.model), target: target.clone() });
                    } else if a.model.is_empty() && db.model(&a.body).is_none() {
                        out.push(Problem { severity: Severity::Warning, area: Area::Definitions, place: place.clone(), text: "has no 3D model (it shows as the mannequin)".into(), target: target.clone() });
                    }
                }
                if let Some(m) = db.model(&id).filter(|_| k == RefKind::Model) {
                    for f in std::iter::once(&m.file).chain(m.clip_paths()) {
                        if f.is_empty() || !self.project.root.join(f).is_file() {
                            out.push(Problem { severity: Severity::Error, area: Area::Definitions, place: place.clone(), text: if f.is_empty() { "has no file".into() } else { format!("its file {f} is missing") }, target: target.clone() });
                        }
                    }
                    // Mapped clips its files don't have (a clip file removed, a clip renamed).
                    if let Some(info) = self.assets.models.info(&id).filter(|_| m.rig.is_none()) {
                        for c in m.clips.iter().filter(|c| !info.clips.iter().any(|(n, _)| *n == c.clip)) {
                            out.push(Problem { severity: Severity::Warning, area: Area::Definitions, place: place.clone(), text: format!("its animation {} plays the clip {}, which none of its files has", c.anim, c.clip), target: target.clone() });
                        }
                    }
                }
                if let Some(m) = db.model(&id).filter(|_| k == RefKind::Model) {
                    // Too large for one draw: parts of it won't show.
                    if let Some(w) = self.assets.models.warnings.iter().find(|w| w.starts_with(&format!("model {id}:"))) {
                        out.push(Problem { severity: Severity::Error, area: Area::Definitions, place: place.clone(), text: w.split_once(": ").map_or(w.as_str(), |x| x.1).to_string(), target: target.clone() });
                    }
                    // Pieces: what they fit and what a figure wears must exist and be the right kind.
                    for b in m.part.iter().flat_map(|p| &p.fits).filter(|b| db.model(b).is_none_or(|x| x.part.is_some())) {
                        out.push(Problem { severity: Severity::Warning, area: Area::Definitions, place: place.clone(), text: format!("fits {b}, which is no figure of this project"), target: target.clone() });
                    }
                    if m.part.as_ref().is_some_and(|p| p.slot.is_empty()) {
                        out.push(Problem { severity: Severity::Error, area: Area::Definitions, place: place.clone(), text: "is a piece without a slot (it never shows)".into(), target: target.clone() });
                    }
                    for p in m.parts.iter().filter(|p| db.model(p).is_none_or(|x| x.part.is_none())) {
                        out.push(Problem { severity: Severity::Error, area: Area::Definitions, place: place.clone(), text: format!("wears {p}, which is no piece of this project"), target: target.clone() });
                    }
                }
                if let Some(item) = db.item(&id).filter(|_| k == RefKind::Item) {
                    // A style no piece has: nothing would show.
                    if !item.wear.is_empty() && !db.models.iter().any(|m| m.part.as_ref().is_some_and(|p| p.style == item.wear)) {
                        out.push(Problem { severity: Severity::Warning, area: Area::Definitions, place: place.clone(), text: format!("is shown as {}, a style no piece of this project has", item.wear), target: target.clone() });
                    }
                    // Animation names no model maps: characters play their plain animation.
                    let a = &item.anims;
                    for name in [&a.attack, &a.idle, &a.walk, &a.block].into_iter().filter(|n| !n.is_empty()) {
                        if !db.models.iter().any(|m| m.clip_of(name).is_some()) {
                            out.push(Problem { severity: Severity::Warning, area: Area::Definitions, place: place.clone(), text: format!("names the animation {name}, which no 3D model has (characters play their plain one)"), target: target.clone() });
                        }
                    }
                }
                let faction = match k {
                    RefKind::Character => db.character(&id).map(|d| d.faction.clone()),
                    RefKind::Race => db.race(&id).map(|d| d.faction.clone()),
                    _ => None,
                };
                // (A project without factions leaves the fields dormant: library characters name theirs.)
                let factions = &self.project.config.factions;
                if let Some(f) = faction.filter(|f| !f.is_empty() && !factions.is_empty() && factions.get(f).is_none()) {
                    out.push(Problem { severity: Severity::Warning, area: Area::Definitions, place: place.clone(), text: format!("belongs to the faction \"{f}\", which isn't in Database › Factions (it has no faction until it is added there)"), target: target.clone() });
                }
                if k == RefKind::Quest && db.quest(&id).is_some_and(|q| q.stages.is_empty()) {
                    out.push(Problem { severity: Severity::Warning, area: Area::Definitions, place: place.clone(), text: "has no stages: the journal shows nothing while it is active".into(), target: target.clone() });
                }
                if let Some(q) = db.quest(&id).filter(|_| k == RefKind::Quest) {
                    use rustpg2d_engine::quest::StepGoal;
                    let warn = |text: String, out: &mut Vec<Problem>| out.push(Problem { severity: Severity::Warning, area: Area::Definitions, place: place.clone(), text, target: target.clone() });
                    // Givers and talk characters: a placed character's name or a definition.
                    let known = |who: &str| who.trim().is_empty() || self.refs.people.iter().any(|p| p == who);
                    let talks = q.steps.iter().filter_map(|s| match &s.done {
                        StepGoal::Talk { character, .. } => Some(character.as_str()),
                        _ => None,
                    });
                    for who in std::iter::once(q.giver.as_str()).chain(talks).filter(|w| !known(w)) {
                        out.push(Problem { severity: Severity::Error, area: Area::Definitions, place: place.clone(), text: format!("names the character \"{who}\", which is neither a placed character's name nor a character definition"), target: target.clone() });
                    }
                    // Quest objects (placed things) and where Talk steps lead first (also zones): on
                    // their map under that name.
                    let objects = q.objects.iter().map(|o| (o, false));
                    let first = q.steps.iter().map(|s| &s.first_to).filter(|t| !t.is_empty()).map(|o| (o, true));
                    for (o, zones) in objects.chain(first) {
                        let placed = self.problems.maps.get(o.map.trim()).and_then(|f| f.map.as_ref()).map(|m| m.entities.iter().any(|e| e.name == o.name.trim()) || (zones && m.zones.iter().any(|z| z.name == o.name.trim())));
                        match placed {
                            None => out.push(Problem { severity: Severity::Error, area: Area::Definitions, place: place.clone(), text: format!("names \"{}\" on map \"{}\", which doesn't exist", o.name, o.map), target: target.clone() }),
                            Some(false) => out.push(Problem { severity: Severity::Error, area: Area::Definitions, place: place.clone(), text: format!("names \"{}\", which isn't placed on map {} (name it in its inspector)", o.name, o.map), target: target.clone() }),
                            Some(true) => {}
                        }
                    }
                    if q.has_giver() && q.offer.lines.iter().chain([&q.offer.question]).all(|l| l.trim().is_empty()) {
                        warn("its giver says nothing when offering it: the quest starts silently when they are talked to".into(), out);
                    }
                    for (n, s) in q.steps.iter().enumerate().take(q.stages.len()) {
                        match &s.done {
                            StepGoal::Conditions(c) if c.is_empty() => warn(format!("step {n} is done when its conditions hold, but has none: it is done at once"), out),
                            StepGoal::Reach if q.target_of(n as u32).is_none() => warn(format!("step {n} is done on reaching its marker target, but has none (map and name)"), out),
                            StepGoal::Talk { character, .. } if character.trim().is_empty() && !q.has_giver() => warn(format!("step {n} waits for a talk with the giver, but the quest has no giver"), out),
                            _ => {}
                        }
                    }
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
            let mut refs: Vec<(RefKind, String)> = m.materials.iter().map(|t| (RefKind::Material, t.clone())).chain(m.decor_defs.iter().map(|p| (RefKind::Prop, p.clone()))).collect();
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
                if !e.faction.is_empty() && self.project.config.factions.get(&e.faction).is_none() {
                    out.push(Problem { severity: Severity::Warning, area: Area::Maps, place: format!("map {name}: {who}"), text: format!("belongs to the faction \"{}\", which isn't in Database › Factions (it has its definition's until it is added there)", e.faction), target: target.clone() });
                }
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
                    ZoneKind::Encounter { monsters, .. } => {
                        if monsters.is_empty() {
                            out.push(Problem { severity: Severity::Warning, area: Area::Maps, place: place.clone(), text: "the encounter lists no monsters".into(), target: target.clone() });
                        }
                        for m in monsters.iter().filter(|m| !db.has(RefKind::Character, &m.character)) {
                            out.push(Problem { severity: Severity::Error, area: Area::Maps, place: place.clone(), text: format!("the encounter names character \"{}\", which doesn't exist", m.character), target: target.clone() });
                        }
                    }
                    ZoneKind::FishingSpot { rod } => {
                        if !self.problem_id_exists(RefKind::Item, rod) {
                            out.push(Problem { severity: Severity::Error, area: Area::Maps, place: place.clone(), text: format!("needs the rod \"{rod}\", which isn't an item"), target: target.clone() });
                        }
                    }
                    ZoneKind::Ambience { sound, .. } => {
                        if !db.has(RefKind::Sound, sound) {
                            out.push(Problem { severity: Severity::Error, area: Area::Maps, place: place.clone(), text: format!("plays the ambience sound \"{sound}\", which doesn't exist"), target: target.clone() });
                        }
                    }
                    ZoneKind::Drop { target_map, target_spawn, .. } => {
                        let on = if target_map.is_empty() { name.as_str() } else { target_map.as_str() };
                        if !target_map.is_empty() {
                            to.insert(target_map.clone());
                        }
                        if !target_map.is_empty() && !self.problem_id_exists(RefKind::Map, target_map) {
                            out.push(Problem { severity: Severity::Error, area: Area::Maps, place: place.clone(), text: format!("the drop lands on map \"{target_map}\", which doesn't exist"), target: target.clone() });
                        } else if !target_spawn.is_empty() && spawns.get(on).is_some_and(|s| !s.contains(&target_spawn.as_str())) {
                            out.push(Problem { severity: Severity::Error, area: Area::Maps, place: place.clone(), text: format!("the drop lands on spawn \"{target_spawn}\", which map {on} doesn't have"), target: target.clone() });
                        }
                    }
                    ZoneKind::Trap { trap } => {
                        if trap.detect_dc <= 0 {
                            out.push(Problem { severity: Severity::Warning, area: Area::Maps, place: place.clone(), text: "the trap can't be found (find DC 0): players never see it coming".into(), target: target.clone() });
                        }
                        if !trap.damage.is_empty() && rustpg2d_engine::srd::Expr::parse(&trap.damage).is_err() {
                            out.push(Problem { severity: Severity::Error, area: Area::Maps, place: place.clone(), text: format!("the trap's damage \"{}\" isn't dice", trap.damage), target: target.clone() });
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

    /// What the visual scripts' nodes get wrong (their Rhai is a stub until it's fixed).
    fn visual_script_problems(&self, out: &mut Vec<Problem>) {
        let db = &self.project.db;
        let cat = rustpg2d_engine::visual::Catalog { visual: &db.visual_scripts, text_fns: self.project.text_script_functions() };
        for vs in &db.visual_scripts {
            for p in rustpg2d_engine::visual::function_code(vs, &cat).1 {
                let node = p.node.and_then(|n| vs.node(n)).map_or(String::new(), |n| format!(", node {}", cat.title(n)));
                out.push(Problem { severity: Severity::Error, area: Area::Scripts, place: format!("visual script {}{node}", vs.id), text: format!("{} (the function does nothing until it's fixed)", p.text), target: Target::Def(RefKind::VisualScript, vs.id.clone()) });
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
            // Function names passed as strings (then_call("f"), a window button's run: "f").
            for a in rustpg2d_engine::script_refs::function_refs(&src) {
                if !self.refs.functions.contains(&a.value) {
                    let at = src[..a.span.0.min(src.len())].chars().count();
                    out.push(Problem { severity: Severity::Error, area: Area::Scripts, place: format!("{file}:{}", a.line), text: format!("{}(): there is no script function {}()", a.func, a.value), target: Target::Script(f.clone(), at) });
                }
            }
            for issue in rustpg2d_engine::script_refs::check_script(&src, &self.project.db, &self.refs.maps, &self.refs.spawns) {
                let at = src[..issue.span.0.min(src.len())].chars().count();
                let severity = if issue.warning { Severity::Warning } else { Severity::Error };
                out.push(Problem { severity, area: Area::Scripts, place: format!("{file}:{}", issue.line), text: issue.message, target: Target::Script(f.clone(), at) });
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

    /// The characters of `texts` that neither the project's fonts (every role's, the fallbacks)
    /// nor the built-in font have.
    fn uncovered<'t>(fonts: &[rustpg2d_engine::fonts::Coverage], texts: impl Iterator<Item = &'t String>) -> std::collections::BTreeSet<char> {
        use rustpg2d_engine::fonts::{builtin_covers, invisible};
        texts.flat_map(|t| t.chars()).filter(|&c| !builtin_covers(c) && !invisible(c) && !fonts.iter().any(|f| f.has(c))).collect()
    }

    /// A problem for characters of language `code` no font can draw (players see empty boxes).
    fn glyph_problem(code: &str, place: String, missing: std::collections::BTreeSet<char>, out: &mut Vec<Problem>) {
        if missing.is_empty() {
            return;
        }
        // (With their code points: the editor's own font lacks most of them too.)
        let sample = missing.iter().take(6).map(|c| format!("{c} U+{:04X}", *c as u32)).collect::<Vec<_>>().join(", ");
        let more = if missing.len() > 6 { format!(" and {} more", missing.len() - 6) } else { String::new() };
        out.push(Problem {
            severity: Severity::Warning,
            area: Area::Translations,
            place,
            text: format!("{} character(s) of the {code} texts have no glyph in the project's fonts: {sample}{more} (players see empty boxes; Database › Project › Fonts: add a font or fallback that has them)", missing.len()),
            target: Target::Project,
        });
    }

    /// Message codes in the project's texts that aren't valid, and icons of items that don't exist.
    fn code_problems(&self, own: &std::collections::BTreeSet<String>, out: &mut Vec<Problem>) {
        self.menu_problems(out);
        // The game's skin may add colour names ({c:ember}).
        let skin = self.project.db.skin(&self.project.config.skin);
        dc::set_skin_colors(skin.map(|s| s.colors.iter().map(|(n, c)| (n.clone(), c.rgb3())).collect()).unwrap_or_default());
        if !self.project.config.skin.is_empty() && skin.is_none() {
            out.push(Problem { severity: Severity::Error, area: Area::Project, place: "Project › Look".into(), text: format!("the UI skin \"{}\" doesn't exist (the game uses the built-in look)", self.project.config.skin), target: Target::Project });
        }
        let bad: std::collections::BTreeSet<&str> = own.iter().flat_map(|t| dc::unknown_codes(t)).collect();
        if !bad.is_empty() {
            let sample: Vec<&str> = bad.iter().take(6).copied().collect();
            out.push(Problem { severity: Severity::Warning, area: Area::Project, place: "dialog texts".into(), text: format!("{} unknown message code(s), shown as written: {} (codes: {{c:colour}} … {{/c}}, {{i:item}}, {{w:seconds}}, {{s:speed}}, {{name:…}}, {{at:top|middle|bottom}})", bad.len(), sample.join(" ")), target: Target::Project });
        }
        let icons: std::collections::BTreeSet<String> = own.iter().flat_map(|t| dc::icons(t)).filter(|i| self.project.db.item(i).is_none()).collect();
        if !icons.is_empty() {
            out.push(Problem { severity: Severity::Warning, area: Area::Project, place: "dialog texts".into(), text: format!("{{i:…}} names item(s) that don't exist: {}", icons.into_iter().collect::<Vec<_>>().join(", ")), target: Target::Project });
        }
    }

    /// The title screen's map, pictures and music, and both menus' entries.
    fn menu_problems(&self, out: &mut Vec<Problem>) {
        use rustpg2d_engine::menus::{MenuEntry, TitleBackground};
        let c = &self.project.config;
        let mut bad = |text: String, severity: Severity| out.push(Problem { severity, area: Area::Project, place: "Project › Title screen".into(), text, target: Target::Project });
        match &c.title.background {
            TitleBackground::Map { map, .. } if !map.is_empty() && !self.refs.maps.contains(map) => bad(format!("the title screen's map \"{map}\" doesn't exist"), Severity::Error),
            TitleBackground::Picture { sheet } if self.project.db.sheet(sheet).is_none() => bad(format!("the title screen's picture \"{sheet}\" isn't a sprite sheet"), Severity::Error),
            _ => {}
        }
        if let Some(l) = c.title.logo.as_ref().filter(|l| self.project.db.sheet(&l.sheet).is_none()) {
            bad(format!("the logo \"{}\" isn't a sprite sheet", l.sheet), Severity::Error);
        }
        if !c.title.music.is_empty() && self.project.db.sound(&c.title.music).is_none() {
            bad(format!("the title music \"{}\" isn't a sound", c.title.music), Severity::Error);
        }
        for (title, entries) in [(true, &c.title.entries), (false, &c.menu.entries)] {
            let menu = if title { "the title screen" } else { "the pause menu" };
            for e in entries {
                match e {
                    MenuEntry::Custom { function, label, .. } if !self.refs.functions.contains(function) => {
                        bad(format!("{menu}'s entry \"{label}\" runs the script function \"{function}\", which doesn't exist"), Severity::Error)
                    }
                    e if !e.fits(title) => bad(format!("{menu} can't show \"{}\" (left out)", e.label()), Severity::Warning),
                    MenuEntry::Credits if c.credits.trim().is_empty() => bad(format!("{menu} has a Credits entry, but the credits text is empty"), Severity::Warning),
                    _ => {}
                }
            }
        }
        if !c.title.entries.is_empty() && !c.title.entries.iter().any(|e| matches!(e, MenuEntry::NewGame)) {
            bad("the title screen has no New game entry".into(), Severity::Warning);
        }
        // Script hooks: a hook the engine calls, a function that exists.
        for h in &c.hooks {
            let place = "Project › Script hooks".to_string();
            if !rustpg2d_engine::hooks::known(&h.hook) {
                out.push(Problem { severity: Severity::Warning, area: Area::Project, place: place.clone(), text: format!("\"{}\" isn't a hook the engine calls (its function never runs)", h.hook), target: Target::Project });
            }
            if h.function.is_empty() {
                out.push(Problem { severity: Severity::Warning, area: Area::Project, place, text: format!("the {} hook has no function", h.hook), target: Target::Project });
            } else if !self.refs.functions.contains(&h.function) {
                out.push(Problem { severity: Severity::Error, area: Area::Project, place, text: format!("the {} hook runs the script function \"{}\", which doesn't exist", h.hook, h.function), target: Target::Project });
            }
        }
    }

    fn translation_problems(&self, out: &mut Vec<Problem>) {
        let fonts: Vec<rustpg2d_engine::fonts::Coverage> = self
            .project
            .config
            .fonts
            .ids()
            .filter_map(|id| self.project.db.font(id))
            .filter_map(|d| std::fs::read(self.project.root.join(&d.file)).ok().and_then(|b| rustpg2d_engine::fonts::Coverage::of(&b)))
            .collect();
        let own = rustpg2d_engine::lang::content_texts(&self.project);
        Self::glyph_problem(&self.project.config.language, "project texts".into(), Self::uncovered(&fonts, own.iter()), out);
        self.code_problems(&own, out);
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
            // (With the engine's built-in runtime texts of the language: menus, HUD, windows.)
            let builtin = rustpg2d_engine::lang::builtin_ui(&code);
            Self::glyph_problem(&code, format!("lang/{code}.ron"), Self::uncovered(&fonts, file.texts.values().chain(file.ui.values()).chain(builtin.values()).chain(std::iter::once(&file.name))), out);
            let dropped: Vec<&String> = file.texts.iter().filter(|(k, v)| !v.is_empty() && dc::written_codes(k) != dc::written_codes(v)).map(|(k, _)| k).collect();
            if !dropped.is_empty() {
                let sample: Vec<String> = dropped.iter().take(3).map(|t| format!("\"{}\"", shorten(t, 40))).collect();
                let more = if dropped.len() > 3 { format!(" and {} more", dropped.len() - 3) } else { String::new() };
                out.push(Problem { severity: Severity::Warning, area: Area::Translations, place: format!("lang/{code}.ron"), text: format!("{} translation(s) don't keep the message codes of their text ({{c:…}}, {{i:…}}, {{w:…}} …): {}{more}", dropped.len(), sample.join(", ")), target: Target::Project });
            }
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
        // SRD content wants its attribution in the credits (the licence, CC BY 4.0).
        if !self.project.config.srd {
            let srd = rustpg2d_engine::srd::rules::srd_content(&self.project.db);
            if !srd.is_empty() {
                let more = if srd.len() > 3 { format!(" and {} more", srd.len() - 3) } else { String::new() };
                out.push(Problem {
                    severity: Severity::Warning,
                    area: Area::Project,
                    place: "Project › Uses SRD 5.1 content".into(),
                    text: format!("the project uses SRD 5.1 content ({}{more}): tick *Uses SRD 5.1 content* so the credits carry its attribution (CC BY 4.0)", srd.iter().take(3).cloned().collect::<Vec<_>>().join(", ")),
                    target: Target::Project,
                });
            }
        }
        let (_, check) = rustpg2d_engine::release::check(&self.project);
        // (Missing art and sound files are listed per definition.)
        let skip = |m: &String| m.contains("' has no file (");
        for e in check.errors.iter().filter(|m| !skip(m)) {
            out.push(Problem { severity: Severity::Error, area: Area::Project, place: "project".into(), text: e.clone(), target: Target::Project });
        }
        for w in check.warnings.iter().filter(|m| !skip(m)) {
            out.push(Problem { severity: Severity::Warning, area: Area::Project, place: "project".into(), text: w.clone(), target: Target::Project });
        }
        // The enabled plugins' hooks and menu entries run functions that must exist.
        for (id, d) in rustpg2d_engine::plugins::enabled(&self.project).0 {
            let place = format!("plugin {id}");
            let functions = d.hooks.iter().map(|h| h.function.as_str()).chain(d.title_entries.iter().chain(&d.pause_entries).filter_map(|e| match e {
                rustpg2d_engine::menus::MenuEntry::Custom { function, .. } => Some(function.as_str()),
                _ => None,
            }));
            for f in functions.filter(|f| !f.is_empty() && !self.refs.functions.iter().any(|x| x == f)) {
                out.push(Problem { severity: Severity::Error, area: Area::Project, place: place.clone(), text: format!("runs the script function \"{f}\", which neither the project nor its plugins have"), target: Target::Project });
            }
            for h in d.hooks.iter().filter(|h| !rustpg2d_engine::hooks::known(&h.hook)) {
                out.push(Problem { severity: Severity::Warning, area: Area::Project, place: place.clone(), text: format!("\"{}\" isn't a hook the engine calls", h.hook), target: Target::Project });
            }
        }
    }

    /// Rebuilds the list when something was saved (called every frame).
    pub fn problems_tick(&mut self) {
        if self.project_open && !self.library_mode && (self.problems.stale || !self.problems.built) {
            self.refresh_problems();
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
