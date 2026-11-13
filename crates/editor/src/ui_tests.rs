//! Clicks and typing in the editor's forms, without a window: `egui_kittest` drives the
//! Database view of a headless [`Editor`] (no textures, sounds or models) through egui's
//! AccessKit tree, as a person would, and the tests check the definitions, undo and the saved
//! files. Each test works on a copy of the example project's definitions in a temporary folder.

use std::path::{Path, PathBuf};

use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use egui_macroquad::egui;
use rustpg2d_engine::project::Project;

use crate::app::Editor;
use crate::database::{self, Tab};

/// A copy of the example's project file, definitions, scripts and translations (no art: the
/// headless editor loads none).
fn example_copy(name: &str) -> PathBuf {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../example_project");
    let dst = std::env::temp_dir().join(format!("rustpg2d_ui_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dst);
    fn copy(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for e in std::fs::read_dir(from).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                copy(&p, &to.join(e.file_name()));
            } else {
                std::fs::copy(&p, to.join(e.file_name())).unwrap();
            }
        }
    }
    std::fs::create_dir_all(&dst).unwrap();
    std::fs::copy(src.join("project.ron"), dst.join("project.ron")).unwrap();
    for dir in ["defs", "scripts", "lang", "maps"] {
        if src.join(dir).is_dir() {
            copy(&src.join(dir), &dst.join(dir));
        }
    }
    dst
}

/// The Database view of a headless editor on `root`, on `tab` with definition `sel` chosen.
fn harness(root: &Path, tab: Tab, sel: usize) -> Harness<'static, Editor> {
    let mut ed = Editor::headless(Project::load(root).unwrap());
    ed.db.tab = tab;
    ed.db.sel = sel;
    Harness::builder().with_size(egui::vec2(1400.0, 2400.0)).build_ui_state(
        |ui, ed: &mut Editor| {
            database::ui(ed, ui);
            ed.db_history_frame(ui.ctx());
        },
        ed,
    )
}

/// A few frames (the character preview animates, so the UI never stops repainting).
fn step(h: &mut Harness<'static, Editor>) {
    h.run_steps(4);
}

/// The text field showing `text` (the list beside the form shows names too).
fn field<'t>(h: &'t Harness<'static, Editor>, text: &'t str) -> egui_kittest::kittest::Node<'t> {
    h.get_by(|n| n.role() == Role::TextInput && n.value().as_deref() == Some(text))
}

/// The one node matching `f` on the same line as the label `label` (egui's accessibility tree
/// is flat: a form's rows are found by their position).
fn beside<'t>(h: &'t Harness<'static, Editor>, label: &'t str, f: impl Fn(&egui_kittest::kittest::Node<'_>) -> bool + 't) -> egui_kittest::kittest::Node<'t> {
    let line = h.get_by_label(label).bounding_box().expect("the label is on screen");
    let mut found: Vec<_> = h.query_all_by(move |n| f(n) && n.bounding_box().is_some_and(|b| b.y0 < line.y1 && b.y1 > line.y0)).collect();
    assert_eq!(found.len(), 1, "one match beside {label:?}");
    found.remove(0)
}

/// Leaves the text field that has the keyboard (as clicking elsewhere would).
fn unfocus(h: &mut Harness<'static, Editor>) {
    h.press_key(egui::Key::Escape);
    step(h);
}

fn undo(h: &mut Harness<'static, Editor>) {
    h.press_key_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    step(h);
}

#[test]
fn footstep_picks_give_and_take_a_tiles_sound() {
    use rustpg2d_engine::defs::TilesetDef;
    let mut d = TilesetDef::new("t", "t.png");
    crate::database::set_tile_step(&mut d, 6, "step_wood");
    crate::database::set_tile_step(&mut d, 18, "step_wood");
    assert_eq!(d.step_of(6), Some("step_wood"));
    // Another sound moves the tile; the same sound again takes it away; empty groups go.
    crate::database::set_tile_step(&mut d, 6, "step_stone");
    assert_eq!((d.step_of(6), d.steps.len()), (Some("step_stone"), 2));
    crate::database::set_tile_step(&mut d, 6, "step_stone");
    crate::database::set_tile_step(&mut d, 18, "step_wood");
    assert!(d.steps.is_empty());
}

#[test]
fn quest_stage_targets_are_edited_and_saved() {
    let root = example_copy("quest");
    let db = Project::load(&root).unwrap().db;
    let k = db.quests.iter().position(|q| q.id == "strange_lights").expect("the example's first quest");
    assert_eq!(db.quests[k].targets[2].name, "elder");
    let mut h = harness(&root, Tab::Quests, k);
    step(&mut h);
    // Stage 2 leads to the elder: point it at his bed instead.
    field(&h, "elder").type_text("_bed");
    step(&mut h);
    unfocus(&mut h);
    assert_eq!(h.state().project.db.quests[k].targets[2].name, "elder_bed");
    h.state_mut().save_all();
    let saved = Project::load(&root).unwrap().db;
    assert_eq!((saved.quests[k].targets[2].map.as_str(), saved.quests[k].targets[2].name.as_str()), ("village", "elder_bed"));
    // The other stages keep theirs.
    assert_eq!(saved.quests[k].targets[0].name, "gate_key_chest");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn item_form_edits_save_and_undo() {
    let root = example_copy("item");
    let db = Project::load(&root).unwrap().db;
    let k = db.items.iter().position(|i| i.id == "sword").expect("the example's sword");
    let (name, food) = (db.items[k].name.clone(), db.items[k].food);
    let mut h = harness(&root, Tab::Items, k);
    step(&mut h);
    // Type behind the name, tick Food.
    field(&h, &name).type_text(" of Tests");
    step(&mut h);
    unfocus(&mut h);
    h.get_by_label("Food").click();
    step(&mut h);
    let item = |h: &Harness<'static, Editor>| h.state().project.db.items[k].clone();
    assert_eq!(item(&h).name, format!("{name} of Tests"));
    assert_eq!(item(&h).food, !food);
    assert!(h.state().defs_dirty);
    // Ctrl+Z takes the tick back, then the typing.
    undo(&mut h);
    assert_eq!(item(&h).food, food);
    assert_eq!(item(&h).name, format!("{name} of Tests"));
    undo(&mut h);
    assert_eq!(item(&h).name, name);
    // Typed again and saved: the file has it.
    field(&h, &name).type_text("!");
    step(&mut h);
    unfocus(&mut h);
    h.state_mut().save_all();
    assert!(!h.state().defs_dirty);
    assert_eq!(Project::load(&root).unwrap().db.items[k].name, format!("{name}!"));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn character_form_takes_a_race_and_a_speed() {
    let root = example_copy("character");
    let db = Project::load(&root).unwrap().db;
    let k = db.characters.iter().position(|c| c.id == "villager").expect("the example's villager");
    let race = db.races.iter().find(|r| r.id != db.characters[k].race).expect("another race").clone();
    let speed = db.characters[k].speed;
    let mut h = harness(&root, Tab::Characters, k);
    step(&mut h);
    // The race chooser: open it, pick another race.
    // (egui gives a combo box no accessible text: it is the one beside "Race".)
    beside(&h, "Race", |n| n.role() == Role::ComboBox).click();
    step(&mut h);
    h.get_by_label(&format!("{}  ({})", race.name, race.id)).click();
    step(&mut h);
    assert_eq!(h.state().project.db.characters[k].race, race.id);
    // The speed: click the number (it turns into a text field with its text selected), type a
    // new one, Enter.
    let shown = format!("{speed}");
    beside(&h, "Speed (px/s)", move |n| n.role() == Role::SpinButton && n.value().as_deref() == Some(shown.as_str())).click();
    step(&mut h);
    h.get_by(|n| n.is_focused()).type_text("77");
    step(&mut h);
    h.press_key(egui::Key::Enter);
    step(&mut h);
    assert_eq!(h.state().project.db.characters[k].speed, 77.0);
    h.state_mut().save_all();
    let saved = Project::load(&root).unwrap().db;
    assert_eq!((saved.characters[k].race.as_str(), saved.characters[k].speed), (race.id.as_str(), 77.0));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn project_settings_tick_and_number() {
    let root = example_copy("project");
    let config = Project::load(&root).unwrap().config;
    let mut h = harness(&root, Tab::Project, 0);
    step(&mut h);
    h.get_by_label("characters walking diagonally turn diagonally").click();
    step(&mut h);
    assert_eq!(h.state().project.config.eight_facings, !config.eight_facings);
    undo(&mut h);
    assert_eq!(h.state().project.config.eight_facings, config.eight_facings);
    h.get_by_label("characters walking diagonally turn diagonally").click();
    step(&mut h);
    // A number: the pose memory, typed.
    let mb = format!("{} MB", config.model_atlas_mb);
    beside(&h, "Character pose memory", move |n| n.role() == Role::SpinButton && n.value().as_deref() == Some(mb.as_str())).click();
    step(&mut h);
    h.get_by(|n| n.is_focused()).type_text("64");
    step(&mut h);
    h.press_key(egui::Key::Enter);
    step(&mut h);
    assert_eq!(h.state().project.config.model_atlas_mb, 64);
    h.state_mut().save_all();
    let saved = Project::load(&root).unwrap().config;
    assert_eq!((saved.eight_facings, saved.model_atlas_mb), (!config.eight_facings, 64));
    let _ = std::fs::remove_dir_all(&root);
}
