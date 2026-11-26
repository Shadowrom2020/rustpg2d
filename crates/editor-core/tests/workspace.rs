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
