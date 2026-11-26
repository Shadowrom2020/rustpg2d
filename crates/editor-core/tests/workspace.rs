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
