//! Files written by earlier versions keep loading: the 0.9.0 example project's definitions, maps
//! of 0.9.0 and 0.10.0, a frozen format-version-1 map and save games, the last tile release
//! (0.20.0) converting (see `fixtures/README.md`).
//! Tile maps and tilesets (before 0.22) load for the converter and convert into painted maps.

use std::path::{Path, PathBuf};

use rustpg2d_engine::defs::{CharacterDef, ItemDef, PropDef, StructureDef};
use rustpg2d_engine::map::io;
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
    assert!(!db.structures.is_empty() && !db.recipes.is_empty() && !db.spells.is_empty());
    assert!(!db.quests.is_empty() && !db.sounds.is_empty() && !db.achievements.is_empty());
    // Its layered characters (removed in 0.17) load without their figure (drawn as the mannequin);
    // hair, outfit and armor sheets load as plain sprites.
    assert!(db.characters.iter().all(|c| c.appearance.model.is_empty()), "no character has a model");
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
    again::<StructureDef>(&db.structures);
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
    // (and without SRD stat blocks, 0.28)
    assert!(db.characters.iter().all(|c| c.stats.is_none()));
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

#[test]
fn saves_before_classes_load_classless() {
    // 0.22.0's save: no class, nothing waiting for the level-up window, companions without
    // class levels; the hero keeps the d10 of before (no class).
    let s = std::fs::read_to_string(fixture("saves/save-0.22.0.ron")).unwrap();
    let mut w = WorldState::from_ron(&s).unwrap();
    assert!(w.class.is_none() && w.levelups.is_empty());
    assert!(w.companions.iter().all(|c| c.class.is_none() && c.level == 0 && c.xp == 0));
    // A class and a waiting level-up are saved and read back.
    w.class = Some(rustpg2d_engine::classes::HeroClass { class: "rogue".into(), subclass: "thief".into(), picks: [("rogue-expertise-1".to_string(), vec!["stealth".to_string()])].into() });
    w.levelups.push(rustpg2d_engine::state::PendingLevel { who: "village:130".into(), level: 3 });
    let back = WorldState::from_ron(&w.to_ron().unwrap()).unwrap();
    assert_eq!(back.class, w.class);
    assert_eq!(back.levelups, w.levelups);
}

#[test]
fn spells_with_mana_become_srd_spells() {
    use rustpg2d_engine::skills::{Shape, SpellEffect, SpellPart, SpellRange};
    // 0.27.0's spells (mana, cooldown, one effect each) read as SRD spells: a heal of level 1
    // with healing dice, a bolt a cantrip spell attack keeping its projectile, a blast a level 1
    // DEX save in a sphere around the caster, the light cantrip, a script of level 1.
    let db = rustpg2d_engine::project::Project::load(&fixture("project-0.27.0")).unwrap().db;
    let get = |id: &str| db.spell(id).unwrap();
    assert!(db.spells.iter().all(|d| d.effect == SpellEffect::Parts && !d.parts.is_empty() && d.classes.len() == 8));
    let heal = get("heal");
    assert_eq!((heal.level, heal.range), (1, SpellRange::Touch));
    assert!(matches!(&heal.parts[0], SpellPart::Heal { by_level } if by_level[0].1.ends_with("+ MOD")));
    let spark = get("spark");
    assert_eq!(spark.level, 0);
    assert!(matches!(spark.parts[..], [SpellPart::Attack { ranged: true }, SpellPart::Damage { .. }]) && spark.visual.projectile.is_some());
    let burst = get("fire_burst");
    assert_eq!((burst.level, burst.range, burst.area.map(|a| a.shape)), (1, SpellRange::Caster, Some(Shape::Sphere)));
    assert!(matches!(burst.parts[0], SpellPart::Save { half: true, .. }));
    assert!(matches!(get("light").parts[..], [SpellPart::Light { .. }]) && get("light").level == 0);
    assert!(matches!(&get("fireball").parts[..], [SpellPart::Script { function }] if function == "cast_fireball"));
    // A save of 0.22.0 with mana: it loads, the mana is ignored, no slots spent, nothing prepared.
    let s = std::fs::read_to_string(fixture("saves/save-0.22.0.ron")).unwrap();
    let w = WorldState::from_ron(&s).unwrap();
    assert_eq!((w.slots_spent, w.prepared.len(), w.concentration.is_none()), ([0; 9], 0, true));
}

#[test]
fn items_of_0_27_0_become_the_srd_equipment() {
    use rustpg2d_engine::defs::{ArmorCategory, WeaponCategory, WeaponProperty};
    let text = std::fs::read_to_string(fixture("items/items-0.27.0.ron")).unwrap();
    let mut items: Vec<ItemDef> = ron::from_str(&text).unwrap();
    items.iter_mut().for_each(ItemDef::upgrade);
    let get = |id: &str| items.iter().find(|i| i.id == id).unwrap();
    // Weapons: martial from a d8 up; a bow shoots its ammunition with both hands.
    assert_eq!(get("iron_sword").weapon.as_ref().unwrap().category, Some(WeaponCategory::Martial));
    let bow = get("bow").weapon.as_ref().unwrap();
    assert_eq!((bow.category, bow.properties.clone()), (Some(WeaponCategory::Simple), vec![WeaponProperty::TwoHanded, WeaponProperty::Ammunition]));
    // The spear's two-tile swing: the Reach property (10 ft in fights).
    let spear = get("spear").weapon.as_ref().unwrap();
    assert!(spear.properties.contains(&WeaponProperty::Reach) && spear.reach_tiles() == 2);
    // Body armor keeps the armor class it gave (10 + defense) as light armor; a shield adds its defense; a helmet gives none.
    let mail = get("chainmail").armor.as_ref().unwrap();
    assert_eq!((mail.category, mail.base_ac), (Some(ArmorCategory::Light), 13));
    let shield = get("wooden_shield").armor.as_ref().unwrap();
    assert_eq!((shield.category, shield.base_ac), (Some(ArmorCategory::Shield), 1));
    assert_eq!(get("iron_helmet").armor.as_ref().unwrap().category, Some(ArmorCategory::Clothing));
    // Their money is no coin yet (no worth): shops keep counting the one item.
    assert_eq!(get("coin").coin_cp, 0);
    // Upgraded twice: the same.
    let once = items.clone();
    items.iter_mut().for_each(ItemDef::upgrade);
    assert_eq!(items, once);
}
