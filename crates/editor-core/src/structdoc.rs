//! A structure worked on with the map's tools: it is opened as a small map of its own (its
//! size, everything it holds placed on it), and every finished change on that map is taken
//! back into the definition as one step of the definitions' history.
//!
//! The definition stays what is true: its map is a way to change it. Undo, a form, an agent
//! or a rename changing the definition builds the map again from it; the map's own undo
//! history isn't part of the editor's.

use rustpg2d_engine::defs::StructureDef;
use rustpg2d_engine::map::Map;

use crate::doc::MapDoc;
use crate::Workspace;

const PREFIX: &str = "@structure:";

/// The name of the map structure `id` is worked on as.
pub fn doc_name(id: &str) -> String {
    format!("{PREFIX}{id}")
}

/// The structure a map of that name is, if it is one.
pub fn structure_of(map: &str) -> Option<&str> {
    map.strip_prefix(PREFIX)
}

/// A map that is no map of the project (it has no file and no place in the world).
pub fn is_scratch(map: &str) -> bool {
    map.starts_with('@')
}

/// The structure as a map: its size, its top-left corner at 0, 0.
fn as_map(def: &StructureDef) -> Map {
    let mut map = Map::new(&doc_name(&def.id), def.w.max(1), def.h.max(1));
    def.place(&mut map, 0, 0);
    map
}

/// The map as the structure it is now: what `def` is called and filed under is kept.
fn as_structure(map: &Map, def: &StructureDef) -> StructureDef {
    // (With its ground when it had some, or some was painted since.)
    let ground = !def.ground.is_empty() || !map.materials.is_empty();
    let mut new = StructureDef::capture(map, &def.id, &def.name, (0, 0, map.width, map.height), ground);
    new.category = def.category.clone();
    // What the definition holds outside its own tiles (a fence a pixel off its edge) was never
    // on its map: it is kept as it is.
    let ts = rustpg2d_engine::project::TILE_SIZE as i32;
    let inside = |x: i32, y: i32| x.div_euclid(ts) >= 0 && x.div_euclid(ts) < def.w.max(1) as i32 && (y - 1).div_euclid(ts) >= 0 && (y - 1).div_euclid(ts) < def.h.max(1) as i32;
    for d in def.decor.iter().filter(|d| !inside(d.x, d.y)) {
        if let Some(prop) = def.decor_defs.get(d.def as usize) {
            let slot = new.decor_slot(prop);
            new.decor.push(rustpg2d_engine::decor::Decor { def: slot, ..*d });
        }
    }
    // (For things placed, as placing a structure decides it: by the tile their feet are on.)
    let tsf = ts as f32;
    let placed = |x: f32, y: f32| {
        let (tx, ty) = ((x / tsf) as i32, ((y - 1.0) / tsf) as i32);
        tx >= 0 && tx < def.w.max(1) as i32 && ty >= 0 && ty < def.h.max(1) as i32
    };
    new.entities.extend(def.entities.iter().filter(|e| !placed(e.x, e.y)).cloned());
    new
}

impl Workspace {
    /// Opens structure `id` as a map to work on (kept open beside the others; it is not made
    /// the map being worked on). Returns that map's name.
    pub fn open_structure(&mut self, id: &str) -> Result<String, String> {
        let name = doc_name(id);
        let def = self.project.db.structure(id).ok_or_else(|| format!("there is no structure \"{id}\""))?.clone();
        let open = self.doc.as_ref().is_some_and(|d| d.map.name == name) || self.parked.contains_key(&name);
        if !open || !self.struct_docs.contains_key(id) {
            let doc = MapDoc::new(as_map(&def), &self.assets);
            match self.doc.as_mut().filter(|d| d.map.name == name) {
                Some(d) => *d = doc,
                None => {
                    self.parked.insert(name.clone(), doc);
                }
            }
            self.struct_docs.insert(id.to_string(), def);
        }
        Ok(name)
    }

    /// Closes a structure's map (the structure is as its definition says).
    pub fn close_structure(&mut self, id: &str) {
        self.sync_structures();
        self.struct_docs.remove(id);
        self.close_map(&doc_name(id));
    }

    /// Whether structure `id` is open as a map.
    pub fn structure_open(&self, id: &str) -> bool {
        self.struct_docs.contains_key(id)
    }

    /// Once a frame, before the definitions are looked at for changes: a finished change on
    /// a structure's map goes into its definition; a definition changed otherwise builds its
    /// map again; a structure that is gone closes its map.
    pub(crate) fn sync_structures(&mut self) {
        if self.struct_docs.is_empty() {
            return;
        }
        let ids: Vec<String> = self.struct_docs.keys().cloned().collect();
        for id in ids {
            let name = doc_name(&id);
            let current = self.doc.as_ref().is_some_and(|d| d.map.name == name);
            if !current && !self.parked.contains_key(&name) {
                self.struct_docs.remove(&id);
                continue;
            }
            let Some(def) = self.project.db.structure(&id) else {
                // (Deleted, renamed, or its making undone.)
                self.struct_docs.remove(&id);
                self.close_map(&name);
                continue;
            };
            // (Most frames nothing happened: the definition is as last seen, its map untouched.)
            let outside = self.struct_docs.get(&id) != Some(def);
            let touched = self.doc.as_ref().filter(|_| current).or_else(|| self.parked.get(&name)).is_some_and(|d| d.is_dirty() && !d.in_stroke());
            if !outside && !touched {
                continue;
            }
            let def = def.clone();
            let assets = &self.assets;
            let doc = match self.doc.as_mut().filter(|_| current) {
                Some(d) => d,
                None => self.parked.get_mut(&name).expect("checked above"),
            };
            if self.struct_docs.get(&id) != Some(&def) {
                // The definition changed from outside. What it holds on the ground decides
                // whether the map has to be built again (a new name alone leaves it).
                let seen = self.struct_docs.get(&id);
                let same_content = seen.is_some_and(|s| StructureDef { name: String::new(), category: String::new(), ..s.clone() } == StructureDef { name: String::new(), category: String::new(), ..def.clone() });
                if !same_content {
                    *doc = MapDoc::new(as_map(&def), assets);
                }
                self.struct_docs.insert(id.clone(), def);
                continue;
            }
            // (A stroke of the collision brush is finished when the button is let go.)
            if !doc.is_dirty() || doc.in_stroke() {
                continue;
            }
            let new = as_structure(&doc.map, &def);
            doc.mark_saved();
            // (What was done on the map is a step of its own, whatever has the keyboard; only
            // typing into a field of something placed joins the step before, as in any form.)
            let typed = doc.newest_is_typed();
            if new == def {
                continue;
            }
            if !typed {
                self.db_hist.no_merge();
            }
            let label = format!("Change the structure {}", if def.name.is_empty() { &id } else { &def.name });
            self.begin_unnamed(&label);
            if let Some(slot) = self.project.db.structures.iter_mut().find(|s| s.id == id) {
                *slot = new.clone();
            }
            self.struct_docs.insert(id, new);
            self.defs_dirty = true;
            self.stale = true;
        }
    }

    /// Another size for structure `id` (tiles; what lies outside the new size is cut off, new
    /// tiles are empty). One step.
    pub fn resize_structure(&mut self, id: &str, w: u32, h: u32) -> Result<(), String> {
        let def = self.project.db.structure(id).ok_or_else(|| format!("there is no structure \"{id}\""))?.clone();
        let (w, h) = (w.clamp(1, 64), h.clamp(1, 64));
        if (w, h) == (def.w, def.h) {
            return Ok(());
        }
        // (Through a map of it, so cutting and growing are the map's own.)
        let mut map = as_map(&def);
        map.resize(w, h);
        // What stood outside goes with the tiles it stood on.
        let (pw, ph) = (w as f32 * self.ts(), h as f32 * self.ts());
        map.entities.retain(|e| e.x < pw && e.y <= ph);
        map.decor.retain(|d| (d.x as f32) < pw && (d.y as f32) <= ph);
        let new = as_structure(&map, &def);
        self.begin(&format!("Resize the structure {} to {w} x {h}", if def.name.is_empty() { id } else { &def.name }));
        if let Some(slot) = self.project.db.structures.iter_mut().find(|s| s.id == id) {
            *slot = new;
        }
        self.defs_dirty = true;
        self.stale = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustpg2d_engine::entity::EntityKind;

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

    fn scratch(tag: &str) -> (Workspace, std::path::PathBuf) {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir = std::env::temp_dir().join(format!("rustpg2d-structdoc-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        copy(&root.join("example_project"), &dir);
        let mut ws = Workspace::new(dir.clone(), crate::Gpu::No).unwrap();
        ws.history_frame(None);
        (ws, dir)
    }

    fn doc<'a>(ws: &'a Workspace, name: &str) -> &'a MapDoc {
        ws.open_maps().into_iter().find(|d| d.map.name == name).expect("the structure's map is open")
    }

    #[test]
    fn opening_a_structure_changes_nothing_and_every_structure_comes_back_as_it_was() {
        let (mut ws, dir) = scratch("open");
        let before = ws.project.db.structures.clone();
        // As a map and back: what a structure holds on the ground is what it held.
        let mut differing = Vec::new();
        for def in &before {
            let back = as_structure(&as_map(def), def);
            let decor = |d: &StructureDef| {
                let mut v: Vec<(String, i32, i32)> = d.decor.iter().map(|x| (d.decor_defs[x.def as usize].clone(), x.x, x.y)).collect();
                v.sort();
                v
            };
            let things = |d: &StructureDef| {
                let mut v: Vec<(String, i32, i32)> = d.entities.iter().map(|e| (e.def.clone(), e.x as i32, e.y as i32)).collect();
                v.sort();
                v
            };
            if (back.w, back.h) != (def.w, def.h) || decor(&back) != decor(def) || things(&back) != things(def) {
                differing.push(format!("{} {}x{} entities {}->{} decor {}->{}", def.id, def.w, def.h, def.entities.len(), back.entities.len(), def.decor.len(), back.decor.len()));
            }
        }
        assert!(differing.is_empty(), "these come back with other things on them: {differing:?}");
        // Opened and looked at for a few frames, the definitions are untouched and nothing is a step.
        let id = before[0].id.clone();
        let name = ws.open_structure(&id).unwrap();
        assert_eq!(name, doc_name(&id));
        assert_eq!(structure_of(&name), Some(id.as_str()));
        ws.open_map(&name);
        for _ in 0..3 {
            ws.history_frame(None);
        }
        assert!(ws.project.db.structures == before && !ws.is_dirty() && ws.timeline.done.is_empty());
        assert_eq!((doc(&ws, &name).map.width, doc(&ws, &name).map.height), (before[0].w, before[0].h));
        assert!(!ws.maps.iter().any(|m| is_scratch(m)));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_change_on_a_structures_map_is_a_step_of_its_definition() {
        let (mut ws, dir) = scratch("edit");
        let def = ws.project.db.structures.iter().find(|s| s.w >= 3 && s.h >= 3).expect("a structure of some size").clone();
        let id = def.id.clone();
        let name = ws.open_structure(&id).unwrap();
        ws.open_map(&name);
        ws.history_frame(None);
        // Something is placed on its map: the definition holds it, as one step called what was done.
        let prop = ws.project.db.props.iter().find(|p| p.id == "chest").map(|p| p.id.clone()).expect("the example has a chest");
        let ts = ws.ts();
        ws.place_entity(EntityKind::Prop, &prop, gfx::prelude::vec2(1.5 * ts, 2.0 * ts)).unwrap();
        ws.history_frame(None);
        let now = ws.project.db.structure(&id).unwrap().clone();
        assert_eq!(now.entities.len(), def.entities.len() + 1);
        assert!(now.entities.iter().any(|e| e.def == prop));
        assert_eq!((now.name.as_str(), now.category.as_str()), (def.name.as_str(), def.category.as_str()), "what it is called and filed under is kept");
        assert_eq!(ws.timeline.done.len(), 1, "{:?}", ws.timeline.done.iter().map(|c| (&c.label, c.at.describe())).collect::<Vec<_>>());
        assert!(matches!(ws.timeline.done[0].at, crate::timeline::Where::Defs), "a step of the definitions, not of a map");
        assert!(ws.defs_dirty && !doc(&ws, &name).is_dirty());
        // More frames add nothing.
        ws.history_frame(None);
        assert_eq!(ws.timeline.done.len(), 1);
        // A second thing placed right after, the map's view still having the keyboard, is a
        // step of its own (it is no typing on in a field).
        ws.history_frame(Some(7));
        ws.place_entity(EntityKind::Prop, &prop, gfx::prelude::vec2(2.5 * ts, 2.0 * ts)).unwrap();
        ws.history_frame(Some(7));
        ws.place_entity(EntityKind::Prop, &prop, gfx::prelude::vec2(0.5 * ts, 2.0 * ts)).unwrap();
        ws.history_frame(Some(7));
        assert_eq!(ws.timeline.done.len(), 3);
        ws.undo().unwrap();
        ws.undo().unwrap();
        ws.history_frame(None);
        assert_eq!(ws.project.db.structure(&id).unwrap(), &now);
        // Undone: the definition is as it was, and its map shows that again.
        ws.undo().unwrap();
        ws.history_frame(None);
        assert_eq!(ws.project.db.structure(&id).unwrap(), &def);
        assert_eq!(doc(&ws, &name).map.entities.len(), def.entities.len());
        assert!(ws.timeline.done.is_empty());
        // Redone, the chest is back on both.
        ws.redo().unwrap();
        ws.history_frame(None);
        assert_eq!(ws.project.db.structure(&id).unwrap(), &now);
        assert_eq!(doc(&ws, &name).map.entities.len(), now.entities.len());
        // Saving writes the definitions and no map file for it.
        ws.save_all();
        assert!(!ws.is_dirty());
        assert!(!ws.project.map_path(&name).exists() && !ws.maps.contains(&name));
        let saved = rustpg2d_engine::project::Project::load(&dir).unwrap();
        assert_eq!(saved.db.structure(&id), Some(&now));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_house_of_the_build_tool_is_saved_as_a_structure_and_placed_on_another_map() {
        use crate::workspace::Selection;
        let (mut ws, dir) = scratch("house");
        let ts = ws.ts();
        // A house of 5 x 4 tiles built on an empty map, in the project's first house look.
        ws.load_doc(Map::new("yard", 20, 16));
        ws.history_frame(None);
        ws.build_house(gfx::prelude::Rect::new(4.0 * ts, 5.0 * ts, 5.0 * ts, 4.0 * ts), "", 1).unwrap();
        ws.history_frame(None);
        let house = *ws.doc.as_ref().unwrap().map.decor.last().expect("the house stands there");
        let prop = ws.doc.as_ref().unwrap().map.decor_defs[house.def as usize].clone();
        assert!(ws.project.db.prop(&prop).is_some_and(|p| p.building.is_some() && p.hitbox == [5.0 * ts, 4.0 * ts]));
        // Selected, it is saved as a structure: its tiles, the house on them.
        let rect = ws.tiles_of(&[Selection::Scenery(house)]).expect("it stands on tiles");
        assert_eq!((rect.2, rect.3), (5, 4));
        let id = ws.save_as_structure(rect, "Farmhouse", false).unwrap();
        ws.history_frame(None);
        let def = ws.project.db.structure(&id).unwrap().clone();
        assert_eq!((def.w, def.h, def.decor.len()), (5, 4, 1));
        assert_eq!(def.decor_defs[def.decor[0].def as usize], prop);
        // Placed on another map, the same house stands there.
        ws.load_doc(Map::new("field", 30, 30));
        ws.history_frame(None);
        ws.place_structure(&id, (15, 15)).unwrap();
        ws.history_frame(None);
        let field = &ws.doc.as_ref().unwrap().map;
        assert_eq!(field.decor.len(), 1);
        assert_eq!(field.decor_defs[field.decor[0].def as usize], prop);
        // And it opens as a map of its own, the house on it.
        let name = ws.open_structure(&id).unwrap();
        ws.history_frame(None);
        assert_eq!(doc(&ws, &name).map.decor.len(), 1);
        assert_eq!(ws.project.db.structure(&id), Some(&def), "opened, it is as it was saved");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_structure_is_resized_renamed_and_deleted_with_its_map_following() {
        let (mut ws, dir) = scratch("follow");
        let def = ws.project.db.structures.iter().find(|s| s.w >= 3 && s.h >= 3 && !s.entities.is_empty()).expect("a structure with something on it").clone();
        let id = def.id.clone();
        let name = ws.open_structure(&id).unwrap();
        ws.history_frame(None);
        // A new name alone leaves its map as it is (what was selected and done there stays).
        let mut value = ws.def_value(rustpg2d_engine::refactor::RefKind::Structure, &id).unwrap();
        value["name"] = serde_json::Value::String("Another name".into());
        ws.set_def_value(rustpg2d_engine::refactor::RefKind::Structure, &id, value).unwrap();
        ws.history_frame(None);
        assert_eq!(ws.project.db.structure(&id).unwrap().name, "Another name");
        assert_eq!(doc(&ws, &name).map.entities.len(), def.entities.len());
        // Larger by two tiles each way: the map is that size, everything still on it.
        ws.resize_structure(&id, def.w + 2, def.h + 2).unwrap();
        ws.history_frame(None);
        let grown = ws.project.db.structure(&id).unwrap().clone();
        assert_eq!((grown.w, grown.h, grown.entities.len(), grown.decor.len()), (def.w + 2, def.h + 2, def.entities.len(), def.decor.len()));
        assert_eq!((doc(&ws, &name).map.width, doc(&ws, &name).map.height), (def.w + 2, def.h + 2));
        assert_eq!(ws.timeline.done.last().unwrap().label, format!("Resize the structure Another name to {} x {}", def.w + 2, def.h + 2));
        // One tile: what stood outside it went with its tiles.
        ws.resize_structure(&id, 1, 1).unwrap();
        ws.history_frame(None);
        let small = ws.project.db.structure(&id).unwrap();
        assert!((small.w, small.h) == (1, 1) && small.entities.len() <= def.entities.len());
        // Deleted: its map closes.
        ws.delete_def(rustpg2d_engine::refactor::RefKind::Structure, &id, true).unwrap();
        ws.history_frame(None);
        assert!(!ws.structure_open(&id) && !ws.open_maps().iter().any(|d| d.map.name == name));
        let _ = std::fs::remove_dir_all(dir);
    }
}
