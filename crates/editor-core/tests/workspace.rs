//! The workspace on a scratch copy of the example project, without a window or a graphics
//! adapter: opening, changing definitions and a map, undo, renaming, saving.

use std::path::{Path, PathBuf};

use rustpg2d_editor_core::{Gpu, Workspace};
use rustpg2d_engine::refactor::RefKind;

/// A scratch copy of the example project (without save games), as a folder project.
fn example_copy(name: &str) -> PathBuf {
    fn copy(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for e in std::fs::read_dir(from).unwrap().flatten() {
            let (src, dst) = (e.path(), to.join(e.file_name()));
            if src.is_dir() {
                if e.file_name() != "saves" {
                    copy(&src, &dst);
                }
            } else {
                std::fs::copy(&src, &dst).unwrap();
            }
        }
    }
    let to = std::env::temp_dir().join(format!("rustpg2d-core-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&to);
    copy(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../example_project"), &to);
    to
}

/// Every file below `dir` with its bytes.
fn snapshot(dir: &Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
    fn walk(dir: &Path, root: &Path, out: &mut std::collections::BTreeMap<PathBuf, Vec<u8>>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, root, out);
            } else {
                out.insert(p.strip_prefix(root).unwrap().to_path_buf(), std::fs::read(&p).unwrap());
            }
        }
    }
    let mut out = Default::default();
    walk(dir, dir, &mut out);
    out
}

#[test]
fn a_change_undone_and_saved_leaves_the_files_as_they_were() {
    let dir = example_copy("undo");
    let before = snapshot(&dir);
    let mut ws = Workspace::new(dir.clone(), Gpu::No).unwrap();
    assert!(ws.project_open && ws.maps.contains(&"village".to_string()));
    assert_eq!(ws.doc.as_ref().map(|d| d.map.name.as_str()), Some(ws.project.config.start_map.as_str()));
    ws.history_frame(None);

    // A definition's name, as a form would change it; then a second definition.
    let first = ws.project.db.items[0].name.clone();
    ws.project.db.items[0].name = "Changed".into();
    ws.history_frame(Some(7));
    ws.project.db.items[1].name = "Changed too".into();
    ws.history_frame(None);
    assert!(ws.can_undo() && ws.defs_dirty && ws.is_dirty());
    assert_eq!(ws.timeline.status(), "2/0");
    ws.undo();
    assert_eq!(ws.project.db.items[0].name, "Changed");
    ws.undo();
    assert_eq!(ws.project.db.items[0].name, first);
    assert!(!ws.defs_dirty, "back at the saved definitions");
    ws.redo();
    assert_eq!(ws.project.db.items[0].name, "Changed");
    ws.undo();

    // A map edit and its undo.
    let doc = ws.doc.as_mut().unwrap();
    let was = doc.map.collision[0];
    doc.paint_collision(0, 0, if was == 1 { 2 } else { 1 });
    doc.end_stroke();
    assert!(ws.is_dirty());
    ws.begin("Paint collision");
    ws.history_frame(None);
    assert_eq!(ws.timeline.done.last().map(|c| c.label.as_str()), Some("Paint collision"));
    assert!(ws.undo().is_some_and(|m| m.contains("Paint collision") && m.contains("map ")));
    assert!(!ws.is_dirty());

    ws.save_all();
    assert_eq!(snapshot(&dir), before, "nothing was written differently");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn two_maps_stay_open_with_their_changes() {
    let dir = example_copy("maps");
    let mut ws = Workspace::new(dir.clone(), Gpu::No).unwrap();
    ws.open_map("house");
    ws.doc.as_mut().unwrap().paint_collision(1, 1, 1);
    ws.doc.as_mut().unwrap().end_stroke();
    ws.open_map("village");
    assert_eq!(ws.doc.as_ref().unwrap().map.name, "village");
    assert!(ws.parked.get("house").is_some_and(|d| d.is_dirty()), "the house keeps its unsaved change");
    assert!(ws.open_maps().len() >= 2);
    ws.open_map("house");
    assert!(ws.doc.as_ref().unwrap().can_undo(), "and its history");
    ws.save_all();
    assert!(!ws.is_dirty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn renaming_an_item_reaches_maps_and_scripts_and_undoes() {
    let dir = example_copy("rename");
    let mut ws = Workspace::new(dir.clone(), Gpu::No).unwrap();
    ws.history_frame(None);
    assert!(ws.project.db.has(RefKind::Item, "potion"));
    let msg = ws.try_rename_id(RefKind::Item, "potion", "healing_draught").unwrap();
    ws.history_frame(None);
    assert!(msg.contains("reference(s) updated"), "{msg}");
    assert!(ws.project.db.has(RefKind::Item, "healing_draught") && !ws.project.db.has(RefKind::Item, "potion"));
    assert!(ws.try_rename_id(RefKind::Item, "healing_draught", "bad id!").is_err());
    assert!(ws.try_rename_id(RefKind::Item, "healing_draught", "apple").is_err(), "taken");
    ws.undo();
    assert!(ws.project.db.has(RefKind::Item, "potion") && !ws.project.db.has(RefKind::Item, "healing_draught"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn one_history_over_maps_and_definitions() {
    let dir = example_copy("timeline");
    let mut ws = Workspace::new(dir.clone(), Gpu::No).unwrap();
    ws.history_frame(None);
    ws.open_map("house");
    // A map edit, a definition edit, another map's edit: three entries, in that order.
    ws.doc.as_mut().unwrap().paint_collision(2, 2, 1);
    ws.doc.as_mut().unwrap().end_stroke();
    ws.history_frame(None);
    let name = ws.project.db.items[0].name.clone();
    ws.begin("Rename the first item");
    ws.project.db.items[0].name = "Other".into();
    ws.history_frame(None);
    ws.open_map("village");
    ws.doc.as_mut().unwrap().paint_collision(3, 3, 1);
    ws.doc.as_mut().unwrap().end_stroke();
    ws.history_frame(None);
    let labels: Vec<String> = ws.timeline.done.iter().map(|c| format!("{}@{}", c.label, c.at.describe())).collect();
    assert_eq!(labels, ["Edit map@map house", "Rename the first item@definitions", "Edit map@map village"]);

    // A document's own undo leaves the later changes elsewhere alone.
    use rustpg2d_editor_core::timeline::Where;
    assert!(ws.undo_at(&Where::Map("house".into())).is_some());
    assert!(!ws.parked["house"].is_dirty() && ws.doc.as_ref().unwrap().is_dirty());
    assert_eq!(ws.project.db.items[0].name, "Other");
    // Undo takes the latest wherever it was: the village, then the item.
    assert!(ws.undo().unwrap().contains("map village"));
    assert!(ws.undo().unwrap().contains("Rename the first item"));
    assert_eq!(ws.project.db.items[0].name, name);
    assert!(!ws.is_dirty() && !ws.can_undo() && ws.can_redo());
    // And back again.
    while ws.redo().is_some() {}
    assert_eq!(ws.project.db.items[0].name, "Other");
    assert!(ws.parked["house"].is_dirty() && ws.doc.as_ref().unwrap().is_dirty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn commands_run_by_name_and_name_their_changes() {
    let dir = example_copy("commands");
    let mut ws = Workspace::new(dir.clone(), Gpu::No).unwrap();
    assert!(ws.run_line("map.open house").unwrap().contains("house"));
    assert!(ws.run_line("map.open nowhere").is_err());
    assert!(ws.run_line("no.such").is_err());
    // A rename is one history entry under the command's title, and undo takes it back.
    ws.run_line("edit.rename item potion healing_draught").unwrap();
    assert_eq!(ws.timeline.done.last().map(|c| c.label.as_str()), Some("Rename"));
    assert!(ws.project.db.has(RefKind::Item, "healing_draught"));
    let hits = ws.run_line("find.search healing_draught").unwrap();
    assert!(hits.contains("hit(s)") && hits.contains("healing_draught"), "{hits}");
    assert!(ws.run_line("edit.undo").unwrap().contains("Rename"));
    assert!(ws.project.db.has(RefKind::Item, "potion"));
    let usages = ws.run_line("find.usages item potion").unwrap();
    assert!(usages.contains("usage(s) of item"), "{usages}");
    assert!(ws.run_line("edit.undo").is_err(), "nothing left");
    let problems = ws.run_line("check.problems").unwrap();
    assert!(problems.starts_with("0 error(s)"), "the example has no errors: {problems}");
    assert!(ws.run_line("file.save_all").is_ok() && !ws.is_dirty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tags_and_folders_are_written_follow_renames_and_undo() {
    use rustpg2d_editor_core::meta::{Meta, FILE};
    let dir = example_copy("meta");
    let mut ws = Workspace::new(dir.clone(), Gpu::No).unwrap();
    ws.history_frame(None);
    assert!(ws.meta.assets.is_empty() && !dir.join(FILE).exists());

    // Commands, as the palette, a script or the window issue them.
    ws.run_line("meta.tag item potion Act 1").unwrap();
    ws.run_line("meta.tag item potion act 1").unwrap();
    ws.run_line("meta.folder item potion  Shop / Potions ").unwrap();
    ws.run_line("meta.favorite map village").unwrap();
    assert!(ws.run_line("meta.tag item no_such_item x").is_err());
    let potion = ws.meta.of(RefKind::Item, "potion").unwrap().clone();
    assert_eq!((potion.tags.as_slice(), potion.folder.as_str()), (["Act 1".to_string()].as_slice(), "Shop/Potions"));
    assert_eq!(Meta::load(&dir), ws.meta, "written at once");
    assert!(!ws.is_dirty(), "notes aren't unsaved changes");

    // A copy starts with the folder and tags; a rename takes them along.
    ws.run_line("defs.duplicate item potion big_potion").unwrap();
    assert_eq!(ws.meta.of(RefKind::Item, "big_potion").unwrap().tags, ["Act 1"]);
    ws.run_line("edit.rename item big_potion grand_potion").unwrap();
    assert!(ws.meta.of(RefKind::Item, "big_potion").is_none());
    assert_eq!(ws.meta.of(RefKind::Item, "grand_potion").unwrap().folder, "Shop/Potions");

    // Every step undoes: the rename, the copy (with what was noted for it), the favourite…
    ws.run_line("edit.undo").unwrap();
    assert!(ws.meta.of(RefKind::Item, "big_potion").is_some() && ws.meta.of(RefKind::Item, "grand_potion").is_none());
    ws.run_line("edit.undo").unwrap();
    assert!(!ws.project.db.has(RefKind::Item, "big_potion") && ws.meta.of(RefKind::Item, "big_potion").is_none());
    ws.run_line("edit.undo").unwrap();
    assert!(ws.meta.of(RefKind::Map, "village").is_none());
    ws.run_line("edit.undo").unwrap();
    assert_eq!(ws.meta.of(RefKind::Item, "potion").unwrap().folder, "");
    ws.run_line("edit.undo").unwrap();
    assert!(ws.meta.assets.is_empty());
    ws.run_line("edit.redo").unwrap();
    assert_eq!(ws.meta.of(RefKind::Item, "potion").unwrap().tags, ["Act 1"]);
    assert_eq!(Meta::load(&dir), ws.meta);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_new_definition_is_made_from_its_name() {
    let dir = example_copy("make");
    let mut ws = Workspace::new(dir.clone(), Gpu::No).unwrap();
    ws.history_frame(None);
    // Blank, with the name; the id is made from it, and numbered when taken.
    assert_eq!(ws.run_line("defs.make item Moon Blade").unwrap(), "Made item \"moon_blade\"");
    assert_eq!(ws.project.db.item("moon_blade").unwrap().name, "Moon Blade");
    assert_eq!(ws.make_def(RefKind::Item, "Moon Blade", None).unwrap(), "moon_blade_2");
    // A copy of another, renamed; a quest's name is its title.
    ws.run_line("defs.make item from:iron_sword Sun Blade").unwrap();
    let (sun, iron) = (ws.project.db.item("sun_blade").unwrap().clone(), ws.project.db.item("iron_sword").unwrap().clone());
    assert_eq!((sun.name.as_str(), &sun.weapon, &sun.icon), ("Sun Blade", &iron.weapon, &iron.icon));
    let quest = ws.make_def(RefKind::Quest, "Find the Cat", None).unwrap();
    assert_eq!(ws.project.db.quests.iter().find(|q| q.id == quest).unwrap().title, "Find the Cat");
    assert!(ws.make_def(RefKind::Item, "  ", None).is_err());
    // Each is one step.
    for _ in 0..4 {
        ws.run_line("edit.undo").unwrap();
    }
    assert!(ws.project.db.item("moon_blade").is_none() && ws.project.db.item("sun_blade").is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn settings_change_as_data_are_saved_and_undone() {
    let dir = example_copy("settings");
    let mut ws = Workspace::new(dir.clone(), Gpu::No).unwrap();
    ws.history_frame(None);
    let before = ws.project.config.clone();
    // A setting of every page of the settings (`schema::PAGES`), changed as a form does.
    let mut value = ws.config_value();
    value["name"] = "Another Game".into();
    value["inventory_size"] = 31.into();
    value["rules"]["show_checks"] = (!before.rules.show_checks).into();
    value["day_minutes"] = 30.0.into();
    value["dialog_portraits"] = (!before.dialog_portraits).into();
    value["battle_music"] = "".into();
    value["languages"] = serde_json::json!(["de", "fr"]);
    ws.set_config_value(value).unwrap();
    ws.history_frame(None);
    assert!(ws.is_dirty() && ws.timeline.status().starts_with("1/"));
    // What isn't valid settings changes nothing, and says why.
    let mut bad = ws.config_value();
    bad["inventory_size"] = "many".into();
    let e = ws.set_config_value(bad).unwrap_err();
    assert!(e.contains("inventory_size") || e.contains("invalid type"), "{e}");
    assert_eq!(ws.project.config.inventory_size, 31);
    // Saved, a second workspace reads the same.
    ws.save_all();
    let again = Workspace::new(dir.clone(), Gpu::No).unwrap();
    assert_eq!(again.project.config, ws.project.config);
    assert_eq!((again.project.config.name.as_str(), again.project.config.day_minutes, again.project.config.languages.len()), ("Another Game", 30.0, 2));
    // One step back: as it was.
    ws.run_line("edit.undo").unwrap();
    assert_eq!(ws.project.config, before);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn things_are_placed_changed_and_removed_on_a_map_step_by_step() {
    use rustpg2d_editor_core::workspace::Selection;
    use rustpg2d_engine::entity::EntityKind;
    let dir = example_copy("mapedit");
    let mut ws = Workspace::new(dir.clone(), Gpu::No).unwrap();
    ws.history_frame(None);
    ws.open_map("village");
    let ts = ws.ts();
    let (entities, zones) = (ws.doc.as_ref().unwrap().map.entities.len(), ws.doc.as_ref().unwrap().map.zones.len());

    // Placing: on the grid, named by what was placed.
    let feet = ws.snap_feet(EntityKind::Character, "guard", gfx::math::vec2(40.3 * ts, 30.6 * ts), false);
    let uid = ws.place_entity(EntityKind::Character, "guard", feet).unwrap();
    ws.history_frame(None);
    assert_eq!(ws.timeline.done.last().unwrap().label, "Place Town Guard");
    assert!(ws.place_entity(EntityKind::Character, "nobody", feet).is_err());
    assert!(ws.place_entity(EntityKind::Prop, "barrel", gfx::math::vec2(-40.0, 10.0)).is_err(), "not outside the map");

    // The Inspector's round trip: a name; a change of what it is is refused.
    let mut value = ws.entity_value(uid).unwrap();
    value["name"] = "night_watch".into();
    ws.set_entity_value(uid, value.clone()).unwrap();
    ws.history_frame(None);
    assert_eq!(ws.find_object("night_watch"), Some(Selection::Entity(uid)));
    value["kind"] = "Prop".into();
    assert!(ws.set_entity_value(uid, value).is_err());

    // A zone, cut to the map; its name must stay its own.
    let kind = rustpg2d_engine::map::ZoneKind::Spawn;
    let zone = ws.add_zone(kind, (78, 58, 5, 5)).unwrap();
    ws.history_frame(None);
    let z = ws.doc.as_ref().unwrap().map.zones.iter().find(|z| z.uid == zone).unwrap().clone();
    assert_eq!((z.x, z.y, z.w, z.h), (78, 58, 2, 2));
    let mut value = ws.zone_value(zone).unwrap();
    value["name"] = ws.doc.as_ref().unwrap().map.zones[0].name.clone().into();
    assert!(ws.set_zone_value(zone, value).unwrap_err().contains("another zone"));

    // Copies stand a tile further and get names of their own; deleting takes them all.
    let copies = ws.duplicate_objects(&[Selection::Entity(uid), Selection::Zone(zone)]);
    ws.history_frame(None);
    assert_eq!(copies.len(), 2);
    assert!(ws.find_object("night_watch_copy").is_some());
    assert_eq!(ws.turn_entity(uid), Ok(()));
    ws.history_frame(None);
    assert_eq!(ws.delete_objects(&copies), 2);
    ws.history_frame(None);
    let labels: Vec<&str> = ws.timeline.done.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(labels, ["Place Town Guard", "Change night_watch", "Add a spawn zone", "Duplicate 2 things", "Turn night_watch", "Delete 2 things"]);

    // Everything undone: the map is as it was, and clean.
    while ws.undo().is_some() {}
    let map = &ws.doc.as_ref().unwrap().map;
    assert_eq!((map.entities.len(), map.zones.len()), (entities, zones));
    assert!(!ws.is_dirty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn scenery_ground_scatter_and_structures_are_steps_of_a_map() {
    use rustpg2d_editor_core::mapedit::Brush;
    use rustpg2d_editor_core::workspace::Selection;
    use gfx::math::vec2;
    let dir = example_copy("scenery");
    let mut ws = Workspace::new(dir.clone(), Gpu::No).unwrap();
    ws.history_frame(None);
    ws.open_map("village");
    let ts = ws.ts();
    let count = |ws: &Workspace| (ws.doc.as_ref().unwrap().map.entities.len(), ws.doc.as_ref().unwrap().map.decor.len());
    let (entities, decor) = count(&ws);

    // A prop that does something is an object; one that only stands there is scenery.
    assert!(ws.prop_acts("chest") && !ws.prop_acts("small_rock"));
    let rock = ws.place_scenery("small_rock", vec2(22.5 * ts, 52.5 * ts), None, false, 0).unwrap();
    ws.history_frame(None);
    assert_eq!(count(&ws), (entities, decor + 1));
    // Scenery becomes an object on the same spot and back; a named object can't.
    let uid = ws.make_interactive(rock).unwrap();
    ws.history_frame(None);
    assert_eq!(count(&ws), (entities + 1, decor));
    let e = ws.doc.as_ref().unwrap().map.entity(uid).unwrap().clone();
    assert_eq!((e.def.as_str(), e.x as i32, e.y as i32), ("small_rock", rock.x, rock.y));
    let mut named = ws.entity_value(uid).unwrap();
    named["name"] = "marker".into();
    ws.set_entity_value(uid, named.clone()).unwrap();
    assert!(ws.make_scenery(uid).unwrap_err().contains("name"));
    named["name"] = "".into();
    ws.set_entity_value(uid, named).unwrap();
    ws.history_frame(None);
    let rock = ws.make_scenery(uid).unwrap();
    ws.history_frame(None);
    // Moved, scenery is told by where it is now.
    let moved = ws.move_objects(&[Selection::Scenery(rock)], 2, 0);
    ws.history_frame(None);
    let [Selection::Scenery(now)] = moved.as_slice() else { panic!("{moved:?}") };
    assert_eq!((now.x, now.y), (rock.x + 2 * ts as i32, rock.y));
    assert!(ws.doc.as_ref().unwrap().map.decor.contains(now) && !ws.doc.as_ref().unwrap().map.decor.contains(&rock));

    // The ground: a rectangle on tiles is 2 x 2 tiles of cells, one step named by its material.
    let brush = Brush { size: 16.0, square: false, snap: true };
    let value = ws.ground_value("cobblestone").unwrap();
    let cells = ws.ground_rect(brush, vec2(20.1 * ts, 50.2 * ts), vec2(21.9 * ts, 51.8 * ts), value);
    let per_tile = (rustpg2d_engine::ground::CELLS * rustpg2d_engine::ground::CELLS) as usize;
    let painted = ws.ground_commit(cells, value);
    ws.history_frame(None);
    assert!(painted > 0 && painted <= 4 * per_tile, "{painted}");
    assert_eq!(ws.ground_at(vec2(21.0 * ts, 51.0 * ts)).as_deref(), Some("cobblestone"));
    assert_eq!(ws.timeline.done.last().unwrap().label, "Paint Cobblestone");
    assert!(ws.ground_value("no_such_material").is_err());

    // A scatter stroke of several dabs is one step; a structure is one step.
    let snap = ws.snap_map().unwrap();
    let n = ws.scatter("forest", vec2(10.0 * ts, 54.0 * ts), 96.0, 7) + ws.scatter("forest", vec2(12.0 * ts, 54.0 * ts), 96.0, 8);
    assert!(n > 1);
    assert!(ws.commit_map(*snap, "Scatter Forest"));
    ws.history_frame(None);
    assert_eq!(count(&ws).1, decor + 1 + n);
    ws.place_structure("cottage_small", (30, 46)).unwrap();
    ws.history_frame(None);
    let labels: Vec<&str> = ws.timeline.done.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(labels.last().copied(), Some("Place Small cottage"));
    assert_eq!(labels[labels.len() - 2], "Scatter Forest");

    // Everything undone: as it was.
    while ws.undo().is_some() {}
    assert_eq!(count(&ws), (entities, decor));
    assert_eq!(ws.ground_at(vec2(21.0 * ts, 51.0 * ts)).as_deref(), Some("grass"));
    assert!(!ws.is_dirty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_world_is_surveyed_and_two_maps_are_linked() {
    use rustpg2d_editor_core::world::Way;
    use rustpg2d_engine::map::ZoneKind;
    let dir = example_copy("world");
    let mut ws = Workspace::new(dir.clone(), Gpu::No).unwrap();
    ws.history_frame(None);
    // The example: every map is reached from the village, every way leads somewhere.
    let world = ws.survey_world();
    assert_eq!(world.len(), ws.maps.len());
    let village = world.iter().find(|m| m.name == "village").unwrap();
    assert_eq!(village.distance, Some(0));
    assert!(village.links.iter().any(|l| l.way == Way::Portal && l.to_map == "house" && l.to.is_some()));
    assert_eq!(village.picture.len(), (village.width * village.height * 4) as usize);
    let wrong: Vec<String> = world.iter().flat_map(|m| m.links.iter().filter(|l| !l.problem.is_empty()).map(move |l| format!("{}:{} {}", m.name, l.zone, l.problem))).collect();
    assert!(wrong.is_empty(), "{wrong:?}");
    assert!(world.iter().all(|m| m.reached && m.distance.is_some()), "{:?}", world.iter().filter(|m| !m.reached).map(|m| &m.name).collect::<Vec<_>>());

    // A transition drawn on each of two maps: unlinked they are a problem, linked they are a pair.
    let blank = ZoneKind::Transition { target_map: String::new(), target_zone: String::new() };
    ws.open_map("house");
    let a = ws.add_zone(blank.clone(), (3, 3, 2, 1)).unwrap();
    let a = ws.describe_object(rustpg2d_editor_core::workspace::Selection::Zone(a));
    ws.open_map("village");
    let b = ws.add_zone(blank, (22, 50, 2, 1)).unwrap();
    let b = ws.describe_object(rustpg2d_editor_core::workspace::Selection::Zone(b));
    ws.history_frame(None);
    let world = ws.survey_world();
    assert_eq!(world.iter().flat_map(|m| &m.links).filter(|l| l.problem.contains("nowhere")).count(), 2);
    assert!(ws.link_transitions(("house", &a), ("house", &a)).is_err(), "not a map with itself");
    ws.link_transitions(("house", &a), ("village", &b)).unwrap();
    ws.history_frame(None);
    assert_eq!(ws.doc.as_ref().unwrap().map.name, "village", "the map being worked on stays");
    let world = ws.survey_world();
    let pair: Vec<_> = world.iter().flat_map(|m| m.links.iter().map(move |l| (m.name.as_str(), l))).filter(|(_, l)| l.way == Way::Transition && (l.zone == a || l.zone == b)).collect();
    assert_eq!(pair.len(), 2);
    assert!(pair.iter().all(|(_, l)| l.problem.is_empty() && l.to.is_some()), "{pair:?}");
    assert_eq!(ws.timeline.done.iter().rev().take(2).map(|c| c.label.as_str()).collect::<Vec<_>>(), ["Link house and village"; 2]);
    // A map nothing leads to is told.
    while ws.undo().is_some() {}
    assert!(!ws.is_dirty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn walls_rooms_houses_and_roofs_are_built_as_one_step_each() {
    use gfx::prelude::{vec2, Rect};
    use rustpg2d_editor_core::build::{self, Put};
    use rustpg2d_engine::defs::RoofKind;
    let dir = example_copy("build");
    let mut ws = Workspace::new(dir.clone(), Gpu::No).unwrap();
    ws.history_frame(None);
    ws.open_map("village");
    let ts = ws.ts();
    let db = &ws.project.db;
    let pick = |keep: &dyn Fn(&rustpg2d_engine::defs::PropDef) -> bool| db.props.iter().find(|p| keep(p)).map(|p| p.id.clone()).unwrap();
    let thin = pick(&|p| build::is_wall(p) && build::thin(p));
    let thick = pick(&|p| build::is_wall(p) && !build::thin(p));
    let house = pick(&build::is_house);
    let door = pick(&|p| p.door);
    let window = pick(&build::is_window);
    let props = db.props.len();
    let count = |ws: &Workspace| (ws.doc.as_ref().unwrap().map.entities.len(), ws.doc.as_ref().unwrap().map.decor.len(), ws.project.db.props.len());
    let (entities, decor, _) = count(&ws);
    let last = |ws: &Workspace| ws.timeline.done.last().unwrap().label.clone();

    // A thin wall round a corner: three pieces along the edge, two down; again adds nothing.
    assert_eq!(ws.build_walls(&thin, &[(4, 50), (7, 50), (7, 52)]).unwrap(), 5);
    ws.history_frame(None);
    assert!(last(&ws).starts_with("Build 5 pieces of "), "{}", last(&ws));
    assert_eq!(ws.build_walls(&thin, &[(4, 50), (7, 50)]).unwrap(), 0);
    ws.history_frame(None);
    assert_eq!(count(&ws), (entities, decor + 5, props));
    assert!(ws.build_walls("small_rock", &[(1, 1), (2, 1)]).is_err());
    // A piece taken away where the pointer shows it (from above: on its footprint).
    let piece = *ws.doc.as_ref().unwrap().map.decor.last().unwrap();
    let (_, r) = ws.build_footprint(&piece).unwrap();
    ws.take_wall(r.center(), false).unwrap();
    ws.history_frame(None);
    assert_eq!(count(&ws).1, decor + 4);

    // A room of thick walls on all four sides, with a floor: the walls stand around its 4 x 3
    // tiles (a ring of 18 tiles less the doorway in the south). On a site where nothing stands.
    let (ox, oy) = {
        let map = &ws.doc.as_ref().unwrap().map;
        let free = |x: i32, y: i32| {
            let site = Rect::new((x - 2) as f32 * ts, (y - 2) as f32 * ts, 8.0 * ts, 7.0 * ts);
            !map.decor.iter().any(|d| site.contains(vec2(d.x as f32, d.y as f32))) && !map.entities.iter().any(|e| site.contains(vec2(e.x, e.y)))
        };
        (4..map.width as i32 - 8).flat_map(|x| (4..map.height as i32 - 8).map(move |y| (x, y))).find(|&(x, y)| free(x, y)).expect("a free site in the village")
    };
    let at = |x: f32, y: f32| vec2((ox as f32 + x) * ts, (oy as f32 + y) * ts);
    let ground = ws.ground_at(at(1.5, 1.5));
    let room = build::drag_rect(at(0.2, 0.3), at(3.7, 2.5), ts);
    assert_eq!(room, Rect::new(ox as f32 * ts, oy as f32 * ts, 4.0 * ts, 3.0 * ts));
    let before = count(&ws).1;
    assert!(ws.build_room(room, &thick, "cobblestone", true, None).unwrap().contains("17 wall pieces"));
    ws.history_frame(None);
    assert_eq!(last(&ws), "Build a room of 4 x 3 tiles");
    assert_eq!(count(&ws).1, before + 17);
    assert_eq!(ws.ground_at(at(1.5, 1.5)).as_deref(), Some("cobblestone"));
    assert_eq!(ws.ground_at(at(3.5, 2.5)).as_deref(), Some("cobblestone"));
    // Through its doorway the ground isn't closed in; the doorway walled up, its inside is
    // found from a click in it, with the tiles its walls stand on.
    assert!(ws.roof_fill(at(1.5, 1.5)).is_err());
    assert_eq!(ws.build_walls(&thick, &[(ox + 2, oy + 3)]).unwrap(), 1);
    ws.history_frame(None);
    let inside = ws.roof_fill(at(1.5, 1.5));
    assert_eq!(inside.as_ref().map(|c| c.len()), Ok(12 + 14), "{inside:?}");
    assert!(ws.roof_fill(at(-3.5, -3.5)).is_err());
    // A window piece and a door put into its walls: the door is an object.
    let wall_piece = *ws.doc.as_ref().unwrap().map.decor.iter().rev().find(|d| ws.build_footprint(d).is_some_and(|(p, _)| p.id == thick)).unwrap();
    let (_, r) = ws.build_footprint(&wall_piece).unwrap();
    ws.put_opening(r.center(), false, Put::Window, &window).unwrap();
    ws.history_frame(None);
    ws.put_opening(r.center(), false, Put::Door, &door).unwrap();
    ws.history_frame(None);
    assert_eq!(count(&ws).0, entities + 1);
    // The door taken away again: the wall comes back.
    ws.take_opening(r.center(), false, &thick).unwrap();
    ws.history_frame(None);
    assert_eq!(count(&ws).0, entities);

    // A house is a new prop of that size and a thing on the map: one action, undone as one.
    let (_, decor_now, props_now) = count(&ws);
    let done = ws.timeline.done.len();
    let msg = ws.build_house(Rect::new(20.0 * ts, 50.0 * ts, 5.0 * ts, 4.0 * ts), &house, 2).unwrap();
    ws.history_frame(None);
    assert!(msg.contains("_built_1"), "{msg}");
    assert_eq!(count(&ws), (entities, decor_now + 1, props_now + 1));
    assert_eq!(ws.timeline.done.len(), done + 2);
    let built = ws.project.db.props.last().unwrap().clone();
    assert_eq!(built.hitbox, [5.0 * ts, 4.0 * ts]);
    assert!(built.name.contains("5x4, 2 storeys"), "{}", built.name);
    assert!(built.building.as_ref().unwrap().openings.iter().any(|o| o.sill <= 0.0));
    ws.undo().unwrap();
    assert_eq!(count(&ws), (entities, decor_now, props_now));
    ws.redo().unwrap();
    assert_eq!(count(&ws), (entities, decor_now + 1, props_now + 1));
    // A second one against its east wall is its wing: each knows the other.
    ws.build_house(Rect::new(25.0 * ts, 51.0 * ts, 3.0 * ts, 3.0 * ts), &house, 1).unwrap();
    ws.history_frame(None);
    let wing = ws.project.db.props.last().unwrap().clone();
    let joins = &wing.building.as_ref().unwrap().joins;
    assert_eq!(joins.len(), 1);
    assert_eq!(ws.project.db.prop(&joins[0].prop).unwrap().building.as_ref().unwrap().joins[0].prop, wing.id);
    // A door clicked into the first house's south wall (from above the click meets the roof).
    assert!(ws.put_opening(vec2(22.5 * ts, 52.0 * ts), false, Put::Door, "").unwrap_err().contains("roof"));

    // A roof over an L of tiles: two parts, joined; over the room: as high as its walls.
    let l: std::collections::BTreeSet<(i32, i32)> = build::tiles_in(Rect::new(30.0 * ts, 50.0 * ts, 6.0 * ts, 3.0 * ts), ts).into_iter().chain(build::tiles_in(Rect::new(33.0 * ts, 53.0 * ts, 3.0 * ts, 3.0 * ts), ts)).collect();
    let msg = ws.build_roof(&l, &house, RoofKind::Hip, "").unwrap();
    ws.history_frame(None);
    assert!(msg.contains("27 tiles (2 parts)"), "{msg}");
    let roofs: Vec<_> = ws.project.db.props.iter().filter(|p| p.id.contains("_roof_")).collect();
    assert_eq!(roofs.len(), 2);
    assert!(roofs.iter().all(|p| !p.solid && p.building.as_ref().is_some_and(|b| !b.walls && b.joins.len() == 1 && b.roof == RoofKind::Hip)));
    let msg = ws.build_roof(&inside.unwrap(), &house, RoofKind::Gable, "").unwrap();
    ws.history_frame(None);
    let wall_height = ws.project.db.prop(&thick).unwrap().block;
    assert!(msg.ends_with(&format!("at {wall_height} px")), "{msg}");

    // Everything undone: the map and the definitions as they were.
    while ws.undo().is_some() {}
    assert_eq!(count(&ws), (entities, decor, props));
    assert_eq!(ws.ground_at(at(1.5, 1.5)), ground);
    assert!(!ws.is_dirty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn things_are_copied_to_another_map_replaced_and_put_in_a_row() {
    use rustpg2d_editor_core::workspace::Selection;
    let dir = example_copy("clip");
    let mut ws = Workspace::new(dir.clone(), Gpu::No).unwrap();
    ws.history_frame(None);
    ws.open_map("village");
    let ts = ws.ts();
    // The elder, a zone and a piece of scenery of the village, copied.
    let map = &ws.doc.as_ref().unwrap().map;
    let elder = map.entities.iter().find(|e| e.name == "elder").unwrap().clone();
    let zone = map.zones[0].clone();
    let piece = map.decor[0];
    let prop = map.decor_defs[piece.def as usize].clone();
    let what = [Selection::Entity(elder.uid), Selection::Zone(zone.uid), Selection::Scenery(piece)];
    let clip = ws.copy_objects(&what).unwrap();
    assert_eq!((clip.len(), clip.from.as_str()), (3, "village"));
    assert!(ws.copy_objects(&[Selection::Entity(999_999)]).is_none());

    // Pasted on the same map: new numbers, free names, each as far from the others as before.
    let made = ws.paste_objects(&clip, (40, 30));
    ws.history_frame(None);
    assert_eq!(made.len(), 3);
    assert_eq!(ws.timeline.done.last().unwrap().label, "Paste 3 things");
    let map = &ws.doc.as_ref().unwrap().map;
    let [Selection::Entity(e2), Selection::Zone(z2), Selection::Scenery(d2)] = made.as_slice() else { panic!("{made:?}") };
    let (e2, z2) = (map.entity(*e2).unwrap(), map.zones.iter().find(|z| z.uid == *z2).unwrap());
    assert!(e2.uid != elder.uid && e2.name != "elder" && e2.name.starts_with("elder") && e2.def == elder.def);
    assert!(z2.name != zone.name && z2.name.starts_with(&zone.name));
    assert_eq!(((e2.x - elder.x) / ts, (e2.y - elder.y) / ts), ((d2.x - piece.x) as f32 / ts, (d2.y - piece.y) as f32 / ts));
    assert_eq!(map.decor_defs[d2.def as usize], prop);

    // On another map: the scenery's prop is known there by its own number; what would stand
    // outside the map is left out.
    let one = ws.copy_objects(&[Selection::Scenery(piece), Selection::Entity(elder.uid)]).unwrap();
    let alone = ws.copy_objects(&[Selection::Scenery(piece)]).unwrap();
    ws.open_map("house");
    let decor = ws.doc.as_ref().unwrap().map.decor.len();
    let made = ws.paste_objects(&alone, (2, 2));
    ws.history_frame(None);
    let map = &ws.doc.as_ref().unwrap().map;
    let [Selection::Scenery(d)] = made.as_slice() else { panic!("{made:?}") };
    assert_eq!((map.decor.len(), map.decor_defs[d.def as usize].as_str()), (decor + 1, prop.as_str()));
    assert_eq!(ws.timeline.done.last().unwrap().at.describe(), "map house");
    assert!(ws.paste_objects(&one, (5000, 5000)).is_empty());
    // Replaced: the pasted elder becomes a guard where he stands, by his name still.
    ws.open_map("village");
    let Selection::Entity(copy) = ws.doc.as_ref().unwrap().map.entities.iter().rev().find(|e| e.name.starts_with("elder") && e.name != "elder").map(|e| Selection::Entity(e.uid)).unwrap() else { unreachable!() };
    let before = ws.doc.as_ref().unwrap().map.entity(copy).unwrap().clone();
    ws.replace_objects(&[Selection::Entity(copy)], RefKind::Character, "guard").unwrap();
    ws.history_frame(None);
    let after = ws.doc.as_ref().unwrap().map.entity(copy).unwrap();
    assert_eq!((after.def.as_str(), after.name.as_str(), after.x, after.y), ("guard", before.name.as_str(), before.x, before.y));
    assert_eq!(ws.timeline.done.last().unwrap().label, "Replace with Town Guard");
    assert!(ws.replace_objects(&[Selection::Entity(copy)], RefKind::Prop, "chest").is_err());
    assert!(ws.replace_objects(&[Selection::Entity(copy)], RefKind::Character, "no_such_one").is_err());
    // Scenery replaced by another prop is told by what it is now.
    let now = ws.replace_objects(&[Selection::Scenery(piece)], RefKind::Prop, "small_rock").unwrap();
    ws.history_frame(None);
    let [Selection::Scenery(rock)] = now.as_slice() else { panic!("{now:?}") };
    assert_eq!(ws.doc.as_ref().unwrap().map.decor_defs[rock.def as usize], "small_rock");
    assert_eq!((rock.x, rock.y), (piece.x, piece.y));

    // In a row with the first: the same distance from the north edge.
    let now = ws.align_objects(&[Selection::Entity(elder.uid), Selection::Entity(copy), Selection::Scenery(*rock)], true);
    ws.history_frame(None);
    let map = &ws.doc.as_ref().unwrap().map;
    assert_eq!(map.entity(copy).unwrap().y, elder.y);
    let [.., Selection::Scenery(rock)] = now.as_slice() else { panic!("{now:?}") };
    assert_eq!(rock.y, elder.y as i32);
    assert_eq!(ws.timeline.done.last().unwrap().label, "Put in a row");

    while ws.undo().is_some() {}
    assert!(!ws.is_dirty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_characters_route_and_day_are_changed_on_the_map() {
    use rustpg2d_editor_core::mapedit::Spot;
    let dir = example_copy("route");
    let mut ws = Workspace::new(dir.clone(), Gpu::No).unwrap();
    ws.history_frame(None);
    ws.open_map("village");
    let elder = ws.doc.as_ref().unwrap().map.entities.iter().find(|e| e.name == "elder").unwrap().clone();
    assert!(elder.patrol.is_none() && !elder.schedule.is_empty());
    let stops = elder.schedule.iter().filter(|s| s.map.is_empty() || s.map == "village").count();
    assert_eq!(ws.spots_of(elder.uid).len(), stops);

    // Three clicks are three waypoints, each a step; the same tile twice in a row is refused.
    assert_eq!(ws.add_waypoint(elder.uid, (30, 30)), Ok(1));
    ws.history_frame(None);
    assert_eq!(ws.add_waypoint(elder.uid, (33, 30)), Ok(2));
    ws.history_frame(None);
    assert!(ws.add_waypoint(elder.uid, (33, 30)).is_err());
    assert!(ws.add_waypoint(elder.uid, (-1, 30)).is_err());
    assert_eq!(ws.add_waypoint(elder.uid, (33, 33)), Ok(3));
    ws.history_frame(None);
    assert_eq!(ws.timeline.done.last().unwrap().label, "Add a waypoint for elder");
    assert_eq!(ws.spots_of(elder.uid).len(), stops + 3);
    // A waypoint dragged: many moves, one step; a stop of his day likewise.
    let snap = ws.snap_map().unwrap();
    ws.set_spot(elder.uid, Spot::Waypoint(1), (34, 30));
    ws.set_spot(elder.uid, Spot::Waypoint(1), (35, 30));
    let label = ws.spot_label(elder.uid, Spot::Waypoint(1));
    assert!(ws.commit_map(*snap, &label));
    ws.history_frame(None);
    assert_eq!(label, "Move waypoint 2 of elder");
    let snap = ws.snap_map().unwrap();
    ws.set_spot(elder.uid, Spot::Stop(0), (20, 20));
    let label = ws.spot_label(elder.uid, Spot::Stop(0));
    assert!(label.starts_with("Move elder's ") && label.ends_with(" stop"), "{label}");
    assert!(ws.commit_map(*snap, &label));
    ws.history_frame(None);
    let now = ws.doc.as_ref().unwrap().map.entity(elder.uid).unwrap().clone();
    assert_eq!(now.patrol.as_ref().unwrap().points, [(30, 30), (35, 30), (33, 33)]);
    assert_eq!((now.schedule[0].x, now.schedule[0].y), (20, 20));
    // The last waypoint taken away, three times: the route goes with the last of them.
    assert!(ws.remove_waypoint(elder.uid) && ws.remove_waypoint(elder.uid) && ws.remove_waypoint(elder.uid));
    assert!(ws.doc.as_ref().unwrap().map.entity(elder.uid).unwrap().patrol.is_none());
    assert!(!ws.remove_waypoint(elder.uid));
    // A prop walks no route.
    let prop = ws.doc.as_ref().unwrap().map.entities.iter().find(|e| e.kind == rustpg2d_engine::entity::EntityKind::Prop).unwrap().uid;
    assert!(ws.add_waypoint(prop, (30, 30)).is_err());

    while ws.undo().is_some() {}
    assert_eq!(*ws.doc.as_ref().unwrap().map.entity(elder.uid).unwrap(), elder);
    assert!(!ws.is_dirty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_maps_card_place_is_noted_with_the_creators_order() {
    let dir = example_copy("cards");
    let mut ws = Workspace::new(dir.clone(), Gpu::No).unwrap();
    ws.history_frame(None);
    let done = ws.timeline.done.len();
    ws.set_card_place("village", Some([120.0, 80.0]));
    ws.history_frame(None);
    // Written at once, read again, and no step of the history.
    assert_eq!(ws.timeline.done.len(), done);
    let again = Workspace::new(dir.clone(), Gpu::No).unwrap();
    assert_eq!(again.meta.of(RefKind::Map, "village").and_then(|m| m.card), Some([120.0, 80.0]));
    ws.set_card_place("village", None);
    assert!(Workspace::new(dir.clone(), Gpu::No).unwrap().meta.of(RefKind::Map, "village").is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn flags_and_variables_are_surveyed_and_renamed_everywhere() {
    use rustpg2d_editor_core::names::{How, NameKind};
    let dir = example_copy("names");
    let mut ws = Workspace::new(dir.clone(), Gpu::No).unwrap();
    ws.refresh_refs();
    ws.history_frame(None);
    let started = std::time::Instant::now();
    let all = ws.survey_names();
    println!("surveyed {} names in {:?}", all.len(), started.elapsed());
    let get = |all: &[rustpg2d_editor_core::names::Named], kind: NameKind, name: &str| all.iter().find(|n| n.kind == kind && n.name == name).cloned();
    // The example's shrine: its flag is set and read, in scripts and beyond them.
    let shrine = get(&all, NameKind::Flag, "shrine_lit").expect("the example has the flag shrine_lit");
    assert!(shrine.count(How::Set) > 0 && shrine.count(How::Read) > 0, "{shrine:?}");
    assert!(shrine.warning().is_none());
    assert!(shrine.uses.iter().any(|u| u.place.starts_with("main.rhai:")), "{:?}", shrine.uses);
    let talks = get(&all, NameKind::Variable, "mira_talks").expect("the example has the variable mira_talks");
    assert!(talks.count(How::Set) > 0 && talks.count(How::Read) > 0);
    // Every flag the pickers offer is in the survey.
    for f in &ws.refs.all_flags {
        assert!(get(&all, NameKind::Flag, f).is_some(), "the flag {f} of the triggers isn't surveyed");
    }
    let script = |dir: &Path| std::fs::read_to_string(dir.join("scripts/main.rhai")).unwrap();
    let before = script(&dir);
    let uses = shrine.uses.len();

    // Renamed: every place follows, the old name is nowhere, one step.
    assert!(ws.rename_name(NameKind::Flag, "shrine_lit", "mira_talks_flag two").is_err());
    assert!(ws.rename_name(NameKind::Flag, "no_such_flag", "x").is_err());
    let other = all.iter().find(|n| n.kind == NameKind::Flag && n.name != "shrine_lit").unwrap().name.clone();
    assert!(ws.rename_name(NameKind::Flag, "shrine_lit", &other).unwrap_err().contains("already"));
    let msg = ws.rename_name(NameKind::Flag, "shrine_lit", "shrine_awake").unwrap();
    ws.history_frame(None);
    assert!(msg.contains(&format!("in {uses} places")), "{msg} (surveyed {uses})");
    assert_eq!(ws.timeline.done.last().unwrap().label, "Rename the flag shrine_lit");
    let now = ws.survey_names();
    assert!(get(&now, NameKind::Flag, "shrine_lit").is_none());
    assert_eq!(get(&now, NameKind::Flag, "shrine_awake").unwrap().uses.len(), uses);
    // (The event of the same name the script raises is no flag: it keeps its name.)
    assert!(script(&dir).contains("set_flag(\"shrine_awake\", true)") && !script(&dir).contains("flag(\"shrine_lit\")") && script(&dir).contains("emit(\"shrine_lit\")"));
    // A fresh look at the project from its files agrees (the rename was written).
    let again = Workspace::new(dir.clone(), Gpu::No).unwrap();
    assert_eq!(get(&again.survey_names(), NameKind::Flag, "shrine_awake").unwrap().uses.len(), uses);

    // A note about it, kept with the project, follows a rename.
    assert!(ws.set_name_note(NameKind::Flag, "shrine_awake", "Set when the shrine is touched at night."));
    ws.history_frame(None);
    ws.rename_name(NameKind::Flag, "shrine_awake", "shrine_woken").unwrap();
    ws.history_frame(None);
    assert_eq!(ws.name_note(NameKind::Flag, "shrine_woken"), "Set when the shrine is touched at night.");

    // Undone: the old name everywhere again, the script as it was.
    while ws.undo().is_some() {}
    let back = ws.survey_names();
    assert_eq!(get(&back, NameKind::Flag, "shrine_lit").unwrap().uses.len(), uses);
    assert!(get(&back, NameKind::Flag, "shrine_woken").is_none());
    assert_eq!(script(&dir), before);
    let _ = std::fs::remove_dir_all(&dir);
}
