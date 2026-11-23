//! Test-play from here: a right-click on the map offers *Play from here* (a fresh game at that
//! tile) and the same with the **test setup**: level and class, items, flags, variables, quests
//! at a stage, the time and the party, kept per project in the editor's settings. The editor
//! writes a crafted save from it (a fresh hero as a new game has, the setup on top) and starts
//! the game with `--load` on it, which loads it like a saved game (no *Map enter* triggers).

use std::collections::BTreeMap;
use std::path::PathBuf;

use egui_macroquad::egui;
use rustpg2d_engine::entity::EntityKind;
use rustpg2d_engine::inventory::{Inventory, ItemStack};
use rustpg2d_engine::project::{Project, TILE_SIZE};
use rustpg2d_engine::quest::{QuestProgress, QuestStatus};
use rustpg2d_engine::state::{Companion, WorldState};
use serde::{Deserialize, Serialize};

use crate::app::Editor;
use crate::ui::{self as eui, ph};

/// What a test-play starts with (besides the place).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TestSetup {
    /// The hero's level (0 or 1 = a new hero's).
    pub level: u32,
    /// The hero's class id (empty = none, as a new game without the creator).
    pub class: String,
    /// Items in the bag: (item id, count).
    pub items: Vec<(String, u32)>,
    pub flags: Vec<String>,
    pub vars: Vec<(String, i64)>,
    /// Quests started: (quest id, stage, completed).
    pub quests: Vec<(String, u32, bool)>,
    /// The hour (None = the project's start hour) and the day.
    pub hour: Option<f32>,
    pub day: u32,
    /// Party members: names of placed characters (on any map) who join as companions.
    pub party: Vec<String>,
}

impl TestSetup {
    /// For `status` (`testsetup=`): level/class, then counts.
    pub fn summary(&self) -> String {
        format!(
            "L{}{}:items{}:flags{}:vars{}:quests{}:party{}{}",
            self.level.max(1),
            if self.class.is_empty() { String::new() } else { format!("/{}", self.class) },
            self.items.len(),
            self.flags.len(),
            self.vars.len(),
            self.quests.len(),
            self.party.len(),
            self.hour.map_or(String::new(), |h| format!("@{h:.1}h/d{}", self.day)),
        )
    }
}

/// The save a test-play starts from: a new game's hero on `map` at `tile`, with `setup` on top.
/// Party members are looked up by name on the project's maps (the first one found).
pub fn crafted_save(project: &Project, setup: &TestSetup, map: &str, tile: (i32, i32)) -> (WorldState, Vec<String>) {
    let cfg = &project.config;
    let ts = TILE_SIZE as f32;
    let mut w = WorldState {
        current_map: map.to_string(),
        player_pos: (tile.0 as f32 * ts + ts / 2.0, tile.1 as f32 * ts + ts / 2.0),
        hour: setup.hour.unwrap_or(cfg.start_hour),
        day: setup.day,
        abilities: cfg.rules.abilities,
        proficiencies: cfg.rules.proficiencies.iter().map(|p| rustpg2d_engine::srd::abilities::proficiency_id(p)).filter(|p| !p.is_empty()).collect(),
        // (Written as the current version: not converted as a save from before the SRD rules.)
        version: rustpg2d_engine::state::SAVE_VERSION,
        ..Default::default()
    };
    let mut problems = Vec::new();
    if !setup.class.is_empty() {
        if project.db.classes.iter().any(|c| c.id == setup.class) {
            w.class = Some(rustpg2d_engine::classes::HeroClass { class: setup.class.clone(), ..Default::default() });
        } else {
            problems.push(format!("no class '{}'", setup.class));
        }
    }
    if setup.level > 1 {
        w.level = setup.level;
        w.xp = cfg.progression.xp_for_level(setup.level);
    }
    // Items, in stacks as big as the item allows.
    let mut inv = Inventory::default();
    for (id, n) in &setup.items {
        let Some(d) = project.db.items.iter().find(|d| d.id == *id) else {
            problems.push(format!("no item '{id}'"));
            continue;
        };
        let mut left = (*n).max(1);
        while left > 0 {
            let k = left.min(d.max_stack.max(1));
            inv.slots.push(Some(ItemStack::new(id, k)));
            left -= k;
        }
    }
    w.inventory = inv;
    w.flags = setup.flags.iter().filter(|f| !f.trim().is_empty()).map(|f| f.trim().to_string()).collect();
    w.vars = setup.vars.iter().filter(|(v, _)| !v.trim().is_empty()).map(|(v, n)| (v.trim().to_string(), *n)).collect();
    for (k, (q, stage, done)) in setup.quests.iter().enumerate() {
        if project.db.quests.iter().all(|d| d.id != *q) {
            problems.push(format!("no quest '{q}'"));
            continue;
        }
        w.quests.insert(q.clone(), QuestProgress { stage: *stage, status: if *done { QuestStatus::Completed } else { QuestStatus::Active }, order: k as u64 + 1 });
    }
    // The party: placed characters by name.
    if !setup.party.is_empty() {
        let mut maps = project.list_maps();
        // (The test map first: a name there wins.)
        maps.sort_by_key(|m| m != map);
        let mut found: BTreeMap<String, (String, u32)> = BTreeMap::new();
        for m in maps {
            if setup.party.iter().all(|p| found.contains_key(p)) {
                break;
            }
            let Ok(loaded) = project.load_map(&m) else { continue };
            for e in loaded.entities.iter().filter(|e| e.kind == EntityKind::Character) {
                if setup.party.contains(&e.name) && !found.contains_key(&e.name) {
                    found.insert(e.name.clone(), (m.clone(), e.uid));
                }
            }
        }
        for name in &setup.party {
            match found.get(name) {
                Some((m, uid)) => w.companions.push(Companion { map: m.clone(), uid: *uid, name: name.clone(), ..Default::default() }),
                None => problems.push(format!("no character named '{name}' on the maps")),
            }
        }
    }
    (w, problems)
}

/// Where the editor writes the save for the game (its own folder, never the project's saves).
fn save_path() -> PathBuf {
    crate::files::config_dir().unwrap_or_else(std::env::temp_dir).join("testplay.ron")
}

/// The key of the open project in the editor's settings: its project file, else its folder.
fn project_key(ed: &Editor) -> String {
    ed.project_file.as_ref().map_or_else(|| ed.project.root.display().to_string(), |pf| pf.path.display().to_string())
}

/// The right-click menu on the map and the Test setup window.
#[derive(Default)]
pub struct TestPlay {
    /// The open project's setup (loaded from the settings on first use).
    pub setup: Option<TestSetup>,
    /// The right-click menu: its tile and where it shows (None = closed), and whether it was
    /// opened this frame (so the opening click doesn't close it).
    pub menu: Option<((i32, i32), Option<egui::Pos2>, bool)>,
    pub window: bool,
}

impl Editor {
    /// The open project's test setup.
    pub fn test_setup(&mut self) -> &mut TestSetup {
        if self.testplay.setup.is_none() {
            let key = project_key(self);
            self.testplay.setup = Some(crate::files::load_settings().test_setups.get(&key).cloned().unwrap_or_default());
        }
        self.testplay.setup.as_mut().unwrap()
    }

    /// Keeps the setup in the editor's settings.
    pub fn save_test_setup(&mut self) {
        let setup = self.test_setup().clone();
        let key = project_key(self);
        let mut s = crate::files::load_settings();
        if setup == TestSetup::default() {
            s.test_setups.remove(&key);
        } else {
            s.test_setups.insert(key, setup);
        }
        crate::files::save_settings(&s);
    }

    /// A right-click on the map with the Select tool: the menu at tile `tile`.
    pub fn open_here_menu(&mut self, tile: (i32, i32)) {
        self.testplay.menu = Some((tile, None, true));
    }

    /// Play-tests the open map from tile `tile`: a fresh game, or with the test setup.
    pub fn play_here(&mut self, tile: (i32, i32), with_setup: bool) {
        let Some(map) = self.doc.as_ref().map(|d| d.map.name.clone()) else { return };
        if !with_setup {
            let ts = TILE_SIZE as f32;
            self.play_at(&map, (tile.0 as f32 * ts + ts / 2.0, tile.1 as f32 * ts + ts / 2.0), None);
            return;
        }
        // (Saved first: the party is looked up in the map files.)
        self.save_all();
        let setup = self.test_setup().clone();
        let (world, problems) = crafted_save(&self.project, &setup, &map, tile);
        let path = save_path();
        let written = world.to_ron().map_err(|e| format!("{e:#}")).and_then(|text| rustpg2d_engine::sys::write(&path, text.as_bytes()).map_err(|e| format!("{e:#}")));
        if let Err(e) = written {
            self.set_status(format!("Could not write the test save: {e}"));
            return;
        }
        self.play_at(&map, world.player_pos, Some(&path));
        if !problems.is_empty() {
            self.set_status(format!("Play-test started; the test setup has problems: {}", problems.join(", ")));
        }
    }

    /// For `status`: `testsetup=` the setup's summary, `+window` / `+menu@X,Y` when open.
    pub fn testplay_status(&self) -> String {
        let setup = self.testplay.setup.clone().unwrap_or_else(|| crate::files::load_settings().test_setups.get(&project_key(self)).cloned().unwrap_or_default());
        let mut s = setup.summary();
        if self.testplay.window {
            s += "+window";
        }
        if let Some(((x, y), _, _)) = self.testplay.menu {
            s += &format!("+menu@{x},{y}");
        }
        s
    }

    /// The right-click menu and the Test setup window (each frame).
    pub fn testplay_ui(&mut self, ctx: &egui::Context) {
        if let Some((tile, pos, fresh)) = self.testplay.menu {
            let pos = pos.or_else(|| ctx.input(|i| i.pointer.latest_pos())).unwrap_or(egui::pos2(200.0, 200.0));
            let mut pick = None;
            let area = egui::Area::new(egui::Id::new("here_menu")).order(egui::Order::Foreground).fixed_pos(pos).show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.weak(format!("Tile {}, {}", tile.0, tile.1));
                    if ui.button(eui::t(ph::PLAY, "Play from here")).on_hover_text("A new game on this tile").clicked() {
                        pick = Some(1);
                    }
                    if ui.button(eui::t(ph::PLAY_CIRCLE, "Play from here with the test setup")).on_hover_text("On this tile with the level, items, flags, quests, time and party of the test setup").clicked() {
                        pick = Some(2);
                    }
                    if ui.button(eui::t(ph::SLIDERS, "Test setup…")).on_hover_text("What a test-play from here starts with").clicked() {
                        pick = Some(3);
                    }
                });
            });
            let outside = ctx.input(|i| i.pointer.any_pressed()) && !ctx.pointer_latest_pos().is_some_and(|p| area.response.rect.contains(p));
            self.testplay.menu = if pick.is_some() || (outside && !fresh) || ctx.input(|i| i.key_pressed(egui::Key::Escape)) { None } else { Some((tile, Some(pos), false)) };
            match pick {
                Some(1) => self.play_here(tile, false),
                Some(2) => self.play_here(tile, true),
                Some(3) => self.testplay.window = true,
                _ => {}
            }
        }
        if self.testplay.window {
            let mut open = true;
            let refs = self.refs.clone();
            let classes: Vec<String> = self.project.db.classes.iter().map(|c| c.id.clone()).collect();
            let entity_names = self.refs.entity_names.clone();
            let start_hour = self.project.config.start_hour;
            let mut setup = self.test_setup().clone();
            egui::Window::new("Test setup").id(egui::Id::new("test_setup")).open(&mut open).default_width(460.0).show(ctx, |ui| {
                setup_ui(ui, &mut setup, &refs, &classes, &entity_names, start_hour);
            });
            let changed = setup != *self.test_setup();
            *self.test_setup() = setup;
            if !open || (changed && !ctx.input(|i| i.pointer.any_down())) {
                self.save_test_setup();
            }
            self.testplay.window = open;
        }
    }
}

/// The Test setup window's contents.
fn setup_ui(ui: &mut egui::Ui, s: &mut TestSetup, refs: &crate::widgets::Refs, classes: &[String], names: &[String], start_hour: f32) {
    ui.weak("Right-click the map › Play from here with the test setup starts a new hero there with these.");
    egui::Grid::new("ts_hero").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
        ui.label("Level");
        let mut level = s.level.max(1);
        if ui.add(egui::DragValue::new(&mut level).range(1..=20)).changed() {
            s.level = level;
        }
        ui.end_row();
        ui.label("Class");
        egui::ComboBox::from_id_salt("ts_class").selected_text(if s.class.is_empty() { "(none)" } else { s.class.as_str() }).show_ui(ui, |ui| {
            ui.selectable_value(&mut s.class, String::new(), "(none)");
            for c in classes {
                ui.selectable_value(&mut s.class, c.clone(), c);
            }
        });
        ui.end_row();
        ui.label("Time");
        ui.horizontal(|ui| {
            let mut own = s.hour.is_some();
            if ui.checkbox(&mut own, "").on_hover_text("Off: the project's start hour").changed() {
                s.hour = own.then_some(start_hour);
            }
            if let Some(h) = &mut s.hour {
                ui.add(egui::Slider::new(h, 0.0..=23.99).suffix(" h").max_decimals(1));
                ui.label("day");
                ui.add(egui::DragValue::new(&mut s.day).range(0..=9999));
            } else {
                ui.weak(format!("the start hour ({start_hour:.1} h)"));
            }
        });
        ui.end_row();
    });
    ui.separator();
    list(ui, "Items", "ts_items", &mut s.items, (String::new(), 1), |ui, k, (id, n)| {
        crate::widgets::suggest(ui, &format!("ts_item{k}"), id, &refs.items, 180.0);
        ui.add(egui::DragValue::new(n).range(1..=9999)).on_hover_text("How many");
    });
    list(ui, "Flags", "ts_flags", &mut s.flags, String::new(), |ui, k, f| {
        crate::widgets::suggest(ui, &format!("ts_flag{k}"), f, &refs.all_flags, 220.0);
    });
    list(ui, "Variables", "ts_vars", &mut s.vars, (String::new(), 0), |ui, k, (v, n)| {
        crate::widgets::suggest(ui, &format!("ts_var{k}"), v, &refs.vars, 180.0);
        ui.add(egui::DragValue::new(n));
    });
    list(ui, "Quests", "ts_quests", &mut s.quests, (String::new(), 0, false), |ui, k, (q, stage, done)| {
        crate::widgets::suggest(ui, &format!("ts_quest{k}"), q, &refs.quests, 140.0);
        crate::widgets::quest_stage_field(ui, &format!("ts_stage{k}"), q, stage, refs);
        ui.checkbox(done, "done");
    });
    ui.weak("Party: placed characters by name (on any map), who join as companions.");
    list(ui, "Party", "ts_party", &mut s.party, String::new(), |ui, k, name| {
        crate::widgets::suggest(ui, &format!("ts_party{k}"), name, names, 180.0);
    });
    ui.separator();
    if ui.button(eui::t(ph::ERASER, "Clear")).on_hover_text("Back to a new hero's start").clicked() {
        *s = TestSetup::default();
    }
}

/// A heading with its rows (each with a remove button) and an *Add* button.
fn list<T: Clone>(ui: &mut egui::Ui, title: &str, salt: &str, rows: &mut Vec<T>, new: T, row: impl Fn(&mut egui::Ui, usize, &mut T)) {
    ui.horizontal(|ui| {
        ui.strong(title);
        if ui.small_button(eui::t(ph::PLUS, "Add")).clicked() {
            rows.push(new);
        }
    });
    let mut remove = None;
    for (k, r) in rows.iter_mut().enumerate() {
        ui.push_id((salt, k), |ui| {
            ui.horizontal(|ui| {
                if ui.small_button(ph::TRASH).on_hover_text("Remove").clicked() {
                    remove = Some(k);
                }
                row(ui, k, r);
            });
        });
    }
    if let Some(k) = remove {
        rows.remove(k);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_crafted_save_has_the_setup_and_finds_the_party() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../example_project");
        let project = Project::load(&root).unwrap();
        let setup = TestSetup {
            level: 3,
            class: project.db.classes.first().map(|c| c.id.clone()).unwrap_or_default(),
            items: vec![("bread".into(), 3), ("nonsense".into(), 1)],
            flags: vec!["gate_open".into()],
            vars: vec![("gold_found".into(), 7)],
            quests: vec![(project.db.quests[0].id.clone(), 2, false)],
            hour: Some(21.5),
            day: 3,
            party: vec!["guard".into()],
        };
        let (w, problems) = crafted_save(&project, &setup, "village", (10, 12));
        assert_eq!((w.current_map.as_str(), w.player_pos), ("village", (336.0, 400.0)));
        assert_eq!((w.level, w.xp > 0, w.hour, w.day), (3, true, 21.5, 3));
        assert_eq!(w.inventory.count("bread"), 3);
        assert!(w.flags.contains("gate_open") && w.vars["gold_found"] == 7);
        assert_eq!(w.quests[&project.db.quests[0].id].stage, 2);
        assert_eq!(w.companions.len(), 1, "{problems:?}");
        assert_eq!(w.companions[0].map, "village");
        assert_eq!(problems, vec!["no item 'nonsense'".to_string()]);
        // It reads back as a save.
        let back = WorldState::from_ron(&w.to_ron().unwrap()).unwrap();
        assert_eq!(back.version, rustpg2d_engine::state::SAVE_VERSION);
        assert_eq!(back.inventory.count("bread"), 3);
    }
}
