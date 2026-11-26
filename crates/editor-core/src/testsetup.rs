//! What a test-play starts with: the hero's level and class, items, flags, quests, the hour, the party.

use std::collections::BTreeMap;
use std::path::PathBuf;

use rustpg2d_engine::entity::EntityKind;
use rustpg2d_engine::inventory::{Inventory, ItemStack};
use rustpg2d_engine::project::{Project, TILE_SIZE};
use rustpg2d_engine::quest::{QuestProgress, QuestStatus};
use rustpg2d_engine::state::{Companion, WorldState};
use serde::{Deserialize, Serialize};

use crate::workspace::Workspace;

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

impl TestSetup {
    /// The setup a running game would be: its hero's level and class, bag, flags, variables,
    /// quests, clock and party (*Keep as the test setup* of the Play panel's State).
    pub fn of(w: &WorldState) -> TestSetup {
        let mut items: BTreeMap<String, u32> = BTreeMap::new();
        for st in w.inventory.slots.iter().flatten() {
            *items.entry(st.item.clone()).or_default() += st.count;
        }
        let mut quests: Vec<(&String, &QuestProgress)> = w.quests.iter().collect();
        quests.sort_by_key(|(_, q)| q.order);
        TestSetup {
            level: w.level,
            class: w.class.as_ref().map(|c| c.class.clone()).unwrap_or_default(),
            items: items.into_iter().collect(),
            flags: w.flags.iter().cloned().collect(),
            vars: w.vars.iter().map(|(k, v)| (k.clone(), *v)).collect(),
            quests: quests.into_iter().map(|(id, q)| (id.clone(), q.stage, q.status == QuestStatus::Completed)).collect(),
            hour: Some(w.hour),
            day: w.day,
            party: w.companions.iter().filter(|c| !c.name.is_empty()).map(|c| c.name.clone()).collect(),
        }
    }
}

/// The save a test-play starts from: a new game's hero on `map` at `tile`, with `setup` on top.
/// Party members are looked up by name on the project's maps (the first one found). Returns
/// the save and what of the setup the project doesn't have.
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

impl Workspace {
    /// The key of the open project in the editor's settings: its project file, else its folder.
    fn setup_key(&self) -> String {
        self.project_file.as_ref().map_or_else(|| self.project.root.display().to_string(), |pf| pf.path.display().to_string())
    }

    /// The open project's test setup (kept in the editor's settings, not in the project).
    pub fn test_setup(&self) -> TestSetup {
        crate::files::load_settings().test_setups.get(&self.setup_key()).cloned().unwrap_or_default()
    }

    /// Keeps `setup` as the open project's test setup (an empty one is forgotten).
    pub fn set_test_setup(&mut self, setup: TestSetup) {
        let key = self.setup_key();
        let mut s = crate::files::load_settings();
        if setup == TestSetup::default() {
            s.test_setups.remove(&key);
        } else {
            s.test_setups.insert(key, setup);
        }
        crate::files::save_settings(&s);
    }

    /// Writes the save a test-play with the test setup starts from, with the hero on `tile` of
    /// `map` (into the editor's own folder, never the project's saves). Returns the file and
    /// what of the setup the project doesn't have. Save everything first: the party is looked
    /// up in the map files.
    pub fn write_test_save(&self, map: &str, tile: (i32, i32)) -> Result<(PathBuf, Vec<String>), String> {
        let (world, problems) = crafted_save(&self.project, &self.test_setup(), map, tile);
        let path = crate::files::config_dir().unwrap_or_else(std::env::temp_dir).join("testplay.ron");
        let text = world.to_ron().map_err(|e| format!("{e:#}"))?;
        rustpg2d_engine::sys::write(&path, text.as_bytes()).map_err(|e| format!("{e:#}"))?;
        Ok((path, problems))
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
        // It reads back as a save, and a running game's state gives the setup back.
        let back = WorldState::from_ron(&w.to_ron().unwrap()).unwrap();
        assert_eq!(back.version, rustpg2d_engine::state::SAVE_VERSION);
        assert_eq!(back.inventory.count("bread"), 3);
        let again = TestSetup::of(&back);
        assert_eq!((again.level, again.class.as_str(), again.hour, again.day), (3, setup.class.as_str(), Some(21.5), 3));
        assert_eq!(again.items, [("bread".to_string(), 3)]);
        assert_eq!((again.flags, again.vars, again.quests, again.party), (setup.flags, setup.vars, setup.quests, setup.party));
    }
}
