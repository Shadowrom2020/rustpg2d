//! Files written by 0.35 keep loading: the example project's definitions and its village as of
//! the tag v0.35.0-alpha.1, and a quick save of the 0.35 game. A project of an older version is
//! refused with the one message (see `fixtures/README.md`).

use std::path::{Path, PathBuf};

use rustpg2d_engine::defs::{CharacterDef, ItemDef, PropDef, StructureDef};
use rustpg2d_engine::map::io;
use rustpg2d_engine::map::ZoneKind;
use rustpg2d_engine::project::{self, Project};
use rustpg2d_engine::skills::SpellDef;
use rustpg2d_engine::state::{WorldState, SAVE_VERSION};

fn fixture(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(rel)
}

/// The 0.35 example's definitions load whole: no entry is left out, and the fields this version
/// dropped (an item's `held` picture, a spell's `effect`) are read and left out.
#[test]
fn definitions_of_0_35_0_load() {
    let root = fixture("project-0.35.0");
    assert!(!project::made_before_035(&root));
    let (p, warnings) = Project::load_with_warnings(&root).expect("the 0.35 project loads");
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(p.config.name, "R(ust)PG2D example");
    assert_eq!(p.config.rules_version, rustpg2d_engine::srd::RULES_VERSION);
    let db = &p.db;
    for (what, n) in [("items", db.items.len()), ("props", db.props.len()), ("characters", db.characters.len()), ("spells", db.spells.len()), ("classes", db.classes.len()), ("races", db.races.len()), ("models", db.models.len()), ("materials", db.materials.len()), ("structures", db.structures.len()), ("quests", db.quests.len()), ("sheets", db.sheets.len())] {
        assert!(n > 0, "no {what}");
    }
    let text = |f: &str| std::fs::read_to_string(root.join("defs").join(f)).unwrap();
    assert!(text("items.ron").contains("held:") && text("spells.ron").contains("effect: Parts"), "the fixture has the fields 0.36 dropped");
    // Weapons and armor carry their SRD categories as written (nothing is upgraded on reading).
    assert!(db.items.iter().filter_map(|i| i.weapon.as_ref()).all(|w| w.category.is_some()));
    assert!(db.items.iter().filter_map(|i| i.armor.as_ref()).all(|a| a.category.is_some()));
    assert!(db.spells.iter().all(|s| s.story_only || !s.parts.is_empty()), "every spell has its parts");
}

/// The 0.35 definitions written by today's writer read back the same, without the dropped fields.
#[test]
fn definitions_of_0_35_0_round_trip() {
    let db = Project::load(&fixture("project-0.35.0")).unwrap().db;
    fn again<T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug>(v: &Vec<T>) -> String {
        let text = project::ron_text(v).unwrap();
        let back: Vec<T> = ron::from_str(&text).unwrap();
        assert_eq!(&back, v);
        text
    }
    assert!(!again::<ItemDef>(&db.items).contains("held:"));
    again::<PropDef>(&db.props);
    again::<CharacterDef>(&db.characters);
    assert!(!again::<SpellDef>(&db.spells).contains("effect: Parts"));
    again::<StructureDef>(&db.structures);
}

/// The 0.35 village (map format version 3) loads whole and is written again the same.
#[test]
fn map_of_0_35_0_loads() {
    let path = fixture("maps/village-0.35.0.map");
    let mut warnings = Vec::new();
    let m = io::load_with(&path, &mut |w| warnings.push(w)).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(m.name, "village");
    assert!(!m.ground.is_empty() && !m.materials.is_empty() && !m.decor.is_empty() && !m.entities.is_empty());
    assert!(m.zones.iter().any(|z| z.kind == ZoneKind::Spawn && z.name == "start"));
    // Written again, it reads back the same (0.35 wrote an empty `body` in placed characters'
    // looks, a field this version dropped: the file's bytes differ by that).
    let bytes = io::encode(&m).unwrap();
    let back = io::decode(&bytes).unwrap();
    assert_eq!(io::Meta::of(&back), io::Meta::of(&m));
    assert_eq!((&back.materials, &back.decor_defs, &back.decor, &back.collision), (&m.materials, &m.decor_defs, &m.decor, &m.collision));
    assert_eq!(io::encode(&back).unwrap(), bytes, "and again byte for byte");
}

/// A project of a version before 0.35 (here 0.27's `project.ron`: no rules version) is refused
/// with the message that sends it to 0.35.
#[test]
fn projects_before_0_35_are_refused() {
    let root = fixture("project-0.27.0");
    assert!(project::made_before_035(&root));
    assert_eq!(project::refuse_before_035(&root).unwrap_err().to_string(), project::MADE_BEFORE_035);
}

/// A quick save written by the 0.35 game loads, and today's writer writes it the same way.
#[test]
fn save_of_0_35_0_loads() {
    let s = std::fs::read_to_string(fixture("saves/save-0.35.0.ron")).unwrap();
    let w = WorldState::from_ron(&s).unwrap();
    assert_eq!((w.version, w.current_map.as_str()), (SAVE_VERSION, "house"));
    assert!(w.flags.contains("fixture_flag"));
    assert_eq!(w.vars.get("fixture_var"), Some(&7));
    assert_eq!(w.inventory.count("potion"), 2);
    assert!(w.quests.contains_key("lost_cat") && w.player_hp.is_some());
    assert_eq!(w.to_ron().unwrap(), s.trim_end(), "written again unchanged");
}

#[test]
fn minimal_save_loads_with_defaults() {
    let s = std::fs::read_to_string(fixture("saves/minimal.ron")).unwrap();
    let w = WorldState::from_ron(&s).unwrap();
    assert_eq!((w.current_map.as_str(), w.player_pos), ("village", (100.0, 200.0)));
    assert_eq!(w.hour, project::default_start_hour());
    assert!(w.player_hp.is_none() && w.quests.is_empty() && w.companions.is_empty());
    // Written by today's code and read back: the same save, with the version.
    let text = w.to_ron().unwrap();
    assert!(text.contains(&format!("version: {SAVE_VERSION}")));
    assert_eq!(WorldState::from_ron(&text).unwrap().to_ron().unwrap(), text);
}
