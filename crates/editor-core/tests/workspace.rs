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
    assert!(ws.db_hist.can_undo() && ws.defs_dirty && ws.is_dirty());
    ws.db_undo();
    assert_eq!(ws.project.db.items[0].name, "Changed");
    ws.db_undo();
    assert_eq!(ws.project.db.items[0].name, first);
    assert!(!ws.defs_dirty, "back at the saved definitions");

    // A map edit and its undo.
    let doc = ws.doc.as_mut().unwrap();
    let was = doc.map.collision[0];
    doc.paint_collision(0, 0, if was == 1 { 2 } else { 1 });
    doc.end_stroke();
    assert!(ws.is_dirty());
    ws.map_undo();
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
    ws.db_undo();
    assert!(ws.project.db.has(RefKind::Item, "potion") && !ws.project.db.has(RefKind::Item, "healing_draught"));
    let _ = std::fs::remove_dir_all(&dir);
}
