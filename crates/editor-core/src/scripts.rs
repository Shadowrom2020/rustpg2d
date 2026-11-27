//! Scripts for an editor of them, without its window: where the cursor is (in code, in a
//! string argument of which call), what to suggest there (the script API, the project's own
//! functions and ids), the signature of the call being written, what typing a bracket or Enter
//! adds, how a script is painted, and what is wrong in a text before it is saved.

use std::collections::HashMap;
use std::path::PathBuf;

use rustpg2d_engine::defs::Database;

pub use rustpg2d_engine::script_api::{ApiFn, Arg, API};

use crate::workspace::Workspace;

pub const KEYWORDS: &[&str] = &[
    "fn", "let", "const", "if", "else", "while", "loop", "for", "in", "return", "break", "continue", "switch", "do", "until", "true", "false",
    "private", "throw", "try", "catch", "import", "export", "as", "global", "this",
];

/// The engine's lexer: tokens covering every byte (comments and whitespace included).
use rustpg2d_engine::script_refs::Lexeme as Tk;

/// Splits source into contiguous tokens covering every byte: (kind, start, end).
fn lex(src: &str) -> Vec<(Tk, usize, usize)> {
    rustpg2d_engine::script_refs::lex(src).into_iter().map(|t| (t.kind, t.start, t.end)).collect()
}

/// Names of `fn` definitions in a source (the engine's finder).
pub fn defined_functions(src: &str) -> Vec<String> {
    rustpg2d_engine::script_refs::functions(src).into_iter().map(|f| f.name).collect()
}

/// Functions defined in a source with their parameter names: `fn name(a, b)`.
pub fn defined_signatures(src: &str) -> Vec<(String, Vec<String>)> {
    rustpg2d_engine::script_refs::functions(src).into_iter().map(|f| (f.name, f.params)).collect()
}

/// `let` / `const` names in a source.
fn local_names(src: &str) -> Vec<String> {
    let toks: Vec<_> = lex(src).into_iter().filter(|t| t.0 != Tk::Space && t.0 != Tk::Comment).collect();
    let mut v: Vec<String> = toks
        .windows(2)
        .filter(|w| w[0].0 == Tk::Ident && matches!(&src[w[0].1..w[0].2], "let" | "const") && w[1].0 == Tk::Ident)
        .map(|w| src[w[1].1..w[1].2].to_string())
        .collect();
    v.sort();
    v.dedup();
    v
}

/// What a piece of a script is, for its colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Paint {
    Comment,
    Str,
    Num,
    Keyword,
    /// A function of the script API.
    Api,
    /// Another function (the project's own, or anything called).
    Func,
    /// `SELF`.
    Const,
    Text,
    Punct,
}

/// A script as pieces to paint, covering every byte: (what, start, end). `user_fns`: the
/// functions the project's scripts define.
pub fn paint(src: &str, user_fns: &[String]) -> Vec<(Paint, usize, usize)> {
    let toks = lex(src);
    let next_sig = |k: usize| toks[k + 1..].iter().find(|t| t.0 != Tk::Space).map(|t| &src[t.1..t.2]);
    toks.iter()
        .enumerate()
        .map(|(k, &(kind, s, e))| {
            let text = &src[s..e];
            let what = match kind {
                Tk::Comment => Paint::Comment,
                Tk::Str { .. } => Paint::Str,
                Tk::Num => Paint::Num,
                Tk::Space => Paint::Text,
                Tk::Punct => Paint::Punct,
                Tk::Ident if KEYWORDS.contains(&text) => Paint::Keyword,
                Tk::Ident if text == "SELF" => Paint::Const,
                Tk::Ident if next_sig(k) == Some("(") && API.iter().any(|f| f.name == text) => Paint::Api,
                Tk::Ident if next_sig(k) == Some("(") || user_fns.iter().any(|f| f == text) => Paint::Func,
                Tk::Ident => Paint::Text,
            };
            (what, s, e)
        })
        .collect()
}

/// The innermost function call around a position.
#[derive(Clone, Debug, PartialEq)]
pub struct CallAt {
    pub name: String,
    pub arg: usize,
    /// The first argument when it is a complete string literal (teleport's map).
    pub first: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Place {
    /// In code; `partial` = identifier being typed (from byte `start`).
    Code { partial: String, start: usize },
    /// Inside a string literal (content typed so far from byte `start`).
    Str { partial: String, start: usize },
    Comment,
}

/// Where the cursor is (`prefix` = the text before it) and the call it is in.
pub fn analyze(prefix: &str) -> (Place, Option<CallAt>) {
    struct Frame {
        name: Option<String>,
        arg: usize,
        first: Option<String>,
    }
    let toks = lex(prefix);
    let mut stack: Vec<Frame> = Vec::new();
    let mut last_ident: Option<String> = None;
    for &(kind, s, e) in &toks {
        let t = &prefix[s..e];
        match kind {
            Tk::Space | Tk::Comment => continue,
            Tk::Punct if t == "(" => stack.push(Frame { name: last_ident.take(), arg: 0, first: None }),
            Tk::Punct if t == "[" || t == "{" => stack.push(Frame { name: None, arg: 0, first: None }),
            Tk::Punct if t == ")" || t == "]" || t == "}" => {
                stack.pop();
            }
            Tk::Punct if t == "," => {
                if let Some(f) = stack.last_mut() {
                    f.arg += 1;
                }
            }
            Tk::Str { closed: true } if t.starts_with('"') => {
                if let Some(f) = stack.last_mut() {
                    if f.arg == 0 && f.first.is_none() {
                        f.first = Some(t[1..t.len() - 1].to_string());
                    }
                }
            }
            _ => {}
        }
        last_ident = (kind == Tk::Ident).then(|| t.to_string());
    }
    let call = stack.iter().rev().find(|f| f.name.is_some()).map(|f| CallAt { name: f.name.clone().unwrap(), arg: f.arg, first: f.first.clone() });
    // The direct call only counts for string arguments (not inside nested [..]).
    let direct = stack.last().filter(|f| f.name.is_some()).map(|f| CallAt { name: f.name.clone().unwrap(), arg: f.arg, first: f.first.clone() });
    let place = match toks.last() {
        Some(&(Tk::Comment, s, e)) if !prefix[s..e].starts_with("/*") || !prefix[s..e].ends_with("*/") => Place::Comment,
        Some(&(Tk::Str { closed: false }, s, e)) if prefix[s..].starts_with('"') => Place::Str { partial: prefix[s + 1..e].to_string(), start: s + 1 },
        Some(&(Tk::Ident, s, e)) => Place::Code { partial: prefix[s..e].to_string(), start: s },
        _ => Place::Code { partial: String::new(), start: prefix.len() },
    };
    match place {
        Place::Str { .. } => (place, direct),
        _ => (place, call),
    }
}

/// A completion suggestion.
#[derive(Clone, Debug, PartialEq)]
pub struct Cand {
    pub insert: String,
    pub label: String,
    pub detail: String,
}

/// What the project knows, for completion.
pub struct Known<'a> {
    pub db: &'a Database,
    pub maps: &'a [String],
    pub spawns: &'a HashMap<String, Vec<String>>,
    pub targets: &'a [String],
    pub flags: &'a [String],
    pub vars: &'a [String],
    pub functions: &'a [String],
    /// The project's script functions with their parameter names (signature hints).
    pub signatures: &'a [(String, Vec<String>)],
    /// Faction ids (Database › Factions).
    pub factions: &'a [String],
}

/// The kind of the argument a call expects at `arg` (its first overload that has one).
pub fn arg_kind(name: &str, arg: usize) -> Option<Arg> {
    rustpg2d_engine::script_api::arg_kinds(name, arg).next()
}

fn rank(mut v: Vec<Cand>, partial: &str) -> Vec<Cand> {
    let p = partial.to_lowercase();
    v.retain(|c| p.is_empty() || c.insert.to_lowercase().contains(&p) || c.detail.to_lowercase().contains(&p));
    v.sort_by_key(|c| (!c.insert.to_lowercase().starts_with(&p), c.insert.clone()));
    v.dedup_by(|a, b| a.insert == b.insert);
    v.truncate(40);
    v
}

pub fn candidates(place: &Place, call: Option<&CallAt>, known: &Known, src: &str, force: bool) -> Vec<Cand> {
    let c = |insert: &str, detail: &str| Cand { insert: insert.to_string(), label: insert.to_string(), detail: detail.to_string() };
    match place {
        Place::Comment => Vec::new(),
        Place::Str { partial, .. } => {
            let Some(call) = call else { return Vec::new() };
            let db = known.db;
            let list: Vec<Cand> = match arg_kind(&call.name, call.arg) {
                Some(Arg::Item) => db.items.iter().map(|d| c(&d.id, &d.name)).collect(),
                Some(Arg::Map) => known.maps.iter().map(|m| c(m, "map")).collect(),
                Some(Arg::Spawn) => call.first.as_ref().and_then(|m| known.spawns.get(m)).into_iter().flatten().map(|s| c(s, "spawn point")).collect(),
                Some(Arg::Sound) => db.sounds.iter().map(|d| c(&d.id, &d.file)).collect(),
                Some(Arg::Quest) => db.quests.iter().map(|q| c(&q.id, &q.title)).collect(),
                Some(Arg::Skill) => rustpg2d_engine::srd::abilities::proficiency_choices().iter().map(|id| c(id, &rustpg2d_engine::srd::abilities::proficiency_name(id))).collect(),
                Some(Arg::Spell) => db.spells.iter().map(|q| c(&q.id, &q.name)).collect(),
                Some(Arg::Sheet) => db.sheets.iter().map(|q| c(&q.id, &q.image)).collect(),
                Some(Arg::State) => {
                    let mut v: Vec<Cand> = db.props.iter().flat_map(|p| p.state_names().into_iter().map(|s| (s, p.name.clone()))).map(|(s, p)| c(&s, &p)).collect();
                    v.sort_by(|a, b| a.insert.cmp(&b.insert));
                    v
                }
                Some(Arg::Target) => known.targets.iter().map(|t| c(t, if t == "self" { "the trigger's owner" } else if t == "player" { "the player" } else { "entity" })).collect(),
                Some(Arg::Flag) => known.flags.iter().map(|f| c(f, "flag")).collect(),
                Some(Arg::Var) => known.vars.iter().map(|v| c(v, "variable")).collect(),
                Some(Arg::Func) => known.functions.iter().map(|f| c(f, "script function")).collect(),
                Some(Arg::Dir) => ["up", "down", "left", "right"].iter().map(|d| c(d, "direction")).collect(),
                Some(Arg::Character) => db.characters.iter().map(|q| c(&q.id, &q.name)).collect(),
                Some(Arg::Achievement) => db.achievements.iter().map(|q| c(&q.id, &q.name)).collect(),
                Some(Arg::Prop) => db.props.iter().map(|q| c(&q.id, &q.name)).collect(),
                Some(Arg::Common) => db.common_events.iter().map(|q| c(&q.id, &q.name)).collect(),
                Some(Arg::Class) => db.classes.iter().map(|q| c(&q.id, &q.name)).collect(),
                Some(Arg::Disease) => db.diseases.iter().map(|q| c(&q.id, &q.name)).collect(),
                Some(Arg::Faction) => known.factions.iter().map(|f| c(f, "faction")).collect(),
                Some(Arg::Recipe) => db.recipes.iter().map(|q| c(&q.id, db.item(&q.result).map_or(q.result.as_str(), |d| d.name.as_str()))).collect(),
                Some(Arg::Station) => {
                    let mut v: Vec<String> = db.recipes.iter().map(|r| r.station.clone()).filter(|s| !s.is_empty()).collect();
                    v.sort();
                    v.dedup();
                    v.iter().map(|s| c(s, "crafting station")).collect()
                }
                Some(Arg::Effect) => rustpg2d_engine::srd::Condition::ALL.iter().map(|k| c(k.id(), k.name())).collect(),
                Some(Arg::Weather) => {
                    let mut v: Vec<Cand> = rustpg2d_engine::map::WeatherKind::ALL.iter().map(|k| c(k.id(), k.label())).collect();
                    v.push(c("default", "the map's own weather"));
                    v
                }
                _ => Vec::new(),
            };
            rank(list, partial)
        }
        Place::Code { partial, .. } => {
            // Without a typed prefix only Ctrl+Space lists everything.
            if partial.is_empty() && !force {
                return Vec::new();
            }
            let mut v: Vec<Cand> = Vec::new();
            for f in API {
                if v.iter().any(|x| x.insert == f.name) {
                    continue;
                }
                let sigs: Vec<String> = API.iter().filter(|g| g.name == f.name).map(signature).collect();
                // Deprecated ones say so (and what replaces them); new ones since when.
                let detail = match (f.deprecated, f.since) {
                    (Some(instead), _) => format!("DEPRECATED: use {instead}. {}", f.doc),
                    (None, "") => f.doc.to_string(),
                    (None, v) => format!("{} (since {v})", f.doc),
                };
                let label = if f.deprecated.is_some() { format!("{} (deprecated)", sigs.join("  |  ")) } else { sigs.join("  |  ") };
                v.push(Cand { insert: f.name.to_string(), label, detail });
            }
            for f in known.functions {
                v.push(Cand { insert: f.clone(), label: format!("{f}()"), detail: "script function".into() });
            }
            for l in local_names(src) {
                v.push(c(&l, "variable"));
            }
            for k in KEYWORDS {
                v.push(c(k, "keyword"));
            }
            v.push(c("SELF", "uid of the trigger's owner (-1 = none)"));
            let mut v = rank(v, partial);
            if partial.is_empty() {
                v.sort_by_key(|c| (c.detail == "keyword", c.insert.clone()));
            }
            v.retain(|x| x.insert != *partial);
            v
        }
    }
}

pub fn signature(f: &ApiFn) -> String {
    let ps: Vec<&str> = f.params.iter().map(|p| p.0).collect();
    format!("{}({})", f.name, ps.join(", "))
}

/// The call being written, for a hint above the text: its parameters, which of them the
/// cursor is in, and what the function does. Engine functions first (the overload that has
/// the current argument), then the project's own (`signatures`).
pub fn signature_of(call: &CallAt, signatures: &[(String, Vec<String>)]) -> Option<(Vec<String>, String)> {
    match API.iter().filter(|f| f.name == call.name).find(|f| f.params.len() > call.arg).or_else(|| API.iter().find(|f| f.name == call.name)) {
        Some(f) => Some((f.params.iter().map(|p| p.0.to_string()).collect(), f.doc.to_string())),
        None => {
            let (_, ps) = signatures.iter().find(|(n, _)| *n == call.name)?;
            Some((ps.clone(), "script function".to_string()))
        }
    }
}

/// The suggestions open under the cursor.
#[derive(Clone, Debug, Default)]
pub struct Completion {
    pub cands: Vec<Cand>,
    pub sel: usize,
    /// Char range replaced when accepting (start .. cursor).
    pub start: usize,
    pub cursor: usize,
    /// Opened with Ctrl+Space (stays open without a typed prefix).
    pub forced: bool,
}

/// The suggestions for the cursor at char `ci` of `text` (`None`: none to show). `typed`:
/// something was just typed; `force`: asked for with Ctrl+Space; `open`: a list is open.
pub fn complete(text: &str, ci: usize, known: &Known, typed: bool, force: bool, open: Option<&Completion>) -> Option<Completion> {
    let b = byte_of(text, ci);
    let (place, call) = analyze(&text[..b]);
    // Right after "(" or "," an id argument (item, map, sheet, function, ...) offers its ids at
    // once, inserted with their quotes.
    let id_arg = matches!(&place, Place::Code { partial, .. } if partial.is_empty()) && text[..b].trim_end().ends_with(['(', ',']) && call.as_ref().and_then(|c| arg_kind(&c.name, c.arg)).is_some_and(Arg::is_id);
    let start_b = match &place {
        Place::Code { start, .. } | Place::Str { start, .. } => *start,
        Place::Comment => b,
    };
    let show = match &place {
        Place::Str { .. } => typed || force || open.is_some(),
        Place::Code { partial, .. } => !partial.is_empty() || force,
        Place::Comment => false,
    };
    let cands = if id_arg && (typed || force || open.is_some()) {
        let ids = candidates(&Place::Str { partial: String::new(), start: b }, call.as_ref(), known, text, force);
        ids.into_iter().map(|c| Cand { insert: format!("\"{}\"", c.insert), ..c }).collect()
    } else if show {
        candidates(&place, call.as_ref(), known, text, force)
    } else {
        Vec::new()
    };
    let moved = open.is_some_and(|c| c.cursor != ci) && !typed;
    if cands.is_empty() || moved {
        return None;
    }
    let sel = open.map_or(0, |c| c.sel).min(cands.len() - 1);
    Some(Completion { cands, sel, start: text[..start_b].chars().count(), cursor: ci, forced: force })
}

/// Takes suggestion `k` of `comp` into `text`. Returns where the cursor goes (a char index).
pub fn accept_at(text: &mut String, comp: &Completion, k: usize) -> Option<usize> {
    let cand = comp.cands.get(k)?;
    let prefix = text[..byte_of(text, comp.cursor)].to_string();
    let is_str = matches!(analyze(&prefix).0, Place::Str { .. });
    Some(accept(text, comp, cand, is_str))
}

/// What typing adds by itself, after the text changed with the cursor now at char `ci`:
/// the indent of a new line (`enter`), the closer of a bracket or quote just typed (`typed`),
/// and nothing twice when a closer is typed over the one put there. Returns where the cursor
/// goes when it moves.
pub fn after_typing(text: &mut String, ci: usize, typed: Option<char>, enter: bool) -> Option<usize> {
    let b = byte_of(text, ci);
    if enter && text[..b].ends_with('\n') {
        let prev = text[..b - 1].rsplit('\n').next().unwrap_or("");
        let mut indent: String = prev.chars().take_while(|c| *c == ' ' || *c == '\t').collect();
        if prev.trim_end().ends_with('{') {
            indent.push_str("    ");
        }
        // Typing Enter in front of '}' keeps it one level back.
        if text[b..].trim_start_matches([' ', '\t']).starts_with('}') && indent.len() >= 4 && !prev.trim_end().ends_with('{') {
            indent.truncate(indent.len() - 4);
        }
        if !indent.is_empty() {
            text.insert_str(b, &indent);
            return Some(ci + indent.chars().count());
        }
        return None;
    }
    let c = typed?;
    let next = text[b..].chars().next();
    let closer = match c {
        '(' => Some(')'),
        '[' => Some(']'),
        '{' => Some('}'),
        '"' => Some('"'),
        _ => None,
    };
    if matches!(c, ')' | ']' | '}' | '"') && next == Some(c) && b > 0 {
        // Typed over an auto-inserted closer: drop the duplicate.
        text.remove(b);
        return None;
    }
    let cl = closer?;
    let line_before = text[..b].rsplit('\n').next().unwrap_or("");
    let open_quote = c != '"' || line_before.matches('"').count() % 2 == 1;
    let free = next.is_none_or(|n| n.is_whitespace() || matches!(n, ')' | ']' | '}' | ',' | ';'));
    if open_quote && free {
        text.insert(b, cl);
        return Some(ci);
    }
    None
}

/// The function name the char at `ci` of `text` is in ("" when it is in none).
pub fn word_at(text: &str, ci: usize) -> &str {
    let b = byte_of(text, ci);
    let start = text[..b].rfind(|c: char| !(c.is_alphanumeric() || c == '_')).map_or(0, |i| i + 1);
    let end = text[b..].find(|c: char| !(c.is_alphanumeric() || c == '_')).map_or(text.len(), |i| b + i);
    &text[start..end]
}

/// Something wrong in a script's text.
#[derive(Clone, Debug, PartialEq)]
pub struct Issue {
    /// The bytes it is about, and their line (from 1).
    pub span: (usize, usize),
    pub line: usize,
    pub message: String,
    pub warning: bool,
}

impl Workspace {
    /// The project's script files, by their names (`main.rhai`).
    pub fn script_names(&self) -> Vec<(String, PathBuf)> {
        self.project.script_files().into_iter().map(|p| (p.file_name().unwrap_or_default().to_string_lossy().to_string(), p)).collect()
    }

    /// The functions the project's scripts define with their parameters; `instead`: the text
    /// of one file as it is being edited, in place of what its file holds.
    pub fn script_signatures(&self, instead: Option<(&PathBuf, &str)>) -> Vec<(String, Vec<String>)> {
        let mut all = Vec::new();
        for f in self.project.script_files() {
            match instead {
                Some((path, text)) if *path == f => all.extend(defined_signatures(text)),
                _ => all.extend(std::fs::read_to_string(&f).map(|src| defined_signatures(&src)).unwrap_or_default()),
            }
        }
        all
    }

    /// What is wrong in `text`, a script of the project not saved yet: ids that aren't there,
    /// functions named in strings that no script defines (`functions`: those there are), and
    /// what the compiler says.
    pub fn script_issues(&self, text: &str, functions: &[String]) -> Vec<Issue> {
        let mut out: Vec<Issue> = rustpg2d_engine::script_refs::check_script(text, &self.project.db, &self.refs.maps, &self.refs.spawns).into_iter().map(|i| Issue { span: i.span, line: i.line, message: i.message, warning: i.warning }).collect();
        for a in rustpg2d_engine::script_refs::function_refs(text) {
            if !functions.contains(&a.value) {
                out.push(Issue { span: a.span, line: a.line, message: format!("{}(): there is no script function {}()", a.func, a.value), warning: false });
            }
        }
        if let Err(e) = rustpg2d_engine::script::ScriptHost::new().compile_check(text) {
            // (The compiler names a line: the whole of it is marked.)
            let line = crate::problems::message_line(&e).unwrap_or(1);
            let start: usize = text.split_inclusive('\n').take(line.saturating_sub(1)).map(str::len).sum();
            let end = start + text[start.min(text.len())..].lines().next().map_or(0, str::len);
            out.push(Issue { span: (start.min(text.len()), end.min(text.len()).max(start.min(text.len()))), line, message: e, warning: false });
        }
        out.sort_by_key(|i| i.span.0);
        out
    }
}

/// The byte char `ci` of `text` starts at.
pub fn byte_of(text: &str, ci: usize) -> usize {
    text.char_indices().nth(ci).map_or(text.len(), |(b, _)| b)
}

/// Inserts a completion: replaces the typed part and moves the cursor behind it.
fn accept(text: &mut String, comp: &Completion, cand: &Cand, place_is_str: bool) -> usize {
    let (a, b) = (byte_of(text, comp.start), byte_of(text, comp.cursor));
    let mut ins = cand.insert.clone();
    let after = &text[b..];
    let api_fn = !place_is_str && (API.iter().any(|f| f.name == ins) || cand.label.ends_with("()"));
    if api_fn && !after.starts_with('(') {
        // name() with the cursor between the parentheses.
        ins.push_str("()");
        text.replace_range(a..b, &ins);
        return comp.start + ins.chars().count() - 1;
    }
    if !place_is_str && cand.detail == "keyword" && !matches!(ins.as_str(), "true" | "false" | "this") && !after.starts_with(' ') {
        ins.push(' ');
    }
    // In a string: complete the closing quote and continue after it.
    let skip = usize::from(place_is_str && after.starts_with('"'));
    if place_is_str && skip == 0 {
        ins.push('"');
    }
    text.replace_range(a..b, &ins);
    comp.start + ins.chars().count() + skip
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_at_cursor() {
        let (p, c) = analyze("fn a() {\n    give_item(\"po");
        assert_eq!(p, Place::Str { partial: "po".into(), start: 24 });
        assert_eq!(c.unwrap(), CallAt { name: "give_item".into(), arg: 0, first: None });
        let (p, c) = analyze("teleport(\"forest\", \"we");
        assert!(matches!(p, Place::Str { .. }));
        assert_eq!(c.unwrap(), CallAt { name: "teleport".into(), arg: 1, first: Some("forest".into()) });
        let (p, c) = analyze("if has_it");
        assert_eq!(p, Place::Code { partial: "has_it".into(), start: 3 });
        assert!(c.is_none());
        // Inside a nested list the string isn't the call's argument.
        let (p, c) = analyze("patrol(\"guard\", [[1, 2], \"x");
        assert!(matches!(p, Place::Str { .. }) && c.is_none());
        assert_eq!(analyze("say(\"hi\"); // give_item(\"").0, Place::Comment);
        assert_eq!(arg_kind("teleport", 1), Some(Arg::Spawn));
        assert_eq!(arg_kind("say", 1), Some(Arg::Line));
    }

    #[test]
    fn suggestions() {
        let mut db = Database::default();
        db.quests.push(rustpg2d_engine::quest::QuestDef { id: "pest_control".into(), title: "Pest Control".into(), ..Default::default() });
        let maps = vec!["village".to_string(), "forest".to_string()];
        let spawns: HashMap<String, Vec<String>> = [("forest".to_string(), vec!["west".to_string(), "cave".to_string()])].into();
        let none: Vec<String> = Vec::new();
        let funcs = vec!["guard_talk".to_string()];
        let sigs = vec![("guard_talk".to_string(), vec!["who".to_string(), "times".to_string()])];
        let known = Known { db: &db, maps: &maps, spawns: &spawns, targets: &none, flags: &none, vars: &none, functions: &funcs, signatures: &sigs, factions: &none };
        let get = |src: &str| {
            let (p, c) = analyze(src);
            candidates(&p, c.as_ref(), &known, src, false).into_iter().map(|c| c.insert).collect::<Vec<_>>()
        };
        assert_eq!(get("teleport(\"forest\", \""), vec!["cave", "west"]);
        assert_eq!(get("teleport(\"f"), vec!["forest"]);
        assert_eq!(get("start_quest(\"pe"), vec!["pest_control"]);
        assert_eq!(get("then_call(\""), vec!["guard_talk"]);
        // Spell effects: callbacks complete script functions, sprites the sheets.
        assert_eq!(get("aim(\""), vec!["guard_talk"]);
        assert_eq!(get("fly(\"fireball\", x, y, \""), vec!["guard_talk"]);
        assert!(get("light_").contains(&"light_at".to_string()));
        assert!(get("heal_a").contains(&"heal_area".to_string()));
        assert!(get("player_").contains(&"player_max_hp".to_string()));
        // Signature hints after every comma, also for the project's own functions.
        let hint = |src: &str| analyze(src).1.and_then(|c| signature_of(&c, &sigs).map(|(ps, _)| format!("{}({})", c.name, ps.join(", "))));
        for (src, want) in [
            ("light_at(player_x() + 1, ", "light_at(x, y, radius, seconds)"),
            ("fly(\"fireball\", x, y, \"f\", ", "fly(sheet, x, y, function, options)"),
            ("say(\"Mira\", ", "say(speaker, text)"),
            ("guard_talk(1, ", "guard_talk(who, times)"),
            ("patrol(\"guard\", [[1, 2], ", "patrol(target, points)"),
        ] {
            assert!(hint(src).is_some_and(|h| h.starts_with(want)), "{src}: {:?}", hint(src));
        }
        assert_eq!(defined_signatures("fn a() {}\nfn guard_talk(who, times) { a(); }"), vec![("a".to_string(), vec![]), ("guard_talk".to_string(), vec!["who".to_string(), "times".to_string()])]);
        let code = get("let total = 1; tot");
        assert_eq!(code.first().map(String::as_str), Some("total"), "{code:?}");
        assert!(get("stop_pa").starts_with(&["stop_patrol".to_string()]));
        assert!(get("guard").contains(&"guard_talk".to_string()));
    }


    #[test]
    fn scripts_are_painted_and_their_functions_found() {
        let src = "fn a() { say(\"x\"); b(1); } // c\nfn b(n) {}";
        assert_eq!(defined_functions(src), vec!["a", "b"]);
        let pieces = paint(src, &[]);
        let of = |t: &str| pieces.iter().find(|p| &src[p.1..p.2] == t).map(|p| p.0);
        assert_eq!(of("fn"), Some(Paint::Keyword));
        assert_eq!(of("say"), Some(Paint::Api));
        assert_eq!(of("b"), Some(Paint::Func));
        assert_eq!(of("// c"), Some(Paint::Comment));
        assert_eq!(of("\"x\""), Some(Paint::Str));
        assert_eq!(pieces.iter().map(|p| p.2 - p.1).sum::<usize>(), src.len(), "every byte is in a piece");
    }

    #[test]
    fn typing_closes_brackets_and_indents() {
        // A bracket gets its closer, the cursor stays between them.
        let mut t = "say(".to_string();
        assert_eq!(after_typing(&mut t, 4, Some('('), false), Some(4));
        assert_eq!(t, "say()");
        // The closer typed over the one put there isn't there twice.
        let mut t = "say())".to_string();
        assert_eq!(after_typing(&mut t, 5, Some(')'), false), None);
        assert_eq!(t, "say()");
        // Enter after an opening brace indents a level; in front of the closing one, none.
        let mut t = "fn a() {\n".to_string();
        assert_eq!(after_typing(&mut t, 9, None, true), Some(13));
        assert_eq!(t, "fn a() {\n    ");
        let mut t = "    if x {\n        y();\n}".to_string();
        assert_eq!(after_typing(&mut t, 24, None, true), Some(28));
        // A suggestion taken: the name with its brackets, the cursor inside them.
        let db = Database::default();
        let (none, spawns): (Vec<String>, HashMap<String, Vec<String>>) = Default::default();
        let known = Known { db: &db, maps: &none, spawns: &spawns, targets: &none, flags: &none, vars: &none, functions: &none, signatures: &[], factions: &none };
        let mut t = "give_it".to_string();
        let comp = complete(&t, 7, &known, true, false, None).unwrap();
        assert_eq!(comp.cands[0].insert, "give_item");
        assert_eq!(accept_at(&mut t, &comp, 0), Some(10));
        assert_eq!(t, "give_item()");
        assert_eq!(word_at("  guard_talk(1)", 6), "guard_talk");
    }
}
