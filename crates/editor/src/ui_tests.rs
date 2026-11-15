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
fn quest_stage_targets_are_edited_and_saved() {
    let root = example_copy("quest");
    let db = Project::load(&root).unwrap().db;
    let k = db.quests.iter().position(|q| q.id == "strange_lights").expect("the example's first quest");
    assert_eq!(db.quests[k].targets[2].name, "elder");
    let mut h = harness(&root, Tab::Quests, k);
    step(&mut h);
    // Stage 2 leads to the elder: point it at his bed instead (the giver field, above, says
    // "elder" too).
    h.query_all_by(|n| n.role() == Role::TextInput && n.value().as_deref() == Some("elder")).last().expect("the marker's name").type_text("_bed");
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
fn item_animations_are_named_estimated_and_saved() {
    let root = example_copy("item_anims");
    let db = Project::load(&root).unwrap().db;
    let k = db.items.iter().position(|i| i.id == "spear").expect("the example's spear");
    assert!(db.items[k].anims.is_empty(), "the example's spear goes by the estimate");
    let mut h = harness(&root, Tab::Items, k);
    step(&mut h);
    // The estimate shows beside the empty fields: a long two-handed weapon thrusts.
    h.get_by_label_contains("attack_thrust (estimated)");
    beside(&h, "Attacking with it", |n| n.role() == Role::TextInput).type_text("attack_2h");
    step(&mut h);
    unfocus(&mut h);
    let item = |h: &Harness<'static, Editor>| h.state().project.db.items[k].clone();
    assert_eq!(item(&h).anims.attack, "attack_2h");
    assert_eq!(item(&h).anims.idle, "", "the others stay estimated");
    // Estimate empties the names again; typed once more and saved, the file has it.
    h.get_by_label_contains("Estimate animations").click();
    step(&mut h);
    assert!(item(&h).anims.is_empty());
    beside(&h, "Blocking with it", |n| n.role() == Role::TextInput).type_text("block_weapon");
    step(&mut h);
    unfocus(&mut h);
    h.state_mut().save_all();
    let saved = Project::load(&root).unwrap().db.items[k].anims.clone();
    assert_eq!((saved.attack.as_str(), saved.block.as_str()), ("", "block_weapon"));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn an_item_becomes_a_backpack_with_extra_slots() {
    use rustpg2d_engine::defs::EquipSlot;
    let root = example_copy("backpack");
    let db = Project::load(&root).unwrap().db;
    let k = db.items.iter().position(|i| i.id == "apple").expect("the example's apple");
    let mut h = harness(&root, Tab::Items, k);
    step(&mut h);
    // Equipment kind Backpack: the Back slot, six slots to start with.
    h.get_by_label("Backpack").click();
    step(&mut h);
    let item = |h: &Harness<'static, Editor>| h.state().project.db.items[k].clone();
    assert_eq!(item(&h).armor.map(|a| a.slot), Some(EquipSlot::Back));
    assert_eq!(item(&h).bag_slots, 6);
    // Twelve slots typed, saved.
    beside(&h, "Extra inventory slots", |n| n.role() == Role::SpinButton).click();
    step(&mut h);
    h.get_by(|n| n.is_focused()).type_text("12");
    step(&mut h);
    h.press_key(egui::Key::Enter);
    step(&mut h);
    assert_eq!(item(&h).bag_slots, 12);
    h.state_mut().save_all();
    let saved = Project::load(&root).unwrap().db.items[k].clone();
    assert_eq!((saved.armor.map(|a| a.slot), saved.bag_slots), (Some(EquipSlot::Back), 12));
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
fn character_size_is_typed_in_percent() {
    let root = example_copy("char_size");
    let db = Project::load(&root).unwrap().db;
    let k = db.characters.iter().position(|c| c.id == "villager").expect("the example's villager");
    assert_eq!(db.characters[k].appearance.size, 0);
    let mut h = harness(&root, Tab::Characters, k);
    step(&mut h);
    // Shown as 100 %: type 120.
    beside(&h, "Size", |n| n.role() == Role::SpinButton && n.value().as_deref() == Some("100 %")).click();
    step(&mut h);
    h.get_by(|n| n.is_focused()).type_text("120");
    step(&mut h);
    h.press_key(egui::Key::Enter);
    step(&mut h);
    assert_eq!(h.state().project.db.characters[k].appearance.size, 120);
    h.state_mut().save_all();
    assert_eq!(Project::load(&root).unwrap().db.characters[k].appearance.size, 120);
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

#[test]
fn colour_feature_default_is_set_and_cleared() {
    let root = example_copy("feature_default");
    let db = Project::load(&root).unwrap().db;
    let k = db.models.iter().position(|m| m.id == "elf_f").expect("the example's elf woman");
    let skin = |db: &rustpg2d_engine::defs::Database| db.models[k].features.iter().find(|f| f.name == "skin").unwrap().clone();
    assert_eq!(skin(&db).default, None);
    let mut h = harness(&root, Tab::Models, k);
    step(&mut h);
    // The skin feature (its tab in the features row), then "Set": the first colour it offers.
    h.get_by_label("skin").click();
    step(&mut h);
    h.get_by(|n| n.label().as_deref().is_some_and(|l| l.ends_with("Set")) && n.role() == Role::Button).click();
    step(&mut h);
    let first = skin(&db).colors[0];
    assert_eq!(skin(&h.state().project.db).default, Some(first));
    h.state_mut().save_all();
    assert_eq!(skin(&Project::load(&root).unwrap().db).default, Some(first));
    // "As drawn": no default again.
    h.get_by(|n| n.label().as_deref().is_some_and(|l| l.ends_with("As drawn")) && n.role() == Role::Button).click();
    step(&mut h);
    assert_eq!(skin(&h.state().project.db).default, None);
    h.state_mut().save_all();
    assert_eq!(skin(&Project::load(&root).unwrap().db).default, None);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn material_form_sets_edge_rim_and_walking() {
    use rustpg2d_engine::ground::{Edge, Walk};
    let root = example_copy("material");
    let db = Project::load(&root).unwrap().db;
    let k = db.materials.iter().position(|m| m.id == "dirt").expect("the starter dirt");
    assert_eq!((db.materials[k].edge, db.materials[k].rim, db.materials[k].walk), (Edge::Crisp, None, Walk::Walk));
    let mut h = harness(&root, Tab::Materials, k);
    step(&mut h);
    // Soft edges, a rim, and a texture typed behind the path.
    h.get_by_label("Soft").click();
    step(&mut h);
    beside(&h, "Rim", |n| n.role() == Role::CheckBox).click();
    step(&mut h);
    field(&h, "materials/dirt.png").type_text("x");
    step(&mut h);
    unfocus(&mut h);
    let m = |h: &Harness<'static, Editor>| h.state().project.db.materials[k].clone();
    assert_eq!(m(&h).edge, Edge::Soft);
    assert!(m(&h).rim.is_some());
    assert_eq!(m(&h).texture, "materials/dirt.pngx");
    // Ctrl+Z takes the typing back; saved, the file has the rest.
    undo(&mut h);
    assert_eq!(m(&h).texture, "materials/dirt.png");
    h.state_mut().save_all();
    let saved = Project::load(&root).unwrap().db;
    assert_eq!((saved.materials[k].edge, saved.materials[k].rim.is_some()), (Edge::Soft, true));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_material_texture_is_drawn_seamlessly_in_the_form() {
    use crate::pixel::{Layout, PxTool};
    let root = example_copy("material_draw");
    let k = Project::load(&root).unwrap().db.materials.len();
    let mut h = harness(&root, Tab::Materials, k);
    h.state_mut().project.db.materials.push(rustpg2d_engine::ground::MaterialDef { name: "Moss".into(), ..rustpg2d_engine::ground::MaterialDef::fresh("moss") });
    step(&mut h);
    // No texture yet: "Draw a new one" names it, then a size creates it, filled with one colour.
    h.get_by_label_contains("Draw a new one").click();
    step(&mut h);
    assert_eq!(h.state().project.db.materials[k].texture, "materials/moss.png");
    h.get_by_label("32×32 px").click();
    step(&mut h);
    let file = root.join("materials/moss.png");
    let img = rustpg2d_engine::imageio::Rgba::decode(&std::fs::read(&file).unwrap()).unwrap();
    assert_eq!((img.w, img.h, img.get(5, 5)[3]), (32, 32, 255));
    // It opens below the form as one seamless picture: drawing past an edge wraps around.
    let d = h.state().px.doc.as_ref().expect("the texture is open");
    assert_eq!((d.rel.as_str(), d.layout, d.frame), ("materials/moss.png", Layout::Seamless(1), (32, 32)));
    let px = &mut h.state_mut().px;
    px.tool = PxTool::Pencil;
    px.colors[0] = [10, 200, 30, 255];
    px.apply_at(0, (-1, 33));
    let d = h.state().px.doc.as_ref().unwrap();
    assert_eq!(d.img.get(31, 1), [10, 200, 30, 255]);
    assert!(d.is_dirty() && d.version > 0);
    h.state_mut().save_pixel();
    let img = rustpg2d_engine::imageio::Rgba::decode(&std::fs::read(&file).unwrap()).unwrap();
    assert_eq!(img.get(31, 1), [10, 200, 30, 255]);
    // Three animation frames: the texture becomes a strip of copies; each frame wraps on its own.
    let line = h.get_by_label("Animation").bounding_box().unwrap();
    let spin = |n: &egui_kittest::kittest::Node<'_>| n.role() == Role::SpinButton && n.bounding_box().is_some_and(|b| b.y0 < line.y1 && b.y1 > line.y0);
    let frames = h.query_all_by(spin).min_by(|a, b| a.bounding_box().unwrap().x0.total_cmp(&b.bounding_box().unwrap().x0)).expect("the frames field");
    frames.click();
    step(&mut h);
    h.get_by(|n| n.is_focused()).type_text("3");
    step(&mut h);
    h.press_key(egui::Key::Enter);
    step(&mut h);
    let m = h.state().project.db.materials[k].clone();
    assert_eq!((m.frames, m.fps), (3, 4.0));
    let d = h.state().px.doc.as_ref().unwrap();
    assert_eq!((d.img.w, d.frame, d.layout), (96, (32, 32), Layout::Seamless(3)));
    assert_eq!(d.img.get(64 + 31, 1), [10, 200, 30, 255], "the new frames copy the last one");
    let px = &mut h.state_mut().px;
    px.sel_frame = 2;
    px.colors[0] = [250, 20, 20, 255];
    px.apply_at(0, (-1, 0));
    let d = h.state().px.doc.as_ref().unwrap();
    assert_eq!((d.img.get(95, 0), d.img.get(31, 0)), ([250, 20, 20, 255], [img.get(31, 0)[0], img.get(31, 0)[1], img.get(31, 0)[2], 255]));
    h.state_mut().save_pixel();
    h.state_mut().save_all();
    let saved = Project::load(&root).unwrap().db.materials[k].clone();
    assert_eq!((saved.texture.as_str(), saved.frames, saved.fps), ("materials/moss.png", 3, 4.0));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn the_quest_designer_describes_giver_offer_steps_and_rewards() {
    use rustpg2d_engine::quest::{QuestDef, StepGoal};
    let root = example_copy("quest_designer");
    let k = Project::load(&root).unwrap().db.quests.len();
    let mut h = harness(&root, Tab::Quests, k);
    h.state_mut().project.db.quests.push(QuestDef { id: "ashes".into(), title: "Ashes".into(), description: "Old ashes.".into(), stages: vec!["Find the ashes".into()], ..Default::default() });
    step(&mut h);
    let q = |h: &Harness<'static, Editor>| h.state().project.db.quests[k].clone();
    // The giver: typed into its field.
    beside(&h, "Giver", |n| n.role() == Role::TextInput).type_text("elder");
    step(&mut h);
    unfocus(&mut h);
    assert_eq!(q(&h).giver, "elder");
    // An offer line (the first "+ Line": what the giver says first).
    h.query_all_by_label("+ Line").next().expect("the offer's lines").click();
    step(&mut h);
    h.get_by(|n| n.role() == Role::MultilineTextInput && n.value().as_deref() == Some("")).type_text("The shrine left ashes.");
    step(&mut h);
    unfocus(&mut h);
    assert_eq!(q(&h).offer.lines, ["The shrine left ashes."]);
    // A second step: a new last step of a given quest goes back to the giver.
    h.get_by_label(&crate::ui::t(crate::ui::ph::PLUS, "Step")).click();
    step(&mut h);
    assert_eq!(q(&h).stages.len(), 2);
    assert_eq!(q(&h).steps[1].done, StepGoal::Talk { character: String::new(), lines: vec![], when: vec![], speaker: String::new() });
    // A quest object (hidden in the game until the quest starts).
    h.get_by_label(&crate::ui::t(crate::ui::ph::PLUS, "Object")).click();
    step(&mut h);
    assert_eq!(q(&h).objects.len(), 1);
    // A reward item.
    h.get_by_label(&crate::ui::t(crate::ui::ph::PLUS, "Item")).click();
    step(&mut h);
    assert_eq!(q(&h).rewards.items.len(), 1);
    // Undo takes the item back; saved, the rest is in the file.
    undo(&mut h);
    assert!(q(&h).rewards.items.is_empty());
    h.state_mut().save_all();
    let saved = Project::load(&root).unwrap().db.quests[k].clone();
    assert_eq!((saved.giver.as_str(), saved.offer.lines.len(), saved.stages.len()), ("elder", 1, 2));
    assert!(matches!(saved.steps[1].done, StepGoal::Talk { .. }));
    let _ = std::fs::remove_dir_all(&root);
}
