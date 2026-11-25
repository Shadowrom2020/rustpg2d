//! The Build tool (B): walls along tile edges, rooms and houses dragged as rectangles. Walls are
//! props drawn as blocks or thin models (a fence); north-to-south runs are the same prop turned a
//! quarter. Rooms paint their floor and put walls on their back sides (or all four); houses are
//! new building props in the look of an existing one, sized to the rectangle.

use egui_macroquad::egui;
use macroquad::prelude::*;
use rustpg2d_engine::decor::{Decor, DecorLayer};
use rustpg2d_engine::defs::{BuildingShape, Opening, PropDef, WallSide};
use rustpg2d_engine::map::Map;

use crate::app::Editor;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum BuildMode {
    /// Drag along tile edges: a wall.
    #[default]
    Walls,
    /// Drag a rectangle: a floor and walls around it.
    Room,
    /// Drag a rectangle: a building of that size.
    House,
}

#[derive(Default)]
pub struct BuildTool {
    pub mode: BuildMode,
    /// The wall prop (Walls, Room).
    pub wall: String,
    /// The floor's material (Room; "" = leave the ground).
    pub floor: String,
    /// Room: walls on all four sides (with a doorway in the south wall) instead of the back two.
    pub all_sides: bool,
    /// House: the building prop whose look a new house takes.
    pub style: String,
    /// Where the drag started (world pixels).
    pub start: Option<Vec2>,
}

/// Props that make walls: blocks (stone, timber, plaster…) and thin models (fences).
pub fn is_wall(p: &PropDef) -> bool {
    p.block > 0.0 || (!p.model.is_empty() && p.hitbox[0] >= 30.0 && p.hitbox[1] <= 8.0)
}

/// Whether wall prop `p` stands on a tile's edge (thin) rather than filling the tile.
fn thin(p: &PropDef) -> bool {
    p.hitbox[1] <= 12.0
}

/// The tile edge nearest to `v` (world pixels).
fn edge(v: f32, ts: f32) -> f32 {
    (v / ts).round() * ts
}

/// Places a wall of `prop` from `a` to `b` (world pixels, along the longer axis). Thin walls run
/// along the tile edge nearest to the drag, north-to-south runs turned a quarter; `outside`: the
/// wall stands south / east of that edge instead of north / west. Thick walls fill the tiles the
/// drag passes. Skips spots a piece of it stands on already; returns the decorations added.
pub fn wall_run(map: &mut Map, p: &PropDef, a: Vec2, b: Vec2, outside: bool, ts: f32) -> Vec<Decor> {
    let slot = map.decor_slot(&p.id);
    let horizontal = (b.x - a.x).abs() >= (b.y - a.y).abs();
    let mut spots: Vec<(i32, i32, u8)> = Vec::new();
    if thin(p) {
        let depth = p.hitbox[1];
        if horizontal {
            let y = edge(a.y, ts) + if outside { depth } else { 0.0 };
            let (t0, t1) = ((a.x.min(b.x) / ts).round() as i32, (a.x.max(b.x) / ts).round() as i32);
            for t in t0..t1.max(t0 + 1) {
                spots.push(((t as f32 * ts + ts / 2.0) as i32, y as i32, 0));
            }
        } else {
            let x = edge(a.x, ts) + if outside { depth / 2.0 } else { -depth / 2.0 };
            let (t0, t1) = ((a.y.min(b.y) / ts).round() as i32, (a.y.max(b.y) / ts).round() as i32);
            for t in t0..t1.max(t0 + 1) {
                spots.push((x as i32, ((t + 1) as f32 * ts) as i32, 1));
            }
        }
    } else {
        let (ta, tb) = ((a / ts).floor(), (b / ts).floor());
        let (x0, x1, y0, y1) = if horizontal { (ta.x.min(tb.x), ta.x.max(tb.x), ta.y, ta.y) } else { (ta.x, ta.x, ta.y.min(tb.y), ta.y.max(tb.y)) };
        for ty in y0 as i32..=y1 as i32 {
            for tx in x0 as i32..=x1 as i32 {
                spots.push(((tx as f32 * ts + ts / 2.0) as i32, ((ty + 1) as f32 * ts) as i32, 0));
            }
        }
    }
    let mut added = Vec::new();
    for (x, y, turn) in spots {
        if map.decor.iter().any(|d| d.def == slot && d.x == x && d.y == y) {
            continue;
        }
        let mut d = Decor { def: slot, x, y, flip: false, state: 0, layer: DecorLayer::Sorted };
        d.set_turn(turn);
        map.decor.push(d);
        added.push(d);
    }
    added
}

/// A building shape for a house `w` x `d` world pixels in the look of `style`: its walls, roof
/// and pictures, a door in the middle of the south wall and windows spread along the walls.
pub fn house_shape(style: &BuildingShape, w: f32, d: f32) -> BuildingShape {
    let door = style.openings.iter().find(|o| o.sill <= 0.0).cloned();
    let window = style.openings.iter().find(|o| o.sill > 0.0).cloned();
    let mut openings = Vec::new();
    if let Some(mut o) = door {
        o.side = WallSide::South;
        o.at = w / 2.0;
        openings.push(o);
    }
    if let Some(win) = window {
        let step = 64.0;
        let n = ((w - 48.0) / step).floor().max(0.0) as i32;
        for k in 0..=n {
            let at = 32.0 + k as f32 * step;
            if (at - w / 2.0).abs() > 28.0 && at < w - 16.0 {
                openings.push(Opening { side: WallSide::South, at, ..win.clone() });
            }
        }
        if d >= 96.0 {
            openings.push(Opening { side: WallSide::East, at: d / 2.0, ..win.clone() });
        }
    }
    let chimneys = style.chimneys.first().map(|c| vec![rustpg2d_engine::defs::Chimney { at: [w * 0.75, d / 2.0], ..c.clone() }]).unwrap_or_default();
    BuildingShape { openings, chimneys, ..style.clone() }
}

impl Editor {
    pub fn build_down(&mut self, world: Vec2) {
        self.build.start = Some(world);
    }

    /// The drag ended at `world`: the wall, room or house is made (one undo step each part).
    pub fn build_up(&mut self, world: Vec2) {
        let Some(start) = self.build.start.take() else { return };
        let ts = self.ts();
        match self.build.mode {
            BuildMode::Walls => {
                let Some(p) = self.project.db.prop(&self.build.wall).cloned() else { return self.set_status("Pick a wall first") };
                let Some(doc) = &mut self.doc else { return };
                let added = wall_run(&mut doc.map, &p, start, world, false, ts);
                let n = added.len();
                doc.record_decor(added, Vec::new());
                self.set_status(format!("Built {n} pieces of {}", p.name));
            }
            BuildMode::Room => self.build_room(start, world),
            BuildMode::House => self.build_house(start, world),
        }
    }

    /// The rectangle (world pixels, on tile edges) a drag from `a` to `b` covers: at least a tile.
    fn build_rect(&self, a: Vec2, b: Vec2) -> Rect {
        let ts = self.ts();
        let (x0, y0) = ((a.x.min(b.x) / ts).floor() * ts, (a.y.min(b.y) / ts).floor() * ts);
        let (x1, y1) = ((a.x.max(b.x) / ts).ceil() * ts, (a.y.max(b.y) / ts).ceil() * ts);
        Rect::new(x0, y0, (x1 - x0).max(ts), (y1 - y0).max(ts))
    }

    fn build_room(&mut self, a: Vec2, b: Vec2) {
        let r = self.build_rect(a, b);
        let ts = self.ts();
        let wall = self.project.db.prop(&self.build.wall).cloned();
        let (floor, all) = (self.build.floor.clone(), self.build.all_sides);
        let Some(doc) = &mut self.doc else { return };
        if !floor.is_empty() {
            if let Some(v) = doc.map.material_value(&floor) {
                let cells = doc.map.ground.paint_rect((r.x, r.y), (r.x + r.w, r.y + r.h), v);
                doc.record_ground(cells, v, &self.assets);
            }
        }
        let mut added = Vec::new();
        if let Some(p) = &wall {
            let (l, t, rt, bt) = (r.x, r.y, r.x + r.w, r.y + r.h);
            added.extend(wall_run(&mut doc.map, p, vec2(l, t), vec2(rt, t), false, ts));
            added.extend(wall_run(&mut doc.map, p, vec2(l, t), vec2(l, bt), false, ts));
            if all {
                // The south wall with a doorway in its middle.
                let door = ((r.w / ts / 2.0).floor() * ts) + l;
                if door > l {
                    added.extend(wall_run(&mut doc.map, p, vec2(l, bt), vec2(door, bt), true, ts));
                }
                if door + ts < rt {
                    added.extend(wall_run(&mut doc.map, p, vec2(door + ts, bt), vec2(rt, bt), true, ts));
                }
                added.extend(wall_run(&mut doc.map, p, vec2(rt, t), vec2(rt, bt), true, ts));
            }
        }
        let n = added.len();
        doc.record_decor(added, Vec::new());
        self.set_status(format!("Built a room of {} x {} tiles ({n} wall pieces)", (r.w / ts) as i32, (r.h / ts) as i32));
    }

    fn build_house(&mut self, a: Vec2, b: Vec2) {
        let r = self.build_rect(a, b);
        let ts = self.ts();
        let Some(style) = self.project.db.prop(&self.build.style).cloned() else { return self.set_status("Pick a house to build like") };
        let Some(shape) = style.building.clone() else { return self.set_status("That prop is no building") };
        let base = format!("{}_built", style.id);
        let id = (1..).map(|k| format!("{base}_{k}")).find(|c| self.project.db.prop(c).is_none()).unwrap();
        let p = PropDef { id: id.clone(), name: format!("{} ({}x{})", style.name, (r.w / ts) as i32, (r.h / ts) as i32), hitbox: [r.w, r.h], building: Some(house_shape(&shape, r.w, r.h)), model: String::new(), ..style.clone() };
        self.project.db.props.push(p);
        self.project.db.reindex();
        self.defs_dirty = true;
        self.assets.rebuild_buildings(&self.project.db);
        let Some(doc) = &mut self.doc else { return };
        let slot = doc.map.decor_slot(&id);
        let d = Decor { def: slot, x: (r.x + r.w / 2.0) as i32, y: (r.y + r.h) as i32, flip: false, state: 0, layer: DecorLayer::Sorted };
        doc.map.decor.push(d);
        doc.record_decor(vec![d], Vec::new());
        let _ = self.render_prop_pictures(Some(&id));
        self.set_status(format!("Built the house '{id}' ({} x {} tiles): Database › Props › {id} changes its doors, windows and roof", (r.w / ts) as i32, (r.h / ts) as i32));
    }

    /// Canvas: what the drag would build.
    pub fn draw_build_hover(&self, world: Vec2, px: f32) {
        let ts = self.ts();
        let Some(a) = self.build.start else {
            let e = vec2(edge(world.x, ts), edge(world.y, ts));
            draw_circle(e.x, e.y, 4.0 * px, Color::new(1.0, 0.85, 0.2, 0.9));
            return;
        };
        let col = Color::new(1.0, 0.85, 0.2, 0.9);
        match self.build.mode {
            BuildMode::Walls => {
                let horizontal = (world.x - a.x).abs() >= (world.y - a.y).abs();
                let (p, q) = if horizontal { (vec2(edge(a.x, ts), edge(a.y, ts)), vec2(edge(world.x, ts), edge(a.y, ts))) } else { (vec2(edge(a.x, ts), edge(a.y, ts)), vec2(edge(a.x, ts), edge(world.y, ts))) };
                draw_line(p.x, p.y, q.x, q.y, 4.0 * px, col);
            }
            BuildMode::Room | BuildMode::House => {
                let r = self.build_rect(a, world);
                draw_rectangle(r.x, r.y, r.w, r.h, Color::new(1.0, 0.85, 0.2, 0.15));
                draw_rectangle_lines(r.x, r.y, r.w, r.h, 3.0 * px, col);
            }
        }
    }

    /// The Build tool's left panel.
    pub fn build_palette(&mut self, ui: &mut egui::Ui) {
        ui.heading("Build");
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.build.mode, BuildMode::Walls, "Walls").on_hover_text("Drag along tile edges: a wall (north to south it turns by itself)");
            ui.selectable_value(&mut self.build.mode, BuildMode::Room, "Room").on_hover_text("Drag a rectangle: its floor and walls around it");
            ui.selectable_value(&mut self.build.mode, BuildMode::House, "House").on_hover_text("Drag a rectangle: a house of that size in the look of another");
        });
        ui.weak(match self.build.mode {
            BuildMode::Walls => "Drag along a tile edge. Thin walls and fences stand on the edge, thick walls fill the tiles. Erase pieces with the Decorations tool's Erase.",
            BuildMode::Room => "Drag the room's floor. Its back walls (north, west) are built, or all four with a doorway in the south wall.",
            BuildMode::House => "Drag the house's footprint. It becomes a new building prop in the chosen look: its door in the middle of the front, windows along the walls.",
        });
        let db = &self.project.db;
        if self.build.mode == BuildMode::House {
            let styles: Vec<(String, String)> = db.props.iter().filter(|p| p.building.is_some()).map(|p| (p.id.clone(), p.name.clone())).collect();
            ui.label("Look");
            for (id, name) in styles {
                ui.selectable_value(&mut self.build.style, id.clone(), name);
            }
            return;
        }
        if self.build.mode == BuildMode::Room {
            ui.checkbox(&mut self.build.all_sides, "Walls on all four sides").on_hover_text("Off: only the back walls (north and west), open to the viewer like the classic interiors");
            let mats: Vec<(String, String)> = db.materials.iter().map(|m| (m.id.clone(), m.name.clone())).collect();
            egui::ComboBox::from_label("Floor").selected_text(if self.build.floor.is_empty() { "(leave the ground)".to_string() } else { self.build.floor.clone() }).show_ui(ui, |ui| {
                ui.selectable_value(&mut self.build.floor, String::new(), "(leave the ground)");
                for (id, name) in mats {
                    ui.selectable_value(&mut self.build.floor, id, name);
                }
            });
        }
        ui.label("Wall");
        let walls: Vec<(String, String, rustpg2d_engine::defs::SpriteRef)> = db.props.iter().filter(|p| is_wall(p) && !p.id.ends_with("_side")).map(|p| (p.id.clone(), p.name.clone(), p.sprite.clone())).collect();
        egui::ScrollArea::vertical().id_salt("build_walls").max_height(360.0).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (id, name, sprite) in &walls {
                    let on = *id == self.build.wall;
                    if crate::widgets::sprite_thumb(ui, &self.tex, &self.assets, sprite, 40.0, on).on_hover_text(format!("{name} ({id})")).clicked() {
                        self.build.wall = id.clone();
                    }
                }
            });
        });
    }

    /// `build=` for the editor's `status`.
    pub fn build_status(&self) -> String {
        format!("{:?}:{}:{}:{}", self.build.mode, self.build.wall, if self.build.all_sides { "all" } else { "back" }, self.build.style).to_lowercase()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walls_run_along_tile_edges_and_turn_north_to_south() {
        let wall = PropDef { block: 48.0, hitbox: [32.0, 8.0], ..PropDef::fresh("wall") };
        let mut map = Map::new("m", 10, 10);
        // East to west along the edge at y = 64 (dragged a little off it): three pieces on it.
        let a = wall_run(&mut map, &wall, vec2(30.0, 70.0), vec2(130.0, 60.0), false, 32.0);
        assert_eq!(a.len(), 3);
        assert!(a.iter().all(|d| d.y == 64 && d.turn() == 0));
        // North to south along x = 32: turned, west of the edge.
        let b = wall_run(&mut map, &wall, vec2(30.0, 64.0), vec2(34.0, 160.0), false, 32.0);
        assert_eq!(b.len(), 3);
        assert!(b.iter().all(|d| d.x == 28 && d.turn() == 1));
        // Again: nothing new.
        assert!(wall_run(&mut map, &wall, vec2(30.0, 70.0), vec2(130.0, 60.0), false, 32.0).is_empty());
    }

    #[test]
    fn a_house_gets_a_door_in_its_front_and_windows() {
        let door = Opening { side: WallSide::West, at: 5.0, picture: Default::default(), size: [26.0, 40.0], sill: 0.0 };
        let win = Opening { sill: 30.0, size: [20.0, 20.0], ..door.clone() };
        let style = BuildingShape { openings: vec![door, win], ..Default::default() };
        let s = house_shape(&style, 256.0, 128.0);
        let d: Vec<&Opening> = s.openings.iter().filter(|o| o.sill <= 0.0).collect();
        assert_eq!((d.len(), d[0].side, d[0].at), (1, WallSide::South, 128.0));
        assert!(s.openings.iter().filter(|o| o.sill > 0.0 && o.side == WallSide::South).all(|o| (o.at - 128.0).abs() > 28.0));
        assert!(s.openings.iter().any(|o| o.side == WallSide::East));
    }
}
