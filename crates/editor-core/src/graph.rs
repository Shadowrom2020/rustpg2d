//! A map's events as a graph, without its window (0.35's Events mode). Owners (the map, zones,
//! placed things) first, their events next to them, the script functions they run, and the
//! maps they lead to. Edges are what the data implies:
//! a trigger running a script function, *Emit* reaching the triggers listening for that
//! event, a choice / *Set variable* / *Set flag* reaching the conditions and scripts that test
//! it, teleports and portals leading to maps. Nothing new is stored in map files: the graph is
//! computed, and where its cards lie is the window's business.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::path::PathBuf;

use rustpg2d_engine::defs::Database;
use rustpg2d_engine::entity::EntityKind;
use rustpg2d_engine::events::{Action, Cond, Condition, Trigger, TriggerOn};
use rustpg2d_engine::map::{Map, ZoneKind};
use rustpg2d_engine::refactor::RefKind;
use rustpg2d_engine::script_refs;

use crate::workspace::Workspace;

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
    /// Its column, left to right: who, their events, script functions, maps.
    pub fn column(&self) -> usize {
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
    pub const ALL: [EdgeKind; 5] = [EdgeKind::Owns, EdgeKind::Calls, EdgeKind::Event, EdgeKind::State, EdgeKind::Travel];

    /// For scripts and dumps: `owns`, `calls`, `event`, `state`, `travel`.
    pub fn name(self) -> &'static str {
        match self {
            EdgeKind::Owns => "owns",
            EdgeKind::Calls => "calls",
            EdgeKind::Event => "event",
            EdgeKind::State => "state",
            EdgeKind::Travel => "travel",
        }
    }

    /// For the legend: what a line of this kind means.
    pub fn label(self) -> &'static str {
        match self {
            EdgeKind::Owns => "has the event",
            EdgeKind::Calls => "runs a script function",
            EdgeKind::Event => "raises an event",
            EdgeKind::State => "sets what is tested",
            EdgeKind::Travel => "leads to a map",
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
            let at: HashMap<String, usize> = crate::search::script_functions(text).into_iter().collect();
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
                    // (A speaker worked out at run time: the line alone.)
                    info.lines.push(match a.siblings.len() {
                        n if n >= 2 && !s(0).is_empty() => format!("{}: {}", s(0), text(1)),
                        n if n >= 2 => text(1),
                        _ => text(0),
                    });
                }
                "choice" if first_of_call => {
                    info.lines.push(format!("? {}", clip(&s(0), 80)));
                    if let Some(l) = lists.iter().find(|l| l.func == "choice" && l.line == a.line) {
                        for (k, o) in l.values.iter().enumerate() {
                            info.lines.push(format!("   {k} -> {o}"));
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
            Action::Check { then, otherwise, .. } | Action::GroupCheck { then, otherwise, .. } | Action::Contest { then, otherwise, .. } => {
                for a in then.iter().chain(otherwise) {
                    self.action(key, row, a);
                }
            }
            Action::Roll { var, .. } if !var.is_empty() => self.writes.push((key.clone(), r, format!("var:{var}"))),
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

impl Owner {
    /// For scripts and dumps: `map`, `zone 4`, `entity 12`.
    pub fn name(self) -> String {
        match self {
            Owner::Map => "map".into(),
            Owner::Zone(uid) => format!("zone {uid}"),
            Owner::Entity(uid) => format!("entity {uid}"),
        }
    }
}

impl NodeKey {
    /// For scripts and dumps: `owner entity 12`, `trigger entity 12 #0` (`def#0`: one of its
    /// definition's), `fn open_gate`, `map house`.
    pub fn name(&self) -> String {
        match self {
            NodeKey::Owner(o) => format!("owner {}", o.name()),
            NodeKey::Trigger { owner, def, index } => format!("trigger {} {}#{index}", owner.name(), if *def { "def" } else { "" }),
            NodeKey::Func(f) => format!("fn {f}"),
            NodeKey::Map(m) => format!("map {m}"),
        }
    }
}

/// The events an owner has itself (`def` = false) or from its definition.
pub fn triggers_of<'a>(map: &'a Map, db: &'a Database, owner: Owner, def: bool) -> &'a [Trigger] {
    match (owner, def) {
        (Owner::Map, false) => &map.triggers,
        (Owner::Zone(uid), false) => map.zones.iter().find(|z| z.uid == uid).map_or(&[], |z| &z.triggers[..]),
        (Owner::Entity(uid), false) => map.entities.iter().find(|e| e.uid == uid).map_or(&[], |e| &e.triggers[..]),
        (Owner::Entity(uid), true) => map.entities.iter().find(|e| e.uid == uid).map_or(&[], |e| db.def_triggers(e.kind, &e.def)),
        _ => &[],
    }
}

/// What a card of the graph says: its heading, what it is, and its lines.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Card {
    /// "Elder", "When the player uses it or talks to it", "open_gate()", "house".
    pub title: String,
    /// "Character", "Portal zone", "The map", "Event", "Event of its definition", "Script function", "Map".
    pub kind: String,
    pub lines: Vec<String>,
    /// The line an edge's `row` 0 starts at (an event's actions come after its conditions).
    pub first_row: usize,
}

/// A map's events as a graph: the nodes in column order, the edges between them, and what each
/// card says.
#[derive(Clone, Debug, Default)]
pub struct Graph {
    pub map: String,
    pub nodes: Vec<NodeKey>,
    pub edges: Vec<Edge>,
    pub cards: HashMap<NodeKey, Card>,
    /// Where each script function is: its file and the character it starts at.
    pub places: HashMap<String, (PathBuf, usize)>,
}

impl Graph {
    /// One line a node (`trigger entity 12 #0 | When …`) and one an edge other than *owns*
    /// (`calls trigger entity 12 #0 [row 1] -> fn open_gate`).
    pub fn dump(&self) -> Vec<String> {
        let nodes = self.nodes.iter().map(|k| {
            let card = self.cards.get(k).cloned().unwrap_or_default();
            let mut l = format!("{} | {}: {}", k.name(), card.kind, card.title);
            if !card.lines.is_empty() {
                l += &format!(" | {}", card.lines.join(" | "));
            }
            l
        });
        let edges = self.edges.iter().filter(|e| e.kind != EdgeKind::Owns).map(|e| {
            let row = e.row.map_or(String::new(), |r| format!(" [row {r}]"));
            let label = if e.label.is_empty() { String::new() } else { format!(" ({})", e.label) };
            format!("{} {}{row} -> {}{label}", e.kind.name(), e.from.name(), e.to.name())
        });
        nodes.chain(edges).collect()
    }
}

/// A placed thing's name: its own, else its definition's.
fn thing_title(db: &Database, kind: EntityKind, def: &str, own: &str) -> (String, &'static str) {
    let (name, label) = match kind {
        EntityKind::Character => (db.character(def).map(|d| d.name.clone()), "Character"),
        EntityKind::Prop => (db.prop(def).map(|d| d.name.clone()), "Prop"),
        EntityKind::Item => (db.item(def).map(|d| d.name.clone()), "Item"),
    };
    let name = name.filter(|n| !n.is_empty()).unwrap_or_else(|| def.to_string());
    (if own.is_empty() || own.eq_ignore_ascii_case(&name) { name } else { format!("{own} ({name})") }, label)
}

/// The cards of the graph's nodes.
fn cards(map: &Map, db: &Database, fns: &[FnInfo], nodes: &[NodeKey]) -> HashMap<NodeKey, Card> {
    // (Definitions' names for the lines, read once a kind.)
    let named: RefCell<HashMap<RefKind, HashMap<String, String>>> = RefCell::default();
    let names = |kind: RefKind, id: &str| -> Option<String> {
        let mut named = named.borrow_mut();
        let list = named.entry(kind).or_insert_with(|| crate::defs::list(db, kind).into_iter().collect());
        list.get(id).filter(|n| !n.is_empty()).cloned()
    };
    let json = |v: &dyn erased::Json| v.json();
    nodes
        .iter()
        .map(|k| {
            let card = match k {
                NodeKey::Owner(Owner::Map) => Card { title: map.name.clone(), kind: "The map".into(), ..Default::default() },
                NodeKey::Owner(Owner::Zone(uid)) => {
                    let z = map.zones.iter().find(|z| z.uid == *uid);
                    let target = z.and_then(|z| match &z.kind {
                        ZoneKind::Portal { target_map, .. } | ZoneKind::Transition { target_map, .. } if !target_map.is_empty() => Some(format!("Leads to {target_map}")),
                        _ => None,
                    });
                    let title = z.map_or_else(|| format!("zone {uid}"), |z| if z.name.is_empty() { format!("{} {uid}", z.kind.label()) } else { z.name.clone() });
                    Card { title, kind: z.map_or("Zone".into(), |z| format!("{} zone", z.kind.label())), lines: target.into_iter().collect(), first_row: 0 }
                }
                NodeKey::Owner(Owner::Entity(uid)) => {
                    let (title, kind) = map.entities.iter().find(|e| e.uid == *uid).map_or((format!("thing {uid}"), "Thing"), |e| thing_title(db, e.kind, &e.def, &e.name));
                    Card { title, kind: kind.into(), ..Default::default() }
                }
                NodeKey::Trigger { owner, def, index } => match triggers_of(map, db, *owner, *def).get(*index) {
                    Some(t) => {
                        let mut lines: Vec<String> = t.conditions.iter().map(|c| format!("If {}", crate::events::condition_summary(&json(c), &names))).collect();
                        let first_row = lines.len();
                        lines.extend(t.actions.iter().map(|a| crate::events::action_summary(&json(a), &names)));
                        let kind = if *def { "Event of its definition" } else { "Event" };
                        Card { title: format!("When {}", crate::events::trigger_summary(&json(&t.on), &names)), kind: kind.into(), lines, first_row }
                    }
                    None => Card { title: "?".into(), kind: "Event".into(), ..Default::default() },
                },
                NodeKey::Func(name) => Card { title: format!("{name}()"), kind: "Script function".into(), lines: fns.iter().find(|f| f.name == *name).map(|f| f.lines.clone()).unwrap_or_default(), first_row: 0 },
                NodeKey::Map(m) => Card { title: m.clone(), kind: "Map".into(), ..Default::default() },
            };
            (k.clone(), card)
        })
        .collect()
}

/// (The summaries read events as JSON, as the event sheet's rows do.)
mod erased {
    pub trait Json {
        fn json(&self) -> serde_json::Value;
    }

    impl<T: serde::Serialize> Json for T {
        fn json(&self) -> serde_json::Value {
            serde_json::to_value(self).unwrap_or_default()
        }
    }
}

impl Workspace {
    /// The events graph of `map`: as it is being edited when it is open, else from its file; the
    /// script functions from the project's script files as they are saved. `all_fns`: every
    /// function (else those the map reaches); `focus`: only what one owner leads to.
    pub fn events_graph(&self, map: &str, all_fns: bool, focus: Option<Owner>) -> Result<Graph, String> {
        let open = self.open_maps().into_iter().find(|d| d.map.name == map).map(|d| &d.map);
        let loaded;
        let m = match open {
            Some(m) => m,
            None => {
                loaded = self.project.load_map(map).map_err(|e| format!("the map {map}: {e:#}"))?;
                &loaded
            }
        };
        let files: Vec<(PathBuf, String)> = self.project.script_files().into_iter().filter_map(|p| std::fs::read_to_string(&p).ok().map(|t| (p, t))).collect();
        let fns = script_info(&files);
        let (nodes, edges) = build(m, &self.project.db, &fns, all_fns, focus);
        let cards = cards(m, &self.project.db, &fns, &nodes);
        let places = fns.iter().map(|f| (f.name.clone(), (f.file.clone(), f.at))).collect();
        Ok(Graph { map: map.to_string(), nodes, edges, cards, places })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustpg2d_engine::entity::EntityInstance;

    fn trig(on: TriggerOn, conditions: Vec<Condition>, actions: Vec<Action>) -> Trigger {
        Trigger { on, conditions, actions, once: false }
    }

    /// An elder whose event asks, runs a script function, raises an event and sets a flag; the
    /// map listens for the event and tests the answer; a zone tests the flag; a script raises
    /// another event; a portal and a teleport lead to maps; a well nothing reaches.
    fn village() -> (Map, u32, u32, u32) {
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
                Action::SetFlag { name: "asked".into(), value: true },
            ],
        )];
        map.entities.push(e);
        let well = map.alloc_uid();
        let mut w = EntityInstance::new(well, EntityKind::Prop, "well", 96.0, 96.0);
        w.triggers = vec![trig(TriggerOn::Interact, vec![], vec![Action::Wait { seconds: 1.0 }])];
        map.entities.push(w);
        map.triggers = vec![
            trig(TriggerOn::Custom { name: "bell".into() }, vec![Condition::new(Cond::Var { name: "help".into(), op: rustpg2d_engine::events::CmpOp::Eq, value: 0 })], vec![Action::Teleport { map: "forest".into(), spawn: "s".into() }]),
            trig(TriggerOn::Custom { name: "lit".into() }, vec![], vec![]),
        ];
        let gate = map.alloc_uid();
        let triggers = vec![trig(TriggerOn::Enter, vec![Condition::new(Cond::Flag { name: "asked".into() })], vec![])];
        map.zones.push(rustpg2d_engine::map::Zone { uid: gate, name: "Gate".into(), x: 1, y: 1, w: 1, h: 1, kind: ZoneKind::Trigger, triggers, loot: None });
        (map, elder, well, gate)
    }

    fn fns() -> Vec<FnInfo> {
        let src = "fn talk() {\n    say(\"Elder\", \"Go east.\");\n    choice(\"Walk?\", [\"Sure\", \"No\"], \"walk\");\n    then_call(\"walk\");\n}\nfn walk() {\n    if get_var(\"walk\") == 0 { emit(\"lit\"); }\n}\nfn unused() {}\n";
        script_info(&[(PathBuf::from("main.rhai"), src.to_string())])
    }

    #[test]
    fn the_edges_follow_the_data_and_focus_keeps_what_one_owner_leads_to() {
        let (map, elder, well, gate) = village();
        let fns = fns();
        let talk = fns.iter().find(|f| f.name == "talk").unwrap();
        assert_eq!(talk.lines, ["Elder: Go east.", "? Walk?", "   0 -> Sure", "   1 -> No", "   answer in var walk", "then walk()"]);
        let db = Database::default();
        let (nodes, edges) = build(&map, &db, &fns, false, None);
        let tr = |owner, index| NodeKey::Trigger { owner, def: false, index };
        let has = |from: &NodeKey, to: &NodeKey, kind| edges.iter().filter(|e| e.from == *from && e.to == *to && e.kind == kind).count();
        let elder_t = tr(Owner::Entity(elder), 0);
        assert_eq!(has(&NodeKey::Owner(Owner::Entity(elder)), &elder_t, EdgeKind::Owns), 1);
        assert_eq!(has(&elder_t, &NodeKey::Func("talk".into()), EdgeKind::Calls), 1);
        assert_eq!(has(&elder_t, &tr(Owner::Map, 0), EdgeKind::Event), 1);
        assert_eq!(has(&elder_t, &tr(Owner::Map, 0), EdgeKind::State), 1, "the choice's variable is tested there");
        assert_eq!(has(&elder_t, &tr(Owner::Zone(gate), 0), EdgeKind::State), 1, "the flag is tested by the zone");
        assert_eq!(has(&NodeKey::Func("talk".into()), &NodeKey::Func("walk".into()), EdgeKind::Calls), 1);
        assert_eq!(has(&NodeKey::Func("walk".into()), &tr(Owner::Map, 1), EdgeKind::Event), 1);
        assert_eq!(has(&tr(Owner::Map, 0), &NodeKey::Map("forest".into()), EdgeKind::Travel), 1);
        // The rows: the elder's event runs `talk` from its second action, raises from its third.
        assert!(edges.iter().any(|e| e.from == elder_t && e.kind == EdgeKind::Calls && e.row == Some(1)));
        assert!(edges.iter().any(|e| e.from == elder_t && e.kind == EdgeKind::Event && e.row == Some(2)));
        assert!(!nodes.contains(&NodeKey::Func("unused".into())));
        let (all, _) = build(&map, &db, &fns, true, None);
        assert!(all.contains(&NodeKey::Func("unused".into())));
        // Focus on the elder: what it leads to; the well, which nothing reaches, is left out.
        let (focus, fe) = build(&map, &db, &fns, false, Some(Owner::Entity(elder)));
        assert!(focus.contains(&NodeKey::Map("forest".into())) && focus.contains(&NodeKey::Owner(Owner::Map)) && focus.contains(&NodeKey::Owner(Owner::Zone(gate))));
        assert!(!focus.contains(&NodeKey::Owner(Owner::Entity(well))) && !focus.contains(&tr(Owner::Entity(well), 0)));
        assert!(fe.iter().all(|e| focus.contains(&e.from) && focus.contains(&e.to)));
        // Columns: who, events, functions, maps, in that order.
        let cols: Vec<usize> = nodes.iter().map(NodeKey::column).collect();
        assert!(nodes.iter().all(|n| n.column() < 4) && cols.contains(&0) && cols.contains(&3));
        // The cards: the events in the event sheet's words, an action's row after its conditions.
        let cards = cards(&map, &db, &fns, &nodes);
        let c = &cards[&tr(Owner::Map, 0)];
        assert_eq!((c.title.as_str(), c.first_row), ("When the event bell is raised", 1));
        assert_eq!(c.lines, ["If help is 0", "Go to forest (s)"]);
        assert_eq!(cards[&NodeKey::Owner(Owner::Entity(elder))].title, "elder", "no definition: its id");
    }

    #[test]
    fn the_village_of_the_example_has_a_graph() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../example_project");
        let ws = Workspace::new(root, crate::Gpu::No).unwrap();
        let g = ws.events_graph("village", false, None).unwrap();
        assert!(g.nodes.len() > 20, "{}", g.nodes.len());
        let dump = g.dump();
        assert!(dump.iter().any(|l| l.starts_with("calls trigger ")), "{dump:#?}");
        assert!(dump.iter().all(|l| !l.contains('\u{2192}')), "no arrows the editor's font lacks");
        // Every node has a card, every edge ends at nodes.
        assert!(g.nodes.iter().all(|n| g.cards.contains_key(n)));
        assert!(g.edges.iter().all(|e| g.nodes.contains(&e.from) && g.nodes.contains(&e.to)));
        if std::env::var_os("RUSTPG2D_GRAPH_DUMP").is_some() {
            println!("{}", dump.join("\n"));
        }
    }
}
