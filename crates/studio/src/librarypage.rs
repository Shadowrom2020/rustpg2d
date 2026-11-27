//! The Library tab: what the asset library holds, by kind, searched; what is ticked is added
//! to the project with everything it needs (a character's model, a prop's pictures and
//! sounds, the script functions its events call). The other way round a definition's tab
//! sends it to the library.

use eframe::egui;
use editor_core::Workspace;
use rustpg2d_engine::library::{AssetRef, KINDS};
use rustpg2d_engine::refactor::RefKind;

#[derive(Default)]
pub struct LibraryPage {
    filter: String,
    /// The kind shown (`None`: every kind).
    kind: Option<RefKind>,
    /// Only what the project doesn't have yet.
    only_new: bool,
    pub ticked: Vec<AssetRef>,
    /// What the last *Add* said.
    pub said: Option<Result<String, String>>,
}

impl LibraryPage {
    /// `assets=N ticked=N said=…` for the script's `status`.
    pub fn status(&self, ws: &Workspace) -> String {
        let n: usize = ws.lib.lib.as_ref().map_or(0, |lib| KINDS.iter().map(|k| editor_core::defs::list(&lib.db, *k).len()).sum());
        format!("assets={n} ticked={} said={}", self.ticked.len(), match &self.said {
            None => "-".to_string(),
            Some(Ok(m)) => m.chars().take(80).collect(),
            Some(Err(e)) => format!("failed: {}", e.chars().take(80).collect::<String>()),
        })
    }

    /// Ticks a library asset (the script's `library tick KIND ID`). False when the library has none such.
    pub fn tick(&mut self, ws: &mut Workspace, kind: RefKind, id: &str) -> bool {
        ws.lib.ensure_loaded();
        let there = ws.lib.lib.as_ref().is_some_and(|lib| lib.db.has(kind, id));
        if there && !self.ticked.iter().any(|(k, i)| *k == kind && i == id) {
            self.ticked.push((kind, id.to_string()));
        }
        there
    }

    /// Adds what is ticked to the project (with what it needs), as one step.
    pub fn add(&mut self, ws: &mut Workspace) {
        if self.ticked.is_empty() {
            return;
        }
        let n = self.ticked.len();
        ws.begin(&if n == 1 { format!("Add {} from the library", self.ticked[0].1) } else { format!("Add {n} from the library") });
        let r = ws.library_to_project(&self.ticked).map_err(|e| format!("{e:#}"));
        ws.set_status(match &r {
            Ok(m) => m.clone(),
            Err(e) => format!("Not added: {e}"),
        });
        if r.is_ok() {
            self.ticked.clear();
            ws.refresh_refs();
            ws.stale = true;
        }
        self.said = Some(r);
    }

    /// The tab. Returns a definition of the project to open (one that was added, clicked).
    pub fn show(&mut self, ui: &mut egui::Ui, ws: &mut Workspace) -> Option<AssetRef> {
        ws.lib.ensure_loaded();
        let mut open = None;
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new("Asset library").strong());
            ui.add(egui::TextEdit::singleline(&mut self.filter).hint_text("Search").desired_width(200.0)).widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, "Search the library"));
            ui.checkbox(&mut self.only_new, "Only what the project doesn't have");
        });
        if let Some(e) = &ws.lib.error {
            ui.colored_label(ui.visuals().error_fg_color, e.as_str());
            return None;
        }
        let Some(lib) = &ws.lib.lib else { return None };
        ui.weak(format!("{}. What you add comes with what it needs: a character's model, a prop's pictures and sounds, the script functions its events call.", lib.root.display()));
        // The kinds, with how many of each there are.
        let lists: Vec<(RefKind, Vec<(String, String)>)> = KINDS.iter().map(|k| (*k, editor_core::defs::list(&lib.db, *k))).filter(|(_, l)| !l.is_empty()).collect();
        ui.horizontal_wrapped(|ui| {
            if ui.selectable_label(self.kind.is_none(), format!("All ({})", lists.iter().map(|(_, l)| l.len()).sum::<usize>())).clicked() {
                self.kind = None;
            }
            for (kind, list) in &lists {
                if ui.selectable_label(self.kind == Some(*kind), format!("{} ({})", crate::shell::plural(kind.label()), list.len())).clicked() {
                    self.kind = Some(*kind);
                }
            }
        });
        ui.separator();
        let mut add = false;
        ui.horizontal(|ui| {
            let n = self.ticked.len();
            add = ui.add_enabled(n > 0, egui::Button::new(egui::RichText::new(if n == 0 { "Add to the project".to_string() } else { format!("Add {n} to the project") }).strong())).on_disabled_hover_text("Tick what you want first").clicked();
            if n > 0 && ui.button("Untick all").clicked() {
                self.ticked.clear();
            }
            match &self.said {
                Some(Ok(m)) => ui.colored_label(egui::Color32::from_rgb(140, 220, 140), m.as_str()),
                Some(Err(e)) => ui.colored_label(ui.visuals().error_fg_color, format!("Not added: {e}")),
                None => ui.label(""),
            };
        });
        let f = self.filter.trim().to_lowercase();
        let db = &ws.project.db;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let mut shown = 0;
            for (kind, list) in lists.iter().filter(|(k, _)| self.kind.is_none_or(|x| x == *k)) {
                let rows: Vec<&(String, String)> = list.iter().filter(|(id, name)| (f.is_empty() || id.to_lowercase().contains(&f) || name.to_lowercase().contains(&f)) && !(self.only_new && db.has(*kind, id))).collect();
                if rows.is_empty() {
                    continue;
                }
                if self.kind.is_none() {
                    ui.add_space(4.0);
                    ui.weak(format!("{} ({})", crate::shell::plural(kind.label()), rows.len()));
                }
                // (A long kind is cut: searching finds the rest.)
                for (id, name) in rows.iter().take(400) {
                    shown += 1;
                    let has = db.has(*kind, id);
                    ui.horizontal(|ui| {
                        let mut on = self.ticked.iter().any(|(k, i)| k == kind && i == id);
                        let label = if name.is_empty() || name == id { id.clone() } else { format!("{name}  ({id})") };
                        if ui.checkbox(&mut on, label).changed() {
                            if on {
                                self.ticked.push((*kind, id.clone()));
                            } else {
                                self.ticked.retain(|(k, i)| !(k == kind && i == id));
                            }
                        }
                        if has && ui.link("in the project").on_hover_text("Open the project's one. Adding it again leaves the project's as it is.").clicked() {
                            open = Some((*kind, id.clone()));
                        }
                    });
                }
                if rows.len() > 400 {
                    ui.weak(format!("… and {} more: search for them.", rows.len() - 400));
                }
            }
            if shown == 0 {
                ui.weak("Nothing fits.");
            }
        });
        if add {
            self.add(ws);
        }
        open
    }
}

#[cfg(test)]
mod ui_tests {
    use super::*;
    use egui_kittest::kittest::Queryable;
    use egui_kittest::Harness;

    struct State {
        ws: Workspace,
        page: LibraryPage,
        opened: Vec<AssetRef>,
        dir: std::path::PathBuf,
    }

    fn copy(from: &std::path::Path, to: &std::path::Path) {
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

    #[test]
    fn a_new_one_starts_as_a_library_asset_under_its_own_name() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir = std::env::temp_dir().join(format!("rustpg2d-new-from-library-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        copy(&root.join("example_project"), &dir);
        let mut ws = Workspace::new(dir.clone(), editor_core::Gpu::No).unwrap();
        ws.history_frame(None);
        ws.lib.lib = Some(rustpg2d_engine::project::Project::load(&root.join("crates/editor/assets/starter_library")).unwrap());
        assert!(!ws.project.db.has(RefKind::Character, "owlbear") && ws.project.db.has(RefKind::Prop, "chest"));
        let before = ws.project.db.clone();
        // The library's owlbear as "Grumpy": the new character has that name and its own id, the
        // model it is drawn with came along, and the library's own owlbear did not stay.
        let id = ws.make_from_library(RefKind::Character, "owlbear", "Grumpy").unwrap();
        ws.history_frame(None);
        assert_eq!(id, "grumpy");
        let c = ws.project.db.character("grumpy").unwrap().clone();
        assert_eq!(c.name, "Grumpy");
        assert!(ws.project.db.has(RefKind::Model, &c.appearance.model) && !before.has(RefKind::Model, &c.appearance.model));
        assert!(!ws.project.db.has(RefKind::Character, "owlbear"));
        assert_eq!(ws.timeline.done.last().unwrap().label, "New character from the library: Grumpy");
        // One step: undone, the project is as it was.
        ws.undo().unwrap();
        assert!(ws.project.db == before, "undone, nothing of it is left");
        // Called what the library files it under, it is that one (no "owlbear_2" beside nothing).
        assert_eq!(ws.make_from_library(RefKind::Character, "owlbear", "Owlbear").unwrap(), "owlbear");
        ws.history_frame(None);
        assert!(ws.project.db.character("owlbear").is_some() && !ws.project.db.has(RefKind::Character, "owlbear_2"));
        ws.undo().unwrap();
        assert!(ws.project.db == before);
        // Without a name the asset is added as it is.
        assert_eq!(ws.make_from_library(RefKind::Character, "owlbear", "").unwrap(), "owlbear");
        ws.history_frame(None);
        assert!(ws.project.db.has(RefKind::Character, "owlbear"));
        // One the project has already: the new one is a copy, the project's stays; without a
        // name there is nothing to make.
        let id = ws.make_from_library(RefKind::Prop, "chest", "Old chest").unwrap();
        ws.history_frame(None);
        assert_eq!(id, "old_chest");
        assert!(ws.project.db.has(RefKind::Prop, "chest") && ws.project.db.prop("old_chest").is_some_and(|p| p.name == "Old chest"));
        assert!(ws.make_from_library(RefKind::Prop, "chest", "").is_err());
        assert!(ws.make_from_library(RefKind::Prop, "no_such_thing", "X").is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_new_gallery_offers_the_library_and_asks_for_one_of_it() {
        use crate::gallery::{self, NewAsset, Outcome};
        struct G {
            state: NewAsset,
            asked: Vec<(RefKind, String, String)>,
        }
        let lists = vec![(RefKind::Prop, vec![("chest".to_string(), "Chest".to_string())])];
        let mut h = Harness::builder().with_size(egui::vec2(1000.0, 700.0)).build_ui_state(
            move |ui, g: &mut G| {
                let library = |kind: RefKind| if kind == RefKind::Prop { vec![("chest".to_string(), "Chest".to_string()), ("anvil".to_string(), "Anvil".to_string())] } else { Vec::new() };
                if let Some(Outcome::FromLibrary { kind, name, from }) = gallery::show(ui.ctx(), &mut g.state, &[("Objects", &[RefKind::Prop])], &lists, &library, None) {
                    g.asked.push((kind, name, from));
                }
            },
            G { state: NewAsset::of(Some(RefKind::Prop)), asked: Vec::new() },
        );
        h.run();
        h.get_by_label("as one of the asset library (2)").click();
        h.run();
        // What the project has already says so; nothing is asked for before one is picked.
        assert!(h.query_all_by_label("Chest  (chest)  · in the project").next().is_some());
        let search = h.get_by_role_and_label(egui::accesskit::Role::TextInput, "Search the library");
        search.focus();
        search.type_text("anv");
        h.run();
        assert!(h.query_all_by_label_contains("Chest").next().is_none());
        h.get_by_label("Anvil  (anvil)").click();
        h.run();
        // Without a name it is added as it is; with one, under that name.
        h.get_by_label("No name: it is added as it is.");
        let name = h.get_by_role_and_label(egui::accesskit::Role::TextInput, "Name");
        name.focus();
        name.type_text("Old anvil");
        h.run();
        h.get_by_label("Create").click();
        h.run();
        assert_eq!(h.state().asked, [(RefKind::Prop, "Old anvil".to_string(), "anvil".to_string())]);
    }

    #[test]
    fn a_new_project_starts_with_the_basics_of_the_library() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir = std::env::temp_dir().join(format!("rustpg2d-new-project-{}", std::process::id())).join("my-game");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        editor_core::files::create_project(&dir).unwrap();
        let mut ws = Workspace::new(dir.clone(), editor_core::Gpu::No).unwrap();
        ws.lib.lib = Some(rustpg2d_engine::project::Project::load(&root.join("crates/editor/assets/starter_library")).unwrap());
        let said = ws.start_with_basics("My game").unwrap();
        assert!(said.contains("the hero is the player's character") && said.contains("grass on the map start"), "{said}");
        assert!(!ws.is_dirty(), "everything is saved");
        // Read again from its files: a game that can be played.
        let made = rustpg2d_engine::project::Project::load(&dir).unwrap();
        assert_eq!((made.config.name.as_str(), made.config.player.as_str()), ("My game", "hero"));
        let hero = made.db.character("hero").expect("a hero");
        assert!(made.db.model(&hero.appearance.model).is_some() && hero.max_hp > 0);
        assert_eq!(made.config.creation.body_male, "hero");
        for (kind, id) in editor_core::library::BASICS {
            assert!(made.db.has(kind, id), "the library's {} {id} is missing in the new project (or gone from the starter library)", kind.name());
        }
        let map = rustpg2d_engine::map::io::load(&made.map_path("start")).unwrap();
        let grass = map.materials.iter().position(|m| m == "grass").expect("grass on the first map") as u8 + 1;
        assert_eq!(map.ground.get(100, 100), grass);
        // What the basics need came along: the hero's clothes, the chest's sounds…
        for item in hero.equipment.clothing.iter().map(|s| s.item.as_str()) {
            assert!(made.db.item(item).is_some(), "the hero wears {item}");
        }
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }

    #[test]
    fn the_new_gallery_starts_from_a_dropped_file() {
        use crate::gallery::{self, NewAsset, Outcome};
        struct G {
            state: NewAsset,
            asked: Vec<(RefKind, String, std::path::PathBuf)>,
        }
        let dir = std::env::temp_dir().join(format!("rustpg2d-gallery-file-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let picture = dir.join("old_oak-tree.png");
        std::fs::write(&picture, rustpg2d_engine::imageio::Rgba::new(8, 8).encode().unwrap()).unwrap();
        assert!(NewAsset::of_file(&dir.join("notes.txt")).is_none(), "what is no art opens nothing");
        // Dropped, a picture is a prop to be, named after its file.
        let state = NewAsset::of_file(&picture).unwrap();
        assert_eq!((state.kind, state.name.as_str(), state.from_file), (Some(RefKind::Prop), "Old oak tree", true));
        let mut h = Harness::builder().with_size(egui::vec2(1000.0, 700.0)).build_ui_state(
            |ui, g: &mut G| {
                let library = |_: RefKind| Vec::new();
                if let Some(Outcome::FromFile { kind, name, file }) = gallery::show(ui.ctx(), &mut g.state, &[("Objects", &[RefKind::Prop]), ("Art & sound", &[RefKind::Sheet, RefKind::Sound])], &[], &library, None) {
                    g.asked.push((kind, name, file));
                }
            },
            G { state, asked: Vec::new() },
        );
        h.run();
        assert!(h.query_all_by_label_contains("A prop with this picture").next().is_some());
        // A picture can't be a sound: that kind says so and makes nothing; as a sheet it can.
        h.get_by_label("Sound").click();
        h.run();
        assert!(h.query_all_by_label_contains("a picture can become a prop or a sprite sheet").next().is_some());
        h.get_by_label("Create").click();
        h.run();
        assert!(h.state().asked.is_empty());
        h.get_by_label("Sprite sheet").click();
        h.run();
        assert!(h.query_all_by_label_contains("A sprite sheet of this picture").next().is_some());
        h.get_by_label("Create").click();
        h.run();
        assert_eq!(h.state().asked, [(RefKind::Sheet, "Old oak tree".to_string(), picture.clone())]);
        // A file that isn't there is said, and nothing is made of it.
        h.state_mut().state.file = dir.join("gone.png").display().to_string();
        h.state_mut().asked.clear();
        h.run();
        h.get_by_label("There is no such file.");
        h.get_by_label("Create").click();
        h.run();
        assert!(h.state().asked.is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_library_asset_is_found_ticked_and_added_with_what_it_needs() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir = std::env::temp_dir().join(format!("rustpg2d-library-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        copy(&root.join("example_project"), &dir);
        let mut ws = Workspace::new(dir.clone(), editor_core::Gpu::No).unwrap();
        ws.history_frame(None);
        // (The starter library's own folder stands in for the library: it is only read.)
        ws.lib.lib = Some(rustpg2d_engine::project::Project::load(&root.join("crates/editor/assets/starter_library")).unwrap());
        assert!(!ws.project.db.has(RefKind::Character, "owlbear"), "the example got an owlbear: pick another library character for this test");
        let mut h = Harness::builder().with_size(egui::vec2(1200.0, 900.0)).build_ui_state(
            |ui, s: &mut State| {
                s.ws.history_frame(None);
                if let Some(a) = s.page.show(ui, &mut s.ws) {
                    s.opened.push(a);
                }
            },
            State { ws, page: LibraryPage::default(), opened: Vec::new(), dir },
        );
        h.run();
        // Searched: the owlbear, a character the project doesn't have yet.
        let search = h.get_by_role_and_label(egui::accesskit::Role::TextInput, "Search the library");
        search.focus();
        search.type_text("owlbear");
        h.run();
        assert!(h.query_all_by_label("in the project").next().is_none());
        // (Its model has the same name: among the characters there is one owlbear.)
        assert_eq!(h.query_all_by_label("Owlbear  (owlbear)").count(), 2);
        h.query_all_by_label_contains("Characters (").next().unwrap().click();
        h.run();
        h.get_by_label("Owlbear  (owlbear)").click();
        h.run();
        assert_eq!(h.state().page.ticked, [(RefKind::Character, "owlbear".to_string())]);
        // Added: the character and the model it is drawn with, as one step.
        h.get_by_label("Add 1 to the project").click();
        h.run();
        let s = h.state();
        assert!(s.page.said.as_ref().is_some_and(|r| r.is_ok()), "{:?}", s.page.said);
        let owlbear = s.ws.project.db.character("owlbear").expect("the owlbear is in the project").clone();
        assert!(!owlbear.appearance.model.is_empty() && s.ws.project.db.has(RefKind::Model, &owlbear.appearance.model));
        assert!(s.page.ticked.is_empty());
        assert_eq!(s.ws.timeline.done.last().unwrap().label, "Add owlbear from the library");
        // It says so in the list now, and leads to the project's one.
        h.get_by_label("in the project").click();
        h.run();
        assert_eq!(h.state().opened, [(RefKind::Character, "owlbear".to_string())]);
        // Undone, the project has no owlbear again.
        h.state_mut().ws.undo().unwrap();
        assert!(!h.state().ws.project.db.has(RefKind::Character, "owlbear"));
        let dir = h.state().dir.clone();
        let _ = std::fs::remove_dir_all(dir);
    }
}
