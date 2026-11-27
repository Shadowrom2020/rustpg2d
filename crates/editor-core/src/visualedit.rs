//! Visual scripts edited as nodes, without the canvas: what the palette offers (the script
//! API grouped by the manual's sections, the project's own functions, flow and value nodes),
//! adding a node (linked to a link dropped into empty space), linking pins by their types,
//! values typed into inputs, grouping nodes into a function, copying them, and the ids an
//! input may take.

use std::collections::{BTreeMap, BTreeSet};

use rustpg2d_engine::script_api::{Arg, API};
use rustpg2d_engine::visual::{self, Catalog, CmpOp, Link, LogicOp, MathOp, Node, NodeKind, PinType, Value, VisualScript};

use crate::workspace::Workspace;

/// A link being dragged out of a pin.
#[derive(Clone, Debug, PartialEq)]
pub struct LinkDrag {
    pub node: u32,
    pub pin: String,
    /// It starts at an output (else an input).
    pub output: bool,
    pub kind: PinType,
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

/// The text a value shows in an input field.
pub fn value_text(v: Option<&Value>) -> String {
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
pub fn parse_any(s: &str) -> Option<Value> {
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

/// Adds link `l` if its pins exist and fit (types, an output to an input), replacing what the
/// input (or a flow output) had.
pub fn connect(vs: &mut VisualScript, l: Link, cat: &Catalog) {
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

/// The palette's entries that fit what is searched for, the best first: names that are the
/// search, start with it, have it, then sections and descriptions that have it. `connect`: a
/// link dropped into empty space, which the new node must be able to take.
pub fn palette_shown<'e>(entries: &'e [Entry], search: &str, connect: Option<&LinkDrag>, vs: &VisualScript, cat: &Catalog) -> Vec<&'e Entry> {
    let q = search.trim().to_lowercase();
    let fits = |e: &Entry| -> bool {
        let Some(d) = connect else { return true };
        let probe = VisualScript { nodes: vec![Node { id: 1, kind: e.kind.clone(), pos: [0.0, 0.0], values: BTreeMap::new() }], ..vs.clone() };
        let pins = cat.pins(&probe, &probe.nodes[0]);
        let side = if d.output { pins.inputs } else { pins.outputs };
        side.iter().any(|p| if d.output { d.kind.fits(p.kind) } else { p.kind.fits(d.kind) })
    };
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
    shown.into_iter().map(|(_, e)| e).collect()
}

/// Adds a node of `kind` at `at` (canvas) to `vs`, linked to a link dropped there; its id.
pub fn add_node(vs: &mut VisualScript, cat: &Catalog, kind: NodeKind, at: [f32; 2], link: Option<LinkDrag>) -> u32 {
    let id = vs.next_id();
    // (A count starts at 1: giving or taking none is rarely meant.)
    let mut values = BTreeMap::new();
    if let NodeKind::Call { function, .. } = &kind {
        if API.iter().any(|f| f.name == function.as_str() && f.params.iter().any(|p| p.0 == "count")) {
            values.insert("count".to_string(), Value::Num(1.0));
        }
    }
    vs.nodes.push(Node { id, kind, pos: at, values });
    if let (Some(d), Some(node)) = (link, vs.node(id).cloned()) {
        let pins = cat.pins(vs, &node);
        let side = if d.output { pins.inputs } else { pins.outputs };
        if let Some(p) = side.iter().find(|p| if d.output { d.kind.fits(p.kind) } else { p.kind.fits(d.kind) }) {
            let l = if d.output { Link { from: d.node, out: d.pin, to: id, input: p.name.clone() } } else { Link { from: id, out: p.name.clone(), to: d.node, input: d.pin } };
            connect(vs, l, cat);
        }
    }
    id
}

/// A kind of node by a word (scripts): `if`, `repeat`, `value:num`, `math:add`, `compare:gt`,
/// `set:NAME`, `call:FUNCTION`…
pub fn kind_named(kind: &str) -> Option<NodeKind> {
    let (word, arg) = kind.split_once(':').unwrap_or((kind, ""));
    Some(match word {
        "start" => NodeKind::Start,
        "return" => NodeKind::Return,
        "if" => NodeKind::If,
        "sequence" => NodeKind::Sequence { steps: arg.parse().unwrap_or(2) },
        "repeat" => NodeKind::Repeat,
        "foreach" => NodeKind::ForEach,
        "value" => NodeKind::Value {
            kind: match arg {
                "num" => PinType::Num,
                "bool" => PinType::Bool,
                "list" => PinType::List,
                _ => PinType::Text,
            },
        },
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
    })
}

/// A value typed into input `input` of node `node` (by the input's type; "" takes it away).
pub fn set_value(vs: &mut VisualScript, cat: &Catalog, node: u32, input: &str, value: &str) {
    let Some(n) = vs.node(node).cloned() else { return };
    let kind = cat.pins(vs, &n).inputs.into_iter().find(|p| p.name == input).map_or(PinType::Any, |p| p.kind);
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
}

/// The nodes `sel` of `vs` (not Start) and the links between them, to paste.
pub fn copy_nodes(vs: &VisualScript, sel: &BTreeSet<u32>) -> Option<(Vec<Node>, Vec<Link>)> {
    let nodes: Vec<Node> = vs.nodes.iter().filter(|n| sel.contains(&n.id) && n.kind != NodeKind::Start).cloned().collect();
    let ids: BTreeSet<u32> = nodes.iter().map(|n| n.id).collect();
    let links: Vec<Link> = vs.links.iter().filter(|l| ids.contains(&l.from) && ids.contains(&l.to)).cloned().collect();
    (!nodes.is_empty()).then_some((nodes, links))
}

/// Pastes copied nodes (new ids) with their top-left at `at`, else a little below the copies.
/// Returns the new nodes' ids.
pub fn paste_nodes(vs: &mut VisualScript, (nodes, links): &(Vec<Node>, Vec<Link>), at: Option<[f32; 2]>) -> BTreeSet<u32> {
    let first = vs.next_id();
    let map: BTreeMap<u32, u32> = nodes.iter().enumerate().map(|(i, n)| (n.id, first + i as u32)).collect();
    let min = nodes.iter().fold([f32::INFINITY, f32::INFINITY], |m, n| [m[0].min(n.pos[0]), m[1].min(n.pos[1])]);
    let off = at.map_or([40.0, 40.0], |a| [a[0] - min[0], a[1] - min[1]]);
    for n in nodes {
        vs.nodes.push(Node { id: map[&n.id], pos: [n.pos[0] + off[0], n.pos[1] + off[1]], ..n.clone() });
    }
    for l in links {
        vs.links.push(Link { from: map[&l.from], to: map[&l.to], ..l.clone() });
    }
    map.values().copied().collect()
}

/// A visual script's Rhai, and its problems as lines starting with `!` (scripts, agents).
pub fn code_text(vs: &VisualScript, cat: &Catalog) -> String {
    let (mut s, problems) = visual::function_code(vs, cat);
    for p in problems {
        s.push_str(&format!("! {}{}\n", p.node.map_or(String::new(), |n| format!("node {n}: ")), p.text));
    }
    s
}

impl Workspace {
    /// The texts an id input of kind `arg` may take (items, sounds, maps…).
    pub fn arg_options(&self, arg: Arg) -> Vec<String> {
        let r = &self.refs;
        match arg {
            Arg::Item => r.items.clone(),
            Arg::Map => r.maps.clone(),
            Arg::Sound => r.sounds.clone(),
            Arg::Quest => r.quests.clone(),
            Arg::Spell => r.spells.clone(),
            Arg::Sheet => r.sheets.clone(),
            Arg::Character => r.characters.clone(),
            Arg::Prop => r.props.clone(),
            Arg::Target => ["self", "player"].iter().map(|s| s.to_string()).chain(r.entity_names.iter().cloned()).collect(),
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
            Arg::Recipe => self.project.db.recipes.iter().map(|d| d.id.clone()).collect(),
            Arg::Effect => rustpg2d_engine::srd::Condition::ALL.iter().map(|k| k.id().to_string()).collect(),
            Arg::Weather => rustpg2d_engine::map::WeatherKind::ALL.iter().map(|k| k.id().to_string()).chain(std::iter::once("default".to_string())).collect(),
            _ => Vec::new(),
        }
    }

    /// Groups the nodes `sel` of visual script `id` into a new visual script, called in their
    /// place: values coming in are its parameters, one value going out its result. Returns
    /// the new one's id.
    pub fn group_nodes(&mut self, id: &str, sel: &BTreeSet<u32>) -> Result<String, String> {
        let all = self.project.db.visual_scripts.clone();
        let k = all.iter().position(|v| v.id == id).ok_or("no such visual script")?;
        let text_fns = self.project.text_script_functions();
        let mut vs = all[k].clone();
        let new = rustpg2d_engine::refactor::unique_id(&format!("{}_part", vs.id), |c| all.iter().any(|v| v.id == c) || API.iter().any(|f| f.name == c) || text_fns.iter().any(|t| t.0 == c));
        let cat = Catalog { visual: &all, text_fns };
        let mut g = visual::group(&mut vs, sel, &new, &cat)?;
        g.name = format!("{} (part)", vs.label());
        self.begin(&format!("Group {} nodes of {} into a function", sel.len(), vs.label()));
        self.project.db.visual_scripts[k] = vs;
        self.project.db.visual_scripts.push(g);
        self.project.db.reindex();
        self.stale = true;
        Ok(new)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustpg2d_engine::visual::VisualScript;

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

    #[test]
    fn nodes_are_added_linked_copied_and_found_in_the_palette() {
        let cat = Catalog::new(&[]);
        let mut vs = VisualScript::fresh("t");
        // A node added from a link dropped into empty space takes the link.
        let give = add_node(&mut vs, &cat, kind_named("call:give_item").unwrap(), [300.0, 0.0], Some(LinkDrag { node: 1, pin: "out".into(), output: true, kind: PinType::Flow }));
        assert_eq!(vs.links, [Link { from: 1, out: "out".into(), to: give, input: "in".into() }]);
        assert_eq!(vs.node(give).unwrap().values.get("count"), Some(&Value::Num(1.0)), "a count starts at 1");
        set_value(&mut vs, &cat, give, "item", "potion");
        set_value(&mut vs, &cat, give, "count", "3");
        assert_eq!(vs.node(give).unwrap().values.get("item"), Some(&Value::Text("potion".into())));
        assert!(code_text(&vs, &cat).contains("give_item(\"potion\", 3)"), "{}", code_text(&vs, &cat));
        // Copied and pasted: a second node with a new number, the Start node isn't copied.
        let clip = copy_nodes(&vs, &BTreeSet::from([1, give])).unwrap();
        assert_eq!(clip.0.len(), 1);
        let pasted = paste_nodes(&mut vs, &clip, Some([600.0, 80.0]));
        assert_eq!(pasted.len(), 1);
        let copy = vs.node(*pasted.iter().next().unwrap()).unwrap();
        assert_eq!((copy.pos, copy.values.get("item")), ([600.0, 80.0], Some(&Value::Text("potion".into()))));
        // The palette, searched: what starts with the search first; after a number's link only
        // what takes or gives a number.
        let entries = palette_entries(&[], &[], "t");
        let shown = palette_shown(&entries, "give", None, &vs, &cat);
        assert!(shown.iter().take(4).any(|e| e.label == "give_item"), "{:?}", shown.iter().take(4).map(|e| &e.label).collect::<Vec<_>>());
        let num = LinkDrag { node: give, pin: "result".into(), output: true, kind: PinType::Num };
        let fits = palette_shown(&entries, "", Some(&num), &vs, &cat);
        assert!(fits.iter().any(|e| e.label == "Math") && !fits.iter().any(|e| e.label == "Comment"));
        assert!(kind_named("no_such_kind").is_none());
    }
}
