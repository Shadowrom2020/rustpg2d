//! Guided tours: a tour is a list of steps kept as data. Each step says what to do, names the
//! control to use (the editor points at it), and is ticked by a check of what the project is
//! like now: a map is open, something was painted, a character stands on the map and has
//! something to say. A step counts once the steps before it do, so a tour is followed in
//! order; one can be skipped.

use std::collections::BTreeSet;

use rustpg2d_engine::entity::EntityKind;
use rustpg2d_engine::events::{Action, Trigger};

use crate::workspace::Workspace;

/// What ticks a step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Check {
    /// A map is open to work on.
    MapOpen,
    /// The open map's ground was painted since the tour began.
    GroundPainted,
    /// Something more stands on the open map than when the tour began.
    ThingPlaced,
    /// Nothing waits to be saved.
    Saved,
    /// The game was started from the editor since the tour began.
    Played,
    /// The project has a character it didn't have.
    CharacterMade,
    /// A character more stands on the open map.
    CharacterPlaced,
    /// Someone on the open map says something when talked to (a Dialog or Choice action).
    SomeoneTalks,
    /// The project has a quest it didn't have.
    QuestMade,
    /// A new quest's first stage was written (it isn't the line a new quest starts with).
    QuestHasStages,
    /// An event starts or advances a new quest.
    QuestStarted,
}

pub struct Step {
    /// What to do, in a few words (the list's line).
    pub title: &'static str,
    /// How, with the names of what to click.
    pub how: &'static str,
    /// The control the editor points at (`tool:Terrain`, `browser:map`, `menu:Play`; "" = none).
    pub anchor: &'static str,
    pub check: Check,
    /// Where the manual says more: a chapter's file and words of the section's heading.
    pub help: (&'static str, &'static [&'static str]),
}

pub struct Tour {
    pub id: &'static str,
    pub title: &'static str,
    pub about: &'static str,
    pub steps: &'static [Step],
}

pub const TOURS: [Tour; 3] = [
    Tour {
        id: "first-map",
        title: "Your first map",
        about: "Open a map, paint its ground, put something on it, and walk on it in the game.",
        steps: &[
            Step { title: "Open a map", how: "In the Project browser at the left, open World, then Maps, and click a map (a new game has one called start). It opens as a tab.", anchor: "browser:map", check: Check::MapOpen, help: ("03-maps.md", &["creating"]) },
            Step { title: "Paint some ground", how: "Click Terrain in the bar above the map (or press P), pick a material at the left (Dirt, Water, Cobblestone…) and drag over the map.", anchor: "tool:Terrain", check: Check::GroundPainted, help: ("03-maps.md", &["ground tool", "painted ground"]) },
            Step { title: "Put something on the map", how: "Click Place (N), pick a prop at the left (a barrel, a chest, a tree), then click the map where it should stand.", anchor: "tool:Place", check: Check::ThingPlaced, help: ("04-objects-and-zones.md", &["placing"]) },
            Step { title: "Save", how: "Press Ctrl+S: the map and everything else is written. (The status bar at the bottom says unsaved changes until then.)", anchor: "status:saved", check: Check::Saved, help: ("02-the-editor.md", &["saving"]) },
            Step { title: "Play it", how: "Press F6 (Play here): the game starts in a tab, with the hero on the map you have open. Arrow keys or WASD walk; the tab's bar restarts and pauses it.", anchor: "menu:Play", check: Check::Played, help: ("02-the-editor.md", &["play-test", "play"]) },
        ],
    },
    Tour {
        id: "talking-character",
        title: "A character who talks",
        about: "Make a character, stand them on the map, give them something to say, and talk to them in the game.",
        steps: &[
            Step { title: "Open a map", how: "Open the map the character will stand on (Project browser, World, Maps).", anchor: "browser:map", check: Check::MapOpen, help: ("03-maps.md", &["creating"]) },
            Step { title: "Make a character", how: "Press Ctrl+N, pick Character, give it a name. As one of the asset library starts it from a villager, a guard or a merchant with its figure; blank starts with the mannequin.", anchor: "browser:character", check: Check::CharacterMade, help: ("06-characters.md", &[]) },
            Step { title: "Stand them on the map", how: "On the map's tab click Place (N), pick Characters at the left and your character, then click the map.", anchor: "tool:Place", check: Check::CharacterPlaced, help: ("04-objects-and-zones.md", &["placing"]) },
            Step { title: "Give them something to say", how: "Click Select (V) and the character. In the Inspector at the right, under its events, add an event: When Interact, Do Say, and type the line.", anchor: "tool:Select", check: Check::SomeoneTalks, help: ("07-events.md", &["triggers"]) },
            Step { title: "Talk to them", how: "Press F6 (Play here), walk up to the character and press E.", anchor: "menu:Play", check: Check::Played, help: ("02-the-editor.md", &["play-test", "play"]) },
        ],
    },
    Tour {
        id: "a-quest",
        title: "A quest",
        about: "Make a quest with a stage, have someone hand it out, and find it in the journal.",
        steps: &[
            Step { title: "Make a quest", how: "Press Ctrl+N, pick Quest (under Story) and give it a title.", anchor: "browser:quest", check: Check::QuestMade, help: ("05-database.md", &["quests"]) },
            Step { title: "Say what is to be done", how: "In the quest's tab, write its first stage in your own words: the line the journal shows for what is to be done next (Find the lost ring). A new quest starts with two stages to change.", anchor: "", check: Check::QuestHasStages, help: ("05-database.md", &["quests"]) },
            Step { title: "Have someone hand it out", how: "Open a map, select a character (or place one), and in the Inspector add an event: When Interact, Do Quest: your quest, Start. A Say before it makes them ask.", anchor: "browser:map", check: Check::QuestStarted, help: ("07-events.md", &["quest"]) },
            Step { title: "Take it in the game", how: "Press F6 (Play here), talk to the character with E, then press J: the journal lists the quest with its stage.", anchor: "menu:Play", check: Check::Played, help: ("15-players-guide.md", &["journal", "quest"]) },
        ],
    },
];

/// The first stage a new quest starts with (`new_def`): a tour waits for another.
const FRESH_STAGE: &str = "Find out more";

pub fn tour_named(id: &str) -> Option<usize> {
    TOURS.iter().position(|t| t.id == id)
}

/// What a tour compares with: the project as it was when the tour began.
#[derive(Clone, Debug, Default, PartialEq)]
struct Start {
    /// The open map then, with its ground's edit count, things and characters on it.
    map: Option<(String, u64, usize, usize)>,
    characters: BTreeSet<String>,
    quests: BTreeSet<String>,
}

/// A tour being followed.
#[derive(Clone, Debug, PartialEq)]
pub struct TourRun {
    pub tour: usize,
    start: Start,
    /// The game was started from the editor since.
    pub played: bool,
    pub skipped: BTreeSet<usize>,
}

fn talks(triggers: &[Trigger]) -> bool {
    triggers.iter().any(|t| t.actions.iter().any(|a| matches!(a, Action::Dialog { .. } | Action::Choice { .. })))
}

fn starts_quest(triggers: &[Trigger], quests: &dyn Fn(&str) -> bool) -> bool {
    triggers.iter().any(|t| t.actions.iter().any(|a| matches!(a, Action::Quest { quest, .. } if quests(quest))))
}

impl Workspace {
    /// The open map's ground's edit count, things and characters (no map of the project open: none).
    fn tour_map(&self) -> Option<(String, u64, usize, usize)> {
        let doc = self.doc.as_ref().filter(|d| !crate::structdoc::is_scratch(&d.map.name))?;
        let m = &doc.map;
        let (cw, ch) = m.ground.chunks_size();
        let rev: u64 = (0..ch).flat_map(|y| (0..cw).map(move |x| (x, y))).map(|(x, y)| m.ground.rev(x, y) as u64).sum();
        Some((m.name.clone(), rev, m.entities.len() + m.decor.len(), m.entities.iter().filter(|e| e.kind == EntityKind::Character).count()))
    }

    fn tour_check(&self, run: &TourRun, check: Check) -> bool {
        let db = &self.project.db;
        let now = self.tour_map();
        // (Compared on the map the tour began on; another map opened since starts from nothing.)
        let then = run.start.map.clone().filter(|m| now.as_ref().is_some_and(|n| n.0 == m.0)).unwrap_or_default();
        let new_quest = |id: &str| !run.start.quests.contains(id) && db.quests.iter().any(|q| q.id == id);
        match check {
            Check::MapOpen => now.is_some(),
            Check::GroundPainted => now.is_some_and(|n| n.1 != then.1 || (run.start.map.as_ref().is_none_or(|m| m.0 != n.0) && n.1 > 0 && self.doc.as_ref().is_some_and(|d| d.is_dirty()))),
            Check::ThingPlaced => now.is_some_and(|n| n.2 > then.2),
            Check::Saved => !self.is_dirty(),
            Check::Played => run.played,
            Check::CharacterMade => db.characters.iter().any(|c| !run.start.characters.contains(&c.id)),
            Check::CharacterPlaced => now.is_some_and(|n| n.3 > then.3),
            Check::SomeoneTalks => self.doc.as_ref().is_some_and(|d| d.map.entities.iter().any(|e| talks(&e.triggers) || (e.kind == EntityKind::Character && talks(db.def_triggers(e.kind, &e.def))))),
            Check::QuestMade => db.quests.iter().any(|q| !run.start.quests.contains(&q.id)),
            Check::QuestHasStages => db.quests.iter().any(|q| !run.start.quests.contains(&q.id) && q.stages.first().is_some_and(|s| !s.trim().is_empty() && s != FRESH_STAGE)),
            Check::QuestStarted => {
                let on_map = self.doc.as_ref().is_some_and(|d| starts_quest(&d.map.triggers, &new_quest) || d.map.entities.iter().any(|e| starts_quest(&e.triggers, &new_quest)));
                on_map || db.characters.iter().any(|c| starts_quest(&c.triggers, &new_quest)) || db.props.iter().any(|p| starts_quest(&p.triggers, &new_quest))
            }
        }
    }
}

impl TourRun {
    /// Begins tour `tour` on the project as it is.
    pub fn start(ws: &Workspace, tour: usize) -> TourRun {
        let db = &ws.project.db;
        let start = Start { map: ws.tour_map(), characters: db.characters.iter().map(|c| c.id.clone()).collect(), quests: db.quests.iter().map(|q| q.id.clone()).collect() };
        TourRun { tour: tour.min(TOURS.len() - 1), start, played: false, skipped: BTreeSet::new() }
    }

    pub fn tour(&self) -> &'static Tour {
        &TOURS[self.tour]
    }

    /// Each step: done (its check holds and every step before it is done or skipped).
    pub fn done(&self, ws: &Workspace) -> Vec<bool> {
        let mut out = Vec::new();
        let mut so_far = true;
        for (k, step) in self.tour().steps.iter().enumerate() {
            let done = so_far && ws.tour_check(self, step.check);
            out.push(done);
            so_far = done || self.skipped.contains(&k);
        }
        out
    }

    /// The step to do now (`None`: the tour is through).
    pub fn current(&self, ws: &Workspace) -> Option<usize> {
        let done = self.done(ws);
        (0..done.len()).find(|k| !done[*k] && !self.skipped.contains(k))
    }

    /// `first-map 2/5 [x x - . .] now=Put something on the map` (x done, - skipped, . to do).
    pub fn status(&self, ws: &Workspace) -> String {
        let done = self.done(ws);
        let marks: Vec<&str> = (0..done.len()).map(|k| if done[k] { "x" } else if self.skipped.contains(&k) { "-" } else { "." }).collect();
        let now = self.current(ws).map_or("through".to_string(), |k| self.tour().steps[k].title.to_string());
        format!("{} {}/{} [{}] now={now}", self.tour().id, done.iter().filter(|d| **d).count(), done.len(), marks.join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustpg2d_engine::refactor::RefKind;

    /// A new project with the basics of the library (a hero, grass on the first map), as the
    /// home screen makes one.
    fn fresh(tag: &str) -> (Workspace, std::path::PathBuf) {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir = std::env::temp_dir().join(format!("rustpg2d-tours-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        crate::files::create_project(&dir).unwrap();
        let mut ws = Workspace::new(dir.clone(), crate::Gpu::No).unwrap();
        ws.lib.lib = Some(rustpg2d_engine::project::Project::load(&root.join("crates/editor/assets/starter_library")).unwrap());
        ws.start_with_basics("Tour game").unwrap();
        ws.close_map("start");
        ws.history_frame(None);
        (ws, dir)
    }

    #[test]
    fn the_first_map_tour_is_followed_step_by_step() {
        let (mut ws, dir) = fresh("map");
        let mut run = TourRun::start(&ws, tour_named("first-map").unwrap());
        assert_eq!(run.status(&ws), "first-map 0/5 [. . . . .] now=Open a map");
        // Each thing done ticks its step, and only in order: saving counts once the steps before it do.
        ws.open_map("start");
        assert_eq!(run.current(&ws), Some(1));
        let ts = ws.ts();
        let value = ws.ground_value("dirt").unwrap();
        let cells = ws.ground_dab(crate::mapedit::Brush { size: 24.0, square: false, snap: false }, gfx::prelude::vec2(5.0 * ts, 5.0 * ts), value);
        ws.ground_commit(cells, value);
        assert_eq!(run.status(&ws), "first-map 2/5 [x x . . .] now=Put something on the map");
        ws.place_entity(EntityKind::Prop, "barrel", gfx::prelude::vec2(6.5 * ts, 7.0 * ts)).unwrap();
        ws.history_frame(None);
        assert_eq!(run.current(&ws), Some(3), "placed: now it wants saving");
        ws.save_all();
        assert_eq!(run.current(&ws), Some(4));
        run.played = true;
        assert_eq!(run.status(&ws), "first-map 5/5 [x x x x x] now=through");
        assert_eq!(run.current(&ws), None);
        // A step can be skipped: the ones after it count all the same.
        let mut again = TourRun::start(&ws, 0);
        assert_eq!(again.current(&ws), Some(1), "the map is open already");
        again.skipped.insert(1);
        again.skipped.insert(2);
        again.played = true;
        assert_eq!(again.status(&ws), "first-map 3/5 [x - - x x] now=through");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_character_who_talks_and_a_quest_are_followed() {
        let (mut ws, dir) = fresh("talk");
        let mut run = TourRun::start(&ws, tour_named("talking-character").unwrap());
        ws.open_map("start");
        assert_eq!(run.current(&ws), Some(1));
        // A character from the library, placed, with a line to say.
        let id = ws.make_from_library(RefKind::Character, "villager", "Mara").unwrap();
        ws.history_frame(None);
        assert_eq!(run.current(&ws), Some(2));
        let ts = ws.ts();
        let uid = ws.place_entity(EntityKind::Character, &id, gfx::prelude::vec2(8.5 * ts, 8.0 * ts)).unwrap();
        ws.history_frame(None);
        assert_eq!(run.current(&ws), Some(3), "placed: now she needs something to say");
        let hello: Trigger = serde_json::from_value(serde_json::json!({ "on": "Interact", "actions": [{ "Dialog": { "speaker": "Mara", "text": "Lovely day." } }] })).unwrap();
        ws.doc.as_mut().unwrap().map.entities.iter_mut().find(|e| e.uid == uid).unwrap().triggers.push(hello);
        assert_eq!(run.current(&ws), Some(4));
        run.played = true;
        assert_eq!(run.current(&ws), None);
        // A quest: made, a stage, handed out by her.
        let mut quest = TourRun::start(&ws, tour_named("a-quest").unwrap());
        assert_eq!(quest.status(&ws), "a-quest 0/4 [. . . .] now=Make a quest");
        let q = ws.make_def(RefKind::Quest, "The lost ring", None).unwrap();
        ws.history_frame(None);
        assert_eq!(quest.current(&ws), Some(1));
        assert_eq!(ws.project.db.quests.iter().find(|x| x.id == q).unwrap().stages[0], FRESH_STAGE, "what a new quest starts with is what the tour waits to see changed");
        let mut value = ws.def_value(RefKind::Quest, &q).unwrap();
        value["stages"] = serde_json::json!(["Find the lost ring"]);
        ws.set_def_value(RefKind::Quest, &q, value).unwrap();
        assert_eq!(quest.current(&ws), Some(2));
        let give: Trigger = serde_json::from_value(serde_json::json!({ "on": "Interact", "actions": [{ "Quest": { "quest": q, "op": "Start", "stage": 0 } }] })).unwrap();
        ws.doc.as_mut().unwrap().map.entities.iter_mut().find(|e| e.uid == uid).unwrap().triggers.push(give);
        assert_eq!(quest.current(&ws), Some(3));
        quest.played = true;
        assert_eq!(quest.status(&ws), "a-quest 4/4 [x x x x] now=through");
        // Every step's help names a chapter of the manual, and every tour has an id of its own.
        for t in &TOURS {
            assert_eq!(tour_named(t.id).map(|k| TOURS[k].title), Some(t.title));
            for s in t.steps {
                assert!(crate::manual::chapter_named(s.help.0).is_some(), "{}: {}", t.id, s.help.0);
            }
        }
        let _ = std::fs::remove_dir_all(dir);
    }
}
