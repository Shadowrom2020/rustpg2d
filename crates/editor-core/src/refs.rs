//! The lists pickers, completion and checks need: the project's ids, the names placed on its maps,
//! the flags and variables its triggers use. [`Workspace::refresh_refs`] gathers them.

use std::collections::HashMap;

use rustpg2d_engine::entity::EntityKind;
use rustpg2d_engine::script::ScriptHost;
use rustpg2d_engine::map::{Map, ZoneKind};

use crate::workspace::Workspace;

/// Values offered by pickers and checked by the script check.
#[derive(Default, Clone)]
pub struct Refs {
    pub items: Vec<String>,
    /// Prop definition ids.
    pub props: Vec<String>,
    pub maps: Vec<String>,
    /// "self", "player" and entity names on the current map.
    pub targets: Vec<String>,
    pub functions: Vec<String>,
    /// Cut-scene assets (*Play cutscene* takes their ids beside script functions).
    pub cutscenes: Vec<String>,
    /// Animation ids (`defs/animations.ron`).
    pub animations: Vec<String>,
    /// map -> spawn names
    pub spawns: HashMap<String, Vec<String>>,
    /// map -> its transition zones: (zone, target map, target zone)
    pub transitions: HashMap<String, Vec<(String, String, String)>>,
    /// Name of the open map.
    pub current_map: String,
    pub flags: Vec<String>,
    pub sounds: Vec<String>,
    pub characters: Vec<String>,
    /// All prop state names (for "Set prop state" / "Prop state").
    pub states: Vec<String>,
    /// Named entities on all maps (script targets).
    pub entity_names: Vec<String>,
    /// Dialog speakers for the open map: (value, explanation).
    pub speakers: Vec<(String, String)>,
    /// Flags and variables used by triggers anywhere in the project.
    pub all_flags: Vec<String>,
    pub vars: Vec<String>,
    /// Quest ids with their number of stages.
    pub quests: Vec<String>,
    /// Placed characters' names (every map), then character definitions: quest givers.
    pub people: Vec<String>,
    /// The placed characters of every map: (name, definition).
    pub placed_characters: Vec<(String, String)>,
    pub quest_stages: HashMap<String, Vec<String>>,
    pub skills: Vec<String>,
    pub spells: Vec<String>,
    /// Race ids (Database › Races).
    pub races: Vec<String>,
    /// Class and subclass ids (Database › Classes).
    pub classes: Vec<String>,
    /// Disease ids (Database › Diseases).
    pub diseases: Vec<String>,
    /// Common event ids (Database › Common events).
    pub common_events: Vec<String>,
    /// Faction ids (Database › Factions).
    pub factions: Vec<String>,
    /// Crafting stations named by recipes.
    pub stations: Vec<String>,
    /// Achievement ids.
    pub achievements: Vec<String>,
    /// Sprite sheet ids (effects).
    pub sheets: Vec<String>,
    /// Named animations of sprite sheets (Aseprite tags).
    pub sheet_tags: HashMap<String, Vec<rustpg2d_engine::defs::SheetTag>>,
    /// Emotion names (the tags of Emotions sprite sheets).
    pub emotions: Vec<String>,
}

/// What the dropdowns need from one map ([`Workspace::refresh_refs`]), with the file's stamp.
#[derive(Clone, Default)]
pub struct MapRefs {
    stamp: (u64, u64),
    names: Vec<String>,
    /// The placed characters: (name, definition) (quest givers and talk targets).
    people: Vec<(String, String)>,
    flags: Vec<String>,
    vars: Vec<String>,
    spawns: Vec<String>,
    transitions: Vec<(String, String, String)>,
}

impl MapRefs {
    fn of(map: &Map, stamp: (u64, u64)) -> Self {
        let mut r = MapRefs { stamp, ..Default::default() };
        r.names.extend(map.entities.iter().filter(|e| !e.name.is_empty()).map(|e| e.name.clone()));
        r.people.extend(map.entities.iter().filter(|e| !e.name.is_empty() && e.kind == rustpg2d_engine::entity::EntityKind::Character).map(|e| (e.name.clone(), e.def.clone())));
        collect_map_names(map, &mut r.flags, &mut r.vars);
        r.spawns = map.zones.iter().filter(|z| z.kind == ZoneKind::Spawn).map(|z| z.name.clone()).collect();
        r.transitions = map
            .zones
            .iter()
            .filter_map(|z| match &z.kind {
                ZoneKind::Transition { target_map, target_zone } => Some((z.name.clone(), target_map.clone(), target_zone.clone())),
                _ => None,
            })
            .collect();
        r
    }
}

/// (modification time, size) of a file; (0, 0) when it can't be read.
pub fn file_stamp(path: &std::path::Path) -> (u64, u64) {
    std::fs::metadata(path)
        .map(|m| (m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs()), m.len()))
        .unwrap_or((0, 0))
}

/// Flag names used anywhere on a map (for suggestions).
/// Flag and variable names used by triggers (conditions and actions, also nested in "If").
pub fn collect_names(ts: &[rustpg2d_engine::events::Trigger], flags: &mut Vec<String>, vars: &mut Vec<String>) {
    use rustpg2d_engine::events::{Action, Cond, Condition};
    fn conds(cs: &[Condition], flags: &mut Vec<String>, vars: &mut Vec<String>) {
        for c in cs {
            match &c.cond {
                Cond::Flag { name } => flags.push(name.clone()),
                Cond::Var { name, .. } => vars.push(name.clone()),
                _ => {}
            }
        }
    }
    fn actions(a: &[Action], flags: &mut Vec<String>, vars: &mut Vec<String>) {
        rustpg2d_engine::events::walk_actions(a, &mut |a| match a {
            Action::SetFlag { name, .. } => flags.push(name.clone()),
            Action::SetVar { name, .. } => vars.push(name.clone()),
            Action::Choice { var, .. } | Action::InputNumber { var, .. } => vars.push(var.clone()),
            Action::If { conditions, .. } => conds(conditions, flags, vars),
            _ => {}
        });
    }
    for t in ts {
        conds(&t.conditions, flags, vars);
        actions(&t.actions, flags, vars);
    }
}

fn collect_flags(map: &Map, out: &mut Vec<String>) {
    let mut vars = Vec::new();
    collect_map_names(map, out, &mut vars);
}

/// Flags and variables used by a map's triggers (map, entities, zones).
fn collect_map_names(map: &Map, flags: &mut Vec<String>, vars: &mut Vec<String>) {
    rustpg2d_engine::events::walk_map_triggers(&map.name, map, &mut |_, ts| collect_names(ts, flags, vars));
}

/// Dialog speakers to offer for a map: placeholders, the characters on it (definition
/// names and named instances) and speakers its dialogs already use.
pub fn speakers_of(map: &Map, db: &rustpg2d_engine::defs::Database) -> Vec<(String, String)> {
    use rustpg2d_engine::events::Action;
    let mut out: Vec<(String, String)> = vec![("{self}".into(), "name of the trigger's owner".into()), ("{player}".into(), "the player's name".into())];
    let mut add = |v: String, hint: String| {
        if !v.trim().is_empty() && !out.iter().any(|(o, _)| *o == v) {
            out.push((v, hint));
        }
    };
    for e in map.entities.iter().filter(|e| e.kind == EntityKind::Character) {
        if !e.name.is_empty() {
            let mut c = e.name.chars();
            let nice: String = c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default();
            add(nice.replace('_', " "), format!("'{}' on this map", e.name));
        }
        if let Some(d) = db.character(&e.def) {
            add(d.name.clone(), "character on this map".into());
        }
    }
    fn used(acts: &[Action], out: &mut Vec<String>) {
        rustpg2d_engine::events::walk_actions(acts, &mut |a| match a {
            Action::Dialog { speaker, .. } | Action::Choice { speaker, .. } => out.push(speaker.clone()),
            _ => {}
        });
    }
    let mut names = Vec::new();
    rustpg2d_engine::events::walk_map_triggers(&map.name, map, &mut |_, ts| ts.iter().for_each(|t| used(&t.actions, &mut names)));
    names.sort();
    for n in names {
        add(n, "used in a dialog here".into());
    }
    out
}

impl Workspace {
    /// Rebuilds dropdown suggestions (items, maps, spawns, script functions, ...).
    pub fn refresh_refs(&mut self) {
        let mut spawns: HashMap<String, Vec<String>> = HashMap::new();
        let mut transitions: HashMap<String, Vec<(String, String, String)>> = HashMap::new();
        // Names used anywhere in the project (for the script editor's completion).
        let mut all_names: Vec<String> = Vec::new();
        let mut all_flags: Vec<String> = Vec::new();
        let mut all_vars: Vec<String> = Vec::new();
        let mut people: Vec<(String, String)> = Vec::new();
        for m in &self.maps {
            let refs = match &self.doc {
                Some(d) if &d.map.name == m => MapRefs::of(&d.map, (0, 0)),
                _ => {
                    let stamp = file_stamp(&self.project.map_path(m));
                    match self.map_refs.get(m) {
                        Some(r) if r.stamp == stamp && stamp != (0, 0) => r.clone(),
                        _ => match self.project.load_map(m) {
                            Ok(x) => {
                                let r = MapRefs::of(&x, stamp);
                                self.map_refs.insert(m.clone(), r.clone());
                                r
                            }
                            Err(_) => continue,
                        },
                    }
                }
            };
            all_names.extend(refs.names);
            people.extend(refs.people);
            all_flags.extend(refs.flags);
            all_vars.extend(refs.vars);
            spawns.insert(m.clone(), refs.spawns);
            transitions.insert(m.clone(), refs.transitions);
        }
        rustpg2d_engine::events::walk_db_triggers(&self.project.db, &mut |_, ts| collect_names(ts, &mut all_flags, &mut all_vars));
        for v in [&mut all_names, &mut all_flags, &mut all_vars] {
            v.retain(|s| !s.is_empty());
            v.sort();
            v.dedup();
        }
        let mut scripts = ScriptHost::new();
        // (The enabled plugins' functions count too: hooks and menu entries may name them.)
        scripts.set_extra_dirs(rustpg2d_engine::plugins::enabled(&self.project).0.iter().map(|(id, _)| rustpg2d_engine::plugins::plugin_dir(&self.project.root, id).join("scripts")).collect());
        scripts.load_dir(&self.project.scripts_dir());
        let mut targets = vec!["self".to_string(), "player".to_string()];
        let mut flags = Vec::new();
        if let Some(doc) = &self.doc {
            targets.extend(doc.map.entities.iter().filter(|e| !e.name.is_empty()).map(|e| e.name.clone()));
            collect_flags(&doc.map, &mut flags);
        }
        flags.sort();
        flags.dedup();
        self.refs = Refs {
            items: self.project.db.items.iter().map(|i| i.id.clone()).collect(),
            props: self.project.db.props.iter().map(|p| p.id.clone()).collect(),
            maps: self.maps.clone(),
            targets,
            functions: scripts.function_names(),
            spawns,
            transitions,
            current_map: self.doc.as_ref().map(|d| d.map.name.clone()).unwrap_or_default(),
            flags,
            sounds: self.project.db.sounds.iter().map(|s| s.id.clone()).collect(),
            characters: self.project.db.characters.iter().map(|c| c.id.clone()).collect(),
            states: {
                let mut v: Vec<String> = self.project.db.props.iter().flat_map(|p| p.state_names()).collect();
                v.sort();
                v.dedup();
                v
            },
            entity_names: all_names,
            speakers: self.doc.as_ref().map(|d| speakers_of(&d.map, &self.project.db)).unwrap_or_default(),
            all_flags,
            vars: all_vars,
            quests: self.project.db.quests.iter().map(|q| q.id.clone()).collect(),
            people: {
                let mut names: Vec<String> = people.iter().map(|(n, _)| n.clone()).collect();
                names.sort();
                names.dedup();
                names.extend(self.project.db.characters.iter().map(|c| c.id.clone()));
                names
            },
            placed_characters: {
                people.sort();
                people.dedup();
                people
            },
            quest_stages: self.project.db.quests.iter().map(|q| (q.id.clone(), q.stages.clone())).collect(),
            // Proficiency ids (SRD skills, tools, saving throws) for skill fields.
            skills: rustpg2d_engine::srd::abilities::proficiency_choices(),
            races: self.project.db.races.iter().map(|r| r.id.clone()).collect(),
            classes: self.project.db.classes.iter().flat_map(|c| std::iter::once(c.id.clone()).chain(c.subclasses.iter().map(|s| s.id.clone()))).collect(),
            common_events: self.project.db.common_events.iter().map(|r| r.id.clone()).collect(),
            cutscenes: self.project.db.cutscenes.iter().map(|r| r.id.clone()).collect(),
            animations: self.project.db.animations.iter().map(|r| r.id.clone()).collect(),
            diseases: self.project.db.diseases.iter().map(|r| r.id.clone()).collect(),
            factions: self.project.config.factions.list.iter().map(|f| f.id.clone()).collect(),
            spells: self.project.db.spells.iter().map(|q| q.id.clone()).collect(),
            achievements: self.project.db.achievements.iter().map(|a| a.id.clone()).collect(),
            stations: {
                let mut v: Vec<String> = self.project.db.recipes.iter().map(|r| r.station.clone()).filter(|s| !s.is_empty()).collect();
                v.sort();
                v.dedup();
                v
            },
            sheets: self.project.db.sheets.iter().map(|q| q.id.clone()).collect(),
            sheet_tags: self.project.db.sheets.iter().filter(|q| !q.tags.is_empty()).map(|q| (q.id.clone(), q.tags.clone())).collect(),
            emotions: self.project.db.sheets.iter().filter(|q| q.kind == rustpg2d_engine::defs::SheetKind::Emotions).flat_map(|q| q.tags.iter().map(|t| t.name.clone())).collect(),
        };
    }

}
