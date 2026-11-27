//! A visual script as nodes on a canvas, in its tab: drag the background to move the view,
//! Ctrl+wheel to zoom, a node's title to move it, from a pin to another to link them (a link
//! let go in empty space opens the palette for a node that takes it); right-click or Space
//! opens the palette; Delete removes the selected nodes. Beside it what the selected node
//! does, what is wrong, and the Rhai the game runs.
//!
//! The thinking is `editor_core::visualedit` and the engine's `visual.rs`; this is the canvas.

use std::collections::{BTreeSet, HashMap};

use eframe::egui::{self, pos2, vec2, Color32, Pos2, Rect, Sense, Stroke};
use editor_core::visualedit::{self, Entry, LinkDrag};
use editor_core::Workspace;
use rustpg2d_engine::script_api::{Arg, API};
use rustpg2d_engine::visual::{self, Catalog, CmpOp, Link, LogicOp, MathOp, Node, NodeKind, PinType, Value, VisualScript};

/// A node's width on the canvas.
const NODE_W: f32 = 230.0;

/// The palette of nodes to add.
#[derive(Clone, Debug, Default)]
struct Palette {
    /// Where the new node goes (canvas coordinates).
    at: Pos2,
    search: String,
    /// A link dragged into empty space: the new node gets it.
    connect: Option<LinkDrag>,
    focused: bool,
    /// Where its window is (set when it opens: beside the pointer, inside the window).
    window: Option<Pos2>,
}

/// What the canvas asks of the window.
pub enum Ask {
    /// Show this visual script (a node that calls it was opened).
    Open(String),
}

pub struct NodesView {
    /// The part of the canvas in view, and where on the screen that is shown.
    scene_rect: Rect,
    screen_rect: Rect,
    pub selected: BTreeSet<u32>,
    drag: Option<LinkDrag>,
    /// Where the pointer last was on the canvas (a link is let go there, also when the pointer left with it).
    pointer: Option<Pos2>,
    palette: Option<Palette>,
    /// Pins of the last frame: (node, pin, output) -> canvas position.
    pins: HashMap<(u32, String, bool), Pos2>,
    /// The functions of the project's text scripts, read again once a second.
    text_fns: Vec<(String, usize)>,
    text_fns_at: f64,
    /// Nodes copied, with the links between them.
    clip: Option<(Vec<Node>, Vec<Link>)>,
    /// How many times nodes were dragged: each drag is a step of the history of its own.
    moves: u32,
}

impl Default for NodesView {
    fn default() -> NodesView {
        NodesView { scene_rect: Rect::ZERO, screen_rect: Rect::ZERO, selected: BTreeSet::new(), drag: None, pointer: None, palette: None, pins: HashMap::new(), text_fns: Vec::new(), text_fns_at: -10.0, clip: None, moves: 0 }
    }
}

fn pin_color(k: PinType) -> Color32 {
    match k {
        PinType::Flow => Color32::from_gray(235),
        PinType::Num => Color32::from_rgb(110, 200, 120),
        PinType::Bool => Color32::from_rgb(220, 90, 90),
        PinType::Text => Color32::from_rgb(230, 120, 220),
        PinType::List => Color32::from_rgb(110, 170, 240),
        PinType::Map => Color32::from_rgb(240, 190, 90),
        PinType::Any => Color32::from_gray(160),
    }
}

/// A node's title colour by what it is.
fn accent(n: &Node, cat: &Catalog, vs: &VisualScript) -> Color32 {
    match &n.kind {
        NodeKind::Start | NodeKind::Return => Color32::from_rgb(120, 200, 120),
        NodeKind::If | NodeKind::Sequence { .. } | NodeKind::Repeat | NodeKind::ForEach => Color32::from_rgb(190, 150, 240),
        NodeKind::Call { .. } if cat.pins(vs, n).inputs.iter().any(|p| p.kind == PinType::Flow) => Color32::from_rgb(120, 170, 240),
        NodeKind::Call { .. } => Color32::from_rgb(90, 200, 200),
        NodeKind::Comment { .. } => Color32::from_rgb(230, 210, 120),
        _ => Color32::from_gray(190),
    }
}

/// A pin: a triangle for flow, a circle for values (filled when linked).
fn pin_shape(p: &egui::Painter, at: Pos2, kind: PinType, linked: bool) {
    let c = pin_color(kind);
    if kind == PinType::Flow {
        let pts = vec![at + vec2(-5.0, -6.0), at + vec2(6.0, 0.0), at + vec2(-5.0, 6.0)];
        if linked {
            p.add(egui::Shape::convex_polygon(pts, c, Stroke::NONE));
        } else {
            p.add(egui::Shape::closed_line(pts, Stroke::new(1.5_f32, c)));
        }
    } else if linked {
        p.circle_filled(at, 5.0, c);
    } else {
        p.circle(at, 5.0, Color32::from_rgb(30, 32, 38), Stroke::new(1.5_f32, c));
    }
}

fn bezier(p: &egui::Painter, a: Pos2, b: Pos2, c: Color32, w: f32) {
    let dx = ((b.x - a.x).abs() * 0.5).max(40.0);
    p.add(egui::epaint::CubicBezierShape::from_points_stroke([a, a + vec2(dx, 0.0), b - vec2(dx, 0.0), b], false, Color32::TRANSPARENT, Stroke::new(w, c)));
}

/// The field for a value typed into an input of type `kind` (`opts`: the ids it may take).
fn value_field(ui: &mut egui::Ui, id: egui::Id, name: &str, kind: PinType, arg: Option<Arg>, value: &mut Option<Value>, opts: &[String]) {
    match kind {
        PinType::Num => {
            let mut n = match value {
                Some(Value::Num(n)) => *n,
                _ => 0.0,
            };
            // (Untouched, it stays unset: the call leaves an optional last input out.)
            let r = ui.add(egui::DragValue::new(&mut n).speed(0.1));
            r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::SpinButton, true, name));
            if r.changed() {
                *value = Some(Value::Num(n));
            }
        }
        PinType::Bool => {
            let mut b = matches!(value, Some(Value::Bool(true)));
            let r = ui.checkbox(&mut b, "");
            r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::CheckBox, true, name));
            if r.changed() {
                *value = Some(Value::Bool(b));
            }
        }
        PinType::Text => {
            let mut s = visualedit::value_text(value.as_ref());
            let before = s.clone();
            let ids = arg.is_some_and(|a| a.is_id());
            let width = if arg == Some(Arg::Line) { 140.0 } else if ids { 92.0 } else { 110.0 };
            ui.add(egui::TextEdit::singleline(&mut s).desired_width(width).id(id)).widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, name));
            if ids && !opts.is_empty() {
                // What it may name, narrowed by what is typed.
                ui.menu_button("…", |ui| {
                    // (Everything while what is typed is one of them already: the menu then changes it.)
                    let f = if opts.contains(&s) { String::new() } else { s.to_lowercase() };
                    egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                        for o in opts.iter().filter(|o| f.is_empty() || o.to_lowercase().contains(&f)).take(60) {
                            if ui.button(o.as_str()).clicked() {
                                s = o.clone();
                                ui.close();
                            }
                        }
                    });
                });
            }
            if s != before {
                *value = (!s.is_empty()).then_some(Value::Text(s));
            }
        }
        PinType::Any => {
            let mut s = visualedit::value_text(value.as_ref());
            let r = ui.add(egui::TextEdit::singleline(&mut s).desired_width(80.0).id(id).hint_text("value"));
            r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, name));
            if r.changed() {
                *value = visualedit::parse_any(&s);
            }
        }
        _ => {
            ui.weak("(link a value)");
        }
    }
}

/// The widgets in a node's title row: the operator, the steps, the variable's name, the value.
fn header_widgets(ui: &mut egui::Ui, n: &mut Node) {
    let id = egui::Id::unique(("visual script node head", n.id));
    match &mut n.kind {
        NodeKind::Math { op } => {
            egui::ComboBox::from_id_salt(id).width(70.0).selected_text(op.label()).show_ui(ui, |ui| {
                for o in MathOp::ALL {
                    ui.selectable_value(op, o, o.label());
                }
            });
        }
        NodeKind::Compare { op } => {
            egui::ComboBox::from_id_salt(id).width(50.0).selected_text(op.code()).show_ui(ui, |ui| {
                for o in CmpOp::ALL {
                    ui.selectable_value(op, o, o.code());
                }
            });
        }
        NodeKind::Logic { op } => {
            egui::ComboBox::from_id_salt(id).width(60.0).selected_text(if *op == LogicOp::And { "and" } else { "or" }).show_ui(ui, |ui| {
                ui.selectable_value(op, LogicOp::And, "and");
                ui.selectable_value(op, LogicOp::Or, "or");
            });
        }
        NodeKind::Sequence { steps } => {
            ui.add(egui::DragValue::new(steps).range(1..=9).prefix("steps "));
        }
        NodeKind::SetLocal { name } | NodeKind::GetLocal { name } => {
            ui.add(egui::TextEdit::singleline(name).desired_width(80.0).id(id)).widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, "The variable's name"));
        }
        NodeKind::Call { function, arity } if !API.iter().any(|f| f.name == function.as_str()) => {
            ui.add(egui::DragValue::new(arity).range(0..=9).prefix("inputs ")).on_hover_text("How many values a text script function takes");
        }
        NodeKind::Value { kind } => {
            let mut v = n.values.get("value").cloned();
            value_field(ui, id, "The value", *kind, None, &mut v, &[]);
            match v {
                Some(v) => {
                    n.values.insert("value".into(), v);
                }
                None => {
                    n.values.remove("value");
                }
            }
        }
        _ => {}
    }
}

impl NodesView {
    fn refresh_text_fns(&mut self, ws: &Workspace, now: f64) {
        if now - self.text_fns_at > 1.0 {
            self.text_fns = ws.project.text_script_functions();
            self.text_fns_at = now;
        }
    }

    /// Where a place of the canvas is on the screen.
    #[cfg(test)]
    fn on_screen(&self, p: Pos2) -> Pos2 {
        let zoom = self.screen_rect.width() / self.scene_rect.width();
        self.screen_rect.min + (p - self.scene_rect.min) * zoom
    }

    /// Writes a changed visual script back into the definitions (a step of the history).
    fn put(ws: &mut Workspace, vs: VisualScript) -> bool {
        let Some(k) = ws.project.db.visual_scripts.iter().position(|v| v.id == vs.id) else { return false };
        if ws.project.db.visual_scripts[k] == vs {
            return false;
        }
        ws.begin(&format!("Change the visual script {}", vs.label()));
        ws.project.db.visual_scripts[k] = vs;
        ws.stale = true;
        true
    }

    /// `ID:nodes/links:problems[:palette] sel=[…]` for the script's `status`.
    pub fn status(&self, ws: &Workspace, id: &str) -> String {
        let Some(vs) = ws.project.db.visual_scripts.iter().find(|v| v.id == id) else { return "-".into() };
        let cat = Catalog { visual: &ws.project.db.visual_scripts, text_fns: ws.project.text_script_functions() };
        let (_, p) = visual::function_code(vs, &cat);
        format!("{}:{}/{}:{}{} sel={:?}", vs.id, vs.nodes.len(), vs.links.len(), p.len(), if self.palette.is_some() { ":palette" } else { "" }, self.selected)
    }

    /// The script's words for the visual script `id`: `add KIND X Y`, `link FROM OUT TO IN`,
    /// `value NODE INPUT VALUE…`, `select N…`, `delete`, `group`, `copy`, `paste [X Y]`,
    /// `code`. Returns what to print.
    pub fn word(&mut self, ws: &mut Workspace, id: &str, args: &[String]) -> String {
        let arg = |k: usize| args.get(k).map(String::as_str).unwrap_or("");
        let num = |k: usize| arg(k).parse::<f32>().unwrap_or(0.0);
        let all = ws.project.db.visual_scripts.clone();
        let Some(mut vs) = all.iter().find(|v| v.id == id).cloned() else { return format!("there is no visual script {id}") };
        let cat = Catalog { visual: &all, text_fns: ws.project.text_script_functions() };
        match arg(0) {
            "add" => match visualedit::kind_named(arg(1)) {
                Some(kind) => {
                    let n = visualedit::add_node(&mut vs, &cat, kind, [num(2), num(3)], None);
                    self.selected = BTreeSet::from([n]);
                    NodesView::put(ws, vs);
                    return format!("node {n}");
                }
                None => return format!("'{}' is no kind of node", arg(1)),
            },
            "link" => {
                visualedit::connect(&mut vs, Link { from: arg(1).parse().unwrap_or(0), out: arg(2).into(), to: arg(3).parse().unwrap_or(0), input: arg(4).into() }, &cat);
                NodesView::put(ws, vs);
            }
            "value" => {
                visualedit::set_value(&mut vs, &cat, arg(1).parse().unwrap_or(0), arg(2), &args[3.min(args.len())..].join(" "));
                NodesView::put(ws, vs);
            }
            "select" => self.selected = args[1..].iter().filter_map(|a| a.parse().ok()).collect(),
            "delete" => {
                self.delete_selected(&mut vs);
                NodesView::put(ws, vs);
            }
            "group" => {
                let r = ws.group_nodes(id, &self.selected);
                self.selected.clear();
                return r.map_or_else(|e| format!("not grouped: {e}"), |new| format!("grouped into {new}"));
            }
            "copy" => self.clip = visualedit::copy_nodes(&vs, &self.selected),
            "paste" => {
                if let Some(clip) = &self.clip {
                    self.selected = visualedit::paste_nodes(&mut vs, clip, (!arg(1).is_empty()).then(|| [num(1), num(2)]));
                    NodesView::put(ws, vs);
                }
            }
            "code" => return visualedit::code_text(&vs, &cat),
            _ => {}
        }
        self.status(ws, id)
    }

    /// Takes the selected nodes (not Start) and their links out of `vs`.
    fn delete_selected(&mut self, vs: &mut VisualScript) {
        let gone: BTreeSet<u32> = vs.nodes.iter().filter(|n| self.selected.contains(&n.id) && n.kind != NodeKind::Start).map(|n| n.id).collect();
        vs.nodes.retain(|n| !gone.contains(&n.id));
        vs.links.retain(|l| !gone.contains(&l.from) && !gone.contains(&l.to));
        self.selected.clear();
    }

    /// The canvas with its side panel, filling the space left.
    pub fn show(&mut self, ui: &mut egui::Ui, ws: &mut Workspace, id: &str) -> Option<Ask> {
        let now = ui.input(|i| i.time);
        self.refresh_text_fns(ws, now);
        let all = ws.project.db.visual_scripts.clone();
        let Some(k) = all.iter().position(|v| v.id == id) else {
            ui.weak("This visual script is gone.");
            return None;
        };
        let mut vs = all[k].clone();
        let mut ask = None;
        let mut group = false;
        egui::Panel::right(egui::Id::unique(("nodes side", id))).default_size(300.0).resizable(true).show(ui, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| self.side(ui, &all, &mut vs, &mut ask, &mut group));
        });
        self.canvas(ui, ws, &all, &mut vs, &mut ask, &mut group);
        self.palette_window(ui.ctx(), &all, &mut vs);
        if group {
            // (Grouping changes two scripts: what the canvas changed this frame goes first.)
            NodesView::put(ws, vs);
            match ws.group_nodes(id, &self.selected) {
                Ok(new) => ws.set_status(format!("Grouped into the visual script {new} (double-click its node to go into it)")),
                Err(e) => ws.set_status(format!("Can't group: {e}")),
            }
            self.selected.clear();
        } else {
            NodesView::put(ws, vs);
        }
        ask
    }

    /// Beside the canvas: the selected node, the problems, the Rhai.
    fn side(&mut self, ui: &mut egui::Ui, all: &[VisualScript], vs: &mut VisualScript, ask: &mut Option<Ask>, group: &mut bool) {
        let cat = Catalog { visual: all, text_fns: self.text_fns.clone() };
        let (code, problems) = visual::function_code(vs, &cat);
        if !self.selected.is_empty() {
            ui.horizontal_wrapped(|ui| {
                if ui.button("Group into a function").on_hover_text("The selected nodes become a new visual script, called in their place (Ctrl+G): values coming in are its parameters, one value going out its result").clicked() {
                    *group = true;
                }
                if ui.button("Copy").on_hover_text("Ctrl+C; Ctrl+V pastes at the pointer, Ctrl+D duplicates").clicked() {
                    self.clip = visualedit::copy_nodes(vs, &self.selected);
                }
                if ui.button("Delete").on_hover_text("Del").clicked() {
                    self.delete_selected(vs);
                }
            });
        }
        if let Some(n) = self.selected.iter().next().and_then(|id| vs.node(*id)) {
            ui.label(egui::RichText::new(cat.title(n)).heading());
            if let NodeKind::Call { function, .. } = &n.kind {
                if all.iter().any(|v| v.id == *function) && ui.button("Open").on_hover_text("Shows this visual script (or double-click the node's title)").clicked() {
                    *ask = Some(Ask::Open(function.clone()));
                }
                if let Some(f) = API.iter().find(|f| f.name == function.as_str()) {
                    ui.label(f.doc);
                    let params: Vec<String> = f.params.iter().map(|(p, a)| format!("{p}: {a:?}")).collect();
                    ui.weak(format!("{}({})", f.name, params.join(", ")));
                } else if let Some(v) = all.iter().find(|v| v.id == *function) {
                    ui.label(if v.description.is_empty() { "A visual script." } else { v.description.as_str() });
                } else {
                    ui.label("A function of the project's text scripts.");
                }
            }
            let nid = n.id;
            for p in problems.iter().filter(|p| p.node == Some(nid)) {
                ui.colored_label(ui.visuals().error_fg_color, p.text.as_str());
            }
            ui.separator();
        }
        ui.strong(if problems.is_empty() { "No problems".to_string() } else { format!("Problems ({})", problems.len()) });
        let mut goto = None;
        for p in &problems {
            let text = match p.node.and_then(|n| vs.node(n)) {
                Some(n) => format!("{}: {}", cat.title(n), p.text),
                None => p.text.clone(),
            };
            if ui.add(egui::Label::new(egui::RichText::new(text).color(ui.visuals().error_fg_color)).sense(Sense::click())).on_hover_text("Shows the node").clicked() {
                goto = p.node;
            }
        }
        if let Some(node) = goto.and_then(|n| vs.node(n)) {
            self.selected = BTreeSet::from([node.id]);
            self.scene_rect = Rect::from_center_size(pos2(node.pos[0] + NODE_W / 2.0, node.pos[1] + 40.0), self.scene_rect.size());
        }
        ui.separator();
        ui.horizontal(|ui| {
            ui.strong("Rhai");
            if ui.small_button("Copy the code").clicked() {
                ui.ctx().copy_text(code.clone());
            }
        });
        ui.weak("What the game runs (written into scripts/visual_scripts.rhai when you save).");
        egui::Frame::new().fill(Color32::from_rgb(20, 22, 26)).inner_margin(6.0).show(ui, |ui| {
            // (Long lines scroll: the panel keeps its width.)
            egui::ScrollArea::horizontal().id_salt("rhai").show(ui, |ui| {
                ui.add(egui::Label::new(egui::RichText::new(&code).monospace().size(12.0)).wrap_mode(egui::TextWrapMode::Extend));
            });
        });
    }

    fn canvas(&mut self, ui: &mut egui::Ui, ws: &Workspace, all: &[VisualScript], vs: &mut VisualScript, ask: &mut Option<Ask>, group_asked: &mut bool) {
        let k = all.iter().position(|v| v.id == vs.id).unwrap_or(0);
        let text_fns = self.text_fns.clone();
        let cat = Catalog { visual: all, text_fns: text_fns.clone() };
        // The ids each kind of input may take, collected once a frame and kind.
        let mut opts: Vec<(Arg, Vec<String>)> = Vec::new();
        for n in &vs.nodes {
            for p in cat.pins(vs, n).inputs {
                if let Some(a) = p.arg.filter(|a| a.is_id()) {
                    if !opts.iter().any(|o| o.0 == a) {
                        opts.push((a, ws.arg_options(a)));
                    }
                }
            }
        }
        if self.scene_rect == Rect::ZERO {
            // The first look: every node when they stay readable, else from the first ones on.
            let mut r = Rect::NOTHING;
            for n in &vs.nodes {
                r = r.union(Rect::from_min_size(pos2(n.pos[0], n.pos[1]), vec2(NODE_W, 120.0)));
            }
            let r = if r.is_positive() { r.expand(60.0) } else { Rect::from_min_size(Pos2::ZERO, vec2(1200.0, 700.0)) };
            let size = ui.available_size().max(vec2(100.0, 100.0));
            let zoom = (size.x / r.width()).min(size.y / r.height()).clamp(0.7, 1.0);
            let shown = size / zoom;
            let x = if shown.x >= r.width() { r.center().x - shown.x / 2.0 } else { r.left() };
            let y = if shown.y >= r.height() { r.center().y - shown.y / 2.0 } else { r.top() };
            self.scene_rect = Rect::from_min_size(pos2(x, y), shown);
        }
        let mut scene_rect = self.scene_rect;
        let mut new_pins: HashMap<(u32, String, bool), Pos2> = HashMap::new();
        let mut add_link: Option<Link> = None;
        let mut open_palette: Option<(Pos2, Option<LinkDrag>)> = None;
        let mut moved: Vec<(u32, egui::Vec2)> = Vec::new();
        let mut click_node: Option<(u32, bool)> = None;
        let mut pointer_at = None;
        self.screen_rect = ui.available_rect_before_wrap();
        let st = &mut *self;
        let resp = egui::Scene::new().zoom_range(0.2..=2.0).max_inner_size(vec2(20000.0, 20000.0)).show(ui, &mut scene_rect, |ui| {
            let to_scene = ui.ctx().layer_transform_from_global(ui.layer_id());
            let pointer = ui.input(|i| i.pointer.latest_pos()).map(|p| to_scene.map_or(p, |t| t * p));
            pointer_at = pointer;
            if pointer.is_some() {
                st.pointer = pointer;
            }
            // Links (the pins as they were a frame ago), under the nodes.
            for l in &vs.links {
                let (Some(a), Some(b)) = (st.pins.get(&(l.from, l.out.clone(), true)), st.pins.get(&(l.to, l.input.clone(), false))) else { continue };
                let kind = vs.node(l.from).and_then(|n| cat.pins(vs, n).outputs.into_iter().find(|p| p.name == l.out)).map_or(PinType::Any, |p| p.kind);
                bezier(ui.painter(), *a, *b, pin_color(kind), if kind == PinType::Flow { 3.0 } else { 2.0 });
            }
            if let (Some(d), Some(p)) = (&st.drag, pointer) {
                if let Some(from) = st.pins.get(&(d.node, d.pin.clone(), d.output)) {
                    let (a, b) = if d.output { (*from, p) } else { (p, *from) };
                    bezier(ui.painter(), a, b, pin_color(d.kind), 2.0);
                }
            }
            for n in vs.nodes.iter_mut() {
                let pins = cat.pins(&all[k], n);
                let pos = pos2(n.pos[0], n.pos[1]);
                let selected = st.selected.contains(&n.id);
                let acc = accent(n, &cat, &all[k]);
                let fill = if matches!(n.kind, NodeKind::Comment { .. }) { Color32::from_rgb(60, 56, 36) } else { Color32::from_rgb(40, 44, 52) };
                let stroke = if selected { Stroke::new(2.5_f32, Color32::YELLOW) } else { Stroke::new(1.0_f32, acc.gamma_multiply(0.6)) };
                let id = egui::Id::unique(("visual script node", &all[k].id, n.id));
                let title = cat.title(n);
                let mut rows: Vec<(f32, Option<usize>, Option<usize>)> = Vec::new();
                let inner = ui.scope_builder(egui::UiBuilder::new().max_rect(Rect::from_min_size(pos, vec2(NODE_W, 4000.0))).id_salt(id), |ui| {
                    egui::Frame::new().fill(fill).stroke(stroke).corner_radius(6.0).inner_margin(egui::Margin { left: 12, right: 12, top: 4, bottom: 6 }).show(ui, |ui| {
                        ui.set_width(NODE_W - 24.0);
                        let head = ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(&title).strong().color(acc));
                            header_widgets(ui, n);
                        });
                        if let NodeKind::Comment { text } = &mut n.kind {
                            ui.add(egui::TextEdit::multiline(text).desired_rows(2).desired_width(NODE_W - 30.0));
                        }
                        for row in 0..pins.inputs.len().max(pins.outputs.len()) {
                            let r = ui.horizontal(|ui| {
                                ui.set_min_height(20.0);
                                if let Some(p) = pins.inputs.get(row) {
                                    let linked = all[k].link_into(n.id, &p.name).is_some();
                                    ui.label(egui::RichText::new(&p.name).size(13.0).color(Color32::from_gray(200)));
                                    if p.kind != PinType::Flow && !linked {
                                        let mut v = n.values.get(&p.name).cloned();
                                        let empty = Vec::new();
                                        let o = p.arg.and_then(|a| opts.iter().find(|o| o.0 == a)).map_or(&empty, |o| &o.1);
                                        value_field(ui, id.with(&p.name), &format!("{title}: {}", p.name), p.kind, p.arg, &mut v, o);
                                        match v {
                                            Some(v) => {
                                                n.values.insert(p.name.clone(), v);
                                            }
                                            None => {
                                                n.values.remove(&p.name);
                                            }
                                        }
                                    }
                                }
                                if let Some(o) = pins.outputs.get(row) {
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        ui.label(egui::RichText::new(&o.name).size(13.0).color(Color32::from_gray(200)));
                                    });
                                }
                            });
                            rows.push((r.response.rect.center().y, (row < pins.inputs.len()).then_some(row), (row < pins.outputs.len()).then_some(row)));
                        }
                        head.response.rect
                    })
                });
                let frame = inner.inner.response.rect;
                let head = inner.inner.inner;
                // Pins on the node's edges.
                for (y, i, o) in &rows {
                    if let Some(i) = i {
                        let p = &pins.inputs[*i];
                        let at = pos2(frame.left(), *y);
                        new_pins.insert((n.id, p.name.clone(), false), at);
                        pin_shape(ui.painter(), at, p.kind, all[k].link_into(n.id, &p.name).is_some());
                        let r = ui.interact(Rect::from_center_size(at, vec2(16.0, 16.0)), id.with(("in", &p.name)), Sense::drag());
                        r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, format!("{title}: the input {}", p.name)));
                        if r.drag_started() {
                            st.drag = Some(LinkDrag { node: n.id, pin: p.name.clone(), output: false, kind: p.kind });
                        }
                    }
                    if let Some(o) = o {
                        let p = &pins.outputs[*o];
                        let at = pos2(frame.right(), *y);
                        new_pins.insert((n.id, p.name.clone(), true), at);
                        pin_shape(ui.painter(), at, p.kind, all[k].links.iter().any(|l| l.from == n.id && l.out == p.name));
                        let r = ui.interact(Rect::from_center_size(at, vec2(16.0, 16.0)), id.with(("out", &p.name)), Sense::drag());
                        r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, format!("{title}: the output {}", p.name)));
                        if r.drag_started() {
                            st.drag = Some(LinkDrag { node: n.id, pin: p.name.clone(), output: true, kind: p.kind });
                        }
                    }
                }
                let drag = ui.interact(head.expand2(vec2(12.0, 2.0)), id.with("move"), Sense::click_and_drag());
                drag.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, format!("Node {title}")));
                if drag.drag_started() {
                    st.moves += 1;
                }
                if drag.dragged() {
                    moved.push((n.id, drag.drag_delta()));
                }
                if (drag.drag_started() && !selected) || drag.clicked() {
                    click_node = Some((n.id, ui.input(|i| i.modifiers.command)));
                }
                if drag.double_clicked() {
                    if let NodeKind::Call { function, .. } = &n.kind {
                        if all.iter().any(|v| v.id == *function) {
                            *ask = Some(Ask::Open(function.clone()));
                        }
                    }
                }
                drag.on_hover_cursor(egui::CursorIcon::Grab);
            }
            // A link let go: onto a pin, or in empty space (the palette, for a node that takes it).
            if let Some(d) = st.drag.clone() {
                if !ui.input(|i| i.pointer.any_down()) {
                    st.drag = None;
                    if let Some(p) = pointer.or(st.pointer) {
                        let target = new_pins.iter().filter(|((node, _, out), at)| *node != d.node && *out != d.output && at.distance(p) < 14.0).min_by(|a, b| a.1.distance(p).total_cmp(&b.1.distance(p))).map(|(k, _)| k.clone());
                        match target {
                            Some((node, pin, _)) => {
                                let (from, out, to, input) = if d.output { (d.node, d.pin.clone(), node, pin) } else { (node, pin, d.node, d.pin.clone()) };
                                add_link = Some(Link { from, out, to, input });
                            }
                            None => open_palette = Some((p, Some(d))),
                        }
                    }
                }
            }
        });
        let bg = resp.response;
        if bg.clicked() {
            self.selected.clear();
        }
        if bg.secondary_clicked() {
            if let Some(p) = pointer_at {
                open_palette = Some((p, None));
            }
        }
        self.scene_rect = scene_rect;
        self.pins = new_pins;
        if let Some((id, add)) = click_node {
            if add {
                if !self.selected.remove(&id) {
                    self.selected.insert(id);
                }
            } else {
                self.selected = BTreeSet::from([id]);
            }
        }
        if !moved.is_empty() {
            // (A drag is one step of the history, however many frames it takes.)
            crate::shell::hold_step(ui.ctx(), ("move nodes", &vs.id, self.moves));
        }
        for (id, d) in moved {
            let ids: Vec<u32> = if self.selected.contains(&id) { self.selected.iter().copied().collect() } else { vec![id] };
            for n in vs.nodes.iter_mut().filter(|n| ids.contains(&n.id)) {
                n.pos[0] += d.x;
                n.pos[1] += d.y;
            }
        }
        // Keys, while the pointer is over the canvas and nothing is being typed.
        if bg.contains_pointer() && !ui.ctx().egui_wants_keyboard_input() {
            let (del, space, copy, paste, dup, group) = ui.input(|i| {
                let cmd = i.modifiers.command;
                (
                    i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace),
                    i.key_pressed(egui::Key::Space),
                    i.events.iter().any(|e| matches!(e, egui::Event::Copy)),
                    i.events.iter().any(|e| matches!(e, egui::Event::Paste(_))),
                    cmd && i.key_pressed(egui::Key::D),
                    cmd && i.key_pressed(egui::Key::G),
                )
            });
            if del {
                self.delete_selected(vs);
            }
            if space {
                if let Some(p) = pointer_at {
                    open_palette = Some((p, None));
                }
            }
            if copy || dup {
                self.clip = visualedit::copy_nodes(vs, &self.selected);
                // (Something on the clipboard, so that Ctrl+V arrives here afterwards.)
                ui.ctx().copy_text(format!("{} nodes of {}", self.selected.len(), vs.id));
            }
            if paste || dup {
                if let Some(clip) = &self.clip {
                    self.selected = visualedit::paste_nodes(vs, clip, if dup { None } else { pointer_at.map(|p| [p.x, p.y]) });
                }
            }
            if group && !self.selected.is_empty() {
                *group_asked = true;
            }
        }
        if let Some((at, connect)) = open_palette {
            self.palette = Some(Palette { at, connect, ..Default::default() });
        }
        if let Some(l) = add_link {
            visualedit::connect(vs, l, &cat);
        }
    }

    /// The palette's window: a node to add, picked from a searched list.
    fn palette_window(&mut self, ctx: &egui::Context, all: &[VisualScript], vs: &mut VisualScript) {
        let Some(mut pal) = self.palette.take() else { return };
        let cat = Catalog { visual: all, text_fns: self.text_fns.clone() };
        let entries = visualedit::palette_entries(all, &self.text_fns, &vs.id);
        let mut pick: Option<Entry> = None;
        let mut open = true;
        let at = *pal.window.get_or_insert_with(|| {
            let (all, p) = (ctx.content_rect(), ctx.input(|i| i.pointer.latest_pos()).unwrap_or(ctx.content_rect().center()));
            // (A little beside the pointer: the place of the new node stays in sight.)
            pos2((p.x + 16.0).min(all.right() - 340.0).max(all.left()), (p.y - 40.0).min(all.bottom() - 480.0).max(all.top()))
        });
        egui::Window::new("Add a node").id(egui::Id::unique("visual script palette")).open(&mut open).collapsible(false).resizable(false).fixed_pos(at).default_width(320.0).show(ctx, |ui| {
            let r = ui.add(egui::TextEdit::singleline(&mut pal.search).hint_text("Search").desired_width(300.0));
            r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, "Search the nodes"));
            if !pal.focused {
                r.request_focus();
                pal.focused = true;
            }
            let shown = visualedit::palette_shown(&entries, &pal.search, pal.connect.as_ref(), vs, &cat);
            if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                pick = shown.first().map(|e| (*e).clone());
            }
            egui::ScrollArea::vertical().max_height(380.0).show(ui, |ui| {
                let mut section = "";
                for e in shown.iter().take(300) {
                    if e.section != section {
                        section = &e.section;
                        ui.label(egui::RichText::new(section).small().strong().color(Color32::from_gray(150)));
                    }
                    if ui.selectable_label(false, &e.label).on_hover_text(&e.doc).clicked() {
                        pick = Some((*e).clone());
                    }
                }
                if shown.is_empty() {
                    ui.weak(if pal.connect.is_some() { "No node of that name takes this link." } else { "No node of that name." });
                }
            });
        });
        if let Some(e) = pick {
            let n = visualedit::add_node(vs, &cat, e.kind, [pal.at.x, pal.at.y], pal.connect.clone());
            self.selected = BTreeSet::from([n]);
        } else if open && !ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.palette = Some(pal);
        }
    }
}

#[cfg(test)]
mod ui_tests {
    use super::*;
    use egui_kittest::kittest::Queryable;
    use egui_kittest::Harness;

    struct State {
        ws: Workspace,
        view: NodesView,
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

    fn script(s: &State) -> &VisualScript {
        s.ws.project.db.visual_scripts.iter().find(|v| v.id == "greet").unwrap()
    }

    #[test]
    fn a_node_is_added_from_the_palette_linked_filled_in_and_deleted_on_the_canvas() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir = std::env::temp_dir().join(format!("rustpg2d-nodes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        copy(&root.join("example_project"), &dir);
        let mut ws = Workspace::new(dir.clone(), editor_core::Gpu::No).unwrap();
        ws.project.db.visual_scripts.push(VisualScript::fresh("greet"));
        ws.history_frame(None);
        let mut h = Harness::builder().with_size(vec2(1300.0, 800.0)).build_ui_state(
            |ui, s: &mut State| {
                let held = ui.ctx().data_mut(|d| d.remove_temp::<u64>(crate::shell::held_step_id()));
                s.ws.history_frame(held.or(ui.ctx().memory(|m| m.focused()).map(|id| id.value())));
                s.view.show(ui, &mut s.ws, "greet");
            },
            State { ws, view: NodesView::default(), dir },
        );
        h.run();
        // The palette opens where the pointer is (Space), searched; Enter takes the first.
        // (What the tree says of a node is in the canvas's own coordinates.)
        let screen = |h: &Harness<State>, label: &str| h.state().view.on_screen(h.get_by_label(label).rect().center());
        let at = screen(&h, "Node Start") + vec2(40.0, 200.0);
        h.hover_at(at);
        h.run();
        h.key_press(egui::Key::Space);
        h.run();
        let search = h.get_by_role_and_label(egui::accesskit::Role::TextInput, "Search the nodes");
        search.focus();
        search.type_text("give_item");
        h.run();
        h.key_press(egui::Key::Enter);
        h.run();
        let vs = script(h.state());
        assert_eq!(vs.nodes.len(), 2, "{:?}", vs.nodes);
        let new = vs.nodes[1].id;
        assert!(matches!(&vs.nodes[1].kind, NodeKind::Call { function, .. } if function == "give_item"));
        assert_eq!(h.state().view.selected, BTreeSet::from([new]));
        assert!(h.state().view.palette.is_none());
        // From Start's output to its input: dragged pin to pin.
        let (from, to) = (screen(&h, "Start: the output out"), screen(&h, "give_item: the input in"));
        h.hover_at(from);
        h.run();
        h.drag_at(from);
        h.run();
        for k in 1..=4 {
            h.hover_at(from + (to - from) * (k as f32 / 4.0));
            h.run();
        }
        h.drop_at(to);
        h.run();
        let vs = script(h.state());
        assert_eq!(vs.links, [Link { from: 1, out: "out".into(), to: new, input: "in".into() }]);
        // What it gives is typed into the node.
        let item = h.get_by_role_and_label(egui::accesskit::Role::TextInput, "give_item: item");
        item.focus();
        item.type_text("potion");
        h.run();
        h.run();
        let s = h.state();
        assert_eq!(script(s).nodes[1].values.get("item"), Some(&Value::Text("potion".into())));
        let cat = Catalog { visual: &s.ws.project.db.visual_scripts, text_fns: Vec::new() };
        assert!(visualedit::code_text(script(s), &cat).contains("give_item(\"potion\""), "{}", visualedit::code_text(script(s), &cat));
        // Dragged by its title it moves, as one step however many frames that took.
        let steps = h.state().ws.timeline.done.len();
        let title = screen(&h, "Node give_item");
        let was = script(h.state()).nodes[1].pos;
        h.hover_at(title);
        h.run();
        h.drag_at(title);
        h.run();
        for k in 1..=5 {
            h.hover_at(title + vec2(20.0 * k as f32, 10.0 * k as f32));
            h.run();
        }
        h.drop_at(title + vec2(100.0, 50.0));
        h.run();
        h.run();
        let s = h.state();
        let now = script(s).nodes[1].pos;
        assert!((now[0] - was[0] - 100.0).abs() < 12.0 && (now[1] - was[1] - 50.0).abs() < 12.0, "{was:?} -> {now:?}");
        assert_eq!(s.ws.timeline.done.len(), steps + 1, "{:?}", s.ws.timeline.done.iter().map(|c| &c.label).collect::<Vec<_>>());
        assert_eq!(s.ws.timeline.done.last().unwrap().label, "Change the visual script greet");
        // Delete takes the selected node and its link; Start stays whatever is selected.
        // (While a field of a node has the keyboard, Delete is that field's: a click on the canvas first.)
        let empty = at + vec2(-60.0, 150.0);
        h.hover_at(empty);
        h.run();
        h.drag_at(empty);
        h.run();
        h.event(egui::Event::PointerButton { pos: empty, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
        h.run();
        assert!(h.state().view.selected.is_empty(), "a click on the canvas lets go of what was selected");
        h.state_mut().view.selected = BTreeSet::from([1, new]);
        h.key_press(egui::Key::Delete);
        h.run();
        h.run();
        let vs = script(h.state());
        assert_eq!((vs.nodes.len(), vs.links.len()), (1, 0));
        // Undone, it is back with its link.
        h.state_mut().ws.undo().unwrap();
        let vs = script(h.state());
        assert_eq!((vs.nodes.len(), vs.links.len()), (2, 1));
        let dir = h.state().dir.clone();
        let _ = std::fs::remove_dir_all(dir);
    }
}
