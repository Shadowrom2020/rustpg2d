//! A map's events graph in a tab of its own: who has events (the map, zones, placed things),
//! their events, the script functions they run and the maps they lead to, as cards in four
//! columns with lines between them. The logic is `editor_core::graph`; nothing here edits:
//! a click leads to the place where the thing is edited (the map with it selected, the script
//! at the function). Where the cards lie lasts for the session.

use std::collections::{HashMap, HashSet};

use eframe::egui::{self, pos2, vec2, Color32, Pos2, Rect, Sense, Stroke};
use editor_core::graph::{EdgeKind, Graph, NodeKey, Owner};
use editor_core::search::Target;
use editor_core::Workspace;

/// A card's width, by column: who, events, functions, maps.
const WIDTHS: [f32; 4] = [210.0, 380.0, 330.0, 190.0];
/// Where each column starts.
const XS: [f32; 4] = [0.0, 260.0, 720.0, 1110.0];
const GAP: f32 = 18.0;
/// Lines a card shows before "… and N more".
const MAX_LINES: usize = 8;

/// Text in the scene kept sharp below 100 %: the scene scales what is drawn in it, and a glyph
/// scaled down can fall between the screen's pixels (`_` in `cart_bandit_1` was lost below
/// 80 %). So the text is laid out at the size it has on the screen, in whole pixels, scaled back
/// into the scene, and put on a whole pixel: the screen shows the font as it was made.
struct Crisp {
    /// The scene's transform (scene points to the window's).
    t: egui::emath::TSTransform,
    ppp: f32,
}

impl Crisp {
    fn of(ui: &egui::Ui) -> Crisp {
        Crisp { t: ui.ctx().layer_transform_to_global(ui.layer_id()).unwrap_or_default(), ppp: ui.ctx().pixels_per_point() }
    }

    /// Only below 100 %: above it the scene's scaling only magnifies.
    fn on(&self) -> bool {
        self.t.scaling < 0.999
    }

    /// `text` of `size` (scene points) laid out as it is on the screen: on one line cut to
    /// `width` (scene points), or wrapped at it.
    fn galley(&self, ui: &egui::Ui, text: &str, size: f32, color: Color32, width: f32, wrap: bool) -> std::sync::Arc<egui::Galley> {
        let px = (size * self.t.scaling * self.ppp).round().max(1.0);
        let font = egui::FontId::proportional(px / self.ppp);
        let mut job = egui::text::LayoutJob::simple(text.to_string(), font, color, f32::INFINITY);
        job.wrap = if wrap { egui::text::TextWrapping::wrap_at_width(width * self.t.scaling) } else { egui::text::TextWrapping::truncate_at_width(width * self.t.scaling) };
        ui.painter().layout_job(job)
    }

    /// A galley of `galley()` drawn with its top-left corner at `at` (scene points), moved to the
    /// screen's nearest whole pixel. Returns its size in the scene.
    fn paint(&self, ui: &egui::Ui, at: Pos2, galley: std::sync::Arc<egui::Galley>) -> egui::Vec2 {
        let size = galley.size() / self.t.scaling;
        let screen = self.t * at;
        let snapped = pos2((screen.x * self.ppp).round() / self.ppp, (screen.y * self.ppp).round() / self.ppp);
        let mut shape = egui::epaint::TextShape::new(Pos2::ZERO, galley, Color32::WHITE);
        shape.transform(egui::emath::TSTransform::from_scaling(1.0 / self.t.scaling));
        shape.pos = self.t.inverse() * snapped;
        ui.painter().add(shape);
        size
    }

    /// A label of the card: sharp below 100 %, an ordinary label at or above it.
    fn label(&self, ui: &mut egui::Ui, text: &str, size: f32, color: Color32, wrap: bool) -> egui::Response {
        if !self.on() {
            let label = egui::Label::new(egui::RichText::new(text).size(size).color(color));
            return ui.add(if wrap { label.wrap() } else { label.truncate() });
        }
        let galley = self.galley(ui, text, size, color, ui.available_width(), wrap);
        let (rect, r) = ui.allocate_exact_size(galley.size() / self.t.scaling, Sense::hover());
        if ui.is_rect_visible(rect) {
            self.paint(ui, rect.min, galley);
        }
        r
    }
}

pub fn edge_color(kind: EdgeKind) -> Color32 {
    match kind {
        EdgeKind::Owns => Color32::from_gray(120),
        EdgeKind::Calls => Color32::from_rgb(110, 210, 120),
        EdgeKind::Event => Color32::from_rgb(255, 170, 60),
        EdgeKind::State => Color32::from_rgb(90, 200, 240),
        EdgeKind::Travel => Color32::from_rgb(200, 130, 255),
    }
}

#[derive(Default)]
pub struct GraphView {
    pub graph: Graph,
    /// What the graph was made of: made again when it changes.
    key: String,
    /// Where each card lies (its top-left corner); cards dragged keep their place.
    pos: HashMap<NodeKey, Pos2>,
    moved: HashSet<NodeKey>,
    /// Last frame's cards and the height of each line an edge can start at.
    rects: HashMap<NodeKey, Rect>,
    rows: HashMap<(NodeKey, usize), f32>,
    /// What of the canvas is in view (`None`: not looked at yet).
    scene_rect: Option<Rect>,
    /// The canvas's size in the window, in points (last frame).
    shown: Option<egui::Vec2>,
    /// Where the canvas is in the window (last frame).
    screen_rect: Option<Rect>,
    /// The card under the pointer last frame: its lines are lit, the others dimmed.
    hover: Option<NodeKey>,
    /// Only what the thing selected on the map leads to.
    pub only_selected: bool,
    /// Every script function of the project (else those the map reaches).
    pub all_fns: bool,
    /// Cards that don't have this are dimmed.
    pub search: String,
    /// The owner the graph is focused on (with `only_selected`).
    pub focus: Option<Owner>,
}

/// Where a click on a card leads.
pub fn target_of(graph: &Graph, key: &NodeKey) -> Option<Target> {
    let owner = |o: &Owner| match o {
        Owner::Map => Target::Map(graph.map.clone()),
        Owner::Zone(uid) => Target::Zone(graph.map.clone(), *uid),
        Owner::Entity(uid) => Target::Entity(graph.map.clone(), *uid),
    };
    Some(match key {
        NodeKey::Owner(o) | NodeKey::Trigger { owner: o, .. } => owner(o),
        NodeKey::Func(f) => {
            let (file, at) = graph.places.get(f)?;
            Target::Script(file.clone(), *at)
        }
        NodeKey::Map(m) => Target::Map(m.clone()),
    })
}

/// Places of the cards that weren't dragged: owners with their events beside them, functions
/// and maps level with what leads to them (0.35's layout).
fn layout(graph: &Graph, size: &HashMap<NodeKey, Rect>, kept: &HashMap<NodeKey, Pos2>) -> HashMap<NodeKey, Pos2> {
    let nodes = &graph.nodes;
    let edges = &graph.edges;
    let h = |k: &NodeKey| size.get(k).map_or(90.0, |r| r.height());
    let mut pos: HashMap<NodeKey, Pos2> = HashMap::new();
    let mut y = 0.0f32;
    for n in nodes.iter().filter(|n| n.column() == 0) {
        let NodeKey::Owner(o) = n else { continue };
        let start = y;
        let mut ty = y;
        for t in nodes.iter().filter(|t| matches!(t, NodeKey::Trigger { owner, .. } if owner == o)) {
            pos.insert(t.clone(), pos2(XS[1], ty));
            ty += h(t) + GAP;
        }
        pos.insert(n.clone(), pos2(XS[0], start));
        y = (start + h(n) + GAP).max(ty) + GAP;
    }
    for t in nodes.iter().filter(|t| t.column() == 1 && !pos.contains_key(*t)).collect::<Vec<_>>() {
        pos.insert(t.clone(), pos2(XS[1], y));
        y += h(t) + GAP;
    }
    // Functions and maps want to sit level with what leads to them; a function called by
    // another just below its caller.
    fn want(n: &NodeKey, edges: &[editor_core::graph::Edge], placed: &HashMap<NodeKey, Pos2>, memo: &mut HashMap<NodeKey, Option<f32>>, depth: usize) -> Option<f32> {
        if let Some(w) = memo.get(n) {
            return *w;
        }
        if depth > 32 {
            return None;
        }
        memo.insert(n.clone(), None);
        let ys: Vec<f32> = edges
            .iter()
            .filter(|e| e.to == *n && e.from != *n && e.kind != EdgeKind::Owns)
            .filter_map(|e| match placed.get(&e.from) {
                Some(p) if e.from.column() < 2 => Some(p.y),
                _ => want(&e.from, edges, placed, memo, depth + 1).map(|y| y + 1.0),
            })
            .collect();
        let w = (!ys.is_empty()).then(|| ys.iter().sum::<f32>() / ys.len() as f32);
        memo.insert(n.clone(), w);
        w
    }
    let mut placed = pos.clone();
    placed.extend(kept.iter().map(|(k, p)| (k.clone(), *p)));
    let mut memo = HashMap::new();
    for col in [2, 3] {
        let mut list: Vec<(f32, &NodeKey)> = nodes.iter().filter(|n| n.column() == col).map(|n| (want(n, edges, &placed, &mut memo, 0).unwrap_or(f32::MAX), n)).collect();
        list.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut bottom = f32::MIN;
        for (want, n) in list {
            let want = if want == f32::MAX { bottom.max(0.0) } else { want };
            let y = want.max(bottom + GAP);
            pos.insert(n.clone(), pos2(XS[col], y));
            bottom = y + h(n);
        }
    }
    pos
}

impl GraphView {
    /// Makes the graph again when the map, the definitions, the scripts or the choices changed.
    pub fn refresh(&mut self, ws: &Workspace, map: &str, selected: Option<Owner>) {
        self.focus = if self.only_selected { selected } else { None };
        let key = format!("{map}|{}|{}|{}|{}|{:?}", ws.timeline.status(), ws.is_dirty(), ws.scripts_rev, self.all_fns, self.focus);
        if key == self.key {
            return;
        }
        self.key = key;
        match ws.events_graph(map, self.all_fns, self.focus) {
            Ok(g) => self.graph = g,
            Err(e) => {
                self.graph = Graph { map: map.to_string(), ..Default::default() };
                eprintln!("[editor] the events graph: {e}");
            }
        }
        self.pos.retain(|k, _| self.graph.nodes.contains(k));
        self.moved.retain(|k| self.graph.nodes.contains(k));
    }

    /// Where a point of the canvas is in the window (tests click there: what the canvas holds
    /// says where it lies on the canvas).
    #[cfg(test)]
    pub fn on_screen(&self, p: Pos2) -> Pos2 {
        let (Some(scene), Some(screen)) = (self.scene_rect.filter(|r| r.width() > 0.0), self.screen_rect) else { return p };
        let zoom = screen.width() / scene.width();
        screen.min + (p - scene.min) * zoom
    }

    /// `MAP nodes=N edges=N focus=-|entity 12` for scripts.
    pub fn status(&self) -> String {
        let edges = self.graph.edges.iter().filter(|e| e.kind != EdgeKind::Owns).count();
        format!("{} nodes={} edges={edges} focus={} functions={} search={}", self.graph.map, self.graph.nodes.len(), self.focus.map_or("-".to_string(), |o| o.name()), if self.all_fns { "all" } else { "map" }, if self.search.is_empty() { "-" } else { &self.search })
    }

    /// Whether a card has what is searched for.
    fn matches(&self, key: &NodeKey) -> bool {
        if self.search.trim().is_empty() {
            return true;
        }
        let s = self.search.trim().to_lowercase();
        self.graph.cards.get(key).is_some_and(|c| c.title.to_lowercase().contains(&s) || c.kind.to_lowercase().contains(&s) || c.lines.iter().any(|l| l.to_lowercase().contains(&s)))
    }

    /// The tab. `selected`: what is selected on the map (for *Only what is selected*). Returns
    /// where a click leads.
    pub fn show(&mut self, ui: &mut egui::Ui, ws: &Workspace, map: &str, selected: Option<Owner>) -> Option<Target> {
        self.refresh(ws, map, selected);
        let mut go = None;
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(format!("Events of {map}")).strong());
            ui.separator();
            if ui.selectable_label(!self.only_selected, "Everything on the map").on_hover_text("Every event of the map, its zones and what is placed on it").clicked() {
                self.only_selected = false;
            }
            if ui.selectable_label(self.only_selected, "Only what is selected").on_hover_text("What the events of the thing selected on the map lead to").clicked() {
                self.only_selected = true;
            }
            ui.separator();
            ui.weak("Functions:");
            if ui.selectable_label(!self.all_fns, "those the map reaches").clicked() {
                self.all_fns = false;
            }
            if ui.selectable_label(self.all_fns, "all").on_hover_text("Also the script functions nothing on this map runs").clicked() {
                self.all_fns = true;
            }
            ui.separator();
            let find = ui.label("Find");
            ui.add(egui::TextEdit::singleline(&mut self.search).hint_text("a name, a flag, a word").desired_width(150.0)).labelled_by(find.id);
            if ui.button("Arrange").on_hover_text("Lays the cards out again (forgets where you dragged them)").clicked() {
                self.moved.clear();
                self.pos.clear();
                self.scene_rect = None;
            }
        });
        ui.horizontal_wrapped(|ui| {
            for kind in EdgeKind::ALL {
                ui.colored_label(edge_color(kind), format!("-- {}", kind.label()));
            }
            ui.weak("Click a card to go where it is edited; drag one to move it; drag the background or turn the wheel to move the view, Ctrl+wheel zooms.");
        });
        if self.only_selected && self.focus.is_none() {
            ui.weak("Nothing is selected on the map: everything is shown. Select a character, a thing or a zone in the map's tab.");
        }
        if self.graph.nodes.is_empty() {
            ui.add_space(12.0);
            ui.label("Nothing on this map has events yet. Select a character or a thing on the map and give it an event in the Inspector (Add an event); the map's own events are under its settings.");
            return None;
        }
        ui.separator();
        // (Laid out with last frame's sizes; cards not drawn yet count as 90 points high.)
        let kept: HashMap<NodeKey, Pos2> = self.pos.iter().filter(|(k, _)| self.moved.contains(*k)).map(|(k, p)| (k.clone(), *p)).collect();
        let auto = layout(&self.graph, &self.rects, &kept);
        for (k, p) in auto {
            if !self.moved.contains(&k) {
                self.pos.insert(k, p);
            }
        }
        self.shown = Some(ui.available_size());
        self.screen_rect = Some(ui.available_rect_before_wrap());
        // The first look: from the top left, as wide as the columns.
        let mut scene_rect = self.scene_rect.unwrap_or_else(|| {
            // (At 100 %: a smaller zoom loses thin strokes of small text, `_` and `i`.)
            Rect::from_min_size(pos2(-20.0, -20.0), ui.available_size().max(vec2(100.0, 100.0)))
        });
        let mut drags: Vec<(NodeKey, egui::Vec2)> = Vec::new();
        let mut hovered: Option<NodeKey> = None;
        let mut new_rects = HashMap::new();
        let mut new_rows = HashMap::new();
        let st = &*self;
        egui::Scene::new().zoom_range(0.15..=2.0).max_inner_size(vec2(30000.0, 30000.0)).show(ui, &mut scene_rect, |ui| {
            let crisp = Crisp::of(ui);
            let small = ui.style().text_styles.get(&egui::TextStyle::Small).map_or(10.0, |f| f.size);
            let body = ui.style().text_styles.get(&egui::TextStyle::Body).map_or(14.0, |f| f.size);
            // Lines first (under the cards), from last frame's cards.
            let hover = &st.hover;
            for e in &st.graph.edges {
                let (Some(a), Some(b)) = (st.rects.get(&e.from), st.rects.get(&e.to)) else { continue };
                let y0 = e.row.and_then(|r| st.rows.get(&(e.from.clone(), r))).copied().unwrap_or(a.center().y);
                let p0 = pos2(a.right(), y0.clamp(a.top() + 6.0, a.bottom() - 6.0));
                let p3 = pos2(b.left(), (b.top() + 16.0).min(b.bottom()));
                // In the same column: a loop out to the right; backwards: from the left side.
                let same_col = (a.left() - b.left()).abs() < 40.0;
                let (p0, p3) = if same_col {
                    (p0, pos2(b.right(), p3.y))
                } else if b.left() < a.right() {
                    (pos2(a.left(), p0.y), pos2(b.right(), p3.y))
                } else {
                    (p0, p3)
                };
                let dx = ((p3.x - p0.x).abs() * 0.5).max(40.0) * if p3.x >= p0.x { 1.0 } else { -1.0 };
                let (c1, c2) = if same_col { (p0 + vec2(70.0, 0.0), p3 + vec2(70.0, 0.0)) } else { (p0 + vec2(dx, 0.0), p3 - vec2(dx, 0.0)) };
                let lit = hover.as_ref().is_none_or(|h| *h == e.from || *h == e.to) && st.matches(&e.from) && st.matches(&e.to);
                let col = if lit { edge_color(e.kind) } else { edge_color(e.kind).gamma_multiply(0.2) };
                let width = if e.kind == EdgeKind::Owns { 1.5 } else { 2.5 };
                ui.painter().add(egui::epaint::CubicBezierShape::from_points_stroke([p0, c1, c2, p3], false, Color32::TRANSPARENT, Stroke::new(width, col)));
                ui.painter().circle_filled(p3, 3.5, col);
                if !e.label.is_empty() && lit {
                    let mid = if same_col { pos2(p0.x.max(p3.x) + 56.0, (p0.y + p3.y) / 2.0) } else { pos2((p0.x + p3.x) / 2.0, (p0.y + p3.y) / 2.0) };
                    if crisp.on() {
                        let g = crisp.galley(ui, &e.label, 12.0, col, f32::INFINITY, false);
                        let size = g.size() / crisp.t.scaling;
                        crisp.paint(ui, mid - vec2(size.x / 2.0, 6.0 + size.y), g);
                    } else {
                        ui.painter().text(mid - vec2(0.0, 6.0), egui::Align2::CENTER_BOTTOM, &e.label, egui::FontId::proportional(12.0), col);
                    }
                }
            }
            // The cards.
            for key in &st.graph.nodes {
                let Some(card) = st.graph.cards.get(key) else { continue };
                let pos = st.pos.get(key).copied().unwrap_or(Pos2::ZERO);
                let w = WIDTHS[key.column()];
                let accent = match key {
                    NodeKey::Owner(_) => Color32::from_rgb(140, 170, 230),
                    NodeKey::Trigger { def: true, .. } => Color32::from_gray(160),
                    NodeKey::Trigger { .. } => Color32::from_rgb(210, 210, 140),
                    NodeKey::Func(_) => edge_color(EdgeKind::Calls),
                    NodeKey::Map(_) => edge_color(EdgeKind::Travel),
                };
                let dim = !st.matches(key);
                let fill = if dim { Color32::from_gray(28) } else { Color32::from_rgb(38, 42, 50) };
                let hot = hover.as_ref() == Some(key);
                let stroke = Stroke::new(if hot { 2.0 } else { 1.0 }, if dim { accent.gamma_multiply(0.3) } else { accent });
                let id = egui::Id::unique(("events graph card", &st.graph.map, key));
                let text = |c: Color32| if dim { c.gamma_multiply(0.35) } else { c };
                let mut lines_y = Vec::new();
                let inner = ui.scope_builder(egui::UiBuilder::new().max_rect(Rect::from_min_size(pos, vec2(w, 4000.0))).id_salt(id), |ui| {
                    egui::Frame::new().fill(fill).stroke(stroke).corner_radius(6.0).inner_margin(8.0).show(ui, |ui| {
                        ui.set_width(w - 16.0);
                        let head = ui.vertical(|ui| {
                            crisp.label(ui, &card.kind, small, text(Color32::from_gray(150)), true);
                            crisp.label(ui, &card.title, body, text(accent), true);
                        });
                        for (k, line) in card.lines.iter().take(MAX_LINES).enumerate() {
                            let r = crisp.label(ui, line, 12.5, text(Color32::from_gray(210)), false);
                            lines_y.push((k, r.rect.center().y));
                        }
                        if card.lines.len() > MAX_LINES {
                            let r = crisp.label(ui, &format!("… and {} more", card.lines.len() - MAX_LINES), small, text(Color32::from_gray(150)), true);
                            // (Lines that aren't shown start at "… and N more".)
                            for k in MAX_LINES..card.lines.len() {
                                lines_y.push((k, r.rect.center().y));
                            }
                        }
                        head.response.rect
                    })
                });
                let rect = inner.inner.response.rect;
                new_rects.insert(key.clone(), rect);
                for (k, y) in lines_y {
                    if k >= card.first_row {
                        new_rows.insert((key.clone(), k - card.first_row), y);
                    }
                }
                // The heading moves the card; a click anywhere on it goes where it is edited.
                let r = ui.interact(rect, id.with("card"), Sense::click_and_drag());
                r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, format!("{}: {}", card.kind, card.title)));
                if r.dragged() {
                    drags.push((key.clone(), r.drag_delta()));
                }
                if r.clicked() {
                    go = target_of(&st.graph, key);
                }
                if r.hovered() {
                    hovered = Some(key.clone());
                }
                r.on_hover_cursor(egui::CursorIcon::PointingHand);
            }
        });
        self.hover = hovered;
        self.scene_rect = Some(scene_rect);
        // (Cards laid out with sizes they turned out not to have: again, with the right ones.)
        if new_rects.iter().any(|(k, r)| self.rects.get(k).is_none_or(|old| (old.size() - r.size()).length() > 0.5)) {
            ui.ctx().request_repaint();
        }
        self.rects = new_rects;
        self.rows = new_rows;
        for (k, d) in drags {
            if let Some(p) = self.pos.get_mut(&k) {
                *p += d;
                self.moved.insert(k);
            }
        }
        go
    }

    /// `graph …`: `dump` = the graph as text, `focus entity UID|zone UID|-`, `functions all|map`,
    /// `find TEXT|-`, `click NODE` (a node by its name in the dump: where it leads). Returns the
    /// lines to print and where a click leads.
    pub fn word(&mut self, ws: &Workspace, map: &str, selected: Option<Owner>, args: &[String]) -> (Vec<String>, Option<Target>) {
        let arg = |i: usize| args.get(i).map(String::as_str).unwrap_or("");
        let mut go = None;
        match arg(0) {
            "focus" => self.only_selected = arg(1) != "-",
            "functions" => self.all_fns = arg(1) == "all",
            "find" => self.search = if arg(1) == "-" { String::new() } else { args[1..].join(" ") },
            // (What is in view: its top-left corner on the canvas and the zoom, for screenshots.)
            "view" => {
                let (x, y, zoom) = (arg(1).parse().unwrap_or(0.0), arg(2).parse().unwrap_or(0.0), arg(3).parse::<f32>().unwrap_or(1.0).max(0.1));
                let shown = self.shown.unwrap_or(vec2(1200.0, 500.0)) / zoom;
                self.scene_rect = Some(Rect::from_min_size(pos2(x, y), shown));
            }
            "click" => {
                self.refresh(ws, map, selected);
                let name = args[1..].join(" ");
                match self.graph.nodes.iter().find(|n| n.name() == name) {
                    Some(k) => go = target_of(&self.graph, k),
                    None => return (vec![format!("failed: no node {name}")], None),
                }
            }
            _ => {}
        }
        self.refresh(ws, map, selected);
        let mut out = vec![self.status()];
        if arg(0) == "dump" {
            out.extend(self.graph.dump());
        }
        (out, go)
    }
}

#[cfg(test)]
mod ui_tests {
    use super::*;
    use egui_kittest::kittest::Queryable;
    use egui_kittest::Harness;
    use rustpg2d_engine::entity::{EntityInstance, EntityKind};
    use rustpg2d_engine::events::{Action, Trigger, TriggerOn};

    struct State {
        ws: Workspace,
        view: GraphView,
        went: Option<Target>,
    }

    #[test]
    fn the_cards_are_drawn_and_a_click_goes_where_the_thing_is_edited() {
        let dir = std::env::temp_dir().join(format!("rustpg2d-graphview-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        editor_core::files::create_project(&dir).unwrap();
        // A script function the guard's event runs, which raises an event the map listens for.
        std::fs::write(dir.join("scripts/main.rhai"), "fn guard_talk() {\n    say(\"Guard\", \"Halt!\");\n    emit(\"alarm\");\n}\n").unwrap();
        let mut ws = Workspace::new(dir.clone(), editor_core::Gpu::No).unwrap();
        ws.open_map("start");
        let map = &mut ws.doc.as_mut().unwrap().map;
        let uid = map.alloc_uid();
        let mut guard = EntityInstance::new(uid, EntityKind::Character, "guard", 64.0, 64.0);
        guard.triggers = vec![Trigger { on: TriggerOn::Interact, conditions: vec![], actions: vec![Action::RunScript { function: "guard_talk".into() }], once: false }];
        map.entities.push(guard);
        map.triggers = vec![Trigger { on: TriggerOn::Custom { name: "alarm".into() }, conditions: vec![], actions: vec![Action::Teleport { map: "prison".into(), spawn: String::new() }], once: false }];
        let mut h = Harness::builder().with_size(egui::vec2(1500.0, 900.0)).build_ui_state(
            |ui, s: &mut State| {
                if let Some(t) = s.view.show(ui, &s.ws, "start", None) {
                    s.went = Some(t);
                }
            },
            State { ws, view: GraphView::default(), went: None },
        );
        h.run();
        // Who, the events, the function, the map it leads to: a card each, by what it says.
        h.get_by_label("Character: guard");
        h.get_by_label("Event: When the player uses it or talks to it");
        h.get_by_label("Event: When the event alarm is raised");
        h.get_by_label("Script function: guard_talk()");
        h.get_by_label("Map: prison");
        assert_eq!(h.state().view.status(), "start nodes=6 edges=3 focus=- functions=map search=-");
        // The guard's card: the map with the guard selected; the function's: its script.
        // (A card's place is on the canvas: clicked where that is in the window.)
        let click = |h: &mut Harness<State>, label: &str| {
            let at = h.state().view.on_screen(h.get_by_label(label).rect().center());
            h.hover_at(at);
            for pressed in [true, false] {
                h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
                h.run();
            }
            h.state_mut().went.take()
        };
        assert_eq!(click(&mut h, "Character: guard"), Some(Target::Entity("start".into(), uid)));
        assert_eq!(click(&mut h, "Script function: guard_talk()"), Some(Target::Script(dir.join("scripts/main.rhai"), 0)));
        assert_eq!(click(&mut h, "Map: prison"), Some(Target::Map("prison".into())));
        assert_eq!(click(&mut h, "Event: When the event alarm is raised"), Some(Target::Map("start".into())));
        // Every line from a card to a card; the guard's event's line starts at its first action.
        let v = &h.state().view;
        assert!(v.graph.edges.iter().all(|e| v.rects.contains_key(&e.from) && v.rects.contains_key(&e.to)));
        let ev = NodeKey::Trigger { owner: Owner::Entity(uid), def: false, index: 0 };
        assert!(v.rows.contains_key(&(ev.clone(), 0)), "{:?}", v.rows.keys().collect::<Vec<_>>());
        // No card lies on another.
        let rects: Vec<Rect> = v.rects.values().copied().collect();
        for (i, a) in rects.iter().enumerate() {
            assert!(rects[i + 1..].iter().all(|b| !a.intersects(*b)), "cards overlap");
        }
        let _ = std::fs::remove_dir_all(dir);
    }
}
