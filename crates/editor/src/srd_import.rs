//! The SRD 5.1 data import: races, subraces, their traits and choices, and backgrounds from a
//! checkout of 5e-bits/5e-database (`src/2014/en/5e-SRD-*.json`, the data behind the 5e-srd-api)
//! into a library's (or project's) `defs/races.ron` and `defs/backgrounds.ron`. The SRD's index
//! names are the ids (`hill-dwarf`), so running it again updates what it wrote; content fields
//! the SRD doesn't have (bodies, faction, size, colours, playable subraces) are kept. Run by
//! `cargo run -p rustpg2d-editor --example srd_import -- <checkout> [library]`.
//!
//! The SRD 5.1 is by Wizards of the Coast LLC, licensed under CC BY 4.0.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use rustpg2d_engine::backgrounds::BackgroundDef;
use rustpg2d_engine::races::{ChoiceKind, RaceChoice, RaceDef, RaceSkill, RaceTrait};
use serde_json::Value;

/// The 5e-database commit the data is pinned to (updating it is a deliberate step with a diff).
pub const SRD_COMMIT: &str = "a6212beb4b278917c2cff41c2b04c4ef4f3e0d6f";

/// Damage types the SRD's traits give resistance to (they say so only in their text).
const RESISTANCES: [(&str, &str); 2] = [("dwarven-resilience", "poison"), ("hellish-resistance", "fire")];

/// The JSON files of a checkout, by name (`Races`, `Traits`, …).
pub struct SrdData {
    races: Vec<Value>,
    subraces: Vec<Value>,
    traits: BTreeMap<String, Value>,
    backgrounds: Vec<Value>,
}

fn list(v: &Value, key: &str) -> Vec<Value> {
    v.get(key).and_then(Value::as_array).cloned().unwrap_or_default()
}

fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}

/// `desc` paragraphs as one text.
fn desc(v: &Value) -> String {
    match v.get("desc") {
        Some(Value::Array(a)) => a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" "),
        Some(Value::String(t)) => t.clone(),
        _ => String::new(),
    }
}

/// A proficiency's id as the engine names it: skills without their `skill-` prefix.
fn prof_id(index: &str) -> String {
    index.strip_prefix("skill-").unwrap_or(index).to_string()
}

/// The option ids (and names) of an SRD option set (`{choose, from: {options: [{item}]}}`).
fn options(set: &Value) -> (u32, Vec<(String, String)>) {
    let choose = set.get("choose").and_then(Value::as_u64).unwrap_or(1) as u32;
    let opts = set
        .pointer("/from/options")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|o| o.get("item").or_else(|| o.get("ability_score"))).map(|i| (s(i, "index").to_string(), s(i, "name").to_string())).collect())
        .unwrap_or_default();
    (choose, opts)
}

impl SrdData {
    /// Reads the files of a checkout (`<dir>/src/2014/en/`).
    pub fn read(checkout: &Path) -> Result<SrdData> {
        let en = checkout.join("src/2014/en");
        let read = |name: &str| -> Result<Vec<Value>> {
            let p = en.join(format!("5e-SRD-{name}.json"));
            let text = std::fs::read_to_string(&p).with_context(|| format!("reading {}", p.display()))?;
            serde_json::from_str(&text).with_context(|| format!("parsing {}", p.display()))
        };
        let traits = read("Traits")?.into_iter().map(|t| (s(&t, "index").to_string(), t)).collect();
        Ok(SrdData { races: read("Races")?, subraces: read("Subraces")?, traits, backgrounds: read("Backgrounds")? })
    }

    /// Traits, fixed proficiencies, resistances, darkvision and the choices of a list of trait
    /// references.
    fn apply_traits(&self, refs: &[Value], r: &mut RaceDef) {
        r.traits.clear();
        r.skills.clear();
        r.resistances.clear();
        r.darkvision_ft = 0;
        let mut choices = Vec::new();
        for t in refs.iter().filter_map(|t| self.traits.get(s(t, "index"))) {
            let id = s(t, "index");
            r.traits.push(RaceTrait { id: id.into(), name: s(t, "name").into(), description: desc(t) });
            for p in list(t, "proficiencies") {
                let id = prof_id(s(&p, "index"));
                if !r.skills.iter().any(|x| x.skill == id) {
                    r.skills.push(RaceSkill { skill: id, level: 1 });
                }
            }
            if let Some((_, kind)) = RESISTANCES.iter().find(|(t, _)| *t == id) {
                r.resistances.push(kind.to_string());
            }
            if id == "darkvision" {
                r.darkvision_ft = 60;
            }
            if let Some(set) = t.get("proficiency_choices") {
                let (choose, opts) = options(set);
                let kind = if opts.iter().all(|(o, _)| o.starts_with("skill-")) { ChoiceKind::Skill } else { ChoiceKind::Tool };
                choices.push(RaceChoice { id: id.into(), label: s(t, "name").into(), kind, choose, options: opts.iter().map(|(o, _)| prof_id(o)).collect(), ..Default::default() });
            }
            if let Some(set) = t.get("language_options") {
                let (choose, opts) = options(set);
                choices.push(RaceChoice { id: id.into(), label: s(t, "name").into(), kind: ChoiceKind::Language, choose, options: opts.into_iter().map(|(o, _)| o).collect(), ..Default::default() });
            }
            if let Some(set) = t.pointer("/trait_specific/subtrait_options") {
                let (choose, opts) = options(set);
                let mut c = RaceChoice { id: id.into(), label: s(t, "name").into(), kind: ChoiceKind::Ancestry, choose, ..Default::default() };
                for (o, name) in opts {
                    if let Some(dt) = self.traits.get(&o).and_then(|st| st.pointer("/trait_specific/damage_type/index")).and_then(Value::as_str) {
                        c.grants.insert(o.clone(), dt.to_string());
                    }
                    c.names.insert(o.clone(), name);
                    c.options.push(o);
                }
                choices.push(c);
            }
            if let Some(set) = t.pointer("/trait_specific/spell_options") {
                let (choose, opts) = options(set);
                let mut c = RaceChoice { id: id.into(), label: s(t, "name").into(), kind: ChoiceKind::Cantrip, choose, ..Default::default() };
                for (o, name) in opts {
                    c.names.insert(o.clone(), name);
                    c.options.push(o);
                }
                choices.push(c);
            }
        }
        // Choices of the race itself (ability increases, languages) come first.
        let own: Vec<RaceChoice> = r.choices.drain(..).filter(|c| c.id.ends_with("-ability-increase") || c.id.ends_with("-language-choice")).collect();
        r.choices = own.into_iter().chain(choices).collect();
    }

    /// Writes the SRD's races, subraces and backgrounds into `races` / `backgrounds`: existing
    /// ones are updated (their content fields kept), new ones added.
    pub fn import(&self, races: &mut Vec<RaceDef>, backgrounds: &mut Vec<BackgroundDef>) -> (usize, usize) {
        let (mut added, mut updated) = (0, 0);
        let mut upsert = |races: &mut Vec<RaceDef>, id: &str| -> usize {
            match races.iter().position(|r| r.id == id) {
                Some(k) => {
                    updated += 1;
                    k
                }
                None => {
                    added += 1;
                    races.push(RaceDef { id: id.into(), ..Default::default() });
                    races.len() - 1
                }
            }
        };
        for j in &self.races {
            let id = s(j, "index");
            let k = upsert(races, id);
            let new = races[k].name.is_empty();
            let r = &mut races[k];
            r.name = s(j, "name").into();
            r.parent.clear();
            r.ability_bonuses = list(j, "ability_bonuses").iter().map(|b| (b.pointer("/ability_score/index").and_then(Value::as_str).unwrap_or("").to_string(), b.get("bonus").and_then(Value::as_i64).unwrap_or(0) as i32)).collect();
            r.walk_ft = j.get("speed").and_then(Value::as_u64).unwrap_or(30) as u32;
            r.speed = 0.0;
            r.size_category = s(j, "size").into();
            r.languages = list(j, "languages").iter().map(|l| s(l, "index").to_string()).collect();
            let subraces = !list(j, "subraces").is_empty();
            // A race with subraces is chosen as one of them.
            if subraces {
                r.playable = false;
            } else if new {
                r.playable = true;
            }
            if new && r.size_category == "Small" {
                r.size = 0.75;
            }
            let mut own = Vec::new();
            if let Some(set) = j.get("ability_bonus_options") {
                let (choose, opts) = options(set);
                own.push(RaceChoice { id: format!("{id}-ability-increase"), label: "Ability Score Increase".into(), kind: ChoiceKind::Ability, choose, options: opts.into_iter().map(|(o, _)| o).collect(), ..Default::default() });
            }
            if let Some(set) = j.get("language_options") {
                let (choose, opts) = options(set);
                own.push(RaceChoice { id: format!("{id}-language-choice"), label: "Extra Language".into(), kind: ChoiceKind::Language, choose, options: opts.into_iter().map(|(o, _)| o).collect(), ..Default::default() });
            }
            r.choices = own;
            let traits = list(j, "traits");
            self.apply_traits(&traits, r);
        }
        for j in &self.subraces {
            let id = s(j, "index");
            let k = upsert(races, id);
            let new = races[k].name.is_empty();
            let r = &mut races[k];
            r.name = s(j, "name").into();
            r.parent = j.pointer("/race/index").and_then(Value::as_str).unwrap_or("").into();
            r.ability_bonuses = list(j, "ability_bonuses").iter().map(|b| (b.pointer("/ability_score/index").and_then(Value::as_str).unwrap_or("").to_string(), b.get("bonus").and_then(Value::as_i64).unwrap_or(0) as i32)).collect();
            r.description = desc(j);
            r.walk_ft = 0;
            r.speed = 0.0;
            r.languages.clear();
            if new {
                r.playable = true;
            }
            r.choices.clear();
            let traits = list(j, "racial_traits");
            self.apply_traits(&traits, r);
        }
        for j in &self.backgrounds {
            let id = s(j, "index");
            let b = match backgrounds.iter().position(|b| b.id == id) {
                Some(k) => {
                    updated += 1;
                    &mut backgrounds[k]
                }
                None => {
                    added += 1;
                    backgrounds.push(BackgroundDef::fresh(id));
                    backgrounds.last_mut().unwrap()
                }
            };
            b.name = s(j, "name").into();
            b.proficiencies = list(j, "starting_proficiencies").iter().map(|p| prof_id(s(p, "index"))).collect();
            b.language_choices = j.pointer("/language_options/choose").and_then(Value::as_u64).unwrap_or(0) as u32;
            b.feature = j.pointer("/feature/name").and_then(Value::as_str).unwrap_or("").into();
            b.feature_text = j.get("feature").map(desc).unwrap_or_default();
            // (Its equipment is the SRD's, which arrives with 0.31: the starting items stay the
            // project's.)
        }
        (added, updated)
    }
}

/// Imports into the `defs/` of a library or project folder; returns a one-line report.
pub fn import_into(checkout: &Path, root: &Path) -> Result<String> {
    let data = SrdData::read(checkout)?;
    let mut p = rustpg2d_engine::project::Project::load(root)?;
    let (added, updated) = data.import(&mut p.db.races, &mut p.db.backgrounds);
    let defs = root.join("defs");
    std::fs::write(defs.join("races.ron"), rustpg2d_engine::project::ron_text(&p.db.races)?)?;
    std::fs::write(defs.join("backgrounds.ron"), rustpg2d_engine::project::ron_text(&p.db.backgrounds)?)?;
    Ok(format!("{}: {added} added, {updated} updated ({} races, {} backgrounds)", root.display(), p.db.races.len(), p.db.backgrounds.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/srd")
    }

    #[test]
    fn races_and_backgrounds_import_and_a_second_run_changes_nothing() {
        let data = SrdData::read(&fixture()).unwrap();
        // An existing dwarf keeps its looks and faction; its old speed percent goes.
        let mut races = vec![RaceDef { id: "dwarf".into(), name: "Dwarf".into(), playable: true, bodies_male: vec!["dwarf".into()], faction: "dwarves".into(), speed: -10.0, ..Default::default() }];
        let mut bgs = Vec::new();
        let (added, updated) = data.import(&mut races, &mut bgs);
        assert_eq!((added, updated), (4, 1), "{:?}", races.iter().map(|r| &r.id).collect::<Vec<_>>());
        let dwarf = races.iter().find(|r| r.id == "dwarf").unwrap();
        assert_eq!((dwarf.bodies_male.len(), dwarf.faction.as_str(), dwarf.speed, dwarf.walk_ft, dwarf.playable), (1, "dwarves", 0.0, 25, false));
        assert_eq!(dwarf.ability_bonuses.get("con"), Some(&2));
        assert_eq!((dwarf.darkvision_ft, dwarf.resistances.clone()), (60, vec!["poison".to_string()]));
        assert!(dwarf.skills.iter().any(|s| s.skill == "battleaxes"));
        let tool = dwarf.choices.iter().find(|c| c.kind == ChoiceKind::Tool).expect("the dwarf's tool");
        assert_eq!(tool.options, vec!["smiths-tools", "brewers-supplies", "masons-tools"]);
        let hill = races.iter().find(|r| r.id == "hill-dwarf").unwrap();
        assert_eq!((hill.parent.as_str(), hill.playable), ("dwarf", true));
        let merged = hill.merged(Some(dwarf));
        assert_eq!((merged.ability_bonuses.get("con"), merged.ability_bonuses.get("wis")), (Some(&2), Some(&1)));
        let half_elf = races.iter().find(|r| r.id == "half-elf").unwrap();
        let kinds: Vec<(ChoiceKind, u32)> = half_elf.choices.iter().map(|c| (c.kind, c.choose)).collect();
        assert_eq!(kinds, vec![(ChoiceKind::Ability, 2), (ChoiceKind::Language, 1), (ChoiceKind::Skill, 2)]);
        let dragonborn = races.iter().find(|r| r.id == "dragonborn").unwrap();
        let anc = dragonborn.choices.iter().find(|c| c.kind == ChoiceKind::Ancestry).unwrap();
        assert_eq!(anc.grants.get("draconic-ancestry-red").map(String::as_str), Some("fire"));
        assert_eq!(bgs[0].proficiencies, vec!["insight", "religion"]);
        assert_eq!((bgs[0].language_choices, bgs[0].feature.as_str()), (2, "Shelter of the Faithful"));
        // A second run: nothing changes.
        let (r1, b1) = (races.clone(), bgs.clone());
        data.import(&mut races, &mut bgs);
        assert_eq!((races, bgs), (r1, b1));
    }

    #[test]
    fn a_folder_gets_its_races_and_backgrounds_files() {
        assert_eq!(SRD_COMMIT.len(), 40);
        let root = std::env::temp_dir().join(format!("rustpg2d-srd-import-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("defs")).unwrap();
        let p = rustpg2d_engine::project::Project { root: root.clone(), config: rustpg2d_engine::project::ProjectConfig::new("Library"), db: Default::default() };
        p.save_config().unwrap();
        let first = import_into(&fixture(), &root).unwrap();
        assert!(first.contains("5 added"), "{first}");
        let races = std::fs::read_to_string(root.join("defs/races.ron")).unwrap();
        import_into(&fixture(), &root).unwrap();
        assert_eq!(std::fs::read_to_string(root.join("defs/races.ron")).unwrap(), races, "a second run writes the same file");
        let loaded = rustpg2d_engine::project::Project::load(&root).unwrap();
        assert_eq!((loaded.db.races.len(), loaded.db.backgrounds.len()), (4, 1));
        let _ = std::fs::remove_dir_all(&root);
    }
}
