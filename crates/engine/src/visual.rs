//! Visual scripts: script functions built from nodes (the editor's *Nodes* tab), for creators who
//! don't write code. A [`VisualScript`] (`defs/visual_scripts.ron`) is one Rhai function: its
//! nodes and the links between their pins. Flow pins say what runs next; value pins carry numbers,
//! yes / no, texts, lists and options maps.
//!
//! The nodes turn into Rhai ([`rhai_file`]), written into `scripts/visual_scripts.rhai` whenever
//! the definitions are saved, so the game runs them like any script function (triggers, hooks,
//! menus, spells, plugins call them by name). Only that way: Rhai doesn't turn back into nodes.
//!
//! The nodes: every function of the script API ([`crate::script_api::API`]: a function without a
//! result is a step on the flow, a query such as `has_item` a value node, one that gives something
//! back and acts, such as `check` or `give_item`, a step with a result), calls of other visual
//! scripts (node groups) and of text script functions, and the language's own: *Start*, *Return*,
//! *If*, *Sequence*, *Repeat*, *For each*, values, maths, comparisons, and / or / not, joining
//! texts, local variables and comments.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::script_api::{Arg, Ret, API};

/// What a pin carries.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PinType {
    /// What runs next (the white pins).
    Flow,
    Num,
    Bool,
    #[default]
    Text,
    List,
    /// An options map (`#{ key: value }`).
    Map,
    /// Anything (a variable's value).
    Any,
}

impl PinType {
    pub const VALUES: [PinType; 6] = [PinType::Num, PinType::Bool, PinType::Text, PinType::List, PinType::Map, PinType::Any];

    pub fn label(self) -> &'static str {
        match self {
            PinType::Flow => "flow",
            PinType::Num => "number",
            PinType::Bool => "yes / no",
            PinType::Text => "text",
            PinType::List => "list",
            PinType::Map => "options",
            PinType::Any => "any",
        }
    }

    /// The pin type of a script API argument.
    pub fn of_arg(a: Arg) -> PinType {
        match a {
            Arg::Num => PinType::Num,
            Arg::Bool => PinType::Bool,
            Arg::List => PinType::List,
            Arg::Opts => PinType::Map,
            _ => PinType::Text,
        }
    }

    pub fn of_ret(r: Ret) -> PinType {
        match r {
            Ret::Num => PinType::Num,
            Ret::Bool => PinType::Bool,
            Ret::Text => PinType::Text,
            Ret::List => PinType::List,
            Ret::Map => PinType::Map,
            Ret::Any => PinType::Any,
        }
    }

    /// Whether a value of type `self` may go into an input of type `to`.
    pub fn fits(self, to: PinType) -> bool {
        self == to || (self != PinType::Flow && to != PinType::Flow && (self == PinType::Any || to == PinType::Any))
    }

    /// The value an input of this type has when nothing is linked or typed in.
    fn default_code(self) -> &'static str {
        match self {
            PinType::Num => "0",
            PinType::Bool => "false",
            PinType::Text => "\"\"",
            PinType::List => "[]",
            PinType::Map => "#{}",
            PinType::Flow | PinType::Any => "()",
        }
    }
}

/// A value typed into an input that nothing is linked to.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Value {
    Num(f64),
    Bool(bool),
    Text(String),
}

impl Value {
    fn code(&self) -> String {
        match self {
            Value::Num(n) => num_code(*n),
            Value::Bool(b) => b.to_string(),
            Value::Text(t) => text_code(t),
        }
    }
}

/// A whole number as an integer (the API's numbers are mostly integers), else a decimal.
fn num_code(n: f64) -> String {
    if n.is_finite() && n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else if n.is_finite() {
        format!("{n:?}")
    } else {
        "0".into()
    }
}

/// A Rhai string literal.
fn text_code(t: &str) -> String {
    let mut s = String::from("\"");
    for c in t.chars() {
        match c {
            '"' => s.push_str("\\\""),
            '\\' => s.push_str("\\\\"),
            '\n' => s.push_str("\\n"),
            '\r' => {}
            '\t' => s.push_str("\\t"),
            c => s.push(c),
        }
    }
    s.push('"');
    s
}

/// A parameter of a visual script (an output of its *Start* node).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Param {
    pub name: String,
    #[serde(default)]
    pub kind: PinType,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MathOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Min,
    Max,
}

impl MathOp {
    pub const ALL: [MathOp; 7] = [MathOp::Add, MathOp::Sub, MathOp::Mul, MathOp::Div, MathOp::Rem, MathOp::Min, MathOp::Max];

    pub fn label(self) -> &'static str {
        match self {
            MathOp::Add => "+",
            MathOp::Sub => "-",
            MathOp::Mul => "*",
            MathOp::Div => "/",
            MathOp::Rem => "remainder",
            MathOp::Min => "smaller",
            MathOp::Max => "larger",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CmpOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl CmpOp {
    pub const ALL: [CmpOp; 6] = [CmpOp::Eq, CmpOp::Ne, CmpOp::Lt, CmpOp::Le, CmpOp::Gt, CmpOp::Ge];

    pub fn code(self) -> &'static str {
        match self {
            CmpOp::Eq => "==",
            CmpOp::Ne => "!=",
            CmpOp::Lt => "<",
            CmpOp::Le => "<=",
            CmpOp::Gt => ">",
            CmpOp::Ge => ">=",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogicOp {
    And,
    Or,
}

/// What a node is.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum NodeKind {
    /// Where the function starts: a flow output and one value output per parameter.
    Start,
    /// Ends the function (with its result, when it gives one back).
    Return,
    /// A script API function, another visual script, or a text script function (`arity` inputs:
    /// those aren't known to the nodes).
    Call {
        function: String,
        #[serde(default, skip_serializing_if = "is_zero")]
        arity: u8,
    },
    /// Runs *then* or *else* by a yes / no.
    If,
    /// Runs its outputs one after the other.
    Sequence { steps: u8 },
    /// Runs *body* `count` times (`index` 0, 1, …), then *done*.
    Repeat,
    /// Runs *body* for each item of a list, then *done*.
    ForEach,
    /// A fixed value of a type.
    Value { kind: PinType },
    Math { op: MathOp },
    Compare { op: CmpOp },
    Logic { op: LogicOp },
    Not,
    /// Two texts (or anything) joined into one text.
    Join,
    /// Sets a local variable (a step).
    SetLocal { name: String },
    /// A local variable's value.
    GetLocal { name: String },
    /// A note on the canvas; nothing in the code.
    Comment { text: String },
}

fn is_zero(n: &u8) -> bool {
    *n == 0
}

/// A node on the canvas.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub id: u32,
    pub kind: NodeKind,
    /// Canvas position (the node's top-left corner).
    #[serde(default)]
    pub pos: [f32; 2],
    /// Values typed into inputs nothing is linked to, by input name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub values: BTreeMap<String, Value>,
}

/// A link from an output pin to an input pin.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Link {
    pub from: u32,
    pub out: String,
    pub to: u32,
    pub input: String,
}

/// A script function made of nodes.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct VisualScript {
    /// The Rhai function's name: what triggers, hooks and other scripts call.
    pub id: String,
    /// What the editor lists ("" = the id).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<Param>,
    /// What it gives back (None = nothing).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub returns: Option<PinType>,
    #[serde(default)]
    pub nodes: Vec<Node>,
    #[serde(default)]
    pub links: Vec<Link>,
}

impl VisualScript {
    /// A new one: a *Start* node to build from.
    pub fn fresh(id: &str) -> VisualScript {
        VisualScript { id: id.into(), nodes: vec![Node { id: 1, kind: NodeKind::Start, pos: [40.0, 80.0], values: BTreeMap::new() }], ..Default::default() }
    }

    pub fn label(&self) -> &str {
        if self.name.is_empty() {
            &self.id
        } else {
            &self.name
        }
    }

    pub fn node(&self, id: u32) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// A free node id.
    pub fn next_id(&self) -> u32 {
        self.nodes.iter().map(|n| n.id).max().unwrap_or(0) + 1
    }

    /// The link into input `input` of node `to` (inputs take one link; flow inputs several).
    pub fn link_into(&self, to: u32, input: &str) -> Option<&Link> {
        self.links.iter().find(|l| l.to == to && l.input == input)
    }

    /// The link out of flow output `out` of node `from` (a flow output leads to one node).
    pub fn link_out(&self, from: u32, out: &str) -> Option<&Link> {
        self.links.iter().find(|l| l.from == from && l.out == out)
    }
}

/// A pin of a node.
#[derive(Clone, Debug, PartialEq)]
pub struct Pin {
    pub name: String,
    pub kind: PinType,
    /// What a text input names (an item, a sound…): the editor offers the project's ids.
    pub arg: Option<Arg>,
}

impl Pin {
    fn new(name: &str, kind: PinType) -> Pin {
        Pin { name: name.into(), kind, arg: None }
    }

    fn flow(name: &str) -> Pin {
        Pin::new(name, PinType::Flow)
    }
}

/// A node's pins.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pins {
    pub inputs: Vec<Pin>,
    pub outputs: Vec<Pin>,
}

/// What the nodes can call besides the API: the visual scripts and the text scripts' functions
/// (name, parameter count).
#[derive(Clone, Debug, Default)]
pub struct Catalog<'a> {
    pub visual: &'a [VisualScript],
    pub text_fns: Vec<(String, usize)>,
}

/// What a called function is.
enum Callee<'a> {
    Api(&'a crate::script_api::ApiFn),
    Visual(&'a VisualScript),
    Text,
}

impl<'a> Catalog<'a> {
    pub fn new(visual: &'a [VisualScript]) -> Catalog<'a> {
        Catalog { visual, text_fns: Vec::new() }
    }

    /// What call node `n` calls: a visual script, else the script API function of that name
    /// (its longest form: every input shows; [`api_call`] leaves out the unused last ones), else
    /// a text script function.
    fn callee(&self, _vs: &VisualScript, _n: &Node, function: &str) -> Callee<'a> {
        if let Some(v) = self.visual.iter().find(|v| v.id == function) {
            return Callee::Visual(v);
        }
        // (Of equally long forms the first: say(speaker, text) before say(entity, text).)
        let longest = API.iter().filter(|f| f.name == function).map(|f| f.params.len()).max();
        match API.iter().find(|f| f.name == function && Some(f.params.len()) == longest) {
            Some(f) => Callee::Api(f),
            None => Callee::Text,
        }
    }

    /// Node `n`'s pins.
    pub fn pins(&self, vs: &VisualScript, n: &Node) -> Pins {
        let flow_in = || vec![Pin::flow("in")];
        match &n.kind {
            NodeKind::Start => Pins { inputs: vec![], outputs: std::iter::once(Pin::flow("out")).chain(vs.params.iter().map(|p| Pin::new(&p.name, p.kind))).collect() },
            NodeKind::Return => Pins { inputs: flow_in().into_iter().chain(vs.returns.map(|k| Pin::new("value", k))).collect(), outputs: vec![] },
            NodeKind::Call { function, arity } => {
                let arity = (*arity as usize).max(self.text_fns.iter().find(|t| &t.0 == function).map_or(0, |t| t.1));
                match self.callee(vs, n, function) {
                    Callee::Api(f) => {
                        let ins: Vec<Pin> = f.params.iter().map(|(p, a)| Pin { name: p.to_string(), kind: PinType::of_arg(*a), arg: Some(*a) }).collect();
                        let step = f.ret.is_none() || f.acts;
                        let mut inputs = if step { flow_in() } else { vec![] };
                        inputs.extend(ins);
                        let mut outputs = if step { vec![Pin::flow("out")] } else { vec![] };
                        outputs.extend(f.ret.map(|r| Pin::new("result", PinType::of_ret(r))));
                        Pins { inputs, outputs }
                    }
                    Callee::Visual(v) => {
                        let mut inputs = flow_in();
                        inputs.extend(v.params.iter().map(|p| Pin::new(&p.name, p.kind)));
                        let mut outputs = vec![Pin::flow("out")];
                        outputs.extend(v.returns.map(|k| Pin::new("result", k)));
                        Pins { inputs, outputs }
                    }
                    Callee::Text => {
                        let mut inputs = flow_in();
                        inputs.extend((1..=arity).map(|k| Pin::new(&format!("arg{k}"), PinType::Any)));
                        Pins { inputs, outputs: vec![Pin::flow("out"), Pin::new("result", PinType::Any)] }
                    }
                }
            }
            NodeKind::If => Pins { inputs: vec![Pin::flow("in"), Pin::new("condition", PinType::Bool)], outputs: vec![Pin::flow("then"), Pin::flow("else")] },
            NodeKind::Sequence { steps } => Pins { inputs: flow_in(), outputs: (1..=(*steps).max(1)).map(|k| Pin::flow(&k.to_string())).collect() },
            NodeKind::Repeat => Pins { inputs: vec![Pin::flow("in"), Pin::new("count", PinType::Num)], outputs: vec![Pin::flow("body"), Pin::flow("done"), Pin::new("index", PinType::Num)] },
            NodeKind::ForEach => Pins { inputs: vec![Pin::flow("in"), Pin::new("list", PinType::List)], outputs: vec![Pin::flow("body"), Pin::flow("done"), Pin::new("item", PinType::Any), Pin::new("index", PinType::Num)] },
            NodeKind::Value { kind } => Pins { inputs: vec![], outputs: vec![Pin::new("value", *kind)] },
            NodeKind::Math { .. } => Pins { inputs: vec![Pin::new("a", PinType::Num), Pin::new("b", PinType::Num)], outputs: vec![Pin::new("result", PinType::Num)] },
            NodeKind::Compare { .. } => Pins { inputs: vec![Pin::new("a", PinType::Any), Pin::new("b", PinType::Any)], outputs: vec![Pin::new("result", PinType::Bool)] },
            NodeKind::Logic { .. } => Pins { inputs: vec![Pin::new("a", PinType::Bool), Pin::new("b", PinType::Bool)], outputs: vec![Pin::new("result", PinType::Bool)] },
            NodeKind::Not => Pins { inputs: vec![Pin::new("value", PinType::Bool)], outputs: vec![Pin::new("result", PinType::Bool)] },
            NodeKind::Join => Pins { inputs: vec![Pin::new("a", PinType::Any), Pin::new("b", PinType::Any)], outputs: vec![Pin::new("result", PinType::Text)] },
            NodeKind::SetLocal { .. } => Pins { inputs: vec![Pin::flow("in"), Pin::new("value", PinType::Any)], outputs: vec![Pin::flow("out")] },
            NodeKind::GetLocal { .. } => Pins { inputs: vec![], outputs: vec![Pin::new("value", PinType::Any)] },
            NodeKind::Comment { .. } => Pins::default(),
        }
    }

    /// A node's title.
    pub fn title(&self, n: &Node) -> String {
        match &n.kind {
            NodeKind::Start => "Start".into(),
            NodeKind::Return => "Return".into(),
            NodeKind::Call { function, .. } => match self.visual.iter().find(|v| &v.id == function) {
                Some(v) => v.label().to_string(),
                None => function.clone(),
            },
            NodeKind::If => "If".into(),
            NodeKind::Sequence { .. } => "Sequence".into(),
            NodeKind::Repeat => "Repeat".into(),
            NodeKind::ForEach => "For each".into(),
            NodeKind::Value { kind } => format!("Value ({})", kind.label()),
            NodeKind::Math { op } => format!("Math {}", op.label()),
            NodeKind::Compare { op } => format!("Compare {}", op.code()),
            NodeKind::Logic { op: LogicOp::And } => "And".into(),
            NodeKind::Logic { op: LogicOp::Or } => "Or".into(),
            NodeKind::Not => "Not".into(),
            NodeKind::Join => "Join text".into(),
            NodeKind::SetLocal { name } => format!("Set {name}"),
            NodeKind::GetLocal { name } => name.clone(),
            NodeKind::Comment { .. } => "Comment".into(),
        }
    }
}

/// Something wrong with a visual script (on a node, or the whole script).
#[derive(Clone, Debug, PartialEq)]
pub struct Problem {
    pub node: Option<u32>,
    pub text: String,
}

/// Rhai's reserved words (a visual script's id or a local can't be one).
const RESERVED: [&str; 38] = [
    "true", "false", "let", "const", "if", "else", "switch", "do", "while", "until", "loop", "for", "in", "continue", "break", "return", "throw", "try", "catch", "import", "export", "as", "global", "private", "fn", "this", "is", "var", "static", "shared", "with", "go", "sync", "async", "await", "yield", "Fn", "call",
];

/// Whether `name` can be a Rhai function or variable name.
pub fn valid_name(name: &str) -> bool {
    let mut c = name.chars();
    c.next().is_some_and(|f| f.is_ascii_lowercase() || f == '_') && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') && !RESERVED.contains(&name) && name.chars().all(|c| !c.is_ascii_uppercase())
}

/// Writes one visual script's Rhai.
struct Gen<'a, 'b> {
    vs: &'a VisualScript,
    cat: &'a Catalog<'b>,
    out: String,
    problems: Vec<Problem>,
    /// Steps whose results exist now (written before, on this path).
    done: Vec<BTreeSet<u32>>,
    /// The flow path being written (a loop back is an error).
    path: Vec<u32>,
}

impl Gen<'_, '_> {
    fn problem(&mut self, node: Option<u32>, text: impl Into<String>) {
        let p = Problem { node, text: text.into() };
        if !self.problems.contains(&p) {
            self.problems.push(p);
        }
    }

    fn line(&mut self, depth: usize, s: &str) {
        for _ in 0..depth {
            self.out.push_str("    ");
        }
        self.out.push_str(s);
        self.out.push('\n');
    }

    fn written(&self, id: u32) -> bool {
        self.done.iter().any(|s| s.contains(&id))
    }

    /// How many of an API call's value inputs go into the call: all, but the last ones nothing
    /// was typed into or linked to are left out when the function has a form without them
    /// (`give_item("potion")` rather than `give_item("potion", 0)`).
    fn used_inputs(&self, n: &Node, pins: &Pins) -> usize {
        let values: Vec<&Pin> = pins.inputs.iter().filter(|p| p.kind != PinType::Flow).collect();
        let NodeKind::Call { function, .. } = &n.kind else { return values.len() };
        // (Only forms whose inputs are the first ones of the node's: say(text) isn't say(speaker, text) cut short.)
        let names: Vec<&str> = values.iter().map(|p| p.name.as_str()).collect();
        let forms: BTreeSet<usize> = API.iter().filter(|f| f.name == function.as_str() && f.params.iter().map(|p| p.0).eq(names.iter().take(f.params.len()).copied())).map(|f| f.params.len()).collect();
        if forms.is_empty() {
            return values.len();
        }
        let mut k = values.len();
        while k > 0 && forms.contains(&(k - 1)) && !n.values.contains_key(&values[k - 1].name) && self.vs.link_into(n.id, &values[k - 1].name).is_none() {
            k -= 1;
        }
        k
    }

    /// The code of input `pin` of node `n`.
    fn input(&mut self, n: &Node, pin: &Pin, seen: &mut Vec<u32>) -> String {
        match self.vs.link_into(n.id, &pin.name).cloned() {
            Some(l) => {
                let Some(src) = self.vs.node(l.from).cloned() else {
                    self.problem(Some(n.id), format!("'{}' is linked to a node that's gone", pin.name));
                    return pin.kind.default_code().into();
                };
                let out_kind = self.cat.pins(self.vs, &src).outputs.iter().find(|p| p.name == l.out).map(|p| p.kind);
                match out_kind {
                    Some(k) if !k.fits(pin.kind) => self.problem(Some(n.id), format!("'{}' wants {}, it gets {}", pin.name, pin.kind.label(), k.label())),
                    None => self.problem(Some(n.id), format!("'{}' is linked to an output that's gone", pin.name)),
                    _ => {}
                }
                self.value(&src, &l.out, seen)
            }
            None => match n.values.get(&pin.name) {
                Some(v) => v.code(),
                None => pin.kind.default_code().into(),
            },
        }
    }

    /// The code of output `out` of node `n` (a value).
    fn value(&mut self, n: &Node, out: &str, seen: &mut Vec<u32>) -> String {
        if seen.contains(&n.id) {
            self.problem(Some(n.id), "its value depends on itself");
            return "()".into();
        }
        seen.push(n.id);
        let pins = self.cat.pins(self.vs, n);
        let mut arg = |g: &mut Self, name: &str| -> String {
            let pin = pins.inputs.iter().find(|p| p.name == name).cloned().unwrap_or(Pin::new(name, PinType::Any));
            g.input(n, &pin, seen)
        };
        let code = match &n.kind {
            NodeKind::Start => match self.vs.params.iter().find(|p| p.name == out) {
                Some(p) => p.name.clone(),
                None => {
                    self.problem(Some(n.id), format!("the script has no parameter '{out}' any more"));
                    "()".into()
                }
            },
            NodeKind::Value { .. } => n.values.get("value").map_or_else(|| pins.outputs[0].kind.default_code().to_string(), |v| v.code()),
            NodeKind::Math { op } => {
                let (a, b) = (arg(self, "a"), arg(self, "b"));
                match op {
                    MathOp::Add => format!("({a} + {b})"),
                    MathOp::Sub => format!("({a} - {b})"),
                    MathOp::Mul => format!("({a} * {b})"),
                    MathOp::Div => format!("({a} / {b})"),
                    MathOp::Rem => format!("({a} % {b})"),
                    MathOp::Min => format!("min({a}, {b})"),
                    MathOp::Max => format!("max({a}, {b})"),
                }
            }
            NodeKind::Compare { op } => {
                let (a, b) = (arg(self, "a"), arg(self, "b"));
                format!("({a} {} {b})", op.code())
            }
            NodeKind::Logic { op } => {
                let (a, b) = (arg(self, "a"), arg(self, "b"));
                format!("({a} {} {b})", if *op == LogicOp::And { "&&" } else { "||" })
            }
            NodeKind::Not => format!("!{}", arg(self, "value")),
            // (Rhai's `+` joins a text with anything; a text first makes it text.)
            NodeKind::Join => {
                let (a, b) = (arg(self, "a"), arg(self, "b"));
                if a.starts_with('"') {
                    format!("({a} + {b})")
                } else {
                    format!("(\"\" + {a} + {b})")
                }
            }
            NodeKind::GetLocal { name } => local(name),
            NodeKind::Call { function, .. } if !pins.inputs.iter().any(|p| p.kind == PinType::Flow) => {
                let used = self.used_inputs(n, &pins);
                let args: Vec<String> = pins.inputs[..used].iter().map(|p| arg(self, &p.name)).collect();
                format!("{function}({})", args.join(", "))
            }
            // A step's result, a loop's item or index: its variable, once the step ran.
            NodeKind::Call { .. } | NodeKind::Repeat | NodeKind::ForEach => {
                if !self.written(n.id) {
                    self.problem(Some(n.id), format!("its '{out}' is used where it hasn't run (link its flow before the node that uses it)"));
                }
                match &n.kind {
                    NodeKind::Call { .. } => step_var(n.id),
                    _ => loop_var(n.id, out),
                }
            }
            _ => {
                self.problem(Some(n.id), format!("has no value '{out}'"));
                "()".into()
            }
        };
        seen.pop();
        code
    }

    /// Writes the steps from flow output `out` of node `from` on.
    fn flow(&mut self, from: u32, out: &str, depth: usize) {
        let Some(l) = self.vs.link_out(from, out).cloned() else { return };
        let Some(n) = self.vs.node(l.to).cloned() else { return };
        if self.path.contains(&n.id) {
            self.problem(Some(n.id), "the flow runs in a circle here (use Repeat or For each for loops)");
            return;
        }
        self.path.push(n.id);
        self.step(&n, depth);
        self.path.pop();
    }

    fn step(&mut self, n: &Node, depth: usize) {
        let pins = self.cat.pins(self.vs, n);
        let mut seen = Vec::new();
        match &n.kind {
            NodeKind::Call { function, .. } => {
                let values: Vec<Pin> = pins.inputs.iter().filter(|p| p.kind != PinType::Flow).cloned().collect();
                let used = self.used_inputs(n, &Pins { inputs: values.clone(), outputs: Vec::new() });
                let args: Vec<String> = values[..used].iter().map(|p| self.input(n, p, &mut seen)).collect();
                let call = format!("{function}({})", args.join(", "));
                // (Its result kept only when something uses it.)
                if self.vs.links.iter().any(|l| l.from == n.id && l.out == "result") {
                    let line = format!("let {} = {call};", step_var(n.id));
                    self.line(depth, &line);
                    if let Some(s) = self.done.last_mut() {
                        s.insert(n.id);
                    }
                } else {
                    self.line(depth, &format!("{call};"));
                }
                self.flow(n.id, "out", depth);
            }
            NodeKind::Return => {
                if self.vs.returns.is_some() {
                    let pin = pins.inputs.iter().find(|p| p.name == "value").cloned().unwrap_or(Pin::new("value", PinType::Any));
                    let v = self.input(n, &pin, &mut seen);
                    self.line(depth, &format!("return {v};"));
                } else {
                    self.line(depth, "return;");
                }
            }
            NodeKind::If => {
                let c = self.input(n, &Pin::new("condition", PinType::Bool), &mut seen);
                self.line(depth, &format!("if {c} {{"));
                self.block(n.id, "then", depth + 1);
                if self.vs.link_out(n.id, "else").is_some() {
                    self.line(depth, "} else {");
                    self.block(n.id, "else", depth + 1);
                }
                self.line(depth, "}");
            }
            NodeKind::Sequence { steps } => {
                for k in 1..=(*steps).max(1) {
                    self.flow(n.id, &k.to_string(), depth);
                }
            }
            NodeKind::Repeat | NodeKind::ForEach => {
                let head = if n.kind == NodeKind::Repeat {
                    let count = self.input(n, &Pin::new("count", PinType::Num), &mut seen);
                    format!("for {} in 0..{count} {{", loop_var(n.id, "index"))
                } else {
                    let list = self.input(n, &Pin::new("list", PinType::List), &mut seen);
                    format!("for ({}, {}) in {list} {{", loop_var(n.id, "item"), loop_var(n.id, "index"))
                };
                self.line(depth, &head);
                self.done.push(BTreeSet::from([n.id]));
                self.flow(n.id, "body", depth + 1);
                self.done.pop();
                self.line(depth, "}");
                self.flow(n.id, "done", depth);
            }
            NodeKind::SetLocal { name } => {
                let v = self.input(n, &Pin::new("value", PinType::Any), &mut seen);
                self.line(depth, &format!("{} = {v};", local(name)));
                self.flow(n.id, "out", depth);
            }
            _ => self.problem(Some(n.id), "isn't a step: link its value instead"),
        }
    }

    /// A branch's steps: their results exist only inside it.
    fn block(&mut self, from: u32, out: &str, depth: usize) {
        self.done.push(BTreeSet::new());
        self.flow(from, out, depth);
        self.done.pop();
    }
}

fn step_var(id: u32) -> String {
    format!("r{id}")
}

fn loop_var(id: u32, what: &str) -> String {
    format!("{}{id}", if what == "item" { "item" } else { "i" })
}

fn local(name: &str) -> String {
    format!("v_{name}")
}

/// One visual script's Rhai function and what's wrong with it (`cat` knows the other functions).
pub fn function_code(vs: &VisualScript, cat: &Catalog) -> (String, Vec<Problem>) {
    let mut g = Gen { vs, cat, out: String::new(), problems: Vec::new(), done: vec![BTreeSet::new()], path: Vec::new() };
    if !valid_name(&vs.id) {
        g.problem(None, format!("'{}' can't be a function's name: small letters, digits and _, starting with a letter", vs.id));
    } else if API.iter().any(|f| f.name == vs.id) {
        g.problem(None, format!("'{}' is a script API function's name", vs.id));
    }
    for p in &vs.params {
        if !valid_name(&p.name) {
            g.problem(None, format!("the parameter '{}' needs a name of small letters, digits and _", p.name));
        }
    }
    let starts: Vec<u32> = vs.nodes.iter().filter(|n| n.kind == NodeKind::Start).map(|n| n.id).collect();
    if starts.len() != 1 {
        g.problem(None, if starts.is_empty() { "it needs a Start node" } else { "it has more than one Start node" });
    }
    // Local variables: declared at the top (they live in the whole function).
    let locals: BTreeSet<String> = vs.nodes.iter().filter_map(|n| match &n.kind {
        NodeKind::SetLocal { name } | NodeKind::GetLocal { name } => Some(name.clone()),
        _ => None,
    }).collect();
    for name in &locals {
        if !valid_name(name) {
            g.problem(None, format!("the local variable '{name}' needs a name of small letters, digits and _"));
        }
    }
    for n in &vs.nodes {
        if let NodeKind::GetLocal { name } = &n.kind {
            if !vs.nodes.iter().any(|m| matches!(&m.kind, NodeKind::SetLocal { name: s } if s == name)) {
                g.problem(Some(n.id), format!("nothing sets the local variable '{name}'"));
            }
        }
        if let NodeKind::Call { function, arity } = &n.kind {
            let known = vs.id == *function || cat.visual.iter().any(|v| &v.id == function) || API.iter().any(|f| f.name == function) || cat.text_fns.iter().any(|t| &t.0 == function) || *arity > 0;
            if !known {
                g.problem(Some(n.id), format!("there's no function '{function}'"));
            }
        }
        // Inputs typed in that a linked pin overrides are fine; links to pins that are gone aren't.
        let pins = cat.pins(vs, n);
        for l in vs.links.iter().filter(|l| l.to == n.id) {
            if !pins.inputs.iter().any(|p| p.name == l.input) {
                g.problem(Some(n.id), format!("a link goes into '{}', which it hasn't got", l.input));
            }
        }
    }
    let params: Vec<&str> = vs.params.iter().map(|p| p.name.as_str()).collect();
    for name in &locals {
        g.line(1, &format!("let {} = ();", local(name)));
    }
    if let Some(&s) = starts.first() {
        g.path.push(s);
        g.flow(s, "out", 1);
    }
    let mut code = String::new();
    if !vs.description.is_empty() {
        for l in vs.description.lines() {
            code.push_str(&format!("// {l}\n"));
        }
    }
    code.push_str(&format!("fn {}({}) {{\n", vs.id, params.join(", ")));
    if g.problems.is_empty() {
        code.push_str(&g.out);
    } else {
        // (A broken script does nothing, so whatever calls it still runs.)
        code.push_str(&format!("    print(\"[visual] {} has problems: open it in the Nodes tab\");\n", vs.id));
    }
    code.push_str("}\n");
    (code, g.problems)
}

/// The header of `scripts/visual_scripts.rhai`.
pub const FILE_HEADER: &str = "// Made by R(ust)PG2D from the visual scripts (the editor's Nodes tab, defs/visual_scripts.ron).\n// Don't edit it: it is written again whenever the definitions are saved.\n";

/// Where the visual scripts' Rhai goes.
pub const FILE: &str = "scripts/visual_scripts.rhai";

/// Every visual script's Rhai, in one file.
pub fn rhai_file(scripts: &[VisualScript], text_fns: Vec<(String, usize)>) -> String {
    let cat = Catalog { visual: scripts, text_fns };
    let mut out = String::from(FILE_HEADER);
    for vs in scripts {
        out.push('\n');
        out.push_str(&format!("// {}\n", vs.label()));
        out.push_str(&function_code(vs, &cat).0);
    }
    out
}

/// The functions a text script defines (name, parameter count), for call nodes.
pub fn text_functions(src: &str) -> Vec<(String, usize)> {
    let mut out = Vec::new();
    for line in src.lines() {
        let l = line.trim_start();
        let Some(rest) = l.strip_prefix("fn ").or_else(|| l.strip_prefix("private fn ")) else { continue };
        let Some((name, args)) = rest.split_once('(') else { continue };
        let name = name.trim();
        if !valid_name(name) && !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            continue;
        }
        let args = args.split(')').next().unwrap_or("");
        let n = if args.trim().is_empty() { 0 } else { args.split(',').count() };
        out.push((name.to_string(), n));
    }
    out
}

/// The ids a visual script names in its inputs (items, sounds, maps…, by the API argument kinds)
/// with the kind of reference, and the visual scripts it calls.
pub fn refs(vs: &VisualScript, cat: &Catalog) -> Vec<(Arg, String)> {
    let mut out = Vec::new();
    for n in &vs.nodes {
        let pins = cat.pins(vs, n);
        for p in &pins.inputs {
            if let (Some(a), Some(Value::Text(t))) = (p.arg, n.values.get(&p.name)) {
                if !t.is_empty() && vs.link_into(n.id, &p.name).is_none() {
                    out.push((a, t.clone()));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: u32, kind: NodeKind) -> Node {
        Node { id, kind, pos: [0.0, 0.0], values: BTreeMap::new() }
    }

    fn with(mut n: Node, k: &str, v: Value) -> Node {
        n.values.insert(k.into(), v);
        n
    }

    fn link(from: u32, out: &str, to: u32, input: &str) -> Link {
        Link { from, out: out.into(), to, input: input.into() }
    }

    fn call(id: u32, f: &str) -> Node {
        node(id, NodeKind::Call { function: f.into(), arity: 0 })
    }

    /// Compiles `code` with the engine's script host (every API function registered).
    fn compiles(code: &str) {
        let r = crate::script::ScriptHost::new().compile_check(code);
        assert!(r.is_ok(), "{code}\n{r:?}");
    }

    #[test]
    fn a_check_opens_a_door_or_says_why_not() {
        // Start -> check("athletics", 12) -> If (result) then say + set_locked else say.
        let vs = VisualScript {
            id: "force_door".into(),
            nodes: vec![
                node(1, NodeKind::Start),
                with(with(call(2, "check"), "what", Value::Text("athletics".into())), "dc", Value::Num(12.0)),
                node(3, NodeKind::If),
                with(with(call(4, "say"), "speaker", Value::Text("Hero".into())), "text", Value::Text("It \"gives\" way!".into())),
                with(with(call(5, "set_locked"), "target", Value::Text("door".into())), "locked", Value::Bool(false)),
                with(with(call(6, "say"), "speaker", Value::Text("Hero".into())), "text", Value::Text("Too heavy.".into())),
            ],
            links: vec![link(1, "out", 2, "in"), link(2, "out", 3, "in"), link(2, "result", 3, "condition"), link(3, "then", 4, "in"), link(4, "out", 5, "in"), link(3, "else", 6, "in")],
            ..Default::default()
        };
        let (code, problems) = function_code(&vs, &Catalog::new(&[]));
        assert!(problems.is_empty(), "{problems:?}");
        assert!(code.contains("let r2 = check(\"athletics\", 12);"), "{code}");
        assert!(code.contains("if r2 {") && code.contains("} else {") && code.contains("set_locked(\"door\", false);") && code.contains("\\\"gives\\\""), "{code}");
        compiles(&code);
    }

    #[test]
    fn queries_go_into_expressions_and_loops_have_their_variables() {
        // If has_item("rope") and player_hp() > 5: repeat 3 times give_xp(index * 10).
        let vs = VisualScript {
            id: "climb".into(),
            params: vec![Param { name: "times".into(), kind: PinType::Num }],
            returns: Some(PinType::Num),
            nodes: vec![
                node(1, NodeKind::Start),
                with(call(2, "has_item"), "item", Value::Text("rope".into())),
                call(3, "player_hp"),
                with(node(4, NodeKind::Compare { op: CmpOp::Gt }), "b", Value::Num(5.0)),
                node(5, NodeKind::Logic { op: LogicOp::And }),
                node(6, NodeKind::If),
                node(7, NodeKind::Repeat),
                with(node(8, NodeKind::Math { op: MathOp::Mul }), "b", Value::Num(10.0)),
                call(9, "give_xp"),
                node(10, NodeKind::Return),
                node(11, NodeKind::SetLocal { name: "total".into() }),
                node(12, NodeKind::GetLocal { name: "total".into() }),
            ],
            links: vec![
                link(1, "out", 6, "in"),
                link(2, "result", 5, "a"),
                link(3, "result", 4, "a"),
                link(4, "result", 5, "b"),
                link(5, "result", 6, "condition"),
                link(6, "then", 7, "in"),
                link(1, "times", 7, "count"),
                link(7, "index", 8, "a"),
                link(8, "result", 11, "value"),
                link(7, "body", 11, "in"),
                link(11, "out", 9, "in"),
                link(12, "value", 9, "amount"),
                link(7, "done", 10, "in"),
                link(12, "value", 10, "value"),
            ],
            ..Default::default()
        };
        let (code, problems) = function_code(&vs, &Catalog::new(&[]));
        assert!(problems.is_empty(), "{problems:?}\n{code}");
        assert!(code.contains("fn climb(times) {") && code.contains("let v_total = ();"), "{code}");
        assert!(code.contains("if (has_item(\"rope\") && (player_hp() > 5)) {"), "{code}");
        assert!(code.contains("for i7 in 0..times {") && code.contains("v_total = (i7 * 10);") && code.contains("give_xp(v_total);") && code.contains("return v_total;"), "{code}");
        compiles(&code);
    }

    #[test]
    fn groups_call_each_other_and_problems_make_a_stub() {
        let heal = VisualScript {
            id: "heal_some".into(),
            params: vec![Param { name: "amount".into(), kind: PinType::Num }],
            nodes: vec![node(1, NodeKind::Start), call(2, "heal")],
            links: vec![link(1, "out", 2, "in"), link(1, "amount", 2, "amount")],
            ..Default::default()
        };
        let user = VisualScript {
            id: "fountain".into(),
            nodes: vec![node(1, NodeKind::Start), with(call(2, "heal_some"), "amount", Value::Num(5.0))],
            links: vec![link(1, "out", 2, "in")],
            ..Default::default()
        };
        // A result used before its step ran, a value of the wrong type, a circle.
        let broken = VisualScript {
            id: "broken".into(),
            nodes: vec![node(1, NodeKind::Start), with(call(2, "roll"), "dice", Value::Text("1d6".into())), call(3, "give_xp"), with(call(4, "flag"), "name", Value::Text("f".into())), call(5, "heal")],
            links: vec![link(1, "out", 3, "in"), link(2, "result", 3, "amount"), link(3, "out", 5, "in"), link(4, "result", 5, "amount"), link(5, "out", 3, "in")],
            ..Default::default()
        };
        let all = vec![heal, user, broken];
        let file = rhai_file(&all, vec![]);
        assert!(file.starts_with(FILE_HEADER) && file.contains("fn fountain() {\n    heal_some(5);\n}"), "{file}");
        let (stub, problems) = function_code(&all[2], &Catalog::new(&all));
        let texts: Vec<&str> = problems.iter().map(|p| p.text.as_str()).collect();
        assert!(texts.iter().any(|t| t.contains("hasn't run")), "{texts:?}");
        assert!(texts.iter().any(|t| t.contains("wants number, it gets yes / no")), "{texts:?}");
        assert!(texts.iter().any(|t| t.contains("circle")), "{texts:?}");
        assert!(stub.contains("has problems") && !stub.contains("give_xp"), "{stub}");
        compiles(&file);
    }

    #[test]
    fn names_and_values_are_written_safely() {
        assert!(valid_name("open_gate") && !valid_name("Open") && !valid_name("1st") && !valid_name("fn") && !valid_name("a-b"));
        assert_eq!(num_code(3.0), "3");
        assert_eq!(num_code(2.5), "2.5");
        assert_eq!(text_code("a\"b\\c\nd"), "\"a\\\"b\\\\c\\nd\"");
        let vs = VisualScript { id: "say".into(), ..VisualScript::fresh("say") };
        assert!(function_code(&vs, &Catalog::new(&[])).1.iter().any(|p| p.text.contains("script API")));
        // A round trip through RON.
        let v = VisualScript { params: vec![Param { name: "n".into(), kind: PinType::Num }], returns: Some(PinType::Bool), ..VisualScript::fresh("x") };
        let back: VisualScript = ron::from_str(&crate::project::ron_text(&v).unwrap()).unwrap();
        assert_eq!(back, v);
    }

    #[test]
    fn saving_writes_the_rhai_beside_the_text_scripts() {
        let root = std::env::temp_dir().join(format!("rustpg2d_visual_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("scripts")).unwrap();
        std::fs::write(root.join("scripts/main.rhai"), "fn greet(who) {\n    say(who, \"Hello!\");\n}\n").unwrap();
        // A visual script calling the text function greet(who).
        let vs = VisualScript {
            id: "welcome".into(),
            nodes: vec![node(1, NodeKind::Start), with(call(2, "greet"), "arg1", Value::Text("Bob".into()))],
            links: vec![link(1, "out", 2, "in")],
            ..Default::default()
        };
        let mut p = crate::project::Project { root: root.clone(), config: crate::project::ProjectConfig::new("T"), db: crate::defs::Database { visual_scripts: vec![vs], ..Default::default() } };
        p.save_config().unwrap();
        p.save_defs().unwrap();
        let code = std::fs::read_to_string(root.join(FILE)).unwrap();
        assert!(code.starts_with(FILE_HEADER) && code.contains("fn welcome() {\n    greet(\"Bob\");\n}"), "{code}");
        assert_eq!(crate::project::Project::load(&root).unwrap().db.visual_scripts.len(), 1, "the definitions load again");
        // No visual scripts: no file.
        p.db.visual_scripts.clear();
        p.save_defs().unwrap();
        assert!(!root.join(FILE).exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn unused_last_inputs_are_left_out_when_a_shorter_form_exists() {
        // bark(who, text[, seconds]), check(what, dc[, mode]): the node shows the longest form.
        let vs = VisualScript {
            id: "barks".into(),
            nodes: vec![
                node(1, NodeKind::Start),
                with(with(call(2, "bark"), "target", Value::Text("self".into())), "text", Value::Text("Here!".into())),
                with(with(with(call(3, "bark"), "target", Value::Text("self".into())), "text", Value::Text("Long".into())), "seconds", Value::Num(4.0)),
                with(with(call(4, "check"), "what", Value::Text("stealth".into())), "dc", Value::Num(10.0)),
            ],
            links: vec![link(1, "out", 2, "in"), link(2, "out", 3, "in"), link(3, "out", 4, "in")],
            ..Default::default()
        };
        assert!(Catalog::new(&[]).pins(&vs, &vs.nodes[1]).inputs.iter().any(|p| p.name == "seconds"), "every input shows");
        let (code, problems) = function_code(&vs, &Catalog::new(&[]));
        assert!(problems.is_empty(), "{problems:?}");
        assert!(code.contains("bark(\"self\", \"Here!\");") && code.contains("bark(\"self\", \"Long\", 4);") && code.contains("check(\"stealth\", 10);"), "{code}");
        // say(text) and say(speaker, text): an empty text stays in.
        let mut say = VisualScript { id: "talk".into(), nodes: vec![node(1, NodeKind::Start), with(call(2, "say"), "speaker", Value::Text("Hero".into()))], links: vec![link(1, "out", 2, "in")], ..Default::default() };
        assert!(function_code(&say, &Catalog::new(&[])).0.contains("say(\"Hero\", \"\");"));
        say.nodes[1].values.insert("text".into(), Value::Text("Hi".into()));
        assert!(function_code(&say, &Catalog::new(&[])).0.contains("say(\"Hero\", \"Hi\");"));
        compiles(&code);
    }

    #[test]
    fn the_ids_in_inputs_are_references() {
        let vs = VisualScript {
            id: "gift".into(),
            nodes: vec![node(1, NodeKind::Start), with(with(call(2, "give_item"), "item", Value::Text("potion".into())), "count", Value::Num(2.0)), with(call(3, "play_sound"), "sound", Value::Text("ding".into()))],
            links: vec![link(1, "out", 2, "in"), link(2, "out", 3, "in")],
            ..Default::default()
        };
        let r = refs(&vs, &Catalog::new(&[]));
        assert!(r.contains(&(Arg::Item, "potion".into())) && r.contains(&(Arg::Sound, "ding".into())), "{r:?}");
    }
}
