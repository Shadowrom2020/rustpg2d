//! The *Nodes* tab: visual scripts (engine `visual.rs`) edited as nodes on a canvas.
//!
//! The visual scripts on the left (name, function name, parameters, result); the canvas in the
//! middle (drag the background to pan, Ctrl+wheel to zoom, a node's title to move it, from a pin
//! to another to link them; right-click or Space for the palette; Delete removes the selected
//! nodes); the selected node's help, the problems and the generated Rhai on the right. Changes go
//! into the definitions (undo with Ctrl+Z); saving writes `scripts/visual_scripts.rhai`.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use egui_macroquad::egui::{self, pos2, vec2, Color32, Pos2, Rect, Sense, Stroke};
use rustpg2d_engine::refactor::RefKind;
use rustpg2d_engine::script_api::{Arg, API};
use rustpg2d_engine::visual::{self, Catalog, CmpOp, Link, LogicOp, MathOp, Node, NodeKind, Param, PinType, Value, VisualScript};

use crate::app::Editor;
use crate::ui::{self, ph};

/// A node's width on the canvas.
const NODE_W: f32 = 230.0;

/// A link being dragged out of a pin.
#[derive(Clone, Debug, PartialEq)]
pub struct LinkDrag {
    pub node: u32,
    pub pin: String,
    /// It starts at an output (else an input).
    pub output: bool,
    pub kind: PinType,
}

/// The palette of nodes to add.
#[derive(Clone, Debug, Default)]
pub struct Palette {
    /// Where the new node goes (canvas coordinates).
    pub at: Pos2,
    pub search: String,
    /// A link dragged into empty space: the new node gets it.
    pub connect: Option<LinkDrag>,
    pub focused: bool,
}

/// The Nodes tab's state.
pub struct NodesState {
    /// The visual script shown (its id).
    pub sel: String,
    pub scene_rect: Rect,
    /// Selected nodes.
    pub selected: BTreeSet<u32>,
    pub drag: Option<LinkDrag>,
    pub palette: Option<Palette>,
    /// Pins of the last frame: (node, pin, output) -> canvas position.
    pins: HashMap<(u32, String, bool), Pos2>,
    /// Text scripts' functions, read again once a second.
    text_fns: Vec<(String, usize)>,
    text_fns_at: f64,
    /// The function name field while typed in (renamed when it loses focus).
    id_edit: Option<String>,
    /// *Convert to text script* asked to confirm.
    confirm_convert: bool,
    /// Last frame's canvas rectangle on screen (palette placement).
    canvas: Rect,
    /// Last frame's transform from the screen into the canvas.
    to_scene: Option<egui::emath::TSTransform>,
    /// Copied nodes and the links between them (Ctrl+C, Ctrl+V).
    clip: Option<(Vec<Node>, Vec<Link>)>,
    /// A visual script to open (a call node's title double-clicked).
    open: Option<String>,
}

impl Default for NodesState {
    fn default() -> Self {
        NodesState {
            sel: String::new(),
            scene_rect: Rect::ZERO,
            selected: BTreeSet::new(),
            drag: None,
            palette: None,
            pins: HashMap::new(),
            text_fns: Vec::new(),
            text_fns_at: -10.0,
            id_edit: None,
            confirm_convert: false,
            canvas: Rect::ZERO,
            to_scene: None,
            clip: None,
            open: None,
        }
    }
}

/// One entry of the palette.
#[derive(Clone, Debug)]
pub struct Entry {
    pub section: String,
    pub label: String,
    pub doc: String,
    pub kind: NodeKind,
}

/// The manual's API sections by function (`08-scripting.md`): the palette's groups.
fn sections() -> &'static BTreeMap<String, String> {
    static S: std::sync::OnceLock<BTreeMap<String, String>> = std::sync::OnceLock::new();
    S.get_or_init(|| {
        let manual = include_str!("../../../Documentation/manual/08-scripting.md");
        let mut out = BTreeMap::new();
        let mut section = String::new();
        let mut in_api = false;
        for line in manual.lines() {
            if line.starts_with("## ") {
                in_api = line.starts_with("## API reference");
            } else if let Some(s) = line.strip_prefix("### ") {
                section = s.trim().to_string();
            } else if in_api {
                // (A row's first cell may name several: `is_female()` / `is_male()`.)
                // (Or a paragraph that starts with it: `move_route(target, steps)` / … walk …)
                let cell = line.strip_prefix("| ").and_then(|r| r.split(" |").next()).or_else(|| line.starts_with('`').then(|| leading_code(line)));
                if let Some(cell) = cell {
                    for part in cell.split('`').skip(1).step_by(2) {
                        let name: String = part.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
                        if !name.is_empty() && part[name.len()..].starts_with('(') {
                            out.entry(name).or_insert_with(|| section.clone());
                        }
                    }
                }
            }
        }
        out
    })
}

/// The code spans a line starts with, joined by " / " (`` `a(x)` / `b(x)` and more``).
fn leading_code(line: &str) -> &str {
    let mut end = 0;
    loop {
        let rest = &line[end..];
        let Some(close) = rest.strip_prefix('`').and_then(|r| r.find('`')) else { break };
        end += close + 2;
        match line[end..].strip_prefix(" / ") {
            Some(_) => end += 3,
            None => break,
        }
    }
    &line[..end]
}

/// Every node the palette offers (`current` = the script being edited: it doesn't call itself).
pub fn palette_entries(visual: &[VisualScript], text_fns: &[(String, usize)], current: &str) -> Vec<Entry> {
    let e = |section: &str, label: &str, doc: &str, kind: NodeKind| Entry { section: section.into(), label: label.into(), doc: doc.into(), kind };
    let mut v = vec![
        e("Flow", "If", "Runs 'then' or 'else' by a yes / no", NodeKind::If),
        e("Flow", "Sequence", "Runs its outputs one after the other", NodeKind::Sequence { steps: 2 }),
        e("Flow", "Repeat", "Runs 'body' a number of times, then 'done'", NodeKind::Repeat),
        e("Flow", "For each", "Runs 'body' for each item of a list, then 'done'", NodeKind::ForEach),
        e("Flow", "Return", "Ends the function (with its result)", NodeKind::Return),
        e("Values", "Number", "A fixed number", NodeKind::Value { kind: PinType::Num }),
        e("Values", "Text", "A fixed text", NodeKind::Value { kind: PinType::Text }),
        e("Values", "Yes / no", "A fixed yes or no", NodeKind::Value { kind: PinType::Bool }),
        e("Values", "Join text", "Two values joined into one text", NodeKind::Join),
        e("Values", "Set local variable", "Keeps a value under a name while the function runs", NodeKind::SetLocal { name: "value".into() }),
        e("Values", "Get local variable", "The value kept under a name", NodeKind::GetLocal { name: "value".into() }),
        e("Maths and logic", "Math", "+, -, *, /, remainder, smaller, larger of two numbers", NodeKind::Math { op: MathOp::Add }),
        e("Maths and logic", "Compare", "Whether two values are equal, smaller, larger…", NodeKind::Compare { op: CmpOp::Eq }),
        e("Maths and logic", "And", "Yes when both are yes", NodeKind::Logic { op: LogicOp::And }),
        e("Maths and logic", "Or", "Yes when either is yes", NodeKind::Logic { op: LogicOp::Or }),
        e("Maths and logic", "Not", "Yes becomes no and no yes", NodeKind::Not),
        e("Notes", "Comment", "A note on the canvas", NodeKind::Comment { text: "Note".into() }),
    ];
    for s in visual.iter().filter(|s| s.id != current) {
        v.push(e("Visual scripts", s.label(), &s.description, NodeKind::Call { function: s.id.clone(), arity: 0 }));
    }
    for (name, n) in text_fns {
        v.push(e("Script functions", name, "A function of the project's text scripts", NodeKind::Call { function: name.clone(), arity: *n as u8 }));
    }
    let mut seen = BTreeSet::new();
    for f in API.iter().filter(|f| f.deprecated.is_none()) {
        // (One entry per function: the node takes the form whose inputs are used.)
        if !seen.insert(f.name) {
            continue;
        }
        let section = sections().get(f.name).cloned().unwrap_or_else(|| "More".into());
        v.push(e(&section, f.name, f.doc, NodeKind::Call { function: f.name.into(), arity: 0 }));
    }
    v
}

/// The colour of a pin type.
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

/// The texts an id input may take (items, sounds, maps…).
fn options(ed: &Editor, arg: Arg) -> Vec<String> {
    let r = &ed.refs;
    let db = &ed.project.db;
    match arg {
        Arg::Item => r.items.clone(),
        Arg::Map => r.maps.clone(),
        Arg::Sound => r.sounds.clone(),
        Arg::Quest => r.quests.clone(),
        Arg::Spell => r.spells.clone(),
        Arg::Sheet => r.sheets.clone(),
        Arg::Character => r.characters.clone(),
        Arg::Prop => r.props.clone(),
        Arg::Target => {
            let mut v = vec!["self".to_string(), "player".to_string()];
            v.extend(r.entity_names.iter().cloned());
            v
        }
        Arg::Flag => r.all_flags.clone(),
        Arg::Var => r.vars.clone(),
        Arg::Func => r.functions.clone(),
        Arg::Dir => ["up", "down", "left", "right"].iter().map(|s| s.to_string()).collect(),
        Arg::Achievement => r.achievements.clone(),
        Arg::Class => r.classes.clone(),
        Arg::Disease => r.diseases.clone(),
        Arg::Common => r.common_events.clone(),
        Arg::Faction => r.factions.clone(),
        Arg::Station => r.stations.clone(),
        Arg::State => r.states.clone(),
        Arg::Skill => r.skills.clone(),
        Arg::Recipe => db.recipes.iter().map(|d| d.id.clone()).collect(),
        Arg::Effect => rustpg2d_engine::srd::Condition::ALL.iter().map(|k| k.id().to_string()).collect(),
        Arg::Weather => {
            let mut v: Vec<String> = rustpg2d_engine::map::WeatherKind::ALL.iter().map(|k| k.id().to_string()).collect();
            v.push("default".into());
            v
        }
        _ => Vec::new(),
    }
}

/// The text a value shows in an input field.
fn value_text(v: Option<&Value>) -> String {
    match v {
        Some(Value::Text(t)) => t.clone(),
        Some(Value::Num(n)) => {
            if n.fract() == 0.0 {
                format!("{}", *n as i64)
            } else {
                n.to_string()
            }
        }
        Some(Value::Bool(b)) => b.to_string(),
        None => String::new(),
    }
}

/// A value typed into an input of any type: a number, yes / no, else text.
fn parse_any(s: &str) -> Option<Value> {
    if s.is_empty() {
        None
    } else if let Ok(n) = s.trim().parse::<f64>() {
        Some(Value::Num(n))
    } else if s == "true" || s == "false" {
        Some(Value::Bool(s == "true"))
    } else {
        Some(Value::Text(s.to_string()))
    }
}

/// The editor for a value typed into an input of type `kind`.
fn value_field(ui: &mut egui::Ui, id: egui::Id, kind: PinType, arg: Option<Arg>, value: &mut Option<Value>, opts: &[String]) {
    match kind {
        PinType::Num => {
            let mut n = match value {
                Some(Value::Num(n)) => *n,
                _ => 0.0,
            };
            // (Untouched, it stays unset: the call leaves an optional last input out.)
            if ui.add(egui::DragValue::new(&mut n).speed(0.1)).changed() {
                *value = Some(Value::Num(n));
            }
        }
        PinType::Bool => {
            let mut b = matches!(value, Some(Value::Bool(true)));
            if ui.checkbox(&mut b, "").changed() {
                *value = Some(Value::Bool(b));
            }
        }
        PinType::Text => {
            let mut s = value_text(value.as_ref());
            let before = s.clone();
            if arg.is_some_and(|a| a.is_id()) {
                crate::widgets::suggest(ui, &format!("{id:?}"), &mut s, opts, 100.0);
            } else {
                let lines = arg == Some(Arg::Line);
                ui.add(egui::TextEdit::singleline(&mut s).desired_width(if lines { 140.0 } else { 110.0 }).id(id));
            }
            if s != before {
                *value = (!s.is_empty()).then_some(Value::Text(s));
            }
        }
        PinType::Any => {
            let mut s = value_text(value.as_ref());
            if ui.add(egui::TextEdit::singleline(&mut s).desired_width(80.0).id(id).hint_text("value")).changed() {
                *value = parse_any(&s);
            }
        }
        _ => {
            ui.weak("(link a value)");
        }
    }
}

impl Editor {
    /// The visual script shown (index), choosing one when none is.
    fn vs_index(&mut self) -> Option<usize> {
        let list = &self.project.db.visual_scripts;
        if let Some(k) = list.iter().position(|v| v.id == self.nodes.sel) {
            return Some(k);
        }
        let k = (!list.is_empty()).then_some(0)?;
        self.nodes.sel = list[k].id.clone();
        self.nodes.selected.clear();
        Some(k)
    }

    fn refresh_text_fns(&mut self) {
        let now = ui::now();
        if now - self.nodes.text_fns_at > 1.0 {
            self.nodes.text_fns = self.project.text_script_functions();
            self.nodes.text_fns_at = now;
        }
    }

    /// Writes a changed visual script back into the definitions.
    fn put_vs(&mut self, k: usize, vs: VisualScript) {
        if self.project.db.visual_scripts.get(k) != Some(&vs) {
            self.project.db.visual_scripts[k] = vs;
            self.defs_dirty = true;
        }
    }

    pub fn nodes_ui(&mut self, ctx: &egui::Context) {
        self.refresh_text_fns();
        if !ctx.wants_keyboard_input() {
            let (cmd, shift) = ctx.input(|i| (i.modifiers.command, i.modifiers.shift));
            let key = |k| ctx.input(|i| i.key_pressed(k));
            if cmd && key(egui::Key::Z) && !shift {
                self.db_undo();
            } else if cmd && (key(egui::Key::Y) || (key(egui::Key::Z) && shift)) {
                self.db_redo();
            } else if cmd && key(egui::Key::S) {
                self.save_all();
            } else if cmd && key(egui::Key::G) {
                self.nodes_group_status();
            } else if cmd && key(egui::Key::C) {
                self.nodes_copy();
            } else if cmd && key(egui::Key::V) {
                let at = ctx.input(|i| i.pointer.hover_pos()).filter(|p| self.nodes.canvas.contains(*p)).map(|p| self.nodes_to_scene(ctx, p));
                self.nodes_paste(at);
            } else if cmd && key(egui::Key::D) {
                self.nodes_copy();
                self.nodes_paste(None);
            } else if key(egui::Key::Delete) || key(egui::Key::Backspace) {
                self.nodes_delete_selected();
            } else if key(egui::Key::Space) && self.nodes.palette.is_none() {
                let at = ctx.input(|i| i.pointer.hover_pos()).filter(|p| self.nodes.canvas.contains(*p)).unwrap_or(self.nodes.canvas.center());
                let at = self.nodes_to_scene(ctx, at);
                self.nodes.palette = Some(Palette { at, ..Default::default() });
            } else if key(egui::Key::Escape) {
                self.nodes.palette = None;
                self.nodes.drag = None;
            }
        }
        egui::SidePanel::left("nodes_list").resizable(true).default_width(260.0).show(ctx, |ui| {
            egui::ScrollArea::vertical().id_salt("nodes_list_scroll").show(ui, |ui| self.nodes_list(ui));
        });
        egui::SidePanel::right("nodes_side").resizable(true).default_width(360.0).show(ctx, |ui| {
            egui::ScrollArea::vertical().id_salt("nodes_side_scroll").show(ui, |ui| self.nodes_side(ui));
        });
        egui::CentralPanel::default().frame(egui::Frame::new().fill(Color32::from_rgb(24, 26, 30))).show(ctx, |ui| self.nodes_canvas(ui));
        self.nodes_palette(ctx);
        if let Some(id) = self.nodes.open.take() {
            self.nodes_open(&id);
        }
    }

    /// Shows visual script `id` (a group from its call node).
    pub fn nodes_open(&mut self, id: &str) {
        if self.project.db.visual_scripts.iter().any(|v| v.id == id) {
            self.nodes.sel = id.to_string();
            self.nodes.selected.clear();
            self.nodes.scene_rect = Rect::ZERO;
            self.nodes.id_edit = None;
        }
    }

    /// Groups the selected nodes into a new visual script called in their place (Ctrl+G); the
    /// new one's id, or why not.
    pub fn nodes_group(&mut self) -> Result<String, String> {
        let k = self.vs_index().ok_or("no visual script")?;
        let all = self.project.db.visual_scripts.clone();
        let mut vs = all[k].clone();
        let id = rustpg2d_engine::refactor::unique_id(&format!("{}_part", vs.id), |c| all.iter().any(|v| v.id == c) || API.iter().any(|f| f.name == c) || self.nodes.text_fns.iter().any(|t| t.0 == c));
        let cat = Catalog { visual: &all, text_fns: self.nodes.text_fns.clone() };
        let mut g = visual::group(&mut vs, &self.nodes.selected, &id, &cat)?;
        g.name = format!("{} (part)", vs.label());
        self.put_vs(k, vs);
        self.project.db.visual_scripts.push(g);
        self.defs_dirty = true;
        self.nodes.selected.clear();
        Ok(id)
    }

    fn nodes_group_status(&mut self) {
        match self.nodes_group() {
            Ok(id) => self.set_status(format!("Grouped into the visual script {id} (Open goes into it)")),
            Err(e) => self.set_status(format!("Can't group: {e}")),
        }
    }

    /// Copies the selected nodes (not Start) and the links between them.
    pub fn nodes_copy(&mut self) {
        let Some(k) = self.vs_index() else { return };
        let vs = &self.project.db.visual_scripts[k];
        let nodes: Vec<Node> = vs.nodes.iter().filter(|n| self.nodes.selected.contains(&n.id) && n.kind != NodeKind::Start).cloned().collect();
        let ids: BTreeSet<u32> = nodes.iter().map(|n| n.id).collect();
        let links: Vec<Link> = vs.links.iter().filter(|l| ids.contains(&l.from) && ids.contains(&l.to)).cloned().collect();
        if !nodes.is_empty() {
            self.nodes.clip = Some((nodes, links));
        }
    }

    /// Pastes the copied nodes (new ids) with their top-left at `at`, else a little below the
    /// copies; they become the selection.
    pub fn nodes_paste(&mut self, at: Option<Pos2>) {
        let (Some(k), Some((nodes, links))) = (self.vs_index(), self.nodes.clip.clone()) else { return };
        let mut vs = self.project.db.visual_scripts[k].clone();
        let first = vs.next_id();
        let map: BTreeMap<u32, u32> = nodes.iter().enumerate().map(|(i, n)| (n.id, first + i as u32)).collect();
        let min = nodes.iter().fold(pos2(f32::INFINITY, f32::INFINITY), |m, n| pos2(m.x.min(n.pos[0]), m.y.min(n.pos[1])));
        let off = at.map_or(vec2(40.0, 40.0), |a| a - min);
        for n in &nodes {
            vs.nodes.push(Node { id: map[&n.id], pos: [n.pos[0] + off.x, n.pos[1] + off.y], ..n.clone() });
        }
        for l in &links {
            vs.links.push(Link { from: map[&l.from], to: map[&l.to], ..l.clone() });
        }
        self.nodes.selected = map.values().copied().collect();
        self.put_vs(k, vs);
    }

    /// The canvas position of a screen position (the canvas's last transform).
    fn nodes_to_scene(&self, _ctx: &egui::Context, p: Pos2) -> Pos2 {
        self.nodes.to_scene.map_or(p, |t| t * p)
    }

    fn nodes_list(&mut self, ui: &mut egui::Ui) {
        ui.heading(ui::t(ph::FLOW_ARROW, "Visual scripts"));
        ui.weak("Script functions made of nodes. Triggers (Run script), hooks, menus, spells and other scripts call them by their function name.");
        ui.horizontal(|ui| {
            if ui.button(ui::t(ph::PLUS, "New")).on_hover_text("A new visual script with a Start node").clicked() {
                let id = rustpg2d_engine::refactor::unique_id("new_function", |c| self.project.db.visual_scripts.iter().any(|v| v.id == c) || API.iter().any(|f| f.name == c));
                self.project.db.visual_scripts.push(VisualScript::fresh(&id));
                self.nodes.sel = id;
                self.nodes.selected.clear();
                self.nodes.scene_rect = Rect::ZERO;
                self.defs_dirty = true;
            }
            if let Some(k) = self.vs_index() {
                if ui.button(ui::t(ph::COPY, "Duplicate")).clicked() {
                    let mut c = self.project.db.visual_scripts[k].clone();
                    c.id = rustpg2d_engine::refactor::unique_id(&c.id, |x| self.project.db.visual_scripts.iter().any(|v| v.id == x));
                    self.nodes.sel = c.id.clone();
                    self.project.db.visual_scripts.push(c);
                    self.defs_dirty = true;
                }
                if ui.button(ui::t(ph::TRASH, "Delete")).on_hover_text("Deletes this visual script (Ctrl+Z brings it back)").clicked() {
                    self.project.db.visual_scripts.remove(k);
                    self.nodes.sel.clear();
                    self.defs_dirty = true;
                }
            }
        });
        ui.separator();
        let mut pick = None;
        for v in &self.project.db.visual_scripts {
            let label = if v.name.is_empty() { v.id.clone() } else { format!("{}  ({})", v.name, v.id) };
            if ui.selectable_label(v.id == self.nodes.sel, label).clicked() {
                pick = Some(v.id.clone());
            }
        }
        if let Some(p) = pick {
            self.nodes.sel = p;
            self.nodes.selected.clear();
            self.nodes.scene_rect = Rect::ZERO;
            self.nodes.id_edit = None;
        }
        let Some(k) = self.vs_index() else {
            ui.add_space(8.0);
            ui.label("No visual scripts yet: New makes one.");
            return;
        };
        ui.separator();
        let mut vs = self.project.db.visual_scripts[k].clone();
        egui::Grid::new("vs_props").num_columns(2).show(ui, |ui| {
            ui.label("Name");
            ui.add(egui::TextEdit::singleline(&mut vs.name).desired_width(160.0).hint_text(&vs.id));
            ui.end_row();
            ui.label("Function").on_hover_text("What triggers and scripts call: small letters, digits and _");
            let mut id = self.nodes.id_edit.clone().unwrap_or_else(|| vs.id.clone());
            let r = ui.add(egui::TextEdit::singleline(&mut id).desired_width(160.0));
            if r.changed() {
                self.nodes.id_edit = Some(id.clone());
            }
            if r.lost_focus() {
                if let Some(new) = self.nodes.id_edit.take().filter(|n| *n != vs.id) {
                    if visual::valid_name(&new) {
                        let old = vs.id.clone();
                        self.put_vs(k, vs.clone());
                        self.rename_id(RefKind::VisualScript, &old, &new);
                        self.nodes.sel = new;
                        return;
                    }
                    self.set_status("A function name has small letters, digits and _, starting with a letter");
                }
            }
            ui.end_row();
        });
        if self.vs_index() != Some(k) {
            return;
        }
        ui.label("Description");
        ui.add(egui::TextEdit::multiline(&mut vs.description).desired_rows(2).desired_width(f32::INFINITY));
        ui.add_space(4.0);
        ui.strong("Parameters").on_hover_text("Values the caller passes: the Start node's outputs");
        let mut remove = None;
        for (i, p) in vs.params.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut p.name).desired_width(100.0));
                kind_combo(ui, ("vs_param", i), &mut p.kind);
                if ui.small_button(ph::X).on_hover_text("Remove").clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            vs.params.remove(i);
        }
        if ui.small_button(ui::t(ph::PLUS, "Parameter")).clicked() {
            let name = rustpg2d_engine::refactor::unique_id("value", |c| vs.params.iter().any(|p| p.name == c));
            vs.params.push(Param { name, kind: PinType::Num });
        }
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.strong("Result").on_hover_text("What the function gives back (a Return node's value)");
            let label = vs.returns.map_or("nothing", |k| k.label());
            egui::ComboBox::from_id_salt("vs_returns").selected_text(label).show_ui(ui, |ui| {
                ui.selectable_value(&mut vs.returns, None, "nothing");
                for k in PinType::VALUES {
                    ui.selectable_value(&mut vs.returns, Some(k), k.label());
                }
            });
        });
        ui.add_space(8.0);
        if !self.nodes.confirm_convert {
            if ui.button(ui::t(ph::CODE, "Convert to text script…")).on_hover_text("Moves this function into a text script for good (scripts/<name>.rhai) and removes the nodes").clicked() {
                self.nodes.confirm_convert = true;
            }
        } else {
            ui.label(format!("Write fn {}() into scripts/{}.rhai and remove the nodes? It can't be turned back into nodes.", vs.id, vs.id));
            ui.horizontal(|ui| {
                if ui.button("Convert").clicked() {
                    self.nodes.confirm_convert = false;
                    self.put_vs(k, vs.clone());
                    self.convert_to_text(k);
                }
                if ui.button("Cancel").clicked() {
                    self.nodes.confirm_convert = false;
                }
            });
        }
        ui.add_space(8.0);
        ui.weak("Right-click the canvas (or Space) to add a node. Drag from a pin to another to link them, into empty space to add a linked node. Drag a node's title to move it, Ctrl+click to select more, Delete removes them. Ctrl+Z undoes; Ctrl+S saves (and writes the Rhai).");
        if self.vs_index() == Some(k) {
            self.put_vs(k, vs);
        }
    }

    /// *Convert to text script*: the function's Rhai into `scripts/<id>.rhai`, the nodes removed.
    fn convert_to_text(&mut self, k: usize) {
        let all = self.project.db.visual_scripts.clone();
        let cat = Catalog { visual: &all, text_fns: self.nodes.text_fns.clone() };
        let (code, problems) = visual::function_code(&all[k], &cat);
        if !problems.is_empty() {
            self.set_status("Fix the problems first: a script with problems would be written as one that does nothing");
            return;
        }
        let path = self.project.scripts_dir().join(format!("{}.rhai", all[k].id));
        if path.exists() {
            self.set_status(format!("{} exists already", path.display()));
            return;
        }
        let text = format!("// Converted from the visual script \"{}\".\n{code}", all[k].label());
        if let Err(e) = std::fs::create_dir_all(self.project.scripts_dir()).and_then(|_| std::fs::write(&path, text)) {
            self.set_status(format!("Could not write {}: {e}", path.display()));
            return;
        }
        self.project.db.visual_scripts.remove(k);
        self.nodes.sel.clear();
        self.defs_dirty = true;
        self.save_defs();
        self.set_status(format!("Converted to {}", path.display()));
    }

    pub fn nodes_delete_selected(&mut self) {
        let Some(k) = self.vs_index() else { return };
        let mut vs = self.project.db.visual_scripts[k].clone();
        let gone: BTreeSet<u32> = self.nodes.selected.iter().copied().filter(|id| vs.node(*id).is_some_and(|n| n.kind != NodeKind::Start)).collect();
        if gone.is_empty() {
            return;
        }
        vs.nodes.retain(|n| !gone.contains(&n.id));
        vs.links.retain(|l| !gone.contains(&l.from) && !gone.contains(&l.to));
        self.nodes.selected.clear();
        self.put_vs(k, vs);
    }

    fn nodes_side(&mut self, ui: &mut egui::Ui) {
        let Some(k) = self.vs_index() else { return };
        let all = self.project.db.visual_scripts.clone();
        let cat = Catalog { visual: &all, text_fns: self.nodes.text_fns.clone() };
        let vs = &all[k];
        let (code, problems) = visual::function_code(vs, &cat);
        if !self.nodes.selected.is_empty() {
            ui.horizontal(|ui| {
                if ui.button(ui::t(ph::SELECTION_PLUS, "Group into a function")).on_hover_text("The selected nodes become a new visual script, called in their place (Ctrl+G): values coming in are its parameters, one value going out its result").clicked() {
                    self.nodes_group_status();
                }
                if ui.button(ui::t(ph::COPY, "Copy")).on_hover_text("Ctrl+C; Ctrl+V pastes at the mouse, Ctrl+D duplicates").clicked() {
                    self.nodes_copy();
                }
            });
        }
        if let Some(&id) = self.nodes.selected.iter().next() {
            if let Some(n) = vs.node(id) {
                ui.heading(cat.title(n));
                if let NodeKind::Call { function, .. } = &n.kind {
                    if all.iter().any(|v| v.id == *function) && ui.button(ui::t(ph::ARROW_SQUARE_IN, "Open")).on_hover_text("Shows this visual script (or double-click the node's title)").clicked() {
                        self.nodes.open = Some(function.clone());
                    }
                }
                if let NodeKind::Call { function, .. } = &n.kind {
                    if let Some(f) = API.iter().find(|f| f.name == function.as_str()) {
                        ui.label(f.doc);
                        let params: Vec<String> = f.params.iter().map(|(p, a)| format!("{p}: {:?}", a)).collect();
                        ui.weak(format!("{}({})", f.name, params.join(", ")));
                    } else if let Some(v) = all.iter().find(|v| v.id == *function) {
                        ui.label(if v.description.is_empty() { "A visual script." } else { v.description.as_str() });
                    } else {
                        ui.label("A function of the project's text scripts.");
                    }
                }
                for p in problems.iter().filter(|p| p.node == Some(id)) {
                    ui.colored_label(Color32::from_rgb(240, 120, 110), format!("{} {}", ph::WARNING, p.text));
                }
                ui.separator();
            }
        }
        ui.horizontal(|ui| {
            ui.strong(if problems.is_empty() { "No problems".to_string() } else { format!("Problems ({})", problems.len()) });
        });
        let mut goto = None;
        for p in &problems {
            let text = match p.node.and_then(|n| vs.node(n)) {
                Some(n) => format!("{}: {}", cat.title(n), p.text),
                None => p.text.clone(),
            };
            if ui.add(egui::Label::new(egui::RichText::new(format!("{} {text}", ph::WARNING)).color(Color32::from_rgb(240, 120, 110))).sense(Sense::click())).clicked() {
                goto = p.node;
            }
        }
        if let Some(n) = goto {
            self.nodes.selected = BTreeSet::from([n]);
            if let Some(node) = vs.node(n) {
                let size = self.nodes.scene_rect.size();
                self.nodes.scene_rect = Rect::from_center_size(pos2(node.pos[0] + NODE_W / 2.0, node.pos[1] + 40.0), size);
            }
        }
        ui.separator();
        ui.horizontal(|ui| {
            ui.strong("Rhai");
            if ui.small_button(ui::t(ph::COPY, "Copy")).clicked() {
                ui.ctx().copy_text(code.clone());
            }
        });
        ui.weak("What the game runs (written into scripts/visual_scripts.rhai when you save).");
        egui::Frame::new().fill(Color32::from_rgb(20, 22, 26)).inner_margin(6.0).show(ui, |ui| {
            ui.add(egui::Label::new(egui::RichText::new(&code).monospace().size(12.0)).wrap_mode(egui::TextWrapMode::Extend));
        });
    }

    fn nodes_canvas(&mut self, ui: &mut egui::Ui) {
        self.nodes.canvas = ui.max_rect();
        let Some(k) = self.vs_index() else {
            ui.centered_and_justified(|ui| ui.label("New (left) makes a visual script."));
            return;
        };
        let all = self.project.db.visual_scripts.clone();
        let mut vs = all[k].clone();
        let text_fns = self.nodes.text_fns.clone();
        // The ids each kind of input may take, collected once per frame and argument kind.
        let mut opts: Vec<(Arg, Vec<String>)> = Vec::new();
        for n in &vs.nodes {
            for p in (Catalog { visual: &all, text_fns: text_fns.clone() }).pins(&vs, n).inputs {
                if let Some(a) = p.arg.filter(|a| a.is_id()) {
                    if !opts.iter().any(|o| o.0 == a) {
                        opts.push((a, options(self, a)));
                    }
                }
            }
        }
        let st = &mut self.nodes;
        if st.scene_rect == Rect::ZERO {
            // Fit the nodes (or a default view).
            let mut r = Rect::NOTHING;
            for n in &vs.nodes {
                r = r.union(Rect::from_min_size(pos2(n.pos[0], n.pos[1]), vec2(NODE_W, 120.0)));
            }
            let r = if r.is_positive() { r.expand(80.0) } else { Rect::from_min_size(Pos2::ZERO, vec2(1200.0, 700.0)) };
            // (At most 100 %: a few nodes aren't blown up.)
            let size = ui.max_rect().size();
            st.scene_rect = Rect::from_center_size(r.center(), vec2(r.width().max(size.x), r.height().max(size.y)));
        }
        let mut scene_rect = st.scene_rect;
        let mut new_pins: HashMap<(u32, String, bool), Pos2> = HashMap::new();
        let mut add_link: Option<Link> = None;
        let mut open_palette: Option<(Pos2, Option<LinkDrag>)> = None;
        let mut moved: Vec<(u32, egui::Vec2)> = Vec::new();
        let mut click_node: Option<(u32, bool)> = None;
        let mut bg_click = false;
        let mut transform = None;
        let mut pointer_at = None;
        let resp = egui::Scene::new().zoom_range(0.2..=2.0).max_inner_size(vec2(20000.0, 20000.0)).show(ui, &mut scene_rect, |ui| {
            let to_scene = ui.ctx().layer_transform_from_global(ui.layer_id());
            transform = to_scene;
            let pointer = ui.input(|i| i.pointer.latest_pos()).map(|p| to_scene.map_or(p, |t| t * p));
            pointer_at = pointer;
            let cat = Catalog { visual: &all, text_fns: text_fns.clone() };
            // Links (last frame's pin positions), under the nodes.
            for l in &vs.links {
                let (Some(a), Some(b)) = (st.pins.get(&(l.from, l.out.clone(), true)), st.pins.get(&(l.to, l.input.clone(), false))) else { continue };
                let kind = vs.node(l.from).and_then(|n| cat.pins(&vs, n).outputs.into_iter().find(|p| p.name == l.out)).map_or(PinType::Any, |p| p.kind);
                bezier(ui.painter(), *a, *b, pin_color(kind), if kind == PinType::Flow { 3.0 } else { 2.0 });
            }
            if let (Some(d), Some(p)) = (&st.drag, pointer) {
                if let Some(from) = st.pins.get(&(d.node, d.pin.clone(), d.output)) {
                    let (a, b) = if d.output { (*from, p) } else { (p, *from) };
                    bezier(ui.painter(), a, b, pin_color(d.kind), 2.0);
                }
            }
            // Nodes.
            for n in vs.nodes.iter_mut() {
                let pins = cat.pins(&all[k], n);
                let pos = pos2(n.pos[0], n.pos[1]);
                let selected = st.selected.contains(&n.id);
                let acc = accent(n, &cat, &all[k]);
                let fill = if matches!(n.kind, NodeKind::Comment { .. }) { Color32::from_rgb(60, 56, 36) } else { Color32::from_rgb(40, 44, 52) };
                let stroke = if selected { Stroke::new(2.5_f32, Color32::YELLOW) } else { Stroke::new(1.0_f32, acc.gamma_multiply(0.6)) };
                let id = egui::Id::new(("vsnode", n.id));
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
                        let count = pins.inputs.len().max(pins.outputs.len());
                        for row in 0..count {
                            let r = ui.horizontal(|ui| {
                                ui.set_min_height(20.0);
                                if let Some(p) = pins.inputs.get(row) {
                                    let linked = all[k].link_into(n.id, &p.name).is_some();
                                    ui.label(egui::RichText::new(&p.name).size(13.0).color(Color32::from_gray(200)));
                                    if p.kind != PinType::Flow && !linked {
                                        let mut v = n.values.get(&p.name).cloned();
                                        let empty = Vec::new();
                                        let o = p.arg.and_then(|a| opts.iter().find(|o| o.0 == a)).map_or(&empty, |o| &o.1);
                                        value_field(ui, id.with(&p.name), p.kind, p.arg, &mut v, o);
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
                        if r.drag_started() {
                            st.drag = Some(LinkDrag { node: n.id, pin: p.name.clone(), output: true, kind: p.kind });
                        }
                    }
                }
                let drag = ui.interact(head.expand2(vec2(12.0, 2.0)), id.with("move"), Sense::click_and_drag());
                if drag.dragged() {
                    moved.push((n.id, drag.drag_delta()));
                }
                if drag.drag_started() && !selected {
                    click_node = Some((n.id, ui.input(|i| i.modifiers.command)));
                }
                if drag.clicked() {
                    click_node = Some((n.id, ui.input(|i| i.modifiers.command)));
                }
                if drag.double_clicked() {
                    if let NodeKind::Call { function, .. } = &n.kind {
                        if all.iter().any(|v| v.id == *function) {
                            st.open = Some(function.clone());
                        }
                    }
                }
                drag.on_hover_cursor(egui::CursorIcon::Grab);
            }
            // A link dropped: onto a pin, or into empty space (the palette, to add a linked node).
            if let Some(d) = st.drag.clone() {
                if !ui.input(|i| i.pointer.any_down()) {
                    st.drag = None;
                    if let Some(p) = pointer {
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
            bg_click = true;
        }
        if bg.secondary_clicked() {
            if let Some(p) = pointer_at {
                open_palette = Some((p, None));
            }
        }
        let st = &mut self.nodes;
        st.scene_rect = scene_rect;
        st.pins = new_pins;
        st.to_scene = transform;
        if bg_click {
            st.selected.clear();
        }
        if let Some((id, add)) = click_node {
            if add {
                if !st.selected.remove(&id) {
                    st.selected.insert(id);
                }
            } else {
                st.selected = BTreeSet::from([id]);
            }
        }
        for (id, d) in moved {
            let ids: Vec<u32> = if st.selected.contains(&id) { st.selected.iter().copied().collect() } else { vec![id] };
            for n in vs.nodes.iter_mut().filter(|n| ids.contains(&n.id)) {
                n.pos[0] += d.x;
                n.pos[1] += d.y;
            }
        }
        if let Some((at, connect)) = open_palette {
            st.palette = Some(Palette { at, connect, ..Default::default() });
        }
        if let Some(l) = add_link {
            let cat = Catalog { visual: &all, text_fns };
            connect(&mut vs, l, &cat);
        }
        self.put_vs(k, vs);
    }

    /// The palette window: pick a node to add (linked to a dropped link).
    fn nodes_palette(&mut self, ctx: &egui::Context) {
        let Some(mut pal) = self.nodes.palette.take() else { return };
        let Some(k) = self.vs_index() else { return };
        let all = self.project.db.visual_scripts.clone();
        let entries = palette_entries(&all, &self.nodes.text_fns, &all[k].id);
        let text_fns = self.nodes.text_fns.clone();
        let mut pick: Option<Entry> = None;
        let mut open = true;
        let screen = ctx.input(|i| i.pointer.hover_pos()).unwrap_or(self.nodes.canvas.center());
        egui::Window::new("Add node").id(egui::Id::new("vs_palette")).open(&mut open).collapsible(false).resizable(false).default_pos(screen).default_width(320.0).show(ctx, |ui| {
            let r = ui.add(egui::TextEdit::singleline(&mut pal.search).hint_text("search").desired_width(300.0));
            if !pal.focused {
                r.request_focus();
                pal.focused = true;
            }
            let q = pal.search.to_lowercase();
            let fits = |e: &Entry| -> bool {
                let Some(d) = &pal.connect else { return true };
                let probe = VisualScript { nodes: vec![Node { id: 1, kind: e.kind.clone(), pos: [0.0, 0.0], values: BTreeMap::new() }], ..all[k].clone() };
                let cat = Catalog { visual: &all, text_fns: text_fns.clone() };
                let pins = cat.pins(&probe, &probe.nodes[0]);
                let side = if d.output { pins.inputs } else { pins.outputs };
                side.iter().any(|p| if d.output { d.kind.fits(p.kind) } else { p.kind.fits(d.kind) })
            };
            // (Names that start with the search first, then names that have it, then the rest.)
            let rank = |e: &Entry| -> Option<u8> {
                let l = e.label.to_lowercase();
                if q.is_empty() || l == q {
                    Some(0)
                } else if l.starts_with(&q) {
                    Some(1)
                } else if l.contains(&q) {
                    Some(2)
                } else if e.section.to_lowercase().contains(&q) || e.doc.to_lowercase().contains(&q) {
                    Some(3)
                } else {
                    None
                }
            };
            let mut shown: Vec<(u8, &Entry)> = entries.iter().filter_map(|e| rank(e).filter(|_| fits(e)).map(|r| (r, e))).collect();
            if !q.is_empty() {
                shown.sort_by_key(|(r, _)| *r);
            }
            let shown: Vec<&Entry> = shown.into_iter().map(|(_, e)| e).collect();
            let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
            if enter {
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
            });
        });
        if let Some(e) = pick {
            self.nodes_add(e.kind, pal.at, pal.connect.clone());
        } else if open {
            self.nodes.palette = Some(pal);
        }
    }

    /// Adds a node of `kind` at `at` (canvas), linked to a dropped link; returns its id.
    pub fn nodes_add(&mut self, kind: NodeKind, at: Pos2, link: Option<LinkDrag>) -> Option<u32> {
        let k = self.vs_index()?;
        let all = self.project.db.visual_scripts.clone();
        let mut vs = all[k].clone();
        let id = vs.next_id();
        // (A count starts at 1: giving or taking none is rarely meant.)
        let mut values = BTreeMap::new();
        if let NodeKind::Call { function, .. } = &kind {
            if API.iter().any(|f| f.name == function.as_str() && f.params.iter().any(|p| p.0 == "count")) {
                values.insert("count".to_string(), Value::Num(1.0));
            }
        }
        vs.nodes.push(Node { id, kind, pos: [at.x, at.y], values });
        if let Some(d) = link {
            let cat = Catalog { visual: &all, text_fns: self.nodes.text_fns.clone() };
            let node = vs.node(id).cloned()?;
            let pins = cat.pins(&vs, &node);
            let side = if d.output { pins.inputs } else { pins.outputs };
            if let Some(p) = side.iter().find(|p| if d.output { d.kind.fits(p.kind) } else { p.kind.fits(d.kind) }) {
                let l = if d.output { Link { from: d.node, out: d.pin, to: id, input: p.name.clone() } } else { Link { from: id, out: p.name.clone(), to: d.node, input: d.pin } };
                connect(&mut vs, l, &cat);
            }
        }
        self.nodes.selected = BTreeSet::from([id]);
        self.put_vs(k, vs);
        Some(id)
    }

    /// `status` for the editor script: `nodes=ID:N/M:P` (nodes, links, problems) or `-`.
    pub fn nodes_status(&self) -> String {
        let Some(vs) = self.project.db.visual_scripts.iter().find(|v| v.id == self.nodes.sel) else { return "-".into() };
        let cat = Catalog { visual: &self.project.db.visual_scripts, text_fns: self.nodes.text_fns.clone() };
        let (_, p) = visual::function_code(vs, &cat);
        format!("{}:{}/{}:{}{}", vs.id, vs.nodes.len(), vs.links.len(), p.len(), if self.nodes.palette.is_some() { ":palette" } else { "" })
    }

    /// The selected visual script's Rhai and problems (editor script `vscode`).
    pub fn nodes_code(&mut self) -> String {
        self.refresh_text_fns();
        let Some(vs) = self.project.db.visual_scripts.iter().find(|v| v.id == self.nodes.sel) else { return "no visual script selected".into() };
        let cat = Catalog { visual: &self.project.db.visual_scripts, text_fns: self.nodes.text_fns.clone() };
        let (code, problems) = visual::function_code(vs, &cat);
        let mut s = code;
        for p in problems {
            s.push_str(&format!("! {}{}\n", p.node.map_or(String::new(), |n| format!("node {n}: ")), p.text));
        }
        s
    }

    /// Editor script `vsadd KIND X Y`: a node of a kind written as `if`, `repeat`, `value:num`,
    /// `math:add`, `compare:gt`, `set:NAME`, `call:FUNCTION`…
    pub fn nodes_add_named(&mut self, kind: &str, x: f32, y: f32) -> Option<u32> {
        let (word, arg) = kind.split_once(':').unwrap_or((kind, ""));
        let k = match word {
            "start" => NodeKind::Start,
            "return" => NodeKind::Return,
            "if" => NodeKind::If,
            "sequence" => NodeKind::Sequence { steps: arg.parse().unwrap_or(2) },
            "repeat" => NodeKind::Repeat,
            "foreach" => NodeKind::ForEach,
            "value" => NodeKind::Value { kind: match arg {
                "num" => PinType::Num,
                "bool" => PinType::Bool,
                "list" => PinType::List,
                _ => PinType::Text,
            } },
            "math" => NodeKind::Math { op: MathOp::ALL.into_iter().find(|o| format!("{o:?}").eq_ignore_ascii_case(arg)).unwrap_or(MathOp::Add) },
            "compare" => NodeKind::Compare { op: CmpOp::ALL.into_iter().find(|o| format!("{o:?}").eq_ignore_ascii_case(arg)).unwrap_or(CmpOp::Eq) },
            "and" => NodeKind::Logic { op: LogicOp::And },
            "or" => NodeKind::Logic { op: LogicOp::Or },
            "not" => NodeKind::Not,
            "join" => NodeKind::Join,
            "set" => NodeKind::SetLocal { name: arg.into() },
            "get" => NodeKind::GetLocal { name: arg.into() },
            "comment" => NodeKind::Comment { text: arg.into() },
            "call" => NodeKind::Call { function: arg.into(), arity: 0 },
            _ => return None,
        };
        self.nodes_add(k, pos2(x, y), None)
    }

    /// Editor script `vslink FROM OUT TO IN`.
    pub fn nodes_link(&mut self, from: u32, out: &str, to: u32, input: &str) {
        let Some(k) = self.vs_index() else { return };
        let all = self.project.db.visual_scripts.clone();
        let mut vs = all[k].clone();
        let cat = Catalog { visual: &all, text_fns: self.nodes.text_fns.clone() };
        connect(&mut vs, Link { from, out: out.into(), to, input: input.into() }, &cat);
        self.put_vs(k, vs);
    }

    /// Editor script `vsval NODE INPUT VALUE`: a value typed into an input (by its type).
    pub fn nodes_value(&mut self, node: u32, input: &str, value: &str) {
        let Some(k) = self.vs_index() else { return };
        let all = self.project.db.visual_scripts.clone();
        let mut vs = all[k].clone();
        let cat = Catalog { visual: &all, text_fns: self.nodes.text_fns.clone() };
        let Some(n) = vs.node(node).cloned() else { return };
        let kind = cat.pins(&vs, &n).inputs.into_iter().find(|p| p.name == input).map_or(PinType::Any, |p| p.kind);
        let v = match kind {
            PinType::Num => value.parse().ok().map(Value::Num),
            PinType::Bool => Some(Value::Bool(value == "true")),
            PinType::Text => Some(Value::Text(value.into())),
            _ => parse_any(value),
        };
        if let Some(n) = vs.nodes.iter_mut().find(|n| n.id == node) {
            match v {
                Some(v) => {
                    n.values.insert(input.into(), v);
                }
                None => {
                    n.values.remove(input);
                }
            }
        }
        self.put_vs(k, vs);
    }
}

/// Adds link `l` if its pins exist and fit (types, an output to an input), replacing what the
/// input (or a flow output) had.
fn connect(vs: &mut VisualScript, l: Link, cat: &Catalog) {
    let (Some(a), Some(b)) = (vs.node(l.from).cloned(), vs.node(l.to).cloned()) else { return };
    if l.from == l.to {
        return;
    }
    let out = cat.pins(vs, &a).outputs.into_iter().find(|p| p.name == l.out);
    let inp = cat.pins(vs, &b).inputs.into_iter().find(|p| p.name == l.input);
    let (Some(out), Some(inp)) = (out, inp) else { return };
    if !out.kind.fits(inp.kind) {
        return;
    }
    if inp.kind == PinType::Flow {
        // A flow output leads to one node; a flow input takes many.
        vs.links.retain(|x| !(x.from == l.from && x.out == l.out));
    } else {
        vs.links.retain(|x| !(x.to == l.to && x.input == l.input));
    }
    vs.links.push(l);
}

/// The widgets in a node's title row: the operator, the steps, the variable's name, the note.
fn header_widgets(ui: &mut egui::Ui, n: &mut Node) {
    let id = egui::Id::new(("vshead", n.id));
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
            ui.add(egui::TextEdit::singleline(name).desired_width(80.0).id(id));
        }
        NodeKind::Call { function, arity } if !API.iter().any(|f| f.name == function.as_str()) => {
            ui.add(egui::DragValue::new(arity).range(0..=9).prefix("inputs ")).on_hover_text("How many values a text script function takes");
        }
        NodeKind::Value { kind } => {
            let mut v = n.values.get("value").cloned();
            value_field(ui, id, *kind, None, &mut v, &[]);
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

/// The type combo of a parameter.
fn kind_combo(ui: &mut egui::Ui, id: impl std::hash::Hash, kind: &mut PinType) {
    egui::ComboBox::from_id_salt(id).width(90.0).selected_text(kind.label()).show_ui(ui, |ui| {
        for k in PinType::VALUES {
            ui.selectable_value(kind, k, k.label());
        }
    });
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_palette_groups_the_api_by_the_manuals_sections() {
        let e = palette_entries(&[], &[("greet".into(), 1)], "");
        let find = |l: &str| e.iter().find(|x| x.label == l).map(|x| x.section.clone());
        assert_eq!(find("give_item").as_deref(), Some("Inventory"));
        assert_eq!(find("If").as_deref(), Some("Flow"));
        assert_eq!(find("greet").as_deref(), Some("Script functions"));
        assert!(e.iter().all(|x| !API.iter().any(|f| f.name == x.label && f.deprecated.is_some())), "no deprecated functions");
        let more: Vec<&str> = e.iter().filter(|x| x.section == "More").map(|x| x.label.as_str()).collect();
        assert!(more.is_empty(), "every API function has a manual section: {more:?}");
    }

    #[test]
    fn links_replace_what_an_input_had_and_check_types() {
        let cat = Catalog::new(&[]);
        let mut vs = VisualScript::fresh("t");
        vs.nodes.push(Node { id: 2, kind: NodeKind::Call { function: "give_xp".into(), arity: 0 }, pos: [0.0, 0.0], values: BTreeMap::new() });
        vs.nodes.push(Node { id: 3, kind: NodeKind::Value { kind: PinType::Num }, pos: [0.0, 0.0], values: BTreeMap::new() });
        vs.nodes.push(Node { id: 4, kind: NodeKind::Value { kind: PinType::Bool }, pos: [0.0, 0.0], values: BTreeMap::new() });
        connect(&mut vs, Link { from: 1, out: "out".into(), to: 2, input: "in".into() }, &cat);
        connect(&mut vs, Link { from: 4, out: "value".into(), to: 2, input: "amount".into() }, &cat);
        assert_eq!(vs.links.len(), 1, "yes / no doesn't go into a number");
        connect(&mut vs, Link { from: 3, out: "value".into(), to: 2, input: "amount".into() }, &cat);
        connect(&mut vs, Link { from: 3, out: "value".into(), to: 2, input: "amount".into() }, &cat);
        assert_eq!(vs.links.len(), 2, "an input takes one link");
    }
}
