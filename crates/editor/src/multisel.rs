//! Several objects selected on a map at once (Select tool): Ctrl+click adds or removes an entity
//! or zone, Ctrl+drag on empty space selects the entities standing in the box and the zones
//! lying inside it. Dragging one of them moves them all, Del / Ctrl+D / Ctrl+C / Ctrl+X act on
//! all of them, each as one undo step. `Editor::sel` stays the primary object (what the rest of
//! the editor looks at); `Editor::multi` holds the whole set together with the primary it was
//! made with, so anything that changes `sel` ends the multi-selection by itself.

use macroquad::prelude::*;
use rustpg2d_engine::defs::StructureDef;
use rustpg2d_engine::map::Map;

use crate::app::{Editor, Selection};
use crate::doc::Objects;

/// Where a selected object was when a move started.
#[derive(Clone, Copy, Debug)]
pub enum Orig {
    Entity(u32, Vec2),
    Zone(u32, (u32, u32)),
}

/// The tile an entity stands on (its feet).
fn feet_tile(x: f32, y: f32, ts: f32) -> (i32, i32) {
    ((x / ts).floor() as i32, ((y - 0.5) / ts).floor() as i32)
}

/// Entities whose feet are in tile rectangle `r`, and zones lying completely inside it.
pub fn objects_in(map: &Map, r: (u32, u32, u32, u32), ts: f32, zones: bool) -> Vec<Selection> {
    let (x0, y0, x1, y1) = (r.0 as i32, r.1 as i32, (r.0 + r.2) as i32, (r.1 + r.3) as i32);
    let inside = |(x, y): (i32, i32)| x >= x0 && x < x1 && y >= y0 && y < y1;
    let mut out: Vec<Selection> = map.entities.iter().filter(|e| inside(feet_tile(e.x, e.y, ts))).map(|e| Selection::Entity(e.uid)).collect();
    if zones {
        out.extend(
            map.zones
                .iter()
                .filter(|z| z.x as i32 >= x0 && z.y as i32 >= y0 && (z.x + z.w) as i32 <= x1 && (z.y + z.h) as i32 <= y1)
                .map(|z| Selection::Zone(z.uid)),
        );
    }
    out
}

/// A rectangle of tiles: x, y, w, h.
type TileRect = (i32, i32, i32, i32);

/// Moves objects by whole tiles from where they were (`origs`). The move stops at the map's
/// edge for the whole group, so the objects keep their arrangement.
pub fn place_moved(map: &mut Map, origs: &[Orig], dx: i32, dy: i32, ts: f32) {
    let (mw, mh) = (map.width as i32, map.height as i32);
    // Each object's tile rectangle where it started: (x, y, w, h).
    let rects: Vec<TileRect> = origs
        .iter()
        .filter_map(|o| match *o {
            Orig::Entity(_, p) => {
                let t = feet_tile(p.x, p.y, ts);
                Some((t.0, t.1, 1, 1))
            }
            Orig::Zone(uid, (x, y)) => map.zones.iter().find(|z| z.uid == uid).map(|z| (x as i32, y as i32, z.w as i32, z.h as i32)),
        })
        .collect();
    // How far the group can go left / up (the object nearest that edge) and right / down.
    let room = |f: fn(&TileRect, i32, i32) -> i32| rects.iter().map(|r| f(r, mw, mh)).min().unwrap_or(0);
    let (left, right) = (room(|r, _, _| r.0), room(|r, w, _| w - r.0 - r.2));
    let (up, down) = (room(|r, _, _| r.1), room(|r, _, h| h - r.1 - r.3));
    let dx = dx.clamp(-left, right.max(-left));
    let dy = dy.clamp(-up, down.max(-up));
    for o in origs {
        match *o {
            Orig::Entity(uid, p) => {
                if let Some(e) = map.entities.iter_mut().find(|e| e.uid == uid) {
                    e.x = p.x + dx as f32 * ts;
                    e.y = p.y + dy as f32 * ts;
                }
            }
            Orig::Zone(uid, (x, y)) => {
                if let Some(z) = map.zones.iter_mut().find(|z| z.uid == uid) {
                    z.x = (x as i32 + dx).max(0) as u32;
                    z.y = (y as i32 + dy).max(0) as u32;
                }
            }
        }
    }
}

impl Editor {
    /// The selected entities and zones: the multi-selection, or the single selected object.
    pub fn selected_set(&self) -> Vec<Selection> {
        match &self.multi {
            Some((primary, set)) if *primary == self.sel && set.len() > 1 => set.clone(),
            _ => match self.sel {
                Selection::Entity(_) | Selection::Zone(_) => vec![self.sel],
                _ => Vec::new(),
            },
        }
    }

    /// Two or more objects are selected.
    pub fn is_multi(&self) -> bool {
        self.selected_set().len() > 1
    }

    /// Selects exactly these entities / zones (the first becomes the primary `sel`).
    pub fn set_selection(&mut self, set: Vec<Selection>) {
        let mut clean: Vec<Selection> = Vec::new();
        for s in set {
            if matches!(s, Selection::Entity(_) | Selection::Zone(_)) && !clean.contains(&s) {
                clean.push(s);
            }
        }
        self.sel = clean.first().copied().unwrap_or(Selection::None);
        self.multi = (clean.len() > 1).then_some((self.sel, clean));
    }

    /// Ctrl+click: adds an object to the selection, or takes it out.
    pub fn toggle_selected(&mut self, s: Selection) {
        let mut set = self.selected_set();
        match set.iter().position(|x| *x == s) {
            Some(i) => {
                set.remove(i);
            }
            None => set.push(s),
        }
        self.set_selection(set);
        self.say_selection();
    }

    /// Ctrl+drag: selects the entities standing in the box and the zones inside it.
    pub fn box_select(&mut self, r: (u32, u32, u32, u32)) {
        let ts = self.ts();
        let Some(doc) = &self.doc else { return };
        let found = objects_in(&doc.map, r, ts, self.view.zones);
        self.set_selection(found);
        self.say_selection();
    }

    fn say_selection(&mut self) {
        let n = self.selected_set().len();
        if n > 1 {
            self.set_status(format!("{n} objects selected · drag one to move them all · Del delete · Ctrl+C / Ctrl+X / Ctrl+D"));
        }
    }

    /// Where the selected objects are now (to move them from there).
    pub fn selection_origins(&self) -> Vec<Orig> {
        let Some(doc) = &self.doc else { return Vec::new() };
        self.selected_set()
            .into_iter()
            .filter_map(|s| match s {
                Selection::Entity(uid) => doc.map.entity(uid).map(|e| Orig::Entity(uid, vec2(e.x, e.y))),
                Selection::Zone(uid) => doc.map.zones.iter().find(|z| z.uid == uid).map(|z| Orig::Zone(uid, (z.x, z.y))),
                _ => None,
            })
            .collect()
    }

    /// Moves the whole selection by whole tiles (one undo step); the editor script's `movesel`.
    pub fn move_selection(&mut self, dx: i32, dy: i32) {
        let origs = self.selection_origins();
        let ts = self.ts();
        let Some(doc) = &mut self.doc else { return };
        if origs.is_empty() {
            return;
        }
        let before = Objects::of(&doc.map);
        place_moved(&mut doc.map, &origs, dx, dy, ts);
        doc.record_objects(before, "move");
    }

    /// Del on a multi-selection: removes every selected object (one undo step).
    pub fn delete_many(&mut self) {
        let set = self.selected_set();
        let Some(doc) = &mut self.doc else { return };
        let before = Objects::of(&doc.map);
        doc.map.entities.retain(|e| !set.contains(&Selection::Entity(e.uid)));
        doc.map.zones.retain(|z| !set.contains(&Selection::Zone(z.uid)));
        doc.record_objects(before, "delete");
        self.set_selection(Vec::new());
        self.set_status(format!("Deleted {} objects", set.len()));
    }

    /// Ctrl+D on a multi-selection: copies of every selected object one tile to the right,
    /// which become the selection (one undo step).
    pub fn duplicate_many(&mut self) {
        let set = self.selected_set();
        let ts = self.ts();
        let Some(doc) = &mut self.doc else { return };
        let before = Objects::of(&doc.map);
        let mut copies = Vec::new();
        for s in set {
            match s {
                Selection::Entity(uid) => {
                    let Some(mut e) = doc.map.entity(uid).cloned() else { continue };
                    e.uid = doc.map.alloc_uid();
                    e.x += ts;
                    if !e.name.is_empty() {
                        e.name = format!("{}_copy", e.name);
                    }
                    copies.push(Selection::Entity(e.uid));
                    doc.map.entities.push(e);
                }
                Selection::Zone(uid) => {
                    let Some(mut z) = doc.map.zones.iter().find(|z| z.uid == uid).cloned() else { continue };
                    z.uid = doc.map.alloc_uid();
                    z.x = (z.x + 1).min(doc.map.width - 1);
                    z.name = format!("{}_copy", z.name);
                    copies.push(Selection::Zone(z.uid));
                    doc.map.zones.push(z);
                }
                _ => {}
            }
        }
        doc.record_objects(before, "duplicate");
        self.set_selection(copies);
    }

    /// Ctrl+C on a multi-selection: the selected entities (zones aren't copied), placed in the
    /// smallest tile rectangle around them.
    pub fn copy_many(&self) -> Option<StructureDef> {
        let ts = self.ts();
        let doc = self.doc.as_ref()?;
        let ents: Vec<_> = self.selected_set().into_iter().filter_map(|s| if let Selection::Entity(uid) = s { doc.map.entity(uid).cloned() } else { None }).collect();
        if ents.is_empty() {
            return None;
        }
        let tiles: Vec<(i32, i32)> = ents.iter().map(|e| feet_tile(e.x, e.y, ts)).collect();
        let (x0, y0) = (tiles.iter().map(|t| t.0).min()?, tiles.iter().map(|t| t.1).min()?);
        let (x1, y1) = (tiles.iter().map(|t| t.0).max()?, tiles.iter().map(|t| t.1).max()?);
        let entities = ents
            .into_iter()
            .map(|mut e| {
                e.uid = 0;
                e.x -= x0 as f32 * ts;
                e.y -= y0 as f32 * ts;
                e
            })
            .collect();
        Some(StructureDef { id: "clipboard".into(), name: "Clipboard".into(), w: (x1 - x0 + 1) as u32, h: (y1 - y0 + 1) as u32, entities, ..Default::default() })
    }

    /// The inspector of a multi-selection: what is selected (click one to select only it),
    /// *Active* for all entities, Delete.
    pub fn multi_inspector(&mut self, ui: &mut egui_macroquad::egui::Ui) {
        use crate::ui::{self, ph};
        let set = self.selected_set();
        let Some(doc) = &mut self.doc else { return };
        ui.heading(format!("{} objects selected", set.len()));
        ui.weak("Drag one of them to move them all. Ctrl+click adds or removes an object, Ctrl+drag on empty ground selects everything in a box.");
        let mut pick = None;
        for s in &set {
            let label = match *s {
                Selection::Entity(uid) => doc.map.entity(uid).map(|e| format!("{} {}{} #{uid}", e.kind.label(), e.def, if e.name.is_empty() { String::new() } else { format!(" \"{}\"", e.name) })),
                Selection::Zone(uid) => doc.map.zones.iter().find(|z| z.uid == uid).map(|z| format!("Zone {} #{uid}", z.name)),
                _ => None,
            };
            if let Some(l) = label {
                if ui.selectable_label(false, l).on_hover_text("Select only this one").clicked() {
                    pick = Some(*s);
                }
            }
        }
        ui.separator();
        let ents: Vec<u32> = set.iter().filter_map(|s| if let Selection::Entity(u) = s { Some(*u) } else { None }).collect();
        if !ents.is_empty() {
            let all_active = ents.iter().all(|u| doc.map.entity(*u).is_some_and(|e| e.active));
            let mut active = all_active;
            if ui.checkbox(&mut active, "Active (all selected entities)").on_hover_text("Inactive entities are hidden in the game until an action or script shows them").changed() {
                let before = Objects::of(&doc.map);
                for e in doc.map.entities.iter_mut().filter(|e| ents.contains(&e.uid)) {
                    e.active = active;
                }
                doc.record_objects(before, "active");
            }
        }
        let (mut delete, mut duplicate) = (false, false);
        ui.horizontal(|ui| {
            if ui.button(ui::t(ph::COPY, "Duplicate")).on_hover_text("Ctrl+D").clicked() {
                duplicate = true;
            }
            if ui.button(ui::t(ph::TRASH, "Delete")).on_hover_text("Del").clicked() {
                delete = true;
            }
        });
        if delete {
            self.delete_many();
        } else if duplicate {
            self.duplicate_many();
        } else if let Some(s) = pick {
            self.set_selection(vec![s]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustpg2d_engine::entity::{EntityInstance, EntityKind};
    use rustpg2d_engine::map::{Zone, ZoneKind};

    fn map() -> Map {
        let mut m = Map::new("m", 20, 20);
        for (uid, tx, ty) in [(1, 2, 2), (2, 5, 5), (3, 12, 12)] {
            m.entities.push(EntityInstance::new(uid, EntityKind::Prop, "barrel", tx as f32 * 32.0 + 16.0, ty as f32 * 32.0 + 31.0));
        }
        m.zones.push(Zone { uid: 4, name: "inside".into(), x: 3, y: 3, w: 2, h: 2, kind: ZoneKind::Trigger, triggers: vec![], loot: None });
        m.zones.push(Zone { uid: 5, name: "big".into(), x: 0, y: 0, w: 20, h: 20, kind: ZoneKind::Trigger, triggers: vec![], loot: None });
        m
    }

    #[test]
    fn a_box_takes_entities_by_their_feet_and_zones_inside_it() {
        let m = map();
        let got = objects_in(&m, (0, 0, 8, 8), 32.0, true);
        assert_eq!(got, vec![Selection::Entity(1), Selection::Entity(2), Selection::Zone(4)], "the big zone reaches outside the box");
        assert_eq!(objects_in(&m, (0, 0, 8, 8), 32.0, false).len(), 2, "hidden zones aren't selected");
    }

    #[test]
    fn moving_keeps_zones_on_the_map() {
        let mut m = map();
        let origs = [Orig::Entity(1, vec2(m.entities[0].x, m.entities[0].y)), Orig::Zone(4, (3, 3))];
        place_moved(&mut m, &origs, -5, 2, 32.0);
        // The entity on tile 2 can go two tiles left: the zone (from 3) follows by as much.
        assert_eq!(feet_tile(m.entities[0].x, m.entities[0].y, 32.0), (0, 4));
        let z = m.zones.iter().find(|z| z.uid == 4).unwrap();
        assert_eq!((z.x, z.y), (1, 5));
    }
}
