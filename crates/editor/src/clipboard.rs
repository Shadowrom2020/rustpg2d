//! Region copy, cut and paste on the map. The Select tool drags a rectangle
//! (`Selection::Region`); Ctrl+C captures it (all layers, collision overrides, the entities
//! standing in it) as an in-memory `StructureDef`, Ctrl+X also empties it, Ctrl+V arms the
//! clipboard: it follows the mouse as a ghost and every click pastes it as one undo step,
//! until Esc / right-click. A selected entity can be copied on its own (a 1×1 clipboard
//! without tiles). The clipboard survives switching maps; zones are never copied.

use std::collections::HashSet;

use egui_macroquad::egui;
use rustpg2d_engine::defs::StructureDef;
use rustpg2d_engine::entity::EntityInstance;
use rustpg2d_engine::map::Map;

use crate::app::{Editor, Selection, Tool};
use crate::ui::{self, ph};

/// A button of the region inspector.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RegionReq {
    Copy,
    Cut,
    Paste,
    Clear,
    SaveStructure,
}

/// Gives pasted entities names no other entity on the map has (`guard` → `guard_2`,
/// `guard_2` → `guard_3`): scripts and triggers find named entities by name. Unnamed
/// entities stay unnamed.
pub fn unique_names(new: &mut [EntityInstance], existing: &[EntityInstance]) {
    let mut taken: HashSet<String> = existing.iter().filter(|e| !e.name.is_empty()).map(|e| e.name.clone()).collect();
    for e in new.iter_mut().filter(|e| !e.name.is_empty()) {
        if taken.contains(&e.name) {
            let base = match e.name.rsplit_once('_') {
                Some((b, n)) if !b.is_empty() && !n.is_empty() && n.bytes().all(|c| c.is_ascii_digit()) => b.to_string(),
                _ => e.name.clone(),
            };
            e.name = (2..).map(|i| format!("{base}_{i}")).find(|n| !taken.contains(n)).unwrap();
        }
        taken.insert(e.name.clone());
    }
}

/// A rectangle `(x0, y0)`–`(x1, y1)` in tiles (either corner first, inclusive) clipped to the
/// map; `None` if nothing of it is inside.
pub fn clip_rect(map: &Map, a: (i32, i32), b: (i32, i32)) -> Option<(u32, u32, u32, u32)> {
    let (x0, y0) = (a.0.min(b.0).max(0), a.1.min(b.1).max(0));
    let (x1, y1) = ((a.0.max(b.0) + 1).min(map.width as i32), (a.1.max(b.1) + 1).min(map.height as i32));
    (x1 > x0 && y1 > y0).then(|| (x0 as u32, y0 as u32, (x1 - x0) as u32, (y1 - y0) as u32))
}

/// Entities whose feet are inside a tile rectangle (what a copy takes along).
fn entities_in(map: &Map, r: (u32, u32, u32, u32)) -> usize {
    let ts = rustpg2d_engine::project::TILE_SIZE as f32;
    let (l, t) = (r.0 as f32 * ts, r.1 as f32 * ts);
    let (rt, b) = (l + r.2 as f32 * ts, t + r.3 as f32 * ts);
    map.entities.iter().filter(|e| e.x >= l && e.x < rt && e.y >= t && e.y < b).count()
}

/// Inspector of a selected region: size, contents and the clipboard buttons.
pub fn region_inspector(ui: &mut egui::Ui, map: &Map, r: (u32, u32, u32, u32), clipboard: Option<&StructureDef>) -> Option<RegionReq> {
    let mut req = None;
    ui.heading(format!("Region {}×{}", r.2, r.3));
    ui.label(format!("From tile {},{} to {},{} · {} entities inside", r.0, r.1, r.0 + r.2 - 1, r.1 + r.3 - 1, entities_in(map, r)));
    ui.weak("Copy takes the tiles of all layers, collision overrides and the entities standing in the region. Zones are not copied.");
    ui.horizontal_wrapped(|ui| {
        if ui.button(ui::t(ph::COPY, "Copy")).on_hover_text("Ctrl+C").clicked() {
            req = Some(RegionReq::Copy);
        }
        if ui.button(ui::t(ph::SCISSORS, "Cut")).on_hover_text("Ctrl+X: copy, then empty the region").clicked() {
            req = Some(RegionReq::Cut);
        }
        if ui.add_enabled(clipboard.is_some(), egui::Button::new(ui::t(ph::CLIPBOARD_TEXT, "Paste"))).on_hover_text("Ctrl+V: the clipboard follows the mouse, click to place it").clicked() {
            req = Some(RegionReq::Paste);
        }
        if ui.button(ui::t(ph::ERASER, "Clear")).on_hover_text("Del: empty the tiles of all layers, reset collision, remove the entities").clicked() {
            req = Some(RegionReq::Clear);
        }
    });
    if ui.button(ui::t(ph::SELECTION_PLUS, "Save as structure…")).on_hover_text("A reusable structure for the Structures tool (H)").clicked() {
        req = Some(RegionReq::SaveStructure);
    }
    if let Some(c) = clipboard {
        ui.separator();
        ui.weak(format!("Clipboard: {}×{} tiles, {} entities", c.w, c.h, c.entities.len()));
    }
    req
}

impl Editor {
    /// Copies the selected region or entity to the clipboard. Returns false (with a status
    /// message) when there is nothing to copy.
    pub fn copy_selection(&mut self) -> bool {
        let Some(doc) = &self.doc else { return false };
        let ts = self.ts();
        let clip = if self.is_multi() {
            match self.copy_many() {
                Some(c) => c,
                None => {
                    self.set_status("Zones aren't copied: select entities (Ctrl+click) or drag a region of tiles");
                    return false;
                }
            }
        } else {
            match self.sel {
            Selection::Region(r) => StructureDef::capture(&doc.map, "clipboard", "Clipboard", r, true),
            Selection::Entity(uid) => {
                let Some(e) = doc.map.entity(uid) else { return false };
                // The tile the entity stands on becomes the 1×1 clipboard.
                let (tx, ty) = ((e.x / ts).floor(), ((e.y - 0.5) / ts).floor());
                let mut c = e.clone();
                c.uid = 0;
                c.x -= tx * ts;
                c.y -= ty * ts;
                StructureDef { id: "clipboard".into(), name: "Clipboard".into(), w: 1, h: 1, entities: vec![c], ..Default::default() }
            }
            Selection::Zone(_) => {
                self.set_status("Zones aren't copied: use Duplicate (Ctrl+D) for a zone, or drag a region of tiles with the Select tool");
                return false;
            }
            Selection::None => {
                self.set_status("Nothing to copy: drag a region with the Select tool (V), or select an entity");
                return false;
            }
            }
        };
        let what = if clip.layers.is_empty() && clip.entities.len() > 1 {
            format!("Copied {} entities", clip.entities.len())
        } else if clip.layers.is_empty() {
            format!("Copied {}", clip.entities.first().map_or("", |e| e.def.as_str()))
        } else {
            format!("Copied {}×{} tiles and {} entities", clip.w, clip.h, clip.entities.len())
        };
        self.set_status(format!("{what} · Ctrl+V to paste (also on another map)"));
        self.clipboard = Some(clip);
        true
    }

    /// Copies the selection, then removes it: a region's tiles, collision overrides and
    /// entities (one undo step), or the selected entity.
    pub fn cut_selection(&mut self) {
        if !self.copy_selection() {
            return;
        }
        match self.sel {
            _ if self.is_multi() => self.delete_many(),
            Selection::Region(r) => {
                if let Some(doc) = &mut self.doc {
                    doc.clear_region(r, &self.assets);
                }
            }
            Selection::Entity(_) => self.delete_selection(),
            _ => {}
        }
        let msg = self.status.0.replacen("Copied", "Cut", 1);
        self.set_status(msg);
    }

    /// Ctrl+V: arms the clipboard on the Select tool; clicks paste it.
    pub fn start_paste(&mut self) {
        let Some(c) = &self.clipboard else {
            self.set_status("Nothing to paste: drag a region with the Select tool and press Ctrl+C first");
            return;
        };
        let msg = format!("Pasting {}×{}: click to place it (again for more copies) · Esc / right-click: stop", c.w, c.h);
        self.tool = Tool::Select;
        self.pasting = true;
        self.set_status(msg);
    }

    /// Top-left tile of the clipboard while it follows the mouse (centered on the cursor).
    pub fn paste_origin(&self, hover: (i32, i32)) -> Option<(i32, i32)> {
        self.clipboard.as_ref().map(|c| self.structure_origin(c, hover))
    }

    /// Pastes the clipboard with its top-left corner at tile `at` (one undo step) and selects
    /// what was pasted.
    pub fn paste_at(&mut self, at: (i32, i32)) {
        let Some(mut clip) = self.clipboard.clone() else { return };
        let Some(doc) = &mut self.doc else { return };
        unique_names(&mut clip.entities, &doc.map.entities);
        let added = doc.place_structure(&clip, at.0, at.1, &self.assets);
        let mut msg = format!("Pasted {}×{} at {},{}", clip.w, clip.h, at.0, at.1);
        let missing: Vec<&str> = clip.tilesets.iter().filter(|t| self.project.db.tileset(t).is_none()).map(String::as_str).collect();
        if !missing.is_empty() {
            msg += &format!(" (missing tileset {})", missing.join(", "));
        }
        if clip.layers.len() > doc.map.layers.len() {
            msg += &format!(" (the map has only {} layers; the rest was skipped)", doc.map.layers.len());
        }
        self.sel = if clip.layers.is_empty() && added.len() == 1 {
            Selection::Entity(added[0])
        } else {
            clip_rect(&doc.map, at, (at.0 + clip.w as i32 - 1, at.1 + clip.h as i32 - 1)).map_or(Selection::None, Selection::Region)
        };
        self.set_status(msg);
    }

    /// Stops pasting (Esc, right-click, another tool).
    pub fn stop_paste(&mut self) {
        if self.pasting {
            self.pasting = false;
            self.set_status("Paste done");
        }
    }

    /// Canvas: the clipboard's ghost under the mouse while pasting.
    pub fn draw_paste_ghost(&self, hover: (i32, i32)) {
        if let (Some(c), Some(o)) = (&self.clipboard, self.paste_origin(hover)) {
            self.draw_structure_def_ghost(c, o);
        }
    }

    /// Canvas: outline of a selected region with its size.
    pub fn draw_region(&self, r: (u32, u32, u32, u32)) {
        use macroquad::prelude::*;
        let ts = self.ts();
        let px = 1.0 / self.cam.zoom;
        let (x, y, w, h) = (r.0 as f32 * ts, r.1 as f32 * ts, r.2 as f32 * ts, r.3 as f32 * ts);
        draw_rectangle(x, y, w, h, Color::new(0.35, 0.75, 1.0, 0.15));
        // Dashed outline: alternating light / dark segments stay visible on any ground.
        let dash = 8.0 * px;
        let seg = |a: Vec2, b: Vec2| {
            let len = (b - a).length();
            let n = (len / dash).ceil().max(1.0) as usize;
            for i in 0..n {
                let (s, e) = (a.lerp(b, i as f32 / n as f32), a.lerp(b, (i + 1) as f32 / n as f32));
                draw_line(s.x, s.y, e.x, e.y, 2.0 * px, if i % 2 == 0 { Color::new(0.5, 0.85, 1.0, 1.0) } else { Color::new(0.0, 0.1, 0.25, 1.0) });
            }
        };
        let (a, b, c, d) = (vec2(x, y), vec2(x + w, y), vec2(x + w, y + h), vec2(x, y + h));
        seg(a, b);
        seg(b, c);
        seg(c, d);
        seg(d, a);
    }

    /// Carries out a region inspector button.
    pub fn region_request(&mut self, req: RegionReq) {
        match (req, self.sel) {
            (RegionReq::Copy, _) => {
                self.copy_selection();
            }
            (RegionReq::Cut, _) => self.cut_selection(),
            (RegionReq::Paste, _) => self.start_paste(),
            (RegionReq::Clear, _) => self.delete_selection(),
            (RegionReq::SaveStructure, Selection::Region(r)) => self.capture_dragged(r),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustpg2d_engine::entity::EntityKind;

    #[test]
    fn pasted_names_stay_unique() {
        let named = |uid, name: &str| {
            let mut e = EntityInstance::new(uid, EntityKind::Character, "guard", 0.0, 0.0);
            e.name = name.into();
            e
        };
        let map = vec![named(1, "guard"), named(2, "guard_2"), named(3, "")];
        let mut new = vec![named(0, "guard"), named(0, "guard"), named(0, "elder"), named(0, ""), named(0, "guard_2")];
        unique_names(&mut new, &map);
        let names: Vec<&str> = new.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["guard_3", "guard_4", "elder", "", "guard_5"]);
    }

    #[test]
    fn regions_are_clipped_to_the_map() {
        let m = Map::new("m", 10, 8);
        assert_eq!(clip_rect(&m, (4, 5), (1, 2)), Some((1, 2, 4, 4)));
        assert_eq!(clip_rect(&m, (-3, 6), (12, 20)), Some((0, 6, 10, 2)));
        assert_eq!(clip_rect(&m, (11, 0), (14, 3)), None);
    }
}
