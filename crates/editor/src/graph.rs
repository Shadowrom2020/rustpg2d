//! Events mode: a map's events as a graph. Owners (the map, zones, placed entities) on the
//! left, their triggers next to them (conditions and actions readable at a glance, dialog
//! lines and choice options included; editable in place with the inspector's editors), the
//! script functions they run, and the maps they lead to. Edges are what the data implies:
//! a trigger running a script function, *Emit* reaching the triggers listening for that
//! event, a choice / *Set variable* / *Set flag* reaching the conditions and scripts that test
//! it, teleports and portals leading to maps. Nothing new is stored in map files: node
//! positions only live for the session.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::path::PathBuf;

use egui_macroquad::egui::{self, pos2, vec2, Color32, Pos2, Rect, Sense, Stroke};
use rustpg2d_engine::defs::Database;
use rustpg2d_engine::entity::EntityKind;
use rustpg2d_engine::events::{Action, Cond, Condition, Trigger, TriggerOn};
use rustpg2d_engine::map::{Map, ZoneKind};
use rustpg2d_engine::script_refs;

use crate::app::{Editor, Mode, Selection};
use crate::ui::{self, ph};

/// Who a trigger belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Owner {
    Map,
    Zone(u32),
    Entity(u32),
}

/// A node of the graph.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NodeKey {
    Owner(Owner),
    /// Trigger `index` of an owner; `def` = from the entity's definition (read-only here).
    Trigger { owner: Owner, def: bool, index: usize },
    Func(String),
    Map(String),
}

impl NodeKey {
    fn column(&self) -> usize {
        match self {
            NodeKey::Owner(_) => 0,
            NodeKey::Trigger { .. } => 1,
            NodeKey::Func(_) => 2,
            NodeKey::Map(_) => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeKind {
    /// Owner → its trigger.
    Owns,
    /// Runs a script function (Run script, cut-scene, aim, fly, script condition, calls).
    Calls,
    /// *Emit* / `emit()` → triggers on that custom event.
    Event,
    /// Sets a flag or variable → conditions / scripts that test it.
    State,
    /// Teleport / portal / transition → a map.
    Travel,
}

impl EdgeKind {
    fn color(self) -> Color32 {
        match self {
            EdgeKind::Owns => Color32::from_gray(120),
            EdgeKind::Calls => Color32::from_rgb(110, 210, 120),
            EdgeKind::Event => Color32::from_rgb(255, 170, 60),
            EdgeKind::State => Color32::from_rgb(90, 200, 240),
            EdgeKind::Travel => Color32::from_rgb(200, 130, 255),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Edge {
    pub from: NodeKey,
    /// The action row (of a trigger) or summary line (of a function) the edge starts at.
    pub row: Option<usize>,
    pub to: NodeKey,
    pub kind: EdgeKind,
    pub label: String,
}

/// What the graph needs to know about a script function.
#[derive(Clone, Debug, Default)]
pub struct FnInfo {
    pub name: String,
    pub file: PathBuf,
    /// Character offset of its definition in the file (for the script editor's jump).
    pub at: usize,
    /// One line per notable call, in order: dialog lines, choices and their options, events,
    /// flags / variables set, teleports, follow-up calls.
    pub lines: Vec<String>,
    pub calls: BTreeSet<String>,
    /// (line index, event name) of `emit()` calls.
    pub emits: Vec<(usize, String)>,
    /// (line index, "flag:x" / "var:x") written.
    pub writes: Vec<(usize, String)>,
    /// "flag:x" / "var:x" read.
    pub reads: BTreeSet<String>,
    /// (line index, map) of `teleport()` calls.
    pub teleports: Vec<(usize, String)>,
}

fn clip(s: &str, n: usize) -> String {
    let s = s.replace('\n', " ");
    if s.chars().count() > n {
        format!("{}…", s.chars().take(n).collect::<String>())
    } else {
        s
    }
}

/// Reads the functions of the project's scripts (`files`: path and text).
pub fn script_info(files: &[(PathBuf, String)]) -> Vec<FnInfo> {
    let fns: Vec<(PathBuf, script_refs::ScriptFn, usize)> = files
        .iter()
        .flat_map(|(p, text)| {
            let at: HashMap<String, usize> = crate::database::script_functions(text).into_iter().collect();
            script_refs::functions(text).into_iter().map(move |f| {
                let a = at.get(&f.name).copied().unwrap_or(0);
                (p.clone(), f, a)
            })
        })
        .collect();
    let names: BTreeSet<String> = fns.iter().map(|f| f.1.name.clone()).collect();
    let mut out = Vec::new();
    for (file, f, at) in fns {
        let mut info = FnInfo { name: f.name.clone(), file, at, calls: script_refs::used_functions(&f, &names), ..Default::default() };
        let args = script_refs::string_args(&f.text);
        let lists = script_refs::list_args(&f.text);
        // Group string arguments by call (same function, same line, same sibling list).
        let mut seen: HashSet<(String, usize, usize)> = HashSet::new();
        for a in &args {
            let first_of_call = seen.insert((a.func.clone(), a.line, a.siblings.len()));
            let s = |i: usize| a.siblings.get(i).cloned().flatten().unwrap_or_default();
            let n = info.lines.len();
            match a.func.as_str() {
                "say" if first_of_call => {
                    // Texts built at run time ("…" + name) show as "…".
                    let text = |i: usize| if s(i).is_empty() { "…".to_string() } else { clip(&s(i), 90) };
                    info.lines.push(if a.siblings.len() >= 2 { format!("{}: {}", s(0), text(1)) } else { text(0) });
                }
                "choice" if first_of_call => {
                    info.lines.push(format!("? {}", clip(&s(0), 80)));
                    if let Some(l) = lists.iter().find(|l| l.func == "choice" && l.line == a.line) {
                        for (k, o) in l.values.iter().enumerate() {
                            info.lines.push(format!("   {k} {} {o}", ph::ARROW_RIGHT));
                        }
                    }
                    let var = s(a.siblings.len().saturating_sub(1));
                    if a.siblings.len() >= 3 && !var.is_empty() {
                        info.lines.push(format!("   answer in var {var}"));
                        info.writes.push((info.lines.len() - 1, format!("var:{var}")));
                    }
                }
                "emit" => {
                    info.lines.push(format!("emit {}", a.value));
                    info.emits.push((n, a.value.clone()));
                }
                "then_call" => info.lines.push(format!("then {}()", a.value)),
                "set_flag" if a.arg == 0 => {
                    info.lines.push(format!("set flag {}", a.value));
                    info.writes.push((n, format!("flag:{}", a.value)));
                }
                "set_var" | "add_var" if a.arg == 0 => {
                    info.lines.push(format!("{} {}", a.func.replace('_', " "), a.value));
                    info.writes.push((n, format!("var:{}", a.value)));
                }
                "flag" if a.arg == 0 => {
                    info.reads.insert(format!("flag:{}", a.value));
                }
                "get_var" if a.arg == 0 => {
                    info.reads.insert(format!("var:{}", a.value));
                }
                "teleport" if a.arg == 0 => {
                    info.lines.push(format!("teleport {} {}", a.value, s(1)));
                    info.teleports.push((n, a.value.clone()));
                }
                "start_quest" | "advance_quest" | "complete_quest" | "fail_quest" | "set_quest_stage" if a.arg == 0 => info.lines.push(format!("{} {}", a.func.replace('_', " "), a.value)),
                "give_item" | "take_item" if a.arg == 0 => info.lines.push(format!("{} {}", a.func.replace('_', " "), a.value)),
                _ => {}
            }
        }
        out.push(info);
    }
    out
}

/// One line summarising an action.
pub fn action_summary(a: &Action) -> String {
    match a {
        Action::Dialog { speaker, text } if speaker.is_empty() => clip(text, 90),
        Action::Dialog { speaker, text } => format!("{speaker}: {}", clip(text, 90)),
        Action::Choice { speaker, text, .. } if speaker.is_empty() => format!("? {}", clip(text, 80)),
        Action::Choice { speaker, text, .. } => format!("? {speaker}: {}", clip(text, 80)),
        Action::GiveItem { item, count } => format!("give {count}× {item}"),
        Action::TakeItem { item, count } => format!("take {count}× {item}"),
        Action::SetFlag { name, value } => format!("flag {name} = {value}"),
        Action::SetVar { name, op, value } => format!("var {name} {} {value}", if matches!(op, rustpg2d_engine::events::VarOp::Add) { "+=" } else { "=" }),
        Action::Teleport { map, spawn } => format!("teleport {map}:{spawn}"),
        Action::Emit { name } => format!("emit {name}"),
        Action::RunScript { function } => format!("run {function}()"),
        Action::Cutscene { function } => format!("cut-scene {function}()"),
        Action::Quest { quest, op, .. } => format!("quest {quest}: {op:?}"),
        Action::SetState { target, state } => format!("{target} {} state {state}", ph::ARROW_RIGHT),
        Action::SetActive { target, active } => format!("{target} {}", if *active { "appears" } else { "disappears" }),
        Action::PlaySound { sound } => format!("sound {sound}"),
        Action::Wait { seconds } => format!("wait {seconds} s"),
        Action::If { conditions, then, otherwise } => format!("if {} ({} then, {} else)", conds_summary(conditions), then.len(), otherwise.len()),
        other => other.label().to_string(),
    }
}

fn cond_summary(c: &Condition) -> String {
    let s = match &c.cond {
        Cond::Flag { name } => format!("flag {name}"),
        Cond::Var { name, op, value } => format!("{name} {} {value}", op.symbol()),
        Cond::HasItem { item, count } => format!("has {count}× {item}"),
        Cond::Script { function } => format!("{function}()"),
        Cond::Quest { quest, check, .. } => format!("quest {quest} {}", check.label()),
        other => other.label().to_string(),
    };
    if c.negate {
        format!("not {s}")
    } else {
        s
    }
}

fn conds_summary(cs: &[Condition]) -> String {
    cs.iter().map(cond_summary).collect::<Vec<_>>().join(", ")
}

/// The options of a choice action, for the option rows under it.
fn choice_options(a: &Action) -> Option<(&[String], &str)> {
    match a {
        Action::Choice { options, var, .. } => Some((options, var)),
        _ => None,
    }
}

/// Owners of the map, in drawing order, with their (instance) triggers and definition triggers.
fn owners<'a>(map: &'a Map, db: &'a Database) -> Vec<(Owner, &'a [Trigger], &'a [Trigger])> {
    let mut v: Vec<(Owner, &[Trigger], &[Trigger])> = Vec::new();
    if !map.triggers.is_empty() {
        v.push((Owner::Map, &map.triggers, &[]));
    }
    for z in &map.zones {
        let travels = matches!(z.kind, ZoneKind::Portal { .. } | ZoneKind::Transition { .. });
        if !z.triggers.is_empty() || travels {
            v.push((Owner::Zone(z.uid), &z.triggers, &[]));
        }
    }
    for e in &map.entities {
        let def = db.def_triggers(e.kind, &e.def);
        if !e.triggers.is_empty() || !def.is_empty() {
            v.push((Owner::Entity(e.uid), &e.triggers, def));
        }
    }
    v
}

type Rows = Vec<(NodeKey, Option<usize>, String)>;

/// What triggers and functions do: (source, row, name) of calls, emitted events, written flags /
/// variables and teleports, and the flags / variables they read.
#[derive(Default)]
struct Facts {
    calls: Rows,
    emits: Rows,
    writes: Rows,
    travels: Rows,
    reads: Vec<(NodeKey, String)>,
}

impl Facts {
    fn action(&mut self, key: &NodeKey, row: usize, a: &Action) {
        let r = Some(row);
        match a {
            Action::RunScript { function } | Action::Cutscene { function } | Action::Aim { function } | Action::Fly { function, .. } if !function.is_empty() => self.calls.push((key.clone(), r, function.clone())),
            Action::Emit { name } => self.emits.push((key.clone(), r, name.clone())),
            Action::Choice { var, .. } if !var.is_empty() => self.writes.push((key.clone(), r, format!("var:{var}"))),
            Action::SetVar { name, .. } => self.writes.push((key.clone(), r, format!("var:{name}"))),
            Action::SetFlag { name, .. } => self.writes.push((key.clone(), r, format!("flag:{name}"))),
            Action::Teleport { map, .. } if !map.is_empty() => self.travels.push((key.clone(), r, map.clone())),
            Action::If { conditions, then, otherwise } => {
                for c in conditions {
                    self.cond(key, c, r);
                }
                // (Nested actions, with the same row: `walk` would lose the row.)
                for a in then.iter().chain(otherwise) {
                    self.action(key, row, a);
                }
            }
            _ => {}
        }
    }

    fn cond(&mut self, key: &NodeKey, c: &Condition, row: Option<usize>) {
        match &c.cond {
            Cond::Flag { name } => self.reads.push((key.clone(), format!("flag:{name}"))),
            Cond::Var { name, .. } => self.reads.push((key.clone(), format!("var:{name}"))),
            Cond::Script { function } if !function.is_empty() => self.calls.push((key.clone(), row, function.clone())),
            _ => {}
        }
    }
}

/// The graph of a map: nodes in column order and the edges between them. `all_fns` shows every
/// script function (else only those reachable from the map); `focus` keeps only what one
/// owner's triggers lead to.
pub fn build(map: &Map, db: &Database, fns: &[FnInfo], all_fns: bool, focus: Option<Owner>) -> (Vec<NodeKey>, Vec<Edge>) {
    let mut nodes: Vec<NodeKey> = Vec::new();
    let mut edges: Vec<Edge> = Vec::new();
    // Every trigger with its key, for listeners and readers.
    let mut triggers: Vec<(NodeKey, &Trigger)> = Vec::new();
    for (owner, inst, def) in owners(map, db) {
        nodes.push(NodeKey::Owner(owner));
        for (d, list) in [(false, inst), (true, def)] {
            for (index, t) in list.iter().enumerate() {
                let key = NodeKey::Trigger { owner, def: d, index };
                edges.push(Edge { from: NodeKey::Owner(owner), row: None, to: key.clone(), kind: EdgeKind::Owns, label: String::new() });
                nodes.push(key.clone());
                triggers.push((key, t));
            }
        }
        if let Owner::Zone(uid) = owner {
            if let Some(z) = map.zones.iter().find(|z| z.uid == uid) {
                if let ZoneKind::Portal { target_map, .. } | ZoneKind::Transition { target_map, .. } = &z.kind {
                    if !target_map.is_empty() {
                        edges.push(Edge { from: NodeKey::Owner(owner), row: None, to: NodeKey::Map(target_map.clone()), kind: EdgeKind::Travel, label: String::new() });
                    }
                }
            }
        }
    }
    // What each trigger does (nested If actions count as their top-level row).
    let mut f = Facts::default();
    for (key, t) in &triggers {
        for c in &t.conditions {
            f.cond(key, c, None);
        }
        for (row, a) in t.actions.iter().enumerate() {
            f.action(key, row, a);
        }
    }
    let Facts { mut calls, mut emits, mut writes, mut travels, mut reads } = f;
    // Script functions: those the map reaches (or all).
    let by_name: BTreeMap<&str, &FnInfo> = fns.iter().map(|f| (f.name.as_str(), f)).collect();
    let mut used: BTreeSet<String> = BTreeSet::new();
    let mut stack: Vec<String> = calls.iter().map(|c| c.2.clone()).collect();
    if all_fns {
        stack.extend(fns.iter().map(|f| f.name.clone()));
    }
    while let Some(n) = stack.pop() {
        if let Some(f) = by_name.get(n.as_str()) {
            if used.insert(n.clone()) {
                stack.extend(f.calls.iter().cloned());
            }
        }
    }
    for n in &used {
        let f = by_name[n.as_str()];
        let key = NodeKey::Func(n.clone());
        nodes.push(key.clone());
        for c in &f.calls {
            calls.push((key.clone(), None, c.clone()));
        }
        for (l, e) in &f.emits {
            emits.push((key.clone(), Some(*l), e.clone()));
        }
        for (l, w) in &f.writes {
            writes.push((key.clone(), Some(*l), w.clone()));
        }
        for r in &f.reads {
            reads.push((key.clone(), r.clone()));
        }
        for (l, m) in &f.teleports {
            travels.push((key.clone(), Some(*l), m.clone()));
        }
    }
    for (from, row, f) in calls {
        if used.contains(&f) {
            edges.push(Edge { from, row, to: NodeKey::Func(f.clone()), kind: EdgeKind::Calls, label: String::new() });
        }
    }
    for (from, row, name) in emits {
        for (key, t) in &triggers {
            if matches!(&t.on, TriggerOn::Custom { name: n } if *n == name) {
                edges.push(Edge { from: from.clone(), row, to: key.clone(), kind: EdgeKind::Event, label: name.clone() });
            }
        }
    }
    let mut state_seen = HashSet::new();
    for (from, row, w) in &writes {
        for (to, r) in &reads {
            if r == w && to != from && state_seen.insert((from.clone(), *row, to.clone())) {
                let label = w.split_once(':').map_or(w.as_str(), |p| p.1).to_string();
                edges.push(Edge { from: from.clone(), row: *row, to: to.clone(), kind: EdgeKind::State, label });
            }
        }
    }
    let mut maps: BTreeSet<String> = travels.iter().map(|t| t.2.clone()).collect();
    for (from, row, m) in travels {
        edges.push(Edge { from, row, to: NodeKey::Map(m), kind: EdgeKind::Travel, label: String::new() });
    }
    for e in edges.iter().filter(|e| e.kind == EdgeKind::Travel) {
        if let NodeKey::Map(m) = &e.to {
            maps.insert(m.clone());
        }
    }
    nodes.extend(maps.into_iter().map(NodeKey::Map));
    // Focus on one owner: what its triggers lead to, plus the owners of reached triggers.
    if let Some(o) = focus {
        let mut keep: HashSet<NodeKey> = HashSet::new();
        let mut queue = VecDeque::from([NodeKey::Owner(o)]);
        while let Some(k) = queue.pop_front() {
            if !keep.insert(k.clone()) {
                continue;
            }
            for e in edges.iter().filter(|e| e.from == k && (e.kind != EdgeKind::Owns || k == NodeKey::Owner(o))) {
                queue.push_back(e.to.clone());
            }
        }
        let owners: Vec<NodeKey> = keep.iter().filter_map(|k| if let NodeKey::Trigger { owner, .. } = k { Some(NodeKey::Owner(*owner)) } else { None }).collect();
        keep.extend(owners);
        nodes.retain(|n| keep.contains(n));
        edges.retain(|e| keep.contains(&e.from) && keep.contains(&e.to) && (e.kind != EdgeKind::Owns || keep.contains(&e.to)));
    }
    (nodes, edges)
}

/// Session state of the Events mode.
pub struct GraphState {
    /// Only what the selected object's triggers lead to.
    pub focus_selection: bool,
    /// Every script function, not only those the map reaches.
    pub all_functions: bool,
    pub scene_rect: Rect,
    /// Nodes the user dragged somewhere (the rest is laid out automatically).
    pub pinned: HashMap<NodeKey, Pos2>,
    /// Trigger nodes showing the full editors.
    pub editing: HashSet<NodeKey>,
    pub selected: Option<NodeKey>,
    /// Last frame's node rectangles and action-row heights (scene coordinates).
    rects: HashMap<NodeKey, Rect>,
    rows: HashMap<(NodeKey, usize), f32>,
    /// The map the layout belongs to.
    map: String,
    /// Script functions, re-read once a second.
    fns: Vec<FnInfo>,
    fns_at: f64,
    /// (nodes, edges) of the last frame, for `status`.
    pub counts: (usize, usize),
    /// Nodes of the last frame in order (editor script `graphnode N`) and its edges.
    pub order: Vec<NodeKey>,
    pub edges: Vec<Edge>,
}

impl Default for GraphState {
    fn default() -> Self {
        GraphState {
            focus_selection: false,
            all_functions: false,
            scene_rect: Rect::ZERO,
            pinned: HashMap::new(),
            editing: HashSet::new(),
            selected: None,
            rects: HashMap::new(),
            rows: HashMap::new(),
            map: String::new(),
            fns: Vec::new(),
            fns_at: -10.0,
            counts: (0, 0),
            order: Vec::new(),
            edges: Vec::new(),
        }
    }
}

const WIDTHS: [f32; 4] = [210.0, 380.0, 330.0, 190.0];
const XS: [f32; 4] = [0.0, 260.0, 720.0, 1110.0];
const GAP: f32 = 18.0;

/// Positions of the nodes that aren't pinned: owners with their triggers side by side, script
/// functions and maps next to what leads to them.
fn layout(nodes: &[NodeKey], edges: &[Edge], sizes: &HashMap<NodeKey, Rect>, pinned: &HashMap<NodeKey, Pos2>) -> HashMap<NodeKey, Pos2> {
    let h = |k: &NodeKey| sizes.get(k).map_or(90.0, |r| r.height());
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
    // Triggers whose owner isn't shown (focus mode keeps them all with owners anyway).
    let loose: Vec<&NodeKey> = nodes.iter().filter(|t| t.column() == 1 && !pos.contains_key(*t)).collect();
    for t in loose {
        pos.insert(t.clone(), pos2(XS[1], y));
        y += h(t) + GAP;
    }
    // Functions and maps want to sit level with what leads to them; a function called by
    // another one just below its caller.
    fn want(n: &NodeKey, edges: &[Edge], placed: &HashMap<NodeKey, Pos2>, memo: &mut HashMap<NodeKey, Option<f32>>, depth: usize) -> Option<f32> {
        if let Some(w) = memo.get(n) {
            return *w;
        }
        if depth > 32 {
            return None;
        }
        memo.insert(n.clone(), None); // cycle guard
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
    placed.extend(pinned.iter().map(|(k, p)| (k.clone(), *p)));
    let mut memo = HashMap::new();
    for col in [2, 3] {
        // Order by where their sources are, then stack without overlap.
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

/// Mutable access to an owner's instance triggers.
fn triggers_mut(map: &mut Map, owner: Owner) -> Option<&mut Vec<Trigger>> {
    match owner {
        Owner::Map => Some(&mut map.triggers),
        Owner::Zone(uid) => map.zones.iter_mut().find(|z| z.uid == uid).map(|z| &mut z.triggers),
        Owner::Entity(uid) => map.entities.iter_mut().find(|e| e.uid == uid).map(|e| &mut e.triggers),
    }
}

/// What a click inside the graph asks for (handled after drawing).
enum Request {
    ShowOnMap(Owner),
    OpenDef(EntityKind, String),
    OpenScript(PathBuf, usize),
    OpenMap(String),
}

impl Editor {
    /// The Events mode (whole window below the menu bar).
    pub fn events_ui(&mut self, ctx: &egui::Context) {
        // The map's shortcuts (the canvas handles them only in Map mode).
        if !ctx.wants_keyboard_input() {
            let (cmd, shift) = ctx.input(|i| (i.modifiers.command, i.modifiers.shift));
            let key = |k| ctx.input(|i| i.key_pressed(k));
            if cmd && key(egui::Key::Z) && !shift {
                if let Some(d) = &mut self.doc {
                    d.undo(&self.assets);
                }
            } else if cmd && (key(egui::Key::Y) || (key(egui::Key::Z) && shift)) {
                if let Some(d) = &mut self.doc {
                    d.redo(&self.assets);
                }
            } else if cmd && key(egui::Key::S) {
                self.save_all();
            }
        }
        egui::TopBottomPanel::top("graph_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(ui::t(ph::GRAPH, "Events of"));
                ui.strong(self.doc.as_ref().map_or("–".to_string(), |d| d.map.name.clone()));
                ui.separator();
                ui.selectable_value(&mut self.graph.focus_selection, false, "Whole map").on_hover_text("Every trigger of the map, its zones and placed objects");
                let sel_label = match self.sel {
                    Selection::Entity(_) | Selection::Zone(_) => "Selected object",
                    _ => "Selected object (select one on the map)",
                };
                ui.selectable_value(&mut self.graph.focus_selection, true, sel_label).on_hover_text("Only what the selected entity's or zone's triggers lead to");
                ui.separator();
                ui.checkbox(&mut self.graph.all_functions, "All script functions").on_hover_text("Also functions this map doesn't run");
                if ui.button(ui::t(ph::ARROWS_CLOCKWISE, "Arrange")).on_hover_text("Lay the nodes out again (forgets where you dragged them)").clicked() {
                    self.graph.pinned.clear();
                    self.graph.scene_rect = Rect::ZERO;
                }
                if ui.button(ui::t(ph::CORNERS_OUT, "Fit")).on_hover_text("Show the whole graph").clicked() {
                    self.graph.scene_rect = Rect::ZERO;
                }
                ui.separator();
                for (k, l) in [(EdgeKind::Calls, "runs script"), (EdgeKind::Event, "event"), (EdgeKind::State, "flag / variable"), (EdgeKind::Travel, "leads to map")] {
                    ui.colored_label(k.color(), format!("— {l}"));
                }
            });
            ui.weak("Drag the background to pan, Ctrl+wheel to zoom, a node's title to move it. The pencil edits a trigger with the inspector's editors (Ctrl+Z undoes).");
        });
        egui::CentralPanel::default().show(ctx, |ui| self.graph_canvas(ui));
    }

    fn graph_canvas(&mut self, ui: &mut egui::Ui) {
        let Some(mut doc) = self.doc.take() else {
            ui.label("No map open.");
            return;
        };
        if self.graph.map != doc.map.name {
            self.graph = GraphState { focus_selection: self.graph.focus_selection, all_functions: self.graph.all_functions, map: doc.map.name.clone(), ..Default::default() };
        }
        let now = macroquad::time::get_time();
        if now - self.graph.fns_at > 1.0 {
            let dir = self.project.scripts_dir();
            let mut files: Vec<PathBuf> = std::fs::read_dir(&dir).map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "rhai")).collect()).unwrap_or_default();
            files.sort();
            // The script editor's unsaved text counts.
            let texts: Vec<(PathBuf, String)> = files
                .into_iter()
                .map(|f| {
                    let t = if self.db.script_file.as_ref() == Some(&f) { self.db.script_text.clone() } else { std::fs::read_to_string(&f).unwrap_or_default() };
                    (f, t)
                })
                .collect();
            self.graph.fns = script_info(&texts);
            self.graph.fns_at = now;
        }
        let focus = if self.graph.focus_selection {
            match self.sel {
                Selection::Entity(u) => Some(Owner::Entity(u)),
                Selection::Zone(u) => Some(Owner::Zone(u)),
                _ => None,
            }
        } else {
            None
        };
        let (nodes, edges) = build(&doc.map, &self.project.db, &self.graph.fns, self.graph.all_functions, focus);
        self.graph.counts = (nodes.len(), edges.len());
        self.graph.order = nodes.clone();
        self.graph.edges = edges.clone();
        let auto = layout(&nodes, &edges, &self.graph.rects, &self.graph.pinned);
        let before = crate::doc::Objects::of(&doc.map);
        let mut requests: Vec<Request> = Vec::new();
        let refs = &self.refs;
        let db = &self.project.db;
        let st = &mut self.graph;
        let fns: BTreeMap<String, FnInfo> = st.fns.iter().map(|f| (f.name.clone(), f.clone())).collect();
        let mut scene_rect = st.scene_rect;
        egui::Scene::new().zoom_range(0.1..=2.0).max_inner_size(vec2(20000.0, 20000.0)).show(ui, &mut scene_rect, |ui| {
            // Edges first (under the nodes), from last frame's rectangles.
            let sel = st.selected.clone();
            for e in &edges {
                let (Some(a), Some(b)) = (st.rects.get(&e.from), st.rects.get(&e.to)) else { continue };
                let y0 = e.row.and_then(|r| st.rows.get(&(e.from.clone(), r))).copied().unwrap_or(a.center().y);
                let p0 = pos2(a.right(), y0.clamp(a.top() + 6.0, a.bottom() - 6.0));
                let p3 = pos2(b.left(), (b.top() + 16.0).min(b.bottom()));
                // Same column: a loop out to the right; backwards: from the left side.
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
                let lit = sel.as_ref().is_none_or(|s| *s == e.from || *s == e.to);
                let mut col = e.kind.color();
                if !lit {
                    col = col.gamma_multiply(0.25);
                }
                let width: f32 = if e.kind == EdgeKind::Owns { 1.5 } else { 2.5 };
                ui.painter().add(egui::epaint::CubicBezierShape::from_points_stroke([p0, c1, c2, p3], false, Color32::TRANSPARENT, Stroke::new(width, col)));
                ui.painter().circle_filled(p3, 3.5, col);
                if !e.label.is_empty() && lit {
                    let mid = if same_col { pos2(p0.x.max(p3.x) + 56.0, (p0.y + p3.y) / 2.0) } else { pos2((p0.x + p3.x) / 2.0, (p0.y + p3.y) / 2.0) };
                    ui.painter().text(mid - vec2(0.0, 8.0), egui::Align2::CENTER_BOTTOM, &e.label, egui::FontId::proportional(13.0), col);
                }
            }
            // Nodes.
            let mut new_rects: HashMap<NodeKey, Rect> = HashMap::new();
            for key in &nodes {
                let pos = st.pinned.get(key).copied().or_else(|| auto.get(key).copied()).unwrap_or(Pos2::ZERO);
                let w = WIDTHS[key.column()];
                let selected = st.selected.as_ref() == Some(key);
                let (fill, accent) = match key {
                    NodeKey::Owner(_) => (Color32::from_rgb(38, 44, 58), Color32::from_rgb(140, 170, 230)),
                    NodeKey::Trigger { def: true, .. } => (Color32::from_rgb(40, 40, 44), Color32::from_gray(150)),
                    NodeKey::Trigger { .. } => (Color32::from_rgb(44, 48, 40), Color32::from_rgb(200, 210, 140)),
                    NodeKey::Func(_) => (Color32::from_rgb(34, 50, 40), EdgeKind::Calls.color()),
                    NodeKey::Map(_) => (Color32::from_rgb(48, 38, 58), EdgeKind::Travel.color()),
                };
                let stroke = if selected { Stroke::new(2.5_f32, Color32::YELLOW) } else { Stroke::new(1.0_f32, accent) };
                let id = egui::Id::new(("graph_node", key));
                let resp = ui.scope_builder(egui::UiBuilder::new().max_rect(Rect::from_min_size(pos, vec2(w, 4000.0))).id_salt(id), |ui| {
                    egui::Frame::new().fill(fill).stroke(stroke).corner_radius(6.0).inner_margin(8.0).show(ui, |ui| {
                        ui.set_width(w - 16.0);
                        node_body(ui, key, &mut doc.map, db, refs, &fns, st, &mut requests)
                    })
                });
                let rect = resp.inner.response.rect;
                new_rects.insert(key.clone(), rect);
                // The title row drags the node; a click selects it.
                let header = resp.inner.inner;
                let drag = ui.interact(header, id.with("drag"), Sense::click_and_drag());
                if drag.dragged() {
                    let p = st.pinned.entry(key.clone()).or_insert(pos);
                    *p += drag.drag_delta();
                }
                if drag.clicked() {
                    st.selected = if selected { None } else { Some(key.clone()) };
                }
                if drag.double_clicked() {
                    if let NodeKey::Map(m) = key {
                        requests.push(Request::OpenMap(m.clone()));
                    }
                }
                drag.on_hover_cursor(egui::CursorIcon::Grab);
            }
            st.rects = new_rects;
        });
        st.scene_rect = scene_rect;
        doc.record_objects(before, "graph");
        self.doc = Some(doc);
        for r in requests {
            match r {
                Request::ShowOnMap(o) => {
                    self.mode = Mode::Map;
                    self.sel = match o {
                        Owner::Entity(u) => Selection::Entity(u),
                        Owner::Zone(u) => Selection::Zone(u),
                        Owner::Map => Selection::None,
                    };
                    self.center_on_selection();
                }
                Request::OpenDef(kind, id) => {
                    let (tab, idx) = match kind {
                        EntityKind::Prop => (crate::database::Tab::Props, self.project.db.props.iter().position(|d| d.id == id)),
                        EntityKind::Character => (crate::database::Tab::Characters, self.project.db.characters.iter().position(|d| d.id == id)),
                        EntityKind::Item => (crate::database::Tab::Items, self.project.db.items.iter().position(|d| d.id == id)),
                    };
                    self.db.tab = tab;
                    self.db.sel = idx.unwrap_or(0);
                    self.mode = Mode::Database;
                }
                Request::OpenScript(f, at) => {
                    self.db.tab = crate::database::Tab::Scripts;
                    self.mode = Mode::Database;
                    self.open_script(&f, Some(at));
                }
                Request::OpenMap(m) => self.goto_map(&m, None),
            }
        }
    }

    /// Centers the map view on the selected entity / zone.
    pub fn center_on_selection(&mut self) {
        let ts = self.ts();
        let Some(doc) = &self.doc else { return };
        let c = match self.sel {
            Selection::Entity(u) => doc.map.entity(u).map(|e| macroquad::math::vec2(e.x, e.y)),
            Selection::Zone(u) => doc.map.zones.iter().find(|z| z.uid == u).map(|z| {
                let r = z.pixel_rect(ts);
                macroquad::math::vec2(r.x + r.w / 2.0, r.y + r.h / 2.0)
            }),
            _ => None,
        };
        if let Some(c) = c {
            self.cam.center = c;
        }
    }

    /// Editor script `graphview N`: zooms the view onto node N (and its neighbours).
    pub fn graph_view_node(&mut self, node: usize) {
        if let Some(r) = self.graph.order.get(node).and_then(|k| self.graph.rects.get(k)) {
            self.graph.scene_rect = r.expand2(vec2(380.0, 160.0));
        }
    }

    /// Editor script `graphtext N K TEXT`: sets the text of action K of trigger node N (a
    /// dialog or choice) the way an edit in the node does (one undo step).
    pub fn graph_set_text(&mut self, node: usize, action: usize, text: &str) -> bool {
        let Some(NodeKey::Trigger { owner, def: false, index }) = self.graph.order.get(node).cloned() else { return false };
        let Some(doc) = &mut self.doc else { return false };
        let before = crate::doc::Objects::of(&doc.map);
        let ok = match triggers_mut(&mut doc.map, owner).and_then(|t| t.get_mut(index)).and_then(|t| t.actions.get_mut(action)) {
            Some(Action::Dialog { text: t, .. } | Action::Choice { text: t, .. }) => {
                *t = text.to_string();
                true
            }
            _ => false,
        };
        doc.record_objects(before, "graph");
        ok
    }

    /// Editor script `graphdump`: the nodes in order with their summaries.
    pub fn graph_dump(&self) -> Vec<String> {
        let Some(doc) = &self.doc else { return Vec::new() };
        self.graph
            .order
            .iter()
            .enumerate()
            .map(|(i, k)| {
                let what = match k {
                    NodeKey::Owner(o) => owner_title(&doc.map, &self.project.db, *o),
                    NodeKey::Trigger { owner, def, index } => {
                        let list: &[Trigger] = if *def { owner_def_triggers(&doc.map, &self.project.db, *owner) } else { owner_triggers(&doc.map, *owner) };
                        list.get(*index).map_or(String::new(), |t| format!("On {} | {}", t.on.label(), t.actions.iter().map(action_summary).collect::<Vec<_>>().join(" | ")))
                    }
                    NodeKey::Func(n) => format!("{n}() | {}", self.graph.fns.iter().find(|f| f.name == *n).map_or(String::new(), |f| f.lines.join(" | "))),
                    NodeKey::Map(m) => format!("map {m}"),
                };
                format!("{i}: {k:?} {what}")
            })
            .chain(self.graph.edges.iter().filter(|e| e.kind != EdgeKind::Owns).map(|e| {
                let idx = |k: &NodeKey| self.graph.order.iter().position(|o| o == k).map_or("?".to_string(), |i| i.to_string());
                format!("edge {} -> {} {:?} {}", idx(&e.from), idx(&e.to), e.kind, e.label)
            }))
            .collect()
    }
}

fn owner_triggers(map: &Map, owner: Owner) -> &[Trigger] {
    match owner {
        Owner::Map => &map.triggers,
        Owner::Zone(uid) => map.zones.iter().find(|z| z.uid == uid).map_or(&[], |z| &z.triggers[..]),
        Owner::Entity(uid) => map.entities.iter().find(|e| e.uid == uid).map_or(&[], |e| &e.triggers[..]),
    }
}

fn owner_def_triggers<'a>(map: &Map, db: &'a Database, owner: Owner) -> &'a [Trigger] {
    match owner {
        Owner::Entity(uid) => map.entities.iter().find(|e| e.uid == uid).map_or(&[], |e| db.def_triggers(e.kind, &e.def)),
        _ => &[],
    }
}

fn owner_title(map: &Map, db: &Database, owner: Owner) -> String {
    match owner {
        Owner::Map => format!("Map {}", map.name),
        Owner::Zone(uid) => map.zones.iter().find(|z| z.uid == uid).map_or("zone".into(), |z| format!("{} zone {}", z.kind.label(), z.name)),
        Owner::Entity(uid) => map.entities.iter().find(|e| e.uid == uid).map_or("entity".into(), |e| {
            let def_name = match e.kind {
                EntityKind::Character => db.character(&e.def).map(|d| d.name.clone()),
                EntityKind::Prop => db.prop(&e.def).map(|d| d.name.clone()),
                EntityKind::Item => db.item(&e.def).map(|d| d.name.clone()),
            }
            .unwrap_or_else(|| e.def.clone());
            if e.name.is_empty() {
                def_name
            } else {
                format!("{} ({def_name})", e.name)
            }
        }),
    }
}

/// Draws a node's contents; returns the title row's rectangle (the drag handle).
#[allow(clippy::too_many_arguments)]
fn node_body(ui: &mut egui::Ui, key: &NodeKey, map: &mut Map, db: &Database, refs: &crate::widgets::Refs, fns: &BTreeMap<String, FnInfo>, st: &mut GraphState, requests: &mut Vec<Request>) -> Rect {
    match key {
        NodeKey::Owner(o) => {
            let icon = match o {
                Owner::Map => ph::MAP_TRIFOLD,
                Owner::Zone(_) => ph::BOUNDING_BOX,
                Owner::Entity(uid) => match map.entities.iter().find(|e| e.uid == *uid).map(|e| e.kind) {
                    Some(EntityKind::Character) => ph::USER,
                    Some(EntityKind::Item) => ph::COIN,
                    _ => ph::CUBE,
                },
            };
            let h = ui.label(egui::RichText::new(ui::t(icon, &owner_title(map, db, *o))).strong()).rect;
            if *o != Owner::Map && ui.small_button(ui::t(ph::CROSSHAIR, "Show on map")).clicked() {
                requests.push(Request::ShowOnMap(*o));
            }
            h
        }
        NodeKey::Trigger { owner, def, index } => {
            let editing = st.editing.contains(key);
            let mut header = Rect::NOTHING;
            if *def {
                let list = owner_def_triggers(map, db, *owner);
                let Some(t) = list.get(*index) else { return Rect::NOTHING };
                header = ui.label(egui::RichText::new(format!("On {}{} · from the definition", t.on.label(), on_detail(&t.on))).strong()).rect;
                summary(ui, key, t, st);
                if let Owner::Entity(uid) = owner {
                    if let Some(e) = map.entities.iter().find(|e| e.uid == *uid) {
                        if ui.small_button(ui::t(ph::DATABASE, "Open in Database")).clicked() {
                            requests.push(Request::OpenDef(e.kind, e.def.clone()));
                        }
                    }
                }
                return header;
            }
            let Some(t) = triggers_mut(map, *owner).and_then(|l| l.get_mut(*index)) else { return Rect::NOTHING };
            ui.horizontal(|ui| {
                header = ui.label(egui::RichText::new(format!("On {}{}{}", t.on.label(), on_detail(&t.on), if t.once { " · once" } else { "" })).strong()).rect;
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.selectable_label(editing, ph::PENCIL_SIMPLE).on_hover_text(if editing { "Back to the summary" } else { "Edit this trigger's conditions and actions" }).clicked() {
                        if editing {
                            st.editing.remove(key);
                        } else {
                            st.editing.insert(key.clone());
                        }
                    }
                });
            });
            if editing {
                let id = format!("graph/{owner:?}/{index}");
                ui.checkbox(&mut t.once, "once");
                ui.label("If all of:");
                crate::widgets::conditions_ui(ui, &id, &mut t.conditions, refs);
                ui.label("Do:");
                crate::widgets::actions_ui(ui, &id, &mut t.actions, refs);
            } else {
                summary(ui, key, t, st);
            }
            header
        }
        NodeKey::Func(name) => {
            let header = ui.label(egui::RichText::new(ui::t(ph::CODE, &format!("{name}()"))).strong()).rect;
            if let Some(f) = fns.get(name) {
                for (i, l) in f.lines.iter().enumerate() {
                    let r = ui.add(egui::Label::new(egui::RichText::new(l).size(13.0)).wrap()).rect;
                    st.rows.insert((key.clone(), i), r.center().y);
                }
                if f.lines.is_empty() {
                    ui.weak("(no dialog, events or flags)");
                }
                let file = f.file.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                if ui.small_button(ui::t(ph::PENCIL_SIMPLE, &format!("Edit in {file}"))).clicked() {
                    requests.push(Request::OpenScript(f.file.clone(), f.at));
                }
            } else {
                ui.colored_label(Color32::LIGHT_RED, "No such script function");
            }
            header
        }
        NodeKey::Map(m) => {
            let header = ui.label(egui::RichText::new(ui::t(ph::SIGN_IN, &format!("Map {m}"))).strong()).rect;
            if ui.small_button(ui::t(ph::ARROW_SQUARE_OUT, "Open")).on_hover_text("Or double-click the title").clicked() {
                requests.push(Request::OpenMap(m.clone()));
            }
            header
        }
    }
}

fn on_detail(on: &TriggerOn) -> String {
    match on {
        TriggerOn::Custom { name } => format!(" \"{name}\""),
        TriggerOn::UseItem { item } if !item.is_empty() => format!(" [{item}]"),
        TriggerOn::HpBelow { percent } => format!(" [{percent} %]"),
        TriggerOn::Hour { hour } => format!(" [{}]", crate::widgets::hour_text(*hour)),
        _ => String::new(),
    }
}

/// The read-only summary of a trigger: conditions, then one row per action (choice options
/// under their choice).
fn summary(ui: &mut egui::Ui, key: &NodeKey, t: &Trigger, st: &mut GraphState) {
    if !t.conditions.is_empty() {
        ui.label(egui::RichText::new(format!("if {}", conds_summary(&t.conditions))).italics().color(Color32::from_rgb(150, 210, 240)));
    }
    for (i, a) in t.actions.iter().enumerate() {
        let r = ui.add(egui::Label::new(egui::RichText::new(format!("{}. {}", i + 1, action_summary(a))).size(13.0)).wrap()).rect;
        st.rows.insert((key.clone(), i), r.center().y);
        if let Some((options, var)) = choice_options(a) {
            for (k, o) in options.iter().enumerate() {
                ui.label(egui::RichText::new(format!("      {k} {} {o}", ph::ARROW_RIGHT)).size(12.5).color(Color32::from_rgb(220, 220, 170)));
            }
            if !var.is_empty() {
                let r = ui.label(egui::RichText::new(format!("      answer in var {var}")).size(12.0).weak()).rect;
                st.rows.insert((key.clone(), i), r.center().y);
            }
        }
    }
    if t.actions.is_empty() {
        ui.weak("(no actions)");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustpg2d_engine::entity::EntityInstance;
    use rustpg2d_engine::events::Trigger;

    fn trig(on: TriggerOn, conditions: Vec<Condition>, actions: Vec<Action>) -> Trigger {
        Trigger { on, conditions, actions, once: false }
    }

    #[test]
    fn edges_follow_the_data() {
        let mut map = Map::new("village", 10, 10);
        let elder = map.alloc_uid();
        let mut e = EntityInstance::new(elder, EntityKind::Character, "elder", 32.0, 32.0);
        e.triggers = vec![trig(
            TriggerOn::Interact,
            vec![],
            vec![
                Action::Choice { speaker: "Elder".into(), text: "Help?".into(), options: vec!["Yes".into(), "No".into()], var: "help".into() },
                Action::RunScript { function: "talk".into() },
                Action::Emit { name: "bell".into() },
            ],
        )];
        map.entities.push(e);
        map.triggers = vec![
            trig(TriggerOn::Custom { name: "bell".into() }, vec![Condition::new(Cond::Var { name: "help".into(), op: rustpg2d_engine::events::CmpOp::Eq, value: 0 })], vec![Action::Teleport { map: "forest".into(), spawn: "s".into() }]),
            trig(TriggerOn::Custom { name: "lit".into() }, vec![], vec![]),
        ];
        let src = "fn talk() {\n    say(\"Elder\", \"Go east.\");\n    choice(\"Walk?\", [\"Sure\", \"No\"], \"walk\");\n    then_call(\"walk\");\n}\nfn walk() {\n    if get_var(\"walk\") == 0 { emit(\"lit\"); }\n}\nfn unused() {}\n";
        let fns = script_info(&[(PathBuf::from("main.rhai"), src.to_string())]);
        let talk = fns.iter().find(|f| f.name == "talk").unwrap();
        let arrow = |k: usize, o: &str| format!("   {k} {} {o}", ph::ARROW_RIGHT);
        assert_eq!(talk.lines, vec!["Elder: Go east.".to_string(), "? Walk?".into(), arrow(0, "Sure"), arrow(1, "No"), "   answer in var walk".into(), "then walk()".into()]);
        let db = Database::default();
        let (nodes, edges) = build(&map, &db, &fns, false, None);
        let tr = |owner, index| NodeKey::Trigger { owner, def: false, index };
        let has = |from: &NodeKey, to: &NodeKey, kind| edges.iter().any(|e| e.from == *from && e.to == *to && e.kind == kind);
        let elder_t = tr(Owner::Entity(elder), 0);
        assert!(has(&NodeKey::Owner(Owner::Entity(elder)), &elder_t, EdgeKind::Owns));
        assert!(has(&elder_t, &NodeKey::Func("talk".into()), EdgeKind::Calls));
        assert!(has(&elder_t, &tr(Owner::Map, 0), EdgeKind::Event));
        assert!(has(&elder_t, &tr(Owner::Map, 0), EdgeKind::State), "the choice's variable is tested there");
        assert!(has(&NodeKey::Func("talk".into()), &NodeKey::Func("walk".into()), EdgeKind::Calls));
        assert!(has(&NodeKey::Func("talk".into()), &NodeKey::Func("walk".into()), EdgeKind::State), "choice var read by get_var");
        assert!(has(&NodeKey::Func("walk".into()), &tr(Owner::Map, 1), EdgeKind::Event));
        assert!(has(&tr(Owner::Map, 0), &NodeKey::Map("forest".into()), EdgeKind::Travel));
        assert!(!nodes.contains(&NodeKey::Func("unused".into())));
        let (all, _) = build(&map, &db, &fns, true, None);
        assert!(all.contains(&NodeKey::Func("unused".into())));
        // Focus on the elder: everything reached from it, but not the unrelated "lit" trigger's
        // owner unless reached (it is, through walk's emit).
        let (focus, fe) = build(&map, &db, &fns, false, Some(Owner::Entity(elder)));
        assert!(focus.contains(&NodeKey::Map("forest".into())) && focus.contains(&NodeKey::Owner(Owner::Map)));
        assert!(fe.iter().all(|e| focus.contains(&e.from) && focus.contains(&e.to)));
        // Layout: every node gets a position, functions right of triggers.
        let pos = layout(&nodes, &edges, &HashMap::new(), &HashMap::new());
        assert_eq!(pos.len(), nodes.len());
        assert!(pos[&NodeKey::Func("talk".into())].x > pos[&elder_t].x);
    }
}
