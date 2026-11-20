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
    // (On the label's line; for a label in the form not the list's search field in the left panel.)
    const LIST_W: f64 = 260.0;
    let mut found: Vec<_> = h.query_all_by(move |n| f(n) && n.bounding_box().is_some_and(|b| b.y0 < line.y1 && b.y1 > line.y0 && !(line.x0 > LIST_W && b.x1 < LIST_W))).collect();
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
fn a_weapon_gets_a_damage_type_and_a_touch_a_condition() {
    use rustpg2d_engine::srd::{Condition, DamageType};
    let root = example_copy("damage");
    let db = Project::load(&root).unwrap().db;
    let k = db.items.iter().position(|i| i.id == "sword").expect("the example's sword");
    assert_eq!(db.items[k].weapon.as_ref().unwrap().damage_type, Some(DamageType::Piercing), "the SRD's shortsword");
    let mut h = harness(&root, Tab::Items, k);
    step(&mut h);
    // The damage type beside the weapon's damage dice.
    // (egui gives a combo box no accessible text: it is the one beside "Damage dice".)
    beside(&h, "Damage dice", |n| n.role() == Role::ComboBox).click();
    step(&mut h);
    h.get_by_label("Bludgeoning").click();
    step(&mut h);
    assert_eq!(h.state().project.db.items[k].weapon.as_ref().unwrap().damage_type, Some(DamageType::Bludgeoning));
    // The dice: the example's sword rolls 1d6; typed over, 2d4.
    assert_eq!(h.state().project.db.items[k].weapon.as_ref().unwrap().dice, "1d6");
    // (Emptied, the field falls back to the old damage's dice; typed, its own.)
    h.state_mut().project.db.items[k].weapon.as_mut().unwrap().dice.clear();
    step(&mut h);
    beside(&h, "Damage dice", |n| n.role() == Role::TextInput).type_text("2d4");
    step(&mut h);
    unfocus(&mut h);
    assert_eq!(h.state().project.db.items[k].weapon.as_ref().unwrap().dice, "2d4");
    h.state_mut().save_all();
    let saved = Project::load(&root).unwrap().db.items[k].weapon.clone().unwrap();
    assert_eq!((saved.damage_type, saved.dice.as_str()), (Some(DamageType::Bludgeoning), "2d4"));
    // The slime's touch: Poisoned becomes Prone; its saving throw stays.
    let c = db.characters.iter().position(|c| c.id == "slime").expect("the example's slime");
    let mut h = harness(&root, Tab::Characters, c);
    // (Without its stat block: its "Can't get" toggles name the conditions too.)
    h.state_mut().project.db.characters[c].stats = None;
    step(&mut h);
    beside(&h, "On touch:", |n| n.role() == Role::ComboBox).click();
    step(&mut h);
    h.get_by_label("Prone").click();
    step(&mut h);
    let fx = h.state().project.db.characters[c].touch_effect.clone().expect("a touch");
    assert_eq!((fx.condition, fx.save.as_ref().map(|s| s.dc)), (Some(Condition::Prone), Some(10)));
    h.state_mut().save_all();
    assert_eq!(Project::load(&root).unwrap().db.characters[c].touch_effect.as_ref().unwrap().condition, Some(Condition::Prone));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn item_animations_are_named_estimated_and_saved() {
    let root = example_copy("item_anims");
    let db = Project::load(&root).unwrap().db;
    let k = db.items.iter().position(|i| i.id == "greataxe").expect("the example's greataxe");
    assert!(db.items[k].anims.is_empty(), "the example's greataxe goes by the estimate");
    let mut h = harness(&root, Tab::Items, k);
    step(&mut h);
    // The estimate shows beside the empty fields: a two-handed weapon swings overhead.
    h.get_by_label_contains("attack_2h (estimated)");
    beside(&h, "Attacking with it", |n| n.role() == Role::TextInput).type_text("attack_thrust");
    step(&mut h);
    unfocus(&mut h);
    let item = |h: &Harness<'static, Editor>| h.state().project.db.items[k].clone();
    assert_eq!(item(&h).anims.attack, "attack_thrust");
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
fn a_weapon_gets_srd_properties_and_armor_a_category() {
    use rustpg2d_engine::defs::{ArmorCategory, WeaponCategory, WeaponProperty};
    let root = example_copy("srd_gear");
    let db = Project::load(&root).unwrap().db;
    let k = db.items.iter().position(|i| i.id == "dagger").expect("the example's dagger");
    let w = db.items[k].weapon.clone().unwrap();
    assert_eq!((w.category, w.thrown_ft), (Some(WeaponCategory::Simple), Some((20, 60))));
    let mut h = harness(&root, Tab::Items, k);
    step(&mut h);
    // Finesse off, Two-handed on (the old two-handed flag follows).
    h.get_by_label("Finesse").click();
    step(&mut h);
    h.get_by_label("Two-handed").click();
    step(&mut h);
    let w = h.state().project.db.items[k].weapon.clone().unwrap();
    assert!(!w.properties.contains(&WeaponProperty::Finesse) && w.properties.contains(&WeaponProperty::TwoHanded) && w.two_handed);
    // The category: martial.
    beside(&h, "Category", |n| n.role() == Role::ComboBox).click();
    step(&mut h);
    h.get_by_label("Martial").click();
    step(&mut h);
    assert_eq!(h.state().project.db.items[k].weapon.as_ref().unwrap().category, Some(WeaponCategory::Martial));
    h.state_mut().save_all();
    let saved = Project::load(&root).unwrap().db.items[k].weapon.clone().unwrap();
    assert!(saved.two_handed && saved.category == Some(WeaponCategory::Martial) && !saved.properties.contains(&WeaponProperty::Finesse));
    // Leather armor becomes medium armor: 14 + DEX up to 2.
    let a = db.items.iter().position(|i| i.id == "leather_armor").unwrap();
    let mut h = harness(&root, Tab::Items, a);
    step(&mut h);
    beside(&h, "Category", |n| n.role() == Role::ComboBox).click();
    step(&mut h);
    h.get_by_label("Medium armor").click();
    step(&mut h);
    let armor = h.state().project.db.items[a].armor.clone().unwrap();
    assert_eq!((armor.category, armor.base_ac, armor.dex_max), (Some(ArmorCategory::Medium), 14, Some(2)));
    h.state_mut().save_all();
    assert_eq!(Project::load(&root).unwrap().db.items[a].armor.as_ref().unwrap().category, Some(ArmorCategory::Medium));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn an_item_becomes_a_magic_ring_with_charges() {
    use rustpg2d_engine::defs::EquipSlot;
    let root = example_copy("magic_item");
    let db = Project::load(&root).unwrap().db;
    let k = db.items.iter().position(|i| i.id == "apple").expect("the example's apple");
    let mut h = harness(&root, Tab::Items, k);
    step(&mut h);
    // A worn item: a ring.
    h.get_by_label("Worn item").click();
    step(&mut h);
    let item = |h: &Harness<'static, Editor>| h.state().project.db.items[k].clone();
    assert_eq!(item(&h).armor.map(|a| a.slot), Some(EquipSlot::Ring));
    // Magic: attunement and three charges.
    h.get_by_label("Magic item").click();
    step(&mut h);
    assert!(item(&h).magic.is_some());
    h.get_by_label("required").click();
    step(&mut h);
    beside(&h, "Charges", |n| n.role() == Role::SpinButton).click();
    step(&mut h);
    h.get_by(|n| n.is_focused()).type_text("3");
    step(&mut h);
    h.press_key(egui::Key::Enter);
    step(&mut h);
    let m = item(&h).magic.unwrap();
    assert_eq!((m.attunement, m.charges), (true, 3));
    h.state_mut().save_all();
    let saved = Project::load(&root).unwrap().db.items[k].clone();
    assert_eq!((saved.armor.map(|a| a.slot), saved.magic.map(|m| (m.attunement, m.charges))), (Some(EquipSlot::Ring), Some((true, 3))));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn the_item_list_searches_and_filters_by_category() {
    let root = example_copy("item_list");
    let db = Project::load(&root).unwrap().db;
    let mut h = harness(&root, Tab::Items, 0);
    step(&mut h);
    let total = db.items.len();
    assert_eq!(h.state().db.list_shown, (total, total));
    // A search by name: the selection moves to the first match.
    // (The list's search field: the text field on the magnifier's line in the left panel.)
    let line = h.get_by_label(crate::ui::ph::MAGNIFYING_GLASS).bounding_box().unwrap();
    let is_search = move |n: &egui_kittest::kittest::Node<'_>| n.role() == Role::TextInput && n.bounding_box().is_some_and(|b| b.y0 < line.y1 && b.y1 > line.y0 && b.x0 < 300.0);
    assert_eq!(h.query_all_by(is_search).count(), 1, "the list's search field");
    h.get_by(is_search).focus();
    step(&mut h);
    h.get_by(is_search).type_text("ring of");
    step(&mut h);
    let (shown, _) = h.state().db.list_shown;
    assert_eq!(shown, 1, "the Ring of Protection");
    let sel = h.state().db.sel;
    assert_eq!(h.state().project.db.items[sel].id, "ring-of-protection");
    // The category picker: magic items among the matches, weapons none.
    h.state_mut().db.item_filter = crate::item_category::ItemFilter::parse("magic").unwrap();
    step(&mut h);
    assert_eq!(h.state().db.list_shown.0, 1);
    h.state_mut().db.item_filter = crate::item_category::ItemFilter::parse("weapons").unwrap();
    step(&mut h);
    assert_eq!(h.state().db.list_shown.0, 0);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn an_item_gets_uses_and_silvering() {
    let root = example_copy("uses");
    let db = Project::load(&root).unwrap().db;
    let k = db.items.iter().position(|i| i.id == "dagger").expect("the example's dagger");
    let mut h = harness(&root, Tab::Items, k);
    step(&mut h);
    h.get_by_label("Silvered").click();
    step(&mut h);
    // Five uses typed.
    beside(&h, "Uses", |n| n.role() == Role::SpinButton).click();
    step(&mut h);
    h.get_by(|n| n.is_focused()).type_text("5");
    step(&mut h);
    h.press_key(egui::Key::Enter);
    step(&mut h);
    let item = h.state().project.db.items[k].clone();
    assert!(item.silvered);
    assert_eq!((item.uses, item.max_charges()), (5, 5));
    h.state_mut().save_all();
    let saved = Project::load(&root).unwrap().db.items[k].clone();
    assert!(saved.silvered && saved.uses == 5);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn an_item_places_furniture() {
    let root = example_copy("furniture");
    let db = Project::load(&root).unwrap().db;
    let k = db.items.iter().position(|i| i.id == "apple").expect("the example's apple");
    let mut h = harness(&root, Tab::Items, k);
    step(&mut h);
    beside(&h, "Places prop", |n| n.role() == Role::TextInput).type_text("chair");
    step(&mut h);
    unfocus(&mut h);
    assert_eq!(h.state().project.db.items[k].furniture, "chair");
    h.state_mut().save_all();
    assert_eq!(Project::load(&root).unwrap().db.items[k].furniture, "chair");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_prop_lies_flat_or_is_a_door() {
    let root = example_copy("flat_prop");
    let db = Project::load(&root).unwrap().db;
    let k = db.props.iter().position(|p| p.id == "chest").expect("the example's chest");
    assert!(!db.props[k].flat);
    let mut h = harness(&root, Tab::Props, k);
    step(&mut h);
    h.get_by_label("Lies flat on the ground").click();
    step(&mut h);
    assert!(h.state().project.db.props[k].flat);
    h.get_by_label("Door (E opens and closes it)").click();
    step(&mut h);
    assert!(h.state().project.db.props[k].door);
    h.state_mut().save_all();
    let saved = Project::load(&root).unwrap().db.props[k].clone();
    assert!(saved.flat && saved.door);
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
fn a_stat_block_comes_from_a_template() {
    let root = example_copy("statblock");
    let db = Project::load(&root).unwrap().db;
    let k = db.characters.iter().position(|c| c.id == "villager").expect("the example's villager");
    let mut h = harness(&root, Tab::Characters, k);
    step(&mut h);
    h.state_mut().project.db.characters[k].stats = None;
    step(&mut h);
    // The Template chooser beside "Template": the SRD's guard.
    beside(&h, "Template", |n| n.role() == Role::ComboBox).click();
    step(&mut h);
    h.get_by_label("Guard  (CR 1/8, 25 XP)").click();
    step(&mut h);
    let c = &h.state().project.db.characters[k];
    let s = c.stats.as_ref().expect("a stat block");
    assert_eq!((s.template.as_str(), s.ac, s.hp, s.hit_dice.as_str(), c.xp), ("guard", 16, 11, "2d8+2", 25));
    assert!(!s.proficiencies.contains(&"stealth".to_string()));
    // A proficiency toggled on.
    h.get_by_label("Stealth").click();
    step(&mut h);
    assert!(h.state().project.db.characters[k].stats.as_ref().unwrap().proficiencies.contains(&"stealth".to_string()));
    h.state_mut().save_all();
    let saved = Project::load(&root).unwrap().db;
    let s = saved.characters[k].stats.as_ref().expect("saved");
    assert_eq!((s.template.as_str(), s.ac, saved.characters[k].xp), ("guard", 16, 25));
    assert!(s.proficiencies.contains(&"stealth".to_string()));
    // Removed again.
    h.get_by_label(&crate::ui::t(egui_phosphor::regular::TRASH, "Remove stat block")).click();
    step(&mut h);
    assert!(h.state().project.db.characters[k].stats.is_none());
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
fn a_font_is_defined_and_chosen() {
    let root = example_copy("font");
    std::fs::write(root.join("defs/fonts.ron"), r#"[(id: "pixel", name: "Pixel", file: "fonts/pixel.ttf")]"#).unwrap();
    let mut h = harness(&root, Tab::Fonts, 0);
    step(&mut h);
    // Its pixel size, typed.
    beside(&h, "Pixel size", |n| n.role() == Role::SpinButton && n.value().as_deref() == Some("0 px")).click();
    step(&mut h);
    h.get_by(|n| n.is_focused()).type_text("16");
    step(&mut h);
    h.press_key(egui::Key::Enter);
    step(&mut h);
    assert_eq!(h.state().project.db.fonts[0].pixel, 16);
    // Project › Fonts: the windows' font (the combo box beside its label).
    h.state_mut().db.tab = Tab::Project;
    step(&mut h);
    beside(&h, "Windows and menus", |n| n.role() == Role::ComboBox).click();
    step(&mut h);
    h.get_by(|n| n.label().as_deref() == Some("pixel") && n.role() != Role::ComboBox).click();
    step(&mut h);
    assert_eq!(h.state().project.config.fonts.ui, "pixel");
    h.state_mut().save_all();
    let saved = Project::load(&root).unwrap();
    assert_eq!((saved.db.fonts[0].pixel, saved.config.fonts.ui.as_str()), (16, "pixel"));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_skin_is_defined_and_chosen() {
    let root = example_copy("skin");
    std::fs::write(root.join("defs/skins.ron"), r#"[(id: "plain", name: "Plain")]"#).unwrap();
    let mut h = harness(&root, Tab::Skins, 0);
    step(&mut h);
    // A window frame picture and an own border colour (switched on: they start as built in).
    h.get_by(|n| n.role() == Role::CheckBox && n.label().as_deref() == Some("Window frame")).click();
    step(&mut h);
    h.get_by(|n| n.role() == Role::CheckBox && n.label().as_deref() == Some("Border")).click();
    step(&mut h);
    let d = &h.state().project.db.skins[0];
    assert_eq!(d.frame.as_ref().map(|f| (f.corner, f.scale)), Some((4, 2)));
    assert_eq!(d.border.map(|c| c.0), Some([217, 204, 153, 255]));
    // Project › Look: the game's skin.
    h.state_mut().db.tab = Tab::Project;
    step(&mut h);
    beside(&h, "Skin", |n| n.role() == Role::ComboBox).click();
    step(&mut h);
    h.get_by(|n| n.label().as_deref() == Some("Plain") && n.role() != Role::ComboBox).click();
    step(&mut h);
    assert_eq!(h.state().project.config.skin, "plain");
    h.state_mut().save_all();
    let saved = Project::load(&root).unwrap();
    assert_eq!((saved.db.skins[0].border.map(|c| c.hex()), saved.config.skin.as_str()), (Some("#d9cc99".to_string()), "plain"));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_common_event_runs_while_a_flag_is_set() {
    use rustpg2d_engine::common_events::CommonTrigger;
    let root = example_copy("common");
    let k = Project::load(&root).unwrap().db.common_events.iter().position(|d| d.id == "quest_reward").expect("the example's common event");
    let mut h = harness(&root, Tab::CommonEvents, k);
    step(&mut h);
    // Its actions are edited like a trigger's.
    assert!(h.query_by_label("2.").is_some() && h.query_by_label("3.").is_none(), "its two actions are listed");
    // Runs: parallel while a flag is set.
    beside(&h, "Runs", |n| n.role() == Role::ComboBox).click();
    step(&mut h);
    h.get_by(|n| n.label().as_deref() == Some("Parallel while flag") && n.role() != Role::ComboBox).click();
    step(&mut h);
    beside(&h, "flag", |n| n.role() == Role::TextInput).type_text("cheering");
    step(&mut h);
    unfocus(&mut h);
    assert_eq!(h.state().project.db.common_events[k].trigger, CommonTrigger::Parallel { flag: "cheering".into() });
    h.state_mut().save_all();
    assert_eq!(Project::load(&root).unwrap().db.common_events[k].trigger, CommonTrigger::Parallel { flag: "cheering".into() });
    let _ = std::fs::remove_dir_all(&root);
}

/// Database › Props: a barrel becomes an object (Can be broken), its hit points set, saved.
#[test]
fn a_prop_becomes_a_breakable_object() {
    let root = example_copy("objects");
    let db = Project::load(&root).unwrap().db;
    let k = db.props.iter().position(|p| p.id == "barrel").expect("the example's barrel");
    assert!(db.props[k].object.is_none());
    let mut h = harness(&root, Tab::Props, k);
    step(&mut h);
    h.get_by(|n| n.label().as_deref().is_some_and(|l| l.starts_with("Can be broken")) && n.role() == Role::CheckBox).click();
    step(&mut h);
    let o = h.state().project.db.props[k].object.clone().expect("an object now");
    assert_eq!((o.armor_class(), o.max_hp()), (15, 18), "wood, medium, resilient");
    h.state_mut().project.db.props[k].object.as_mut().unwrap().force_dc = 12;
    h.state_mut().save_all();
    let saved = Project::load(&root).unwrap();
    assert_eq!(saved.db.props[k].object.as_ref().map(|o| o.force_dc), Some(12));
    let _ = std::fs::remove_dir_all(&root);
}

/// Database › Props: a barrel gets a trap (the poison needle to start from), the preset switched
/// to the falling net, saved.
#[test]
fn a_prop_gets_a_trap() {
    let root = example_copy("traps");
    let db = Project::load(&root).unwrap().db;
    let k = db.props.iter().position(|p| p.id == "barrel").expect("the example's barrel");
    assert!(db.props[k].trap.is_none());
    let mut h = harness(&root, Tab::Props, k);
    step(&mut h);
    h.get_by(|n| n.label().as_deref().is_some_and(|l| l.starts_with("Trapped")) && n.role() == Role::CheckBox).click();
    step(&mut h);
    assert_eq!(h.state().project.db.props[k].trap.as_ref().map(|t| t.preset.as_str()), Some("poison_needle"));
    // (The preset's combo box is the one beside "Preset".)
    beside(&h, "Preset", |n| n.role() == Role::ComboBox).click();
    step(&mut h);
    h.get_by_label("Falling net").click();
    step(&mut h);
    let t = h.state().project.db.props[k].trap.clone().expect("still trapped");
    assert_eq!((t.preset.as_str(), t.detect_dc, t.condition.map(|c| c.condition)), ("falling_net", 10, Some(Some(rustpg2d_engine::srd::Condition::Restrained))));
    h.state_mut().save_all();
    assert_eq!(Project::load(&root).unwrap().db.props[k].trap.as_ref().map(|t| t.preset.clone()), Some("falling_net".into()));
    let _ = std::fs::remove_dir_all(&root);
}

/// Database › Items: the basic poison (the SRD's vial) switched to Serpent Venom by its preset,
/// saved.
#[test]
fn an_item_becomes_a_poison() {
    let root = example_copy("poisons");
    let db = Project::load(&root).unwrap().db;
    let k = db.items.iter().position(|i| i.id == "poison-basic-vial").expect("the example's basic poison");
    assert_eq!(db.items[k].poison.as_ref().map(|p| p.dc), Some(10));
    let mut h = harness(&root, Tab::Items, k);
    step(&mut h);
    // (The preset's combo box is the one beside "Preset".)
    beside(&h, "Preset", |n| n.role() == Role::ComboBox).click();
    step(&mut h);
    h.get_by_label("Serpent Venom").click();
    step(&mut h);
    let p = h.state().project.db.items[k].poison.clone().expect("still a poison");
    assert_eq!((p.preset.as_str(), p.dc, p.damage.as_str(), p.half), ("serpent-venom", 11, "3d6", true));
    // Switched off, it is a plain vial again.
    h.get_by(|n| n.label().as_deref() == Some("Poison") && n.role() == Role::CheckBox).click();
    step(&mut h);
    assert!(h.state().project.db.items[k].poison.is_none());
    h.state_mut().save_all();
    assert!(Project::load(&root).unwrap().db.items[k].poison.is_none());
    let _ = std::fs::remove_dir_all(&root);
}

/// Project › Menus › Action bar: the built-in entries to start from, a custom one added, saved.
#[test]
fn the_action_bar_gets_a_custom_entry() {
    use rustpg2d_engine::menus::BarEntry;
    let root = example_copy("action_bar");
    let mut h = harness(&root, Tab::Project, 0);
    assert!(h.state().project.config.action_bar.is_empty(), "the example uses the built-in bar");
    h.get_all_by(|n| n.role() == Role::Button && n.label().as_deref().is_some_and(|l| l.ends_with("Change…"))).last().unwrap().click();
    step(&mut h);
    assert_eq!(h.state().project.config.action_bar, BarEntry::DEFAULT.to_vec());
    h.get_all_by(|n| n.label().as_deref().is_some_and(|l| l.ends_with("Add entry"))).last().unwrap().click();
    step(&mut h);
    h.get_by_label("Custom: runs a script function").click();
    step(&mut h);
    assert!(matches!(h.state().project.config.action_bar.last(), Some(BarEntry::Custom { .. })));
    h.state_mut().save_all();
    let saved = Project::load(&root).unwrap();
    assert_eq!(saved.config.action_bar.len(), BarEntry::DEFAULT.len() + 1);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn menu_entries_are_changed_and_saved() {
    use rustpg2d_engine::menus::{MenuEntry, PauseMenu};
    let root = example_copy("menus");
    let mut h = harness(&root, Tab::Project, 0);
    // (The example has its own pause entries: start from the built-in ones.)
    h.state_mut().project.config.menu.entries.clear();
    step(&mut h);
    // The pause menu has the built-in entries: "Change…" copies them to start from.
    // (The pause menu's comes first; the action bar's last.)
    h.get_all_by(|n| n.role() == Role::Button && n.label().as_deref().is_some_and(|l| l.ends_with("Change…"))).next().unwrap().click();
    step(&mut h);
    assert_eq!(h.state().project.config.menu.entries, PauseMenu::DEFAULT.to_vec());
    // Move the second entry (Photo mode) up. Every row has a Move up (the first one's disabled);
    // the title's entries come first on the page.
    let ups: Vec<_> = h.get_all_by(|n| n.role() == Role::Button && n.label().as_deref() == Some(egui_phosphor::regular::ARROW_UP)).collect();
    let before = h.state().project.config.title.entries.len();
    ups[before + 1].click();
    step(&mut h);
    assert_eq!(h.state().project.config.menu.entries[..2], [MenuEntry::Photo, MenuEntry::Resume]);
    // A custom entry on the title screen (its Add entry comes first).
    h.get_all_by(|n| n.label().as_deref().is_some_and(|l| l.ends_with("Add entry"))).next().unwrap().click();
    step(&mut h);
    h.get_by_label("Custom: runs a script function").click();
    step(&mut h);
    assert!(matches!(h.state().project.config.title.entries.last(), Some(MenuEntry::Custom { .. })));
    h.state_mut().save_all();
    let saved = Project::load(&root).unwrap();
    assert_eq!(saved.config.menu.entries[0], MenuEntry::Photo);
    assert_eq!(saved.config.title.entries.len(), before + 1);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn dialog_codes_are_inserted_and_the_box_placed() {
    let root = example_copy("dialog_codes");
    let chars = root.join("defs/characters.ron");
    let ron = std::fs::read_to_string(&chars).unwrap().replacen(
        "triggers: [],",
        r#"triggers: [(on: Interact, conditions: [], actions: [Dialog(speaker: "Greta", text: "Sleep well!")], once: false)],"#,
        1,
    );
    std::fs::write(&chars, ron).unwrap();
    let text = |h: &Harness<'static, Editor>| match &h.state().project.db.characters[0].triggers[0].actions[0] {
        rustpg2d_engine::events::Action::Dialog { text, .. } => text.clone(),
        other => panic!("{other:?}"),
    };
    let mut h = harness(&root, Tab::Characters, 0);
    step(&mut h);
    // Pause: the code goes in at the cursor (the end of a field never clicked).
    h.get_by(|n| n.role() == Role::Button && n.label().as_deref().is_some_and(|l| l.ends_with("Pause"))).click();
    step(&mut h);
    assert_eq!(text(&h), "Sleep well!{w:0.5}");
    // A colour from the Colour menu.
    h.get_by(|n| n.role() == Role::Button && n.label().as_deref().is_some_and(|l| l.ends_with("Colour"))).click();
    step(&mut h);
    h.get_by(|n| n.role() == Role::Button && n.label().as_deref() == Some("gold")).click();
    step(&mut h);
    assert_eq!(text(&h), "Sleep well!{w:0.5}{c:gold}");
    // The box at the top: the code at the start of the text.
    let colour = format!("{} Colour", crate::ui::ph::PALETTE);
    beside(&h, &colour, |n| n.role() == Role::ComboBox).click();
    step(&mut h);
    h.get_by(|n| n.label().as_deref() == Some("top") && n.role() != Role::ComboBox).click();
    step(&mut h);
    assert_eq!(text(&h), "{at:top}Sleep well!{w:0.5}{c:gold}");
    h.state_mut().save_all();
    let saved = Project::load(&root).unwrap();
    assert!(matches!(&saved.db.characters[0].triggers[0].actions[0], rustpg2d_engine::events::Action::Dialog { text, .. } if text == "{at:top}Sleep well!{w:0.5}{c:gold}"));
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

#[test]
fn faction_members_are_changed_from_the_factions_tab() {
    let root = example_copy("faction_members");
    let mut h = harness(&root, Tab::Factions, 0);
    step(&mut h);
    let race = |h: &Harness<'static, Editor>, id: &str| h.state().project.db.race(id).unwrap().faction.clone();
    assert_eq!(race(&h, "goblin"), "goblins");
    // The goblin race's ✕ button under its faction takes it out.
    h.get_by_label(&format!("{} Goblin", egui_phosphor::regular::X)).click();
    step(&mut h);
    assert_eq!(race(&h, "goblin"), "");
    // "Add a race…" under Humans puts it into theirs (egui gives combo boxes no accessible text:
    // the tab's first one is the humans' Add a race).
    h.get_all_by(|n| n.role() == Role::ComboBox).next().unwrap().click();
    step(&mut h);
    h.get_by(|n| n.label().as_deref() == Some("Goblin") && n.role() != Role::ComboBox).click();
    step(&mut h);
    assert_eq!(race(&h, "goblin"), "humans");
    h.state_mut().save_all();
    let saved = Project::load(&root).unwrap();
    assert_eq!(saved.db.race("goblin").unwrap().faction, "humans");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn script_hooks_are_added_in_the_project_form() {
    let root = example_copy("script_hooks");
    let mut h = harness(&root, Tab::Project, 0);
    let before = h.state().project.config.hooks.len();
    h.get_by(|n| n.label().as_deref().is_some_and(|l| l.ends_with("Add hook"))).click();
    step(&mut h);
    let hooks = &h.state().project.config.hooks;
    assert_eq!(hooks.len(), before + 1);
    assert_eq!(hooks.last().unwrap().hook, "damage");
    h.state_mut().project.config.hooks.last_mut().unwrap().function = "friend_buy_price".into();
    h.state_mut().save_all();
    let saved = Project::load(&root).unwrap();
    assert_eq!(saved.config.hooks.last().unwrap().function, "friend_buy_price");
    let _ = std::fs::remove_dir_all(&root);
}

/// Project › Rules (SRD 5.1): the standard array button, a proficiency added and typed in, saved.
#[test]
fn the_heros_rules_are_set_in_the_project_form() {
    let root = example_copy("srd_rules");
    let mut h = harness(&root, Tab::Project, 0);
    h.state_mut().project.config.rules.abilities = rustpg2d_engine::srd::Abilities::default();
    step(&mut h);
    h.get_by(|n| n.label().as_deref() == Some("Standard array") && n.role() == Role::Button).click();
    step(&mut h);
    assert_eq!(h.state().project.config.rules.abilities, rustpg2d_engine::srd::Abilities::STANDARD);
    let before = h.state().project.config.rules.proficiencies.len();
    h.get_by(|n| n.label().as_deref().is_some_and(|l| l.ends_with("Add proficiency"))).click();
    step(&mut h);
    assert_eq!(h.state().project.config.rules.proficiencies.len(), before + 1);
    h.state_mut().project.config.rules.proficiencies.last_mut().unwrap().push_str("stealth");
    h.state_mut().save_all();
    let saved = Project::load(&root).unwrap();
    assert_eq!(saved.config.rules.abilities, rustpg2d_engine::srd::Abilities::STANDARD);
    assert_eq!(saved.config.rules.proficiencies.last().map(String::as_str), Some("stealth"));
    assert_eq!(saved.config.rules_version, rustpg2d_engine::srd::RULES_VERSION);
    let _ = std::fs::remove_dir_all(&root);
}

/// An item's *Use* trigger gets a *Group check* from the + Action menu, saved.
#[test]
fn a_group_check_is_added_to_a_trigger() {
    use rustpg2d_engine::events::Action;
    let root = example_copy("group_check");
    let db = Project::load(&root).unwrap().db;
    let k = db.items.iter().position(|i| i.id == "bread").expect("the example's bread");
    let mut h = harness(&root, Tab::Items, k);
    step(&mut h);
    h.get_by(|n| n.label().as_deref() == Some("+ Action")).click();
    step(&mut h);
    h.get_by(|n| n.label().as_deref() == Some("Group check") && n.role() == Role::Button).click();
    step(&mut h);
    let last = h.state().project.db.items[k].on_use[0].actions.last().cloned();
    assert!(matches!(last, Some(Action::GroupCheck { ref what, dc: 12, .. }) if what == "stealth"), "{last:?}");
    h.state_mut().save_all();
    let saved = Project::load(&root).unwrap();
    assert!(matches!(saved.db.items[k].on_use[0].actions.last(), Some(Action::GroupCheck { .. })));
    let _ = std::fs::remove_dir_all(&root);
}

/// Project › Rules: the survival options (Food and water, Forced march) clicked on, saved.
#[test]
fn survival_rules_are_switched_on_in_the_project_form() {
    let root = example_copy("srd_survival");
    let mut h = harness(&root, Tab::Project, 0);
    assert!(!h.state().project.config.rules.food_water && !h.state().project.config.rules.forced_march, "off in the example");
    h.get_by(|n| n.label().as_deref() == Some("Food and water") && n.role() == Role::CheckBox).click();
    step(&mut h);
    h.get_by(|n| n.label().as_deref() == Some("Forced march") && n.role() == Role::CheckBox).click();
    step(&mut h);
    h.state_mut().save_all();
    let saved = Project::load(&root).unwrap();
    assert!(saved.config.rules.food_water && saved.config.rules.forced_march);
    assert_eq!(saved.config.rules.encumbrance, rustpg2d_engine::srd::survival::Encumbrance::Plain);
    let _ = std::fs::remove_dir_all(&root);
}

/// Database › Backgrounds: a new background, a proficiency added, its feature typed in, saved
/// into defs/backgrounds.ron.
#[test]
fn a_background_is_added_in_the_backgrounds_tab() {
    let root = example_copy("backgrounds");
    let mut h = harness(&root, Tab::Backgrounds, 0);
    let before = h.state().project.db.backgrounds.len();
    h.get_by(|n| n.label().as_deref().is_some_and(|l| l.ends_with("New"))).click();
    step(&mut h);
    assert_eq!(h.state().project.db.backgrounds.len(), before + 1);
    let id = h.state().project.db.backgrounds.last().unwrap().id.clone();
    assert!(id.starts_with("background"), "{id}");
    h.get_by(|n| n.label().as_deref() == Some("+ Proficiency")).click();
    step(&mut h);
    beside(&h, "Feature", |n| n.role() == Role::TextInput).type_text("Old Friends");
    step(&mut h);
    h.state_mut().save_all();
    let saved = Project::load(&root).unwrap();
    let b = saved.db.background(&id).expect("saved");
    assert_eq!((b.proficiencies.clone(), b.feature.as_str()), (vec!["insight".to_string()], "Old Friends"));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn srd_content_asks_for_its_attribution() {
    let root = example_copy("srd_attribution");
    let mut p = Project::load(&root).unwrap();
    assert!(p.config.srd, "the example uses SRD content and says so");
    p.config.srd = false;
    let mut ed = Editor::headless(p);
    let srd = |ed: &Editor| ed.problems.list.iter().filter(|x| x.text.contains("SRD 5.1 content")).count();
    ed.refresh_problems();
    assert_eq!(srd(&ed), 1, "races, backgrounds and stat blocks of the SRD without the setting");
    assert!(ed.problems.list.iter().any(|x| x.text.contains("race dwarf") || x.text.contains("race human")));
    ed.project.config.srd = true;
    ed.refresh_problems();
    assert_eq!(srd(&ed), 0);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_stat_block_gets_an_attack() {
    let root = example_copy("attacks");
    let db = Project::load(&root).unwrap().db;
    let c = db.characters.iter().position(|c| c.id == "guard").expect("the example's guard");
    assert_eq!(db.characters[c].stats.as_ref().unwrap().attacks.len(), 1, "the guard's spear");
    let mut h = harness(&root, Tab::Characters, c);
    step(&mut h);
    h.get_by_label_contains("Add attack").click();
    step(&mut h);
    let attacks = h.state().project.db.characters[c].stats.as_ref().unwrap().attacks.clone();
    assert_eq!((attacks.len(), attacks[1].dice.as_str(), attacks[1].reach_ft), (2, "1d6", 5));
    h.state_mut().save_all();
    let saved = Project::load(&root).unwrap().db.characters[c].stats.clone().unwrap();
    assert_eq!((saved.attacks.len(), saved.attacks[0].name.as_str(), saved.attacks[0].dice.as_str()), (2, "Spear", "1d6+1"));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_stat_block_shows_and_gets_its_traits() {
    let root = example_copy("traits");
    let db = Project::load(&root).unwrap().db;
    let c = db.characters.iter().position(|c| c.id == "wolf").expect("the example's wolf");
    let before = db.characters[c].stats.as_ref().unwrap().features.len();
    assert!(before >= 2, "the SRD wolf's Keen Hearing and Smell, Pack Tactics, Bite");
    let mut h = harness(&root, Tab::Characters, c);
    step(&mut h);
    h.get_by_label_contains("Traits and special actions").click();
    step(&mut h);
    h.get_by_label_contains("Add trait").click();
    step(&mut h);
    h.state_mut().save_all();
    let saved = Project::load(&root).unwrap().db.characters[c].stats.clone().unwrap();
    assert_eq!((saved.features.len(), saved.size), (before + 1, rustpg2d_engine::srd::Size::Medium));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_class_is_edited_and_a_character_gets_one() {
    let root = example_copy("classes");
    let db = Project::load(&root).unwrap().db;
    let k = db.classes.iter().position(|c| c.id == "wizard").expect("the example's wizard");
    let mut h = harness(&root, Tab::Classes, k);
    step(&mut h);
    // The hit die: d6 to d8; a saving throw more.
    beside(&h, "Hit die", |n| n.role() == Role::ComboBox).click();
    step(&mut h);
    h.get_by_label("d8").click();
    step(&mut h);
    h.get_by_label("CON").click();
    step(&mut h);
    let w = h.state().project.db.classes[k].clone();
    assert_eq!(w.hit_die, 8);
    assert!(w.saves.iter().any(|s| s == "con"), "{:?}", w.saves);
    h.state_mut().save_all();
    // A character becomes a party member with a class: the ranger at level 3.
    let c = db.characters.iter().position(|c| c.id == "villager").expect("the example's villager");
    let mut h = harness(&root, Tab::Characters, c);
    step(&mut h);
    beside(&h, "Class", |n| n.role() == Role::ComboBox).click();
    step(&mut h);
    h.get_by_label("Ranger  (ranger)").click();
    step(&mut h);
    let d = h.state().project.db.characters[c].clone();
    assert_eq!((d.class.as_ref().map(|x| x.class.as_str()), d.level), (Some("ranger"), 1));
    beside(&h, "level", |n| n.role() == Role::SpinButton).click();
    step(&mut h);
    h.get_by(|n| n.is_focused()).type_text("3");
    step(&mut h);
    h.press_key(egui::Key::Enter);
    step(&mut h);
    h.state_mut().save_all();
    let saved = Project::load(&root).unwrap().db;
    assert_eq!(saved.class("wizard").map(|w| w.hit_die), Some(8));
    assert_eq!((saved.characters[c].class.as_ref().map(|x| x.class.clone()), saved.characters[c].level), (Some("ranger".to_string()), 3));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_spell_gets_a_level_and_parts() {
    let root = example_copy("spells");
    let db = Project::load(&root).unwrap().db;
    let k = db.spells.iter().position(|s| s.id == "burning-hands").expect("the example's Burning Hands");
    let mut h = harness(&root, Tab::Spells, k);
    step(&mut h);
    // Its level: 1 to 2; a condition added to its parts; a ritual.
    beside(&h, "Level", |n| n.role() == Role::SpinButton).click();
    step(&mut h);
    h.get_by(|n| n.is_focused()).type_text("2");
    step(&mut h);
    h.press_key(egui::Key::Enter);
    step(&mut h);
    h.get_by_label("Condition").click();
    step(&mut h);
    h.get_by_label("Ritual").click();
    step(&mut h);
    let d = h.state().project.db.spells[k].clone();
    assert_eq!(d.level, 2);
    assert!(d.ritual);
    assert!(matches!(d.parts.last(), Some(rustpg2d_engine::skills::SpellPart::Condition { .. })), "{:?}", d.parts);
    h.state_mut().save_all();
    let saved = Project::load(&root).unwrap().db;
    let s = saved.spell("burning-hands").unwrap();
    assert_eq!((s.level, s.ritual, s.parts.len()), (2, true, d.parts.len()));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_stat_block_gets_spell_slots_and_spells() {
    let root = example_copy("stat_spells");
    let db = Project::load(&root).unwrap().db;
    let k = db.characters.iter().position(|c| c.id == "cult-fanatic").expect("the example's cult fanatic");
    let cast = |db: &rustpg2d_engine::defs::Database| db.characters[k].stats.as_ref().unwrap().spellcasting.clone().expect("its spellcasting");
    assert_eq!((cast(&db).dc, cast(&db).slots.clone(), cast(&db).spells.len()), (11, vec![4, 3], 8));
    let mut h = harness(&root, Tab::Characters, k);
    step(&mut h);
    // A third-level slot typed after the others, a spell added.
    field(&h, "4 3").type_text(" 1");
    step(&mut h);
    unfocus(&mut h);
    h.get_by_label_contains("Add spell").click();
    step(&mut h);
    let now = cast(&h.state().project.db);
    assert_eq!((now.slots.clone(), now.spells.len(), now.spells[8].spell.as_str()), (vec![4, 3, 1], 9, "fire-bolt"));
    h.state_mut().save_all();
    let saved = cast(&Project::load(&root).unwrap().db);
    assert_eq!((saved.slots, saved.spells.len()), (vec![4, 3, 1], 9));
    let _ = std::fs::remove_dir_all(&root);
}
