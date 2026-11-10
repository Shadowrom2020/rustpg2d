//! Files written by earlier versions keep loading: the 0.9.0 example project's definitions, maps
//! of 0.9.0 and 0.10.0, a frozen format-version-1 map and save games (see `fixtures/README.md`).

use std::path::{Path, PathBuf};

use rustpg2d_engine::defs::{CharacterDef, ItemDef, PropDef, StructureDef, TilesetDef};
use rustpg2d_engine::map::io;
use rustpg2d_engine::map::{Tile, ZoneKind};
use rustpg2d_engine::project::{self, Project};
use rustpg2d_engine::state::{WorldState, SAVE_VERSION};

fn fixture(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(rel)
}

#[test]
fn definitions_of_0_9_0_load() {
    let p = Project::load(&fixture("project-0.9.0")).expect("the 0.9.0 project loads");
    assert_eq!(p.config.name, "R(ust)PG2D example");
    let db = &p.db;
    assert!(!db.items.is_empty() && !db.props.is_empty() && !db.characters.is_empty() && !db.tilesets.is_empty() && !db.sheets.is_empty());
    assert!(!db.structures.is_empty() && !db.recipes.is_empty() && !db.skills.is_empty() && !db.spells.is_empty());
    assert!(!db.quests.is_empty() && !db.sounds.is_empty() && !db.achievements.is_empty());
    // Four characters of that version carried a pre-drawn `sheet`; it became their body.
    assert!(db.characters.iter().all(|c| c.sheet.is_empty()), "legacy sheets are upgraded on load");
    assert!(db.characters.iter().all(|c| !c.appearance.body.is_empty()), "every character has a body");
}

/// Old definitions written by today's writer read back the same (nothing is lost or changed).
#[test]
fn definitions_of_0_9_0_round_trip() {
    let db = Project::load(&fixture("project-0.9.0")).unwrap().db;
    fn again<T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug>(v: &Vec<T>) {
        let text = project::ron_text(v).unwrap();
        let back: Vec<T> = ron::from_str(&text).unwrap();
        assert_eq!(&back, v);
    }
    again::<ItemDef>(&db.items);
    again::<PropDef>(&db.props);
    again::<CharacterDef>(&db.characters);
    again::<TilesetDef>(&db.tilesets);
    again::<StructureDef>(&db.structures);
}

#[test]
fn maps_of_0_9_0_and_0_10_0_load() {
    for (file, name) in [("house-0.9.0.map", "house"), ("shop-0.9.0.map", "shop"), ("village-0.9.0.map", "village"), ("mine-0.10.0.map", "mine")] {
        let m = io::load(&fixture(&format!("maps/{file}"))).unwrap_or_else(|e| panic!("{file}: {e:#}"));
        assert_eq!(m.name, name, "{file}");
        assert!(m.width > 0 && m.height > 0 && !m.layers.is_empty(), "{file}");
        assert!(m.tilesets.iter().any(|t| t == "terrain"), "{file}");
        assert!(!m.entities.is_empty(), "{file} has entities");
        for l in &m.layers {
            l.validate(m.width, m.height).unwrap();
        }
    }
    let v = io::load(&fixture("maps/village-0.9.0.map")).unwrap();
    assert!(v.zones.iter().any(|z| z.kind == ZoneKind::Spawn && z.name == "start"));
    assert!(v.entities.iter().any(|e| !e.triggers.is_empty()) || !v.triggers.is_empty(), "the old village has triggers");
}

#[test]
fn v1_map_decodes() {
    let bytes = std::fs::read(fixture("maps/v1-synthetic.map")).unwrap();
    let m = io::decode(&bytes).unwrap();
    assert_eq!(m.layers[0].get(3, 3), Tile::new(0, 9));
    assert_eq!((m.entities[0].def.as_str(), m.entities[0].count), ("key", 2));
    assert_eq!((m.next_uid, m.background), (5, [1, 2, 3]));
}

#[test]
fn save_of_0_11_0_loads() {
    let s = std::fs::read_to_string(fixture("saves/save-0.11.0.ron")).unwrap();
    let w = WorldState::from_ron(&s).unwrap();
    assert_eq!(w.version, 0, "written before the version field existed");
    assert_eq!(w.current_map, "house");
    assert!(w.flags.contains("fixture_flag"));
    assert_eq!(w.vars.get("fixture_var"), Some(&7));
    assert_eq!((w.inventory.count("potion"), w.inventory.count("coin")), (2, 30));
    assert!(w.quests.contains_key("lost_cat"));
    // Written by today's code and read back: the same save, now with the version.
    let text = w.to_ron().unwrap();
    assert!(text.contains(&format!("version: {SAVE_VERSION}")));
    let again = WorldState::from_ron(&text).unwrap();
    assert_eq!(again.to_ron().unwrap(), text);
    assert_eq!(again.version, SAVE_VERSION);
}

#[test]
fn save_of_0_12_0_loads() {
    // A quick save of 0.12.0 with the town guard following the player (hurt): before companions
    // had mana and learned spells.
    let s = std::fs::read_to_string(fixture("saves/save-0.12.0.ron")).unwrap();
    let w = WorldState::from_ron(&s).unwrap();
    assert_eq!(w.version, SAVE_VERSION);
    assert_eq!(w.current_map, "village");
    assert_eq!(w.companions.len(), 1);
    let c = &w.companions[0];
    assert_eq!((c.map.as_str(), c.uid, c.name.as_str(), c.hp), ("village", 121, "guard", Some(7)));
    assert_eq!(c.mana, None, "full mana");
    assert!(w.npc_spells.is_empty() && w.npc_spells("village", 121).is_empty(), "no spells learned yet");
    let text = w.to_ron().unwrap();
    assert_eq!(WorldState::from_ron(&text).unwrap().to_ron().unwrap(), text);
}

#[test]
fn minimal_save_loads_with_defaults() {
    let s = std::fs::read_to_string(fixture("saves/minimal.ron")).unwrap();
    let w = WorldState::from_ron(&s).unwrap();
    assert_eq!((w.current_map.as_str(), w.player_pos), ("village", (100.0, 200.0)));
    assert_eq!(w.hour, project::default_start_hour());
    assert!(w.player_hp.is_none() && w.quests.is_empty() && w.companions.is_empty());
}
