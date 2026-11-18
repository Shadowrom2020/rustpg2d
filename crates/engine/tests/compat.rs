//! Files written by earlier versions keep loading: the 0.9.0 example project's definitions, maps
//! of 0.9.0 and 0.10.0, a frozen format-version-1 map and save games, the last tile release
//! (0.20.0) converting (see `fixtures/README.md`).
//! Tile maps and tilesets (before 0.22) load for the converter and convert into painted maps.

use std::path::{Path, PathBuf};

use rustpg2d_engine::defs::{CharacterDef, ItemDef, PropDef, StructureDef};
use rustpg2d_engine::map::io;
use rustpg2d_engine::map::{CollisionOverride, ZoneKind};
use rustpg2d_engine::old_tiles::{read_tilesets, Tile, TilesetDef};
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
    assert!(!db.items.is_empty() && !db.props.is_empty() && !db.characters.is_empty() && !db.sheets.is_empty());
    assert!(!read_tilesets(&fixture("project-0.9.0")).unwrap().is_empty());
    assert!(!db.structures.is_empty() && !db.recipes.is_empty() && !db.spells.is_empty());
    // (Its skill tree, defs/skills.ron, is read only by the SRD conversion since 0.28.)
    assert!(rustpg2d_engine::srd_convert::needs_conversion(&fixture("project-0.9.0")));
    assert!(!db.quests.is_empty() && !db.sounds.is_empty() && !db.achievements.is_empty());
    // Its layered characters (removed in 0.17) keep their body; hair, outfit and armor sheets load
    // as plain sprites.
    assert!(db.characters.iter().all(|c| !c.appearance.body.is_empty()), "every character has a body");
    assert!(db.sheets.iter().any(|s| s.id.starts_with("hair_") && s.kind == rustpg2d_engine::defs::SheetKind::Sprite));
    // Projects before UI skins (0.25): no skins.ron, no `skin` = the built-in look.
    assert!(db.skins.is_empty() && p.config.skin.is_empty());
    // … and before designed menus: the built-in title screen and pause menu.
    assert!(p.config.title.is_default() && p.config.menu.is_default() && p.config.credits.is_empty());
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
    again::<TilesetDef>(&read_tilesets(&fixture("project-0.9.0")).unwrap());
    again::<StructureDef>(&db.structures);
}

#[test]
fn maps_of_0_9_0_and_0_10_0_load() {
    for (file, name) in [("house-0.9.0.map", "house"), ("shop-0.9.0.map", "shop"), ("village-0.9.0.map", "village"), ("mine-0.10.0.map", "mine")] {
        let (m, tiles) = io::load_tiles(&fixture(&format!("maps/{file}"))).unwrap_or_else(|e| panic!("{file}: {e:#}"));
        assert_eq!(m.name, name, "{file}");
        assert!(m.width > 0 && m.height > 0 && !tiles.layers.is_empty(), "{file}");
        assert!(tiles.tilesets.iter().any(|t| t == "terrain"), "{file}");
        assert!(!m.entities.is_empty(), "{file} has entities");
        for l in &tiles.layers {
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
    let (m, tiles) = io::decode_tiles(&bytes, &mut |_| {}).unwrap();
    assert_eq!(tiles.layers[0].get(3, 3), Tile::new(0, 9));
    assert_eq!((m.entities[0].def.as_str(), m.entities[0].count), ("key", 2));
    assert_eq!((m.next_uid, m.background), (5, [1, 2, 3]));
}

/// The 0.9.0 project with the 0.9.0 and 0.10.0 maps converts into a painted project: no tile
/// that could be walked on is blocked afterwards, entities and zones stay as they were, the
/// tilesets file goes and a backup keeps the old project.
#[test]
fn tile_projects_convert() {
    convert_and_check("project-0.9.0", &[("house-0.9.0.map", "house"), ("shop-0.9.0.map", "shop"), ("village-0.9.0.map", "village"), ("mine-0.10.0.map", "mine")]);
}

/// The same for the last tile release, 0.20.0 (its example's definitions, village and mine).
/// A 0.27 project: its skill tree becomes SRD proficiencies, lock levels DCs, recipe quality
/// levels DCs (0.28).
#[test]
fn projects_of_0_27_0_convert_to_the_srd_rules() {
    let root = std::env::temp_dir().join(format!("rustpg2d_compat_srd_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("defs")).unwrap();
    let src = fixture("project-0.27.0");
    std::fs::copy(src.join("project.ron"), root.join("project.ron")).unwrap();
    for e in std::fs::read_dir(src.join("defs")).unwrap() {
        let p = e.unwrap().path();
        std::fs::copy(&p, root.join("defs").join(p.file_name().unwrap())).unwrap();
    }
    assert!(rustpg2d_engine::srd_convert::needs_conversion(&root));
    let r = rustpg2d_engine::srd_convert::convert_project(&root, None).expect("converts");
    assert!(r.mapped.iter().any(|(a, b)| a == "lockpicking" && b == "thieves-tools"), "{:?}", r.mapped);
    assert!(r.dropped.contains(&"swordsmanship".to_string()));
    assert!(!root.join("defs/skills.ron").exists());
    let p = Project::load(&root).unwrap();
    assert_eq!(p.config.rules_version, rustpg2d_engine::srd::RULES_VERSION);
    assert!(p.config.skills.magic_on(), "magic stays on");
    let fine = p.db.recipes.iter().find(|r| r.quality.iter().any(|q| q.result == "fine_iron_sword")).expect("the sword recipe");
    assert_eq!((fine.skill.as_str(), fine.quality[0].dc), ("smiths-tools", 12));
    let known: Vec<String> = rustpg2d_engine::srd::abilities::proficiency_choices();
    for race in &p.db.races {
        assert!(race.skills.iter().all(|s| known.contains(&s.skill)), "{}: {:?}", race.id, race.skills.iter().map(|s| &s.skill).collect::<Vec<_>>());
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn projects_of_0_20_0_convert() {
    convert_and_check("project-0.20.0", &[("village-0.20.0.map", "village"), ("mine-0.20.0.map", "mine")]);
}

fn convert_and_check(project: &str, maps: &[(&str, &str)]) {
    let dir = std::env::temp_dir().join(format!("rustpg2d_compat_convert_{project}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let root = dir.join("old");
    std::fs::create_dir_all(root.join("defs")).unwrap();
    std::fs::create_dir_all(root.join("maps")).unwrap();
    let src = fixture(project);
    std::fs::copy(src.join("project.ron"), root.join("project.ron")).unwrap();
    for e in std::fs::read_dir(src.join("defs")).unwrap() {
        let p = e.unwrap().path();
        std::fs::copy(&p, root.join("defs").join(p.file_name().unwrap())).unwrap();
    }
    for (file, name) in maps {
        std::fs::copy(fixture(&format!("maps/{file}")), root.join(format!("maps/{name}.map"))).unwrap();
    }
    // Stand-in tileset pictures (the fixtures have none): 16 x 64 tiles in eight colours.
    let tilesets = read_tilesets(&root).unwrap();
    for t in &tilesets {
        let mut img = rustpg2d_engine::imageio::Rgba::new(16 * 32, 64 * 32);
        for y in 0..img.h as i32 {
            for x in 0..img.w as i32 {
                let k = ((y / 32) * 16 + x / 32) % 8;
                img.set(x, y, [k as u8 * 30, 200 - k as u8 * 20, 90, 255]);
            }
        }
        let p = root.join(&t.image);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, img.encode().unwrap()).unwrap();
    }
    // Before: blocked by a solid tile on any layer below the characters, or painted so.
    let before: Vec<_> = maps
        .iter()
        .map(|(_, name)| {
            let (m, old) = io::load_tiles(&root.join(format!("maps/{name}.map"))).unwrap();
            let mut solid = vec![false; (m.width * m.height) as usize];
            for l in old.layers.iter().filter(|l| !l.above_entities) {
                for (x, y, t) in l.tiles() {
                    let (slot, i) = t.parts().unwrap();
                    if tilesets.iter().find(|d| Some(&d.id) == old.tilesets.get(slot)).is_some_and(|d| d.solid.contains(&i)) {
                        solid[(y * m.width + x) as usize] = true;
                    }
                }
            }
            (m, solid)
        })
        .collect();
    let backup = dir.join("backup");
    let report = rustpg2d_engine::convert_tiles::convert_project(&root, Some(&backup)).unwrap();
    assert_eq!(report.maps.len(), maps.len());
    assert!(report.structures > 0 && !report.materials_added.is_empty() && !report.props_added.is_empty());
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    assert!(backup.join("defs/tilesets.ron").is_file() && !root.join("defs/tilesets.ron").exists());
    assert!(!rustpg2d_engine::convert_tiles::needs_conversion(&root));
    let p = Project::load(&root).unwrap();
    assert!(p.db.structures.iter().all(|s| s.layers.is_empty() && s.tilesets.is_empty()));
    assert!(p.db.structures.iter().any(|s| !s.ground.is_empty() || !s.decor.is_empty()));
    for ((_, name), (old, old_solid)) in maps.iter().zip(&before) {
        let m = p.load_map(name).unwrap();
        assert!(!m.ground.is_empty() && !m.materials.is_empty(), "{name} has ground");
        assert_eq!(m.entities, old.entities, "{name}: entities");
        assert_eq!(m.zones, old.zones, "{name}: zones");
        let mut solid = m.compute_solid();
        let mut shot = solid.clone();
        rustpg2d_engine::ground::MaterialTable::new(&m.materials, &p.db).apply_to_tiles(&m, &mut solid, &mut shot);
        rustpg2d_engine::decor::mark_solid_tiles(&m, &p.db, &mut solid);
        for y in 0..m.height {
            for x in 0..m.width {
                let i = (y * m.width + x) as usize;
                let (was, now) = match m.collision_at(x, y) {
                    CollisionOverride::Auto => (old_solid[i], solid[i]),
                    // (Painted collision decides, then and now.)
                    _ => continue,
                };
                assert!(was || !now, "{name}: tile {x},{y} could be walked on and is blocked now");
            }
        }
    }
    // Converting twice does nothing.
    assert!(!rustpg2d_engine::convert_tiles::needs_conversion(&root));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Quests of 0.21.0 (before the quest designer) load with nothing described: started and moved
/// on by triggers and scripts as before.
#[test]
fn quests_of_0_21_0_load_without_the_designer_fields() {
    let text = std::fs::read_to_string(fixture("quests/quests-0.21.0.ron")).unwrap();
    let quests: Vec<rustpg2d_engine::quest::QuestDef> = ron::from_str(&text).unwrap();
    assert_eq!(quests.len(), 7);
    let lights = quests.iter().find(|q| q.id == "strange_lights").unwrap();
    assert_eq!((lights.stages.len(), lights.targets.len()), (3, 3));
    for q in &quests {
        assert!(!q.has_giver() && q.steps.is_empty() && q.rewards.is_empty() && q.offer.is_empty() && q.objects.is_empty(), "{}", q.id);
    }
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
    assert_eq!(w.version, 1, "written when the save layout was 1 (before the SRD rules' 2)");
    assert_eq!(w.current_map, "village");
    assert_eq!(w.companions.len(), 1);
    let c = &w.companions[0];
    assert_eq!((c.map.as_str(), c.uid, c.name.as_str(), c.hp), ("village", 121, "guard", Some(7)));
    assert_eq!(c.mana, None, "full mana");
    assert!(w.npc_spells.is_empty() && w.npc_spells("village", 121).is_empty(), "no spells learned yet");
    let text = w.to_ron().unwrap();
    assert_eq!(WorldState::from_ron(&text).unwrap().to_ron().unwrap(), text);
}

/// 0.16.0 had no races: characters have none, the project no races file, the player no race.
#[test]
fn definitions_of_0_16_0_load_without_races() {
    let p = Project::load(&fixture("project-0.16.0")).expect("the 0.16.0 project loads");
    let db = &p.db;
    assert!(!db.characters.is_empty() && !db.sheets.is_empty());
    assert!(db.races.is_empty());
    assert!(db.characters.iter().all(|c| c.race.is_empty()));
    // Its figure sheets load as plain sprites (characters are 3D models now).
    assert!(db.sheets.iter().all(|s| s.kind != rustpg2d_engine::defs::SheetKind::Emotions || !s.tags.is_empty()));
    let w = WorldState::from_ron(&std::fs::read_to_string(fixture("saves/save-0.12.0.ron")).unwrap()).unwrap();
    assert!(w.player_race.is_empty());
}

#[test]
fn minimal_save_loads_with_defaults() {
    let s = std::fs::read_to_string(fixture("saves/minimal.ron")).unwrap();
    let w = WorldState::from_ron(&s).unwrap();
    assert_eq!((w.current_map.as_str(), w.player_pos), ("village", (100.0, 200.0)));
    assert_eq!(w.hour, project::default_start_hour());
    assert!(w.player_hp.is_none() && w.quests.is_empty() && w.companions.is_empty());
}

#[test]
fn save_of_0_22_0_loads_without_homes() {
    // A quick save of 0.22.0 in Tilda's shop at 21:00: before homes and placed furniture.
    let s = std::fs::read_to_string(fixture("saves/save-0.22.0.ron")).unwrap();
    let mut w = WorldState::from_ron(&s).unwrap();
    assert_eq!(w.current_map, "shop");
    assert!(w.homes.is_empty() && w.furniture.is_empty() && w.next_furniture == 0);
    // With a home and a chair: saved and read back.
    w.homes.insert("stone_house".into());
    let uid = w.add_furniture("stone_house", "chair", "chair_item", 80.0, 112.0);
    let back = WorldState::from_ron(&w.to_ron().unwrap()).unwrap();
    assert!(back.homes.contains("stone_house"));
    assert_eq!(back.furniture["stone_house"][0].uid, uid);
    assert_eq!(back.furniture["stone_house"][0].item, "chair_item");
}

#[test]
fn map_and_save_before_faction_membership_load() {
    // The forest of 0.25.0: placed characters before a faction of their own.
    let m = io::load(&fixture("maps/forest-0.25.0.map")).unwrap();
    assert!(m.entities.iter().any(|e| e.name == "outlaw_1"));
    assert!(m.entities.iter().all(|e| e.faction.is_empty()));
    // A placed character's own faction is written and read back.
    let mut m = m;
    m.entities.iter_mut().find(|e| e.name == "outlaw_1").unwrap().faction = "humans".into();
    let dir = std::env::temp_dir().join(format!("rustpg2d_compat_faction_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("forest.map");
    io::save(&m, &path).unwrap();
    let back = io::load(&path).unwrap();
    assert_eq!(back.entities.iter().find(|e| e.name == "outlaw_1").unwrap().faction, "humans");
    assert!(back.entities.iter().filter(|e| e.name != "outlaw_1").all(|e| e.faction.is_empty()));
    let _ = std::fs::remove_dir_all(&dir);
    // A 0.22.0 save: the player joined no faction, nobody's faction was changed.
    let w = WorldState::from_ron(&std::fs::read_to_string(fixture("saves/save-0.22.0.ron")).unwrap()).unwrap();
    assert!(w.player_faction.is_empty());
    assert!(w.entities.values().flat_map(|m| m.values()).all(|s| s.faction.is_none()));
}
