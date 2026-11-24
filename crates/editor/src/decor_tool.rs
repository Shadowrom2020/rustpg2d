//! The Decorations tool: props placed freely as pictures ([`rustpg2d_engine::decor`]): one at a
//! click, a line of wall pieces that pick themselves by their neighbours, a scatter brush with a
//! scatter set (a forest, flowers), and an eraser brush. Each action is one undo step.

use egui_macroquad::egui;
use macroquad::prelude::*;

use crate::app::Editor;
use crate::widgets;
use rustpg2d_engine::decor::{self, Decor, DecorLayer};
use rustpg2d_engine::map::Map;

/// How the Decorations tool places.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DecorMode {
    /// One decoration per click.
    #[default]
    Place,
    /// Drag a straight line: wall pieces on the prop's grid, joined by their neighbours.
    Line,
    /// A brush that scatters a scatter set's props.
    Scatter,
    /// A brush that removes decorations.
    Erase,
}

impl DecorMode {
    pub const ALL: [DecorMode; 4] = [DecorMode::Place, DecorMode::Line, DecorMode::Scatter, DecorMode::Erase];
    pub fn label(self) -> &'static str {
        match self {
            DecorMode::Place => "Place",
            DecorMode::Line => "Line",
            DecorMode::Scatter => "Scatter",
            DecorMode::Erase => "Erase",
        }
    }
    pub fn parse(s: &str) -> Option<DecorMode> {
        DecorMode::ALL.into_iter().find(|m| m.label().eq_ignore_ascii_case(s))
    }
}

/// The tool's settings and a drag in progress.
pub struct DecorTool {
    pub mode: DecorMode,
    /// The prop placed (Place, Line).
    pub prop: String,
    /// The scatter set (Scatter).
    pub scatter: String,
    pub layer: DecorLayer,
    /// Mirror placed props at random.
    pub random_flip: bool,
    /// Brush radius in pixels (Scatter, Erase).
    pub size: f32,
    /// Line: where the drag started; brushes: the decorations added / removed so far.
    pub line_start: Option<Vec2>,
    pub stroke: Option<(Vec<Decor>, Vec<Decor>)>,
    pub last: Vec2,
    pub seed: u64,
    pub filter: String,
}

impl Default for DecorTool {
    fn default() -> Self {
        DecorTool { mode: DecorMode::Place, prop: String::new(), scatter: String::new(), layer: DecorLayer::Sorted, random_flip: false, size: 48.0, line_start: None, stroke: None, last: Vec2::ZERO, seed: 1, filter: String::new() }
    }
}

/// Removes the first decoration equal to `d` (decorations have no ids; order doesn't matter).
pub fn remove_one(list: &mut Vec<Decor>, d: &Decor) -> bool {
    match list.iter().position(|x| x == d) {
        Some(i) => {
            list.swap_remove(i);
            true
        }
        None => false,
    }
}

/// The grid point (feet) of a spot for a prop that snaps to `s` pixels: the middle of the cell's
/// bottom edge.
pub fn snap_point(p: Vec2, s: f32) -> Vec2 {
    vec2((p.x / s).floor() * s + s / 2.0, (p.y / s).floor() * s + s)
}

/// Places a line of wall pieces of `set` from `a` to `b` (along the longer axis) on its grid and
/// picks the pieces of every member of the set around it again; returns (added, removed).
pub fn wall_line(map: &mut Map, db: &rustpg2d_engine::defs::Database, set: &str, a: Vec2, b: Vec2, layer: DecorLayer) -> (Vec<Decor>, Vec<Decor>) {
    let s = db.prop(set).map_or(32, |d| if d.snap == 0 { 32 } else { d.snap }) as f32;
    let (a, b) = (snap_point(a, s), snap_point(b, s));
    let b = if (b.x - a.x).abs() >= (b.y - a.y).abs() { vec2(b.x, a.y) } else { vec2(a.x, b.y) };
    let n = (((b - a).length() / s).round() as i32).max(0);
    let dir = if n == 0 { Vec2::ZERO } else { (b - a) / n as f32 };
    let family: Vec<String> = std::iter::once(set.to_string()).chain(db.prop(set).map(|d| d.pieces.clone()).unwrap_or_default()).filter(|p| !p.is_empty()).collect();
    let is_member = |map: &Map, d: &Decor| map.decor_defs.get(d.def as usize).is_some_and(|id| family.contains(id));
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let slot = map.decor_slot(set);
    for k in 0..=n {
        let p = a + dir * k as f32;
        let (x, y) = (p.x.round() as i32, p.y.round() as i32);
        if map.decor.iter().any(|d| d.x == x && d.y == y && is_member(map, d)) {
            continue;
        }
        let d = Decor { def: slot, x, y, flip: false, state: 0, layer };
        map.decor.push(d);
        added.push(d);
    }
    // Every member of the set near the line takes the piece its neighbours ask for.
    let (lo, hi) = (a.min(b) - Vec2::splat(s * 1.5), a.max(b) + Vec2::splat(s * 1.5));
    let members: Vec<(i32, i32)> = map.decor.iter().filter(|d| is_member(map, d)).map(|d| (d.x, d.y)).collect();
    let si = s.round() as i32;
    for i in 0..map.decor.len() {
        let d = map.decor[i];
        if !is_member(map, &d) || (d.x as f32) < lo.x || (d.x as f32) > hi.x || (d.y as f32) < lo.y || (d.y as f32) > hi.y {
            continue;
        }
        let has = |dx: i32, dy: i32| members.contains(&(d.x + dx * si, d.y + dy * si));
        let mask = if has(0, -1) { decor::NORTH } else { 0 } | if has(1, 0) { decor::EAST } else { 0 } | if has(0, 1) { decor::SOUTH } else { 0 } | if has(-1, 0) { decor::WEST } else { 0 };
        let piece = decor::wall_piece(db, set, mask);
        let def = map.decor_slot(&piece);
        if def != d.def {
            let new = Decor { def, ..d };
            // A piece placed by this line: just its new look; an older one: removed and added.
            if let Some(j) = added.iter().position(|x| *x == d) {
                added[j] = new;
            } else {
                removed.push(d);
                added.push(new);
            }
            map.decor[i] = new;
        }
    }
    (added, removed)
}

impl Editor {
    /// Mouse pressed on the map with the Decorations tool.
    pub(crate) fn decor_down(&mut self, world: Vec2) {
        match self.decor.mode {
            DecorMode::Place => self.decor_place(world),
            DecorMode::Line => {
                if self.decor.prop.is_empty() {
                    self.set_status("Pick a prop in the left panel first");
                    return;
                }
                self.decor.line_start = Some(world);
            }
            DecorMode::Scatter | DecorMode::Erase => {
                if self.decor.mode == DecorMode::Scatter && self.project.db.scatter_set(&self.decor.scatter).is_none() {
                    self.set_status("Pick a scatter set in the left panel first");
                    return;
                }
                self.decor.stroke = Some((Vec::new(), Vec::new()));
                self.decor.last = world;
                self.decor_brush(world);
            }
        }
    }

    pub(crate) fn decor_drag_to(&mut self, world: Vec2) {
        if self.decor.stroke.is_some() && world.distance(self.decor.last) >= self.decor.size * 0.5 {
            self.decor.last = world;
            self.decor_brush(world);
        }
    }

    pub(crate) fn decor_up(&mut self, world: Vec2) {
        if let Some((added, removed)) = self.decor.stroke.take() {
            if let Some(doc) = &mut self.doc {
                doc.record_decor(added, removed);
            }
        }
        if let Some(start) = self.decor.line_start.take() {
            let (prop, layer) = (self.decor.prop.clone(), self.decor.layer);
            let set = decor::wall_set_of(&self.project.db, &prop).unwrap_or(prop);
            let db = &self.project.db;
            if let Some(doc) = &mut self.doc {
                let (added, removed) = wall_line(&mut doc.map, db, &set, start, world, layer);
                let n = added.len();
                doc.record_decor(added, removed);
                self.set_status(format!("Placed {n} pieces of {set}"));
            }
        }
    }

    /// One decoration of the chosen prop at the cursor (on its grid when it snaps).
    fn decor_place(&mut self, world: Vec2) {
        let Some(def) = self.project.db.prop(&self.decor.prop) else {
            self.set_status("Pick a prop in the left panel first");
            return;
        };
        let p = if def.snap > 0 { snap_point(world, def.snap as f32) } else { world.round() };
        self.decor.seed = self.decor.seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let flip = self.decor.random_flip && (self.decor.seed >> 33) & 1 == 1;
        let (prop, layer) = (self.decor.prop.clone(), self.decor.layer);
        if let Some(doc) = &mut self.doc {
            let slot = doc.map.decor_slot(&prop);
            let d = Decor { def: slot, x: p.x as i32, y: p.y as i32, flip, state: 0, layer };
            doc.map.decor.push(d);
            doc.record_decor(vec![d], vec![]);
        }
    }

    /// A dab of the scatter or erase brush at `world`.
    fn decor_brush(&mut self, world: Vec2) {
        let r = self.decor.size;
        let Some(doc) = &mut self.doc else { return };
        let Some((added, removed)) = &mut self.decor.stroke else { return };
        match self.decor.mode {
            DecorMode::Erase => {
                let gone: Vec<Decor> = doc.map.decor.iter().filter(|d| vec2(d.x as f32, d.y as f32).distance(world) <= r).copied().collect();
                for d in &gone {
                    remove_one(&mut doc.map.decor, d);
                    // Erasing what this stroke added just takes it back.
                    if !remove_one(added, d) {
                        removed.push(*d);
                    }
                }
            }
            _ => {
                let Some(set) = self.project.db.scatter_set(&self.decor.scatter).cloned() else { return };
                let near: Vec<(f32, f32)> = doc.map.decor.iter().filter(|d| vec2(d.x as f32, d.y as f32).distance(world) <= r + set.spacing).map(|d| (d.x as f32, d.y as f32)).collect();
                self.decor.seed = self.decor.seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
                for (prop, x, y, flip) in set.scatter((world.x, world.y), r, &near, self.decor.seed) {
                    let slot = doc.map.decor_slot(&prop);
                    let d = Decor { def: slot, x: x as i32, y: y as i32, flip, state: 0, layer: set.layer };
                    doc.map.decor.push(d);
                    added.push(d);
                }
            }
        }
    }

    /// Right-click: the decoration nearest the cursor gives its prop (and layer) to the tool.
    pub(crate) fn pick_decor(&mut self, world: Vec2) {
        let Some(doc) = &self.doc else { return };
        let best = doc.map.decor.iter().min_by(|a, b| vec2(a.x as f32, a.y as f32).distance(world).total_cmp(&vec2(b.x as f32, b.y as f32).distance(world)));
        match best.filter(|d| vec2(d.x as f32, d.y as f32).distance(world) < 48.0) {
            Some(d) => {
                let id = doc.map.decor_defs[d.def as usize].clone();
                self.decor.layer = d.layer;
                self.set_status(format!("Prop: {id}"));
                self.decor.prop = id;
                self.decor.mode = DecorMode::Place;
            }
            None => self.set_status("No decoration here"),
        }
    }

    /// The tool's preview at the cursor (world coordinates).
    pub(crate) fn draw_decor_hover(&self, world: Vec2, px: f32) {
        let ghost = Color::new(1.0, 1.0, 1.0, 0.55);
        match self.decor.mode {
            DecorMode::Place | DecorMode::Line => {
                let Some(def) = self.project.db.prop(&self.decor.prop) else { return };
                let s = def.snap as f32;
                let at = |p: Vec2| if s > 0.0 { snap_point(p, s) } else if self.decor.mode == DecorMode::Line { snap_point(p, 32.0) } else { p };
                if let Some(start) = self.decor.line_start {
                    let (a, b) = (at(start), at(world));
                    let b = if (b.x - a.x).abs() >= (b.y - a.y).abs() { vec2(b.x, a.y) } else { vec2(a.x, b.y) };
                    draw_line(a.x, a.y, b.x, b.y, 3.0 * px, YELLOW);
                }
                if def.flat {
                    self.assets.draw_sprite(&def.sprite, 0, at(world), ghost);
                } else {
                    self.standing(at(world), || self.assets.draw_sprite(&def.sprite, 0, at(world), ghost));
                }
                draw_circle_lines(at(world).x, at(world).y, 3.0 * px, 1.5 * px, YELLOW);
            }
            DecorMode::Scatter => draw_circle_lines(world.x, world.y, self.decor.size, 1.5 * px, Color::new(0.4, 1.0, 0.5, 1.0)),
            DecorMode::Erase => draw_circle_lines(world.x, world.y, self.decor.size, 1.5 * px, RED),
        }
    }

    /// The tool's panel.
    pub(crate) fn decor_palette(&mut self, ui: &mut egui::Ui) {
        ui.heading("Decorations");
        ui.label("Props placed freely as pictures: trees, rocks, flowers, fences, walls, buildings. Cheap in great numbers; for props that do something (doors, chests, trees to chop) use Entities. Right-click picks the one under the cursor.");
        ui.horizontal(|ui| {
            for m in DecorMode::ALL {
                let help = match m {
                    DecorMode::Place => "Click: one of the chosen prop (on its grid when it snaps)",
                    DecorMode::Line => "Drag a straight line of the chosen prop: wall sets join their pieces by their neighbours",
                    DecorMode::Scatter => "Click or drag: the scatter set's props at random (forests, flowers)",
                    DecorMode::Erase => "Click or drag: removes the decorations under the brush",
                };
                if ui.selectable_label(self.decor.mode == m, m.label()).on_hover_text(help).clicked() {
                    self.decor.mode = m;
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label("Layer").on_hover_text("Flat: on the ground under everyone · Sorted: in front of or behind characters by its feet · Above: over everyone (treetops, roofs)");
            for l in DecorLayer::ALL {
                ui.selectable_value(&mut self.decor.layer, l, l.label());
            }
        });
        if matches!(self.decor.mode, DecorMode::Scatter | DecorMode::Erase) {
            ui.horizontal(|ui| {
                ui.label("Size");
                let ts = rustpg2d_engine::project::TILE_SIZE as f32;
                let mut tiles = self.decor.size * 2.0 / ts;
                if ui.add(egui::Slider::new(&mut tiles, 0.5..=16.0).step_by(0.5).suffix(" tiles")).on_hover_text("The brush's diameter; [ and ] change it").changed() {
                    self.decor.size = tiles * ts / 2.0;
                }
            });
        }
        ui.checkbox(&mut self.decor.random_flip, "Mirror at random").on_hover_text("Each placed prop is mirrored left to right half of the time (scatter sets say it themselves)");
        ui.separator();
        if self.decor.mode == DecorMode::Scatter {
            ui.label("Scatter set");
            if self.project.db.scatter.is_empty() {
                ui.weak("None yet: add one in Database › Scatter sets (props and how often each comes).");
            }
            for s in &self.project.db.scatter {
                let on = s.id == self.decor.scatter;
                if ui.selectable_label(on, format!("{}  ({} props)", s.name, s.props.len())).clicked() {
                    self.decor.scatter = s.id.clone();
                }
            }
            return;
        }
        if self.decor.mode == DecorMode::Erase {
            ui.weak("Drag over decorations to remove them. Each stroke is one undo step.");
            return;
        }
        ui.horizontal(|ui| {
            ui.label("\u{E30C}");
            ui.add(egui::TextEdit::singleline(&mut self.decor.filter).hint_text("search props").desired_width(150.0));
        });
        let f = self.decor.filter.to_lowercase();
        let props: Vec<(String, String, rustpg2d_engine::defs::SpriteRef, bool)> = self
            .project
            .db
            .props
            .iter()
            .filter(|d| f.is_empty() || d.id.to_lowercase().contains(&f) || d.name.to_lowercase().contains(&f))
            .filter(|d| self.decor.mode != DecorMode::Line || !d.pieces.is_empty() || d.snap > 0)
            .map(|d| (d.id.clone(), d.name.clone(), d.sprite.clone(), !d.pieces.is_empty()))
            .collect();
        if self.decor.mode == DecorMode::Line && props.is_empty() {
            ui.weak("No wall sets yet: give a prop pieces (Database › Props › Wall pieces) or a grid to snap to.");
        }
        let mut chosen = None;
        egui::ScrollArea::vertical().id_salt("decor_props").max_height(360.0).auto_shrink([false, true]).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (id, name, sprite, set) in &props {
                    let on = *id == self.decor.prop;
                    let tip = format!("{name} ({id}){}", if *set { " · wall set" } else { "" });
                    if widgets::sprite_thumb(ui, &self.tex, &self.assets, sprite, 40.0, on).on_hover_text(tip).clicked() {
                        chosen = Some(id.clone());
                    }
                }
            });
        });
        if let Some(c) = chosen {
            // A prop that lies flat goes on the Flat layer.
            if self.project.db.prop(&c).is_some_and(|d| d.flat) {
                self.decor.layer = DecorLayer::Flat;
            }
            self.decor.prop = c;
        }
        let n = self.doc.as_ref().map_or(0, |d| d.map.decor.len());
        ui.weak(format!("This map: {n} decorations."));
    }

    /// For the editor's `status`: count:mode:prop|set:layer.
    pub(crate) fn decor_status(&self) -> String {
        let what = if self.decor.mode == DecorMode::Scatter { &self.decor.scatter } else { &self.decor.prop };
        format!(
            "{}:{}:{}:{}",
            self.doc.as_ref().map_or(0, |d| d.map.decor.len()),
            self.decor.mode.label().to_lowercase(),
            if what.is_empty() { "-" } else { what },
            self.decor.layer.label().to_lowercase()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustpg2d_engine::defs::{Database, PropDef};

    #[test]
    fn wall_lines_join_their_pieces() {
        // A wall set: the plain piece runs east-west; the mask picks the others.
        let mut set = PropDef { snap: 32, ..PropDef::fresh("wall") };
        set.pieces = vec![String::new(); 16];
        set.pieces[decor::NORTH | decor::SOUTH] = "wall_v".into();
        set.pieces[decor::EAST | decor::SOUTH] = "wall_corner".into();
        let db = Database { props: vec![set, PropDef::fresh("wall_v"), PropDef::fresh("wall_corner")], ..Default::default() };
        let mut map = Map::new("m", 20, 20);
        // East along row 2, then down from its west end: the corner joins them.
        let (added, removed) = wall_line(&mut map, &db, "wall", vec2(70.0, 70.0), vec2(200.0, 75.0), DecorLayer::Sorted);
        assert_eq!(added.len(), 5);
        assert!(removed.is_empty());
        let (added, removed) = wall_line(&mut map, &db, "wall", vec2(70.0, 70.0), vec2(72.0, 200.0), DecorLayer::Sorted);
        assert_eq!(added.len(), 4 + 1, "four new pieces and the corner");
        assert_eq!(removed.len(), 1, "the old end piece became the corner");
        let at = |x: i32, y: i32| map.decor.iter().find(|d| d.x == x && d.y == y).map(|d| map.decor_defs[d.def as usize].clone());
        assert_eq!(at(80, 96).as_deref(), Some("wall_corner"));
        assert_eq!(at(80, 160).as_deref(), Some("wall_v"));
        assert_eq!(at(144, 96).as_deref(), Some("wall"));
        assert_eq!(map.decor.len(), 9);
    }
}
