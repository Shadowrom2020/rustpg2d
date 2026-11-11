//! Ids inside Rhai scripts: string literals passed to API functions that expect an item,
//! map, sound, quest, ... (`give_item("key", 1)`, `teleport("forest", "west")`).
//!
//! A small scanner (not a full Rhai parser) finds calls and their string arguments, skipping
//! comments and other strings. Renames use it to update scripts; the editor's script check
//! uses it to report ids that don't exist.

use std::collections::{BTreeSet, HashMap};

use crate::defs::Database;
use crate::refactor::RefKind;

/// What kind of id an API argument is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdKind {
    Item,
    Map,
    /// A spawn point on the map given by the previous argument.
    Spawn,
    Sound,
    Quest,
    /// A prop state name.
    State,
    Skill,
    Spell,
    /// A sprite sheet (animated effects).
    Sheet,
    /// A character definition.
    Character,
    /// A crafting recipe.
    Recipe,
    Achievement,
    /// A prop definition.
    Prop,
}

impl IdKind {
    pub fn label(self) -> &'static str {
        match self {
            IdKind::Item => "item",
            IdKind::Map => "map",
            IdKind::Spawn => "spawn point",
            IdKind::Sound => "sound",
            IdKind::Quest => "quest",
            IdKind::State => "prop state",
            IdKind::Skill => "skill",
            IdKind::Spell => "spell",
            IdKind::Sheet => "sprite sheet",
            IdKind::Character => "character",
            IdKind::Recipe => "recipe",
            IdKind::Achievement => "achievement",
            IdKind::Prop => "prop",
        }
    }
    /// The definition kind renamed together with this id kind.
    pub fn ref_kind(self) -> Option<RefKind> {
        match self {
            IdKind::Item => Some(RefKind::Item),
            IdKind::Map => Some(RefKind::Map),
            IdKind::Sound => Some(RefKind::Sound),
            IdKind::Sheet => Some(RefKind::Sheet),
            IdKind::Skill => Some(RefKind::Skill),
            IdKind::Spell => Some(RefKind::Spell),
            IdKind::Character => Some(RefKind::Character),
            IdKind::Recipe => Some(RefKind::Recipe),
            IdKind::Prop => Some(RefKind::Prop),
            _ => None,
        }
    }
}


/// A string literal argument of a function call.
#[derive(Clone, Debug, PartialEq)]
pub struct StrArg {
    pub func: String,
    pub arg: usize,
    pub value: String,
    /// Byte range of the literal including its quotes.
    pub span: (usize, usize),
    /// 1-based line.
    pub line: usize,
    /// String literals passed as the other arguments of the same call (for `Spawn`).
    pub siblings: Vec<Option<String>>,
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Ident(String),
    Str(String, usize, usize),
    Punct(char),
    Other,
}

/// A token of Rhai source. [`lex`] covers every byte (comments and whitespace included), so the
/// editor's highlighter and the reference finders share one lexer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lexeme {
    Comment,
    /// A string (`"…"`, `'…'` or `` `…` ``); `closed` = its closing quote was found. An
    /// unterminated `"` / `'` string ends with its line, a backtick string with the file.
    Str { closed: bool },
    Num,
    Ident,
    Punct,
    Space,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Token {
    pub kind: Lexeme,
    pub start: usize,
    pub end: usize,
}

/// Splits source into contiguous tokens covering every byte.
pub fn lex(src: &str) -> Vec<Token> {
    let b = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let start = i;
        let c = b[i];
        let kind = if c == b'/' && b.get(i + 1) == Some(&b'/') {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            Lexeme::Comment
        } else if c == b'/' && b.get(i + 1) == Some(&b'*') {
            let mut depth = 0;
            while i < b.len() {
                if b[i] == b'/' && b.get(i + 1) == Some(&b'*') {
                    depth += 1;
                    i += 2;
                } else if b[i] == b'*' && b.get(i + 1) == Some(&b'/') {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            Lexeme::Comment
        } else if c == b'"' || c == b'`' || c == b'\'' {
            i += 1;
            let mut closed = false;
            while i < b.len() {
                if b[i] == b'\\' {
                    i += 2;
                } else if b[i] == c {
                    i += 1;
                    closed = true;
                    break;
                } else if b[i] == b'\n' && c != b'`' {
                    break; // unterminated: the rest of the line
                } else {
                    i += 1;
                }
            }
            i = i.min(b.len());
            Lexeme::Str { closed }
        } else if c.is_ascii_digit() {
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'.' || b[i] == b'_') {
                i += 1;
            }
            Lexeme::Num
        } else if c.is_ascii_alphabetic() || c == b'_' {
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            Lexeme::Ident
        } else if c.is_ascii_whitespace() {
            while i < b.len() && b[i].is_ascii_whitespace() {
                i += 1;
            }
            Lexeme::Space
        } else {
            i += src[i..].chars().next().map_or(1, |ch| ch.len_utf8());
            Lexeme::Punct
        };
        out.push(Token { kind, start, end: i });
    }
    out
}

/// The value of a `"…"` string token: its text without the quotes, `\n`, `\t` and `\x` (any
/// other escaped character) resolved.
pub fn string_value(src: &str, t: &Token) -> String {
    let inner = &src[t.start + 1..t.end];
    let inner = inner.strip_suffix('"').unwrap_or(inner);
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some(e) => out.push(e),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// The tokens the scanner cares about (with byte offsets): identifiers, `"…"` strings with
/// their value, the brackets and commas; comments and whitespace left out.
fn tokenize(src: &str) -> Vec<(Tok, usize)> {
    lex(src)
        .into_iter()
        .filter_map(|t| {
            let text = &src[t.start..t.end];
            Some(match t.kind {
                Lexeme::Comment | Lexeme::Space => return None,
                Lexeme::Str { .. } if text.starts_with('"') => (Tok::Str(string_value(src, &t), t.start, t.end), t.start),
                // Interpolated strings, characters and numbers: not ids.
                Lexeme::Str { .. } | Lexeme::Num => (Tok::Other, t.start),
                Lexeme::Ident => (Tok::Ident(text.to_string()), t.start),
                Lexeme::Punct => match text.chars().next() {
                    Some(c @ ('(' | ')' | ',' | '[' | ']' | '{' | '}')) => (Tok::Punct(c), t.start),
                    _ => (Tok::Other, t.start),
                },
            })
        })
        .collect()
}

/// The calls in a token stream: function name, its arguments (tokens at nesting depth 1,
/// split by commas) and the byte offset of the name.
fn calls(toks: &[(Tok, usize)]) -> Vec<(String, Vec<Vec<&Tok>>, usize)> {
    let mut out = Vec::new();
    for k in 0..toks.len() {
        let (Tok::Ident(name), pos) = &toks[k] else { continue };
        if toks.get(k + 1).map(|t| &t.0) != Some(&Tok::Punct('(')) {
            continue;
        }
        if k > 0 && toks[k - 1].0 == Tok::Ident("fn".into()) {
            continue; // a definition, not a call
        }
        let mut args: Vec<Vec<&Tok>> = vec![Vec::new()];
        let mut depth = 0;
        for (t, _) in &toks[k + 1..] {
            match t {
                Tok::Punct('(' | '[' | '{') => {
                    depth += 1;
                    if depth == 1 {
                        continue;
                    }
                }
                Tok::Punct(')' | ']' | '}') => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                Tok::Punct(',') if depth == 1 => {
                    args.push(Vec::new());
                    continue;
                }
                _ => {}
            }
            args.last_mut().unwrap().push(t);
        }
        out.push((name.clone(), args, *pos));
    }
    out
}

/// Every string literal that is a whole argument of a function call.
pub fn string_args(src: &str) -> Vec<StrArg> {
    let toks = tokenize(src);
    let line_of = |pos: usize| src[..pos].bytes().filter(|&b| b == b'\n').count() + 1;
    let mut out = Vec::new();
    for (name, args, _) in calls(&toks) {
        let literal = |a: &Vec<&Tok>| match a.as_slice() {
            [Tok::Str(v, s, e)] => Some((v.clone(), *s, *e)),
            _ => None,
        };
        let siblings: Vec<Option<String>> = args.iter().map(|a| literal(a).map(|l| l.0)).collect();
        for (n, a) in args.iter().enumerate() {
            if let Some((value, s, e)) = literal(a) {
                out.push(StrArg { func: name.clone(), arg: n, value, span: (s, e), line: line_of(s), siblings: siblings.clone() });
            }
        }
    }
    out
}

/// An array of string literals passed as an argument (`choice("…", ["Yes", "No"], "var")`).
#[derive(Clone, Debug, PartialEq)]
pub struct ListArg {
    pub func: String,
    pub arg: usize,
    pub values: Vec<String>,
    /// 1-based line of the call.
    pub line: usize,
}

/// Every argument that is an array of string literals only.
pub fn list_args(src: &str) -> Vec<ListArg> {
    let toks = tokenize(src);
    let line_of = |pos: usize| src[..pos].bytes().filter(|&b| b == b'\n').count() + 1;
    let mut out = Vec::new();
    for (name, args, pos) in calls(&toks) {
        for (n, a) in args.iter().enumerate() {
            let (Some(Tok::Punct('[')), Some(Tok::Punct(']'))) = (a.first().copied(), a.last().copied()) else { continue };
            let inner = &a[1..a.len() - 1];
            let values: Vec<String> = inner.iter().filter_map(|t| if let Tok::Str(v, _, _) = t { Some(v.clone()) } else { None }).collect();
            let only_strings = inner.iter().all(|t| matches!(t, Tok::Str(..) | Tok::Punct(',')));
            if only_strings && a.len() >= 2 {
                out.push(ListArg { func: name.clone(), arg: n, values, line: line_of(pos) });
            }
        }
    }
    out
}

/// The id arguments of known API calls, with their kind.
pub fn id_refs(src: &str) -> Vec<(StrArg, IdKind)> {
    string_args(src)
        .into_iter()
        .filter_map(|a| crate::script_api::api_ids().iter().find(|(f, n, _)| *f == a.func && *n == a.arg).map(|(_, _, k)| (a, *k)))
        .collect()
}

fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Renames an id in a script: only string literals that are whole arguments of API calls
/// expecting that kind of id, and only exact matches. Returns the new source and the lines
/// that changed (empty = unchanged).
pub fn rename_in_script(src: &str, kind: RefKind, old: &str, new: &str) -> (String, Vec<usize>) {
    let mut hits: Vec<(usize, usize, usize)> = id_refs(src)
        .into_iter()
        .filter(|(a, k)| k.ref_kind() == Some(kind) && a.value == old)
        .map(|(a, _)| (a.span.0, a.span.1, a.line))
        .collect();
    if hits.is_empty() {
        return (src.to_string(), Vec::new());
    }
    hits.sort();
    let mut out = src.to_string();
    for (s, e, _) in hits.iter().rev() {
        out.replace_range(*s..*e, &quote(new));
    }
    let mut lines: Vec<usize> = hits.iter().map(|h| h.2).collect();
    lines.dedup();
    (out, lines)
}

/// A top-level function of a script file.
#[derive(Clone, Debug, PartialEq)]
pub struct ScriptFn {
    pub name: String,
    pub params: Vec<String>,
    /// The whole definition, with the `//` comment lines right above it.
    pub text: String,
    /// Byte offset of the `fn` keyword in the source.
    pub at: usize,
}

/// The top-level functions of a script source, in order.
pub fn functions(src: &str) -> Vec<ScriptFn> {
    let toks = tokenize(src);
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut k = 0;
    while k < toks.len() {
        match &toks[k].0 {
            Tok::Punct('{') => depth += 1,
            Tok::Punct('}') => depth -= 1,
            Tok::Ident(kw) if kw == "fn" && depth == 0 => {
                let Some((Tok::Ident(name), _)) = toks.get(k + 1) else {
                    k += 1;
                    continue;
                };
                // Parameters up to ")", then the body from "{" to its matching "}".
                let mut j = k + 2;
                let mut params = Vec::new();
                while j < toks.len() && toks[j].0 != Tok::Punct(')') {
                    if let Tok::Ident(p) = &toks[j].0 {
                        params.push(p.clone());
                    }
                    j += 1;
                }
                while j < toks.len() && toks[j].0 != Tok::Punct('{') {
                    j += 1;
                }
                let mut d = 0;
                let mut end = src.len();
                while j < toks.len() {
                    match toks[j].0 {
                        Tok::Punct('{') => d += 1,
                        Tok::Punct('}') => {
                            d -= 1;
                            if d == 0 {
                                end = toks[j].1 + 1;
                                break;
                            }
                        }
                        _ => {}
                    }
                    j += 1;
                }
                // Comment lines directly above belong to the function.
                let mut start = src[..toks[k].1].rfind('\n').map_or(0, |i| i + 1);
                while start > 0 {
                    let prev_start = src[..start - 1].rfind('\n').map_or(0, |i| i + 1);
                    if !src[prev_start..start - 1].trim_start().starts_with("//") {
                        break;
                    }
                    start = prev_start;
                }
                out.push(ScriptFn { name: name.clone(), params, text: src[start..end].to_string(), at: toks[k].1 });
                k = j + 1;
                continue;
            }
            _ => {}
        }
        k += 1;
    }
    out
}

/// Script functions (of `names`) a function uses: calls, and names passed as strings
/// (`then_call("f")`, `aim("f")`, `fly(..., "f")`, `cutscene("f")`).
pub fn used_functions(f: &ScriptFn, names: &BTreeSet<String>) -> BTreeSet<String> {
    let toks = tokenize(&f.text);
    let mut out = BTreeSet::new();
    for k in 0..toks.len() {
        match &toks[k].0 {
            Tok::Ident(n) if names.contains(n) => {
                let call = toks.get(k + 1).is_some_and(|t| t.0 == Tok::Punct('('));
                let def = k > 0 && toks[k - 1].0 == Tok::Ident("fn".into());
                if call && !def {
                    out.insert(n.clone());
                }
            }
            Tok::Str(v, _, _) if names.contains(v) => {
                out.insert(v.clone());
            }
            _ => {}
        }
    }
    out.remove(&f.name);
    out
}

/// `roots` and every function they use, directly or through others.
pub fn needed_functions(fns: &[ScriptFn], roots: &[String]) -> BTreeSet<String> {
    let names: BTreeSet<String> = fns.iter().map(|f| f.name.clone()).collect();
    let mut seen = BTreeSet::new();
    let mut stack: Vec<String> = roots.iter().filter(|r| names.contains(*r)).cloned().collect();
    while let Some(n) = stack.pop() {
        if !seen.insert(n.clone()) {
            continue;
        }
        if let Some(f) = fns.iter().find(|f| f.name == n) {
            stack.extend(used_functions(f, &names).into_iter().filter(|u| !seen.contains(u)));
        }
    }
    seen
}

/// A problem found by [`check_script`].
#[derive(Clone, Debug, PartialEq)]
pub struct ScriptIssue {
    pub line: usize,
    pub message: String,
    /// Byte range of the offending string literal.
    pub span: (usize, usize),
}

/// Reports ids in API calls that don't exist in the project. `spawns`: map -> spawn names.
pub fn check_script(src: &str, db: &Database, maps: &[String], spawns: &HashMap<String, Vec<String>>) -> Vec<ScriptIssue> {
    let states: Vec<String> = db.props.iter().flat_map(|p| p.state_names()).collect();
    let mut out = Vec::new();
    for (a, kind) in id_refs(src) {
        let v = a.value.as_str();
        let known = match kind {
            IdKind::Item => db.item(v).is_some(),
            IdKind::Map => maps.iter().any(|m| m == v),
            IdKind::Spawn => match a.siblings.first().cloned().flatten() {
                Some(map) if maps.contains(&map) => spawns.get(&map).is_some_and(|s| s.iter().any(|x| x == v)),
                _ => true, // map unknown or computed: can't tell
            },
            IdKind::Sound => db.sound(v).is_some(),
            IdKind::Quest => db.quest(v).is_some(),
            IdKind::State => states.iter().any(|s| s == v),
            IdKind::Skill => db.skill(v).is_some(),
            IdKind::Spell => db.spell(v).is_some(),
            IdKind::Sheet => db.sheet(v).is_some(),
            IdKind::Character => db.character(v).is_some(),
            IdKind::Recipe => db.recipe(v).is_some(),
            IdKind::Achievement => db.achievement(v).is_some(),
            IdKind::Prop => db.prop(v).is_some(),
        };
        if !known {
            let extra = match kind {
                IdKind::Spawn => format!(" on map '{}'", a.siblings.first().cloned().flatten().unwrap_or_default()),
                _ => String::new(),
            };
            out.push(ScriptIssue { line: a.line, message: format!("{}(\"{v}\"): unknown {}{extra}", a.func, kind.label()), span: a.span });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The lexer covers every byte, ends an unterminated string with its line and nests comments.
    #[test]
    fn lexer_covers_the_source() {
        let src = "fn a(x) { /* c /* d */ */ say(\"hi\\n\"); 'q' 1.5 }\nlet s = \"open\nfn b() {}";
        let toks = lex(src);
        assert_eq!(toks[0].start, 0);
        assert!(toks.windows(2).all(|w| w[0].end == w[1].start), "contiguous");
        assert_eq!(toks.last().unwrap().end, src.len());
        let kinds: Vec<Lexeme> = toks.iter().filter(|t| !matches!(t.kind, Lexeme::Space)).map(|t| t.kind).take(8).collect();
        assert_eq!(kinds, [Lexeme::Ident, Lexeme::Ident, Lexeme::Punct, Lexeme::Ident, Lexeme::Punct, Lexeme::Punct, Lexeme::Comment, Lexeme::Ident]);
        let strings: Vec<(bool, &str)> = toks.iter().filter_map(|t| match t.kind { Lexeme::Str { closed } => Some((closed, &src[t.start..t.end])), _ => None }).collect();
        assert_eq!(strings, [(true, "\"hi\\n\""), (true, "'q'"), (false, "\"open")]);
        let hi = toks.iter().find(|t| matches!(t.kind, Lexeme::Str { .. })).unwrap();
        assert_eq!(string_value(src, hi), "hi\n");
        // The unterminated string didn't swallow the next line: both functions are found.
        assert_eq!(functions(src).iter().map(|f| (f.name.as_str(), f.at)).collect::<Vec<_>>(), [("a", 0), ("b", src.rfind("fn b").unwrap())]);
    }

    #[test]
    fn list_arguments() {
        let src = "fn talk() {\n    choice(\"Walk?\", [\"Sure\", \"No thanks\"], \"walk\");\n    f([1, 2]); g([]);\n}";
        let l = list_args(src);
        assert_eq!(l, vec![ListArg { func: "choice".into(), arg: 1, values: vec!["Sure".into(), "No thanks".into()], line: 2 }, ListArg { func: "g".into(), arg: 0, values: vec![], line: 3 }]);
        let s = string_args(src);
        assert_eq!(s.iter().map(|a| (a.arg, a.value.as_str())).collect::<Vec<_>>(), vec![(0, "Walk?"), (2, "walk")]);
    }

    const SRC: &str = r#"
// give_item("key", 1) in a comment stays
fn reward() {
    give_item("key", 1);   /* nested /* comment */ give_item("key") */
    say("Here is your key");            // not an id argument
    if has_item( "key" ) && item_count("keys") > 0 { teleport("village", "start"); }
    let k = "key";
    take_item(k, 1);
    set_state("gate", "open");
    play_sound(`key`);
}
"#;

    #[test]
    fn finds_id_arguments() {
        let refs = id_refs(SRC);
        let got: Vec<(String, IdKind, usize)> = refs.iter().map(|(a, k)| (a.value.clone(), *k, a.line)).collect();
        assert_eq!(
            got,
            vec![
                ("key".into(), IdKind::Item, 4),
                ("key".into(), IdKind::Item, 6),
                ("keys".into(), IdKind::Item, 6),
                ("village".into(), IdKind::Map, 6),
                ("start".into(), IdKind::Spawn, 6),
                ("open".into(), IdKind::State, 9),
            ]
        );
    }

    #[test]
    fn renames_only_id_arguments() {
        let (out, lines) = rename_in_script(SRC, RefKind::Item, "key", "gate_key");
        assert_eq!(lines, vec![4, 6]);
        assert!(out.contains(r#"give_item("gate_key", 1)"#) && out.contains(r#"has_item( "gate_key" )"#));
        // Comments, other strings, variables and other ids are untouched.
        assert!(out.contains(r#"// give_item("key", 1) in a comment"#) && out.contains(r#"give_item("key") */"#));
        assert!(out.contains(r#"say("Here is your key")"#) && out.contains(r#"let k = "key";"#) && out.contains(r#"item_count("keys")"#));
        assert_eq!(rename_in_script(SRC, RefKind::Map, "village", "town").1, vec![6]);
        assert!(rename_in_script(SRC, RefKind::Sound, "key", "x").1.is_empty());
    }

    #[test]
    fn example_script_has_no_unknown_ids() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../example_project");
        let project = crate::project::Project::load(&root).unwrap();
        let maps = project.list_maps();
        let spawns: HashMap<String, Vec<String>> = maps
            .iter()
            .map(|m| {
                let map = project.load_map(m).unwrap();
                (m.clone(), map.zones.iter().map(|z| z.name.clone()).collect())
            })
            .collect();
        let src = std::fs::read_to_string(root.join("scripts/main.rhai")).unwrap();
        assert_eq!(check_script(&src, &project.db, &maps, &spawns), vec![]);
    }

    #[test]
    fn reports_unknown_ids() {
        let mut db = Database::default();
        db.items.push(crate::defs::ItemDef { id: "key".into(), name: "Key".into(), icon: crate::defs::SpriteRef::default(), ..Default::default() });
        let maps = vec!["village".to_string()];
        let spawns: HashMap<String, Vec<String>> = [("village".to_string(), vec!["gate".to_string()])].into();
        let issues = check_script(SRC, &db, &maps, &spawns);
        let msgs: Vec<String> = issues.iter().map(|i| format!("{}: {}", i.line, i.message)).collect();
        assert_eq!(
            msgs,
            vec![
                "6: item_count(\"keys\"): unknown item".to_string(),
                "6: teleport(\"start\"): unknown spawn point on map 'village'".to_string(),
                "9: set_state(\"open\"): unknown prop state".to_string(),
            ]
        );
    }
}
