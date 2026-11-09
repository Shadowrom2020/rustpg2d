//! Rhai script editor for Database → Scripts: syntax highlighting, line numbers,
//! context-aware completion (API functions, script functions, keywords, and inside string
//! arguments the project's items, maps, quests, flags, variables, ...), a signature hint,
//! auto-indent, closing pairs and Ctrl+click "go to definition".

use std::collections::HashMap;

use egui_macroquad::egui::{self, text::LayoutJob, Color32, FontId, Key, Modifiers, Stroke, TextFormat};
use rustpg2d_engine::defs::Database;

pub use rustpg2d_engine::script_api::{ApiFn, Arg, API};

const KEYWORDS: &[&str] = &[
    "fn", "let", "const", "if", "else", "while", "loop", "for", "in", "return", "break", "continue", "switch", "do", "until", "true", "false",
    "private", "throw", "try", "catch", "import", "export", "as", "global", "this",
];

// ------------------------------------------------------------------ lexer

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

// ------------------------------------------------------------------ highlighting

const C_COMMENT: Color32 = Color32::from_rgb(110, 150, 110);
const C_STRING: Color32 = Color32::from_rgb(230, 170, 110);
const C_NUMBER: Color32 = Color32::from_rgb(180, 210, 140);
const C_KEYWORD: Color32 = Color32::from_rgb(200, 140, 230);
const C_API: Color32 = Color32::from_rgb(240, 210, 110);
const C_FUNC: Color32 = Color32::from_rgb(120, 190, 250);
const C_CONST: Color32 = Color32::from_rgb(110, 220, 220);
const C_TEXT: Color32 = Color32::from_rgb(215, 215, 215);
const C_PUNCT: Color32 = Color32::from_rgb(170, 170, 170);

/// Colours a script. `problems`: byte ranges to underline in red; `user_fns`: functions
/// defined in the project's scripts.
pub fn highlight(src: &str, font: FontId, problems: &[(usize, usize)], user_fns: &[String]) -> LayoutJob {
    let mut job = LayoutJob::default();
    let toks = lex(src);
    let next_sig = |k: usize| toks[k + 1..].iter().find(|t| t.0 != Tk::Space).map(|t| &src[t.1..t.2]);
    for (k, &(kind, s, e)) in toks.iter().enumerate() {
        let text = &src[s..e];
        let color = match kind {
            Tk::Comment => C_COMMENT,
            Tk::Str { .. } => C_STRING,
            Tk::Num => C_NUMBER,
            Tk::Space => C_TEXT,
            Tk::Punct => C_PUNCT,
            Tk::Ident if KEYWORDS.contains(&text) => C_KEYWORD,
            Tk::Ident if text == "SELF" => C_CONST,
            Tk::Ident if next_sig(k) == Some("(") && API.iter().any(|f| f.name == text) => C_API,
            Tk::Ident if next_sig(k) == Some("(") || user_fns.iter().any(|f| f == text) => C_FUNC,
            Tk::Ident => C_TEXT,
        };
        let bad = problems.iter().any(|&(a, b)| s < b && a < e);
        let fmt = TextFormat {
            font_id: font.clone(),
            color: if bad { Color32::from_rgb(255, 120, 110) } else { color },
            underline: if bad { Stroke::new(1.5_f32, Color32::RED) } else { Stroke::NONE },
            ..Default::default()
        };
        job.append(text, 0.0, fmt);
    }
    job
}

// ------------------------------------------------------------------ context & completion

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
                Some(Arg::Skill) => db.skills.iter().map(|q| c(&q.id, &q.name)).collect(),
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
                Some(Arg::Recipe) => db.recipes.iter().map(|q| c(&q.id, db.item(&q.result).map_or(q.result.as_str(), |d| d.name.as_str()))).collect(),
                Some(Arg::Station) => {
                    let mut v: Vec<String> = db.recipes.iter().map(|r| r.station.clone()).filter(|s| !s.is_empty()).collect();
                    v.sort();
                    v.dedup();
                    v.iter().map(|s| c(s, "crafting station")).collect()
                }
                Some(Arg::Effect) => rustpg2d_engine::effects::EffectKind::ALL.iter().map(|k| c(k.id(), k.label())).collect(),
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
                v.push(Cand { insert: f.name.to_string(), label: sigs.join("  |  "), detail: f.doc.to_string() });
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

/// Signature line for a call: the overload that has the current argument, with that
/// parameter highlighted.
/// Engine functions first, then the project's own script functions (`signatures`).
fn signature_job(call: &CallAt, font: FontId, signatures: &[(String, Vec<String>)]) -> Option<LayoutJob> {
    let (params, doc): (Vec<&str>, &str) = match API.iter().filter(|f| f.name == call.name).find(|f| f.params.len() > call.arg).or_else(|| API.iter().find(|f| f.name == call.name)) {
        Some(f) => (f.params.iter().map(|p| p.0).collect(), f.doc),
        None => {
            let (_, ps) = signatures.iter().find(|(n, _)| *n == call.name)?;
            (ps.iter().map(String::as_str).collect(), "script function")
        }
    };
    let mut job = LayoutJob::default();
    let normal = TextFormat { font_id: font.clone(), color: Color32::LIGHT_GRAY, ..Default::default() };
    let strong = TextFormat { font_id: font.clone(), color: Color32::YELLOW, underline: Stroke::new(1.0_f32, Color32::YELLOW), ..Default::default() };
    job.append(&format!("{}(", call.name), 0.0, normal.clone());
    for (k, p) in params.iter().enumerate() {
        if k > 0 {
            job.append(", ", 0.0, normal.clone());
        }
        job.append(p, 0.0, if k == call.arg { strong.clone() } else { normal.clone() });
    }
    job.append(")", 0.0, normal.clone());
    job.append(&format!("   {doc}"), 0.0, TextFormat { font_id: font, color: Color32::GRAY, ..Default::default() });
    Some(job)
}

// ------------------------------------------------------------------ editor widget

/// Completion popup state.
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

#[derive(Default)]
pub struct EditorState {
    pub comp: Option<Completion>,
    /// Char index to put the cursor at next frame.
    pub set_cursor: Option<usize>,
    /// Char index to scroll into view (top of the editor) next frame.
    pub scroll_to: Option<usize>,
}

/// What the editor asks the caller to do.
#[derive(Default)]
pub struct EditorResult {
    pub changed: bool,
    /// Ctrl+click on a function name: go to its definition.
    pub goto: Option<String>,
}

fn byte_of(text: &str, ci: usize) -> usize {
    text.char_indices().nth(ci).map_or(text.len(), |(b, _)| b)
}

fn set_cursor(ctx: &egui::Context, id: egui::Id, ci: usize) {
    let mut state = egui::text_edit::TextEditState::load(ctx, id).unwrap_or_default();
    state.cursor.set_char_range(Some(egui::text::CCursorRange::one(egui::text::CCursor::new(ci))));
    state.store(ctx, id);
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

/// The script editor. `issues`: byte ranges of problems in `text`.
pub fn show(ui: &mut egui::Ui, text: &mut String, st: &mut EditorState, known: &Known, issues: &[(usize, usize)]) -> EditorResult {
    let mut result = EditorResult::default();
    let id = egui::Id::new("script_editor");
    let font = egui::TextStyle::Monospace.resolve(ui.style());
    let focused = ui.memory(|m| m.has_focus(id));

    // Keys for the open completion popup (taken before the text field sees them).
    let mut accept_now = None;
    if let Some(comp) = &mut st.comp {
        if focused {
            ui.input_mut(|i| {
                if i.consume_key(Modifiers::NONE, Key::ArrowDown) {
                    comp.sel = (comp.sel + 1) % comp.cands.len().max(1);
                }
                if i.consume_key(Modifiers::NONE, Key::ArrowUp) {
                    comp.sel = (comp.sel + comp.cands.len().max(1) - 1) % comp.cands.len().max(1);
                }
                if i.consume_key(Modifiers::NONE, Key::Tab) || i.consume_key(Modifiers::NONE, Key::Enter) {
                    accept_now = Some(comp.sel);
                }
            });
        }
        if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            st.comp = None;
        }
    }
    let force = focused && ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Space));
    if let Some(comp) = accept_now.and_then(|_| st.comp.take()) {
        if let Some(cand) = accept_now.and_then(|k| comp.cands.get(k)) {
            let prefix = text[..byte_of(text, comp.cursor)].to_string();
            let is_str = matches!(analyze(&prefix).0, Place::Str { .. });
            let at = accept(text, &comp, cand, is_str);
            st.set_cursor = Some(at);
            result.changed = true;
        }
    }
    if let Some(ci) = st.set_cursor.take() {
        set_cursor(ui.ctx(), id, ci);
        ui.memory_mut(|m| m.request_focus(id));
    }

    // Signature hint of the call around the cursor (from the last frame's cursor).
    let cursor_before = egui::text_edit::TextEditState::load(ui.ctx(), id).and_then(|s| s.cursor.char_range()).map(|r| r.primary.index);
    let hint = cursor_before.and_then(|ci| analyze(&text[..byte_of(text, ci)]).1).and_then(|c| signature_job(&c, font.clone(), known.signatures));
    ui.horizontal(|ui| {
        ui.set_min_height(18.0);
        match hint {
            Some(job) => {
                ui.label(job);
            }
            None => {
                ui.weak("Ctrl+Space: suggestions · Tab/Enter: accept · Ctrl+click a function: go to it");
            }
        }
    });

    let typed: Vec<String> = ui.input(|i| i.events.iter().filter_map(|e| if let egui::Event::Text(t) = e { Some(t.clone()) } else { None }).collect());
    let enter = ui.input(|i| i.key_pressed(Key::Enter)) && accept_now.is_none();
    let user_fns = known.functions.to_vec();
    let issue_spans = issues.to_vec();
    let mut layouter = |ui: &egui::Ui, s: &str, _wrap: f32| {
        let mut job = highlight(s, font.clone(), &issue_spans, &user_fns);
        job.wrap.max_width = f32::INFINITY;
        ui.fonts(|f| f.layout_job(job))
    };
    let rows = text.lines().count().max(1) + usize::from(text.ends_with('\n'));
    let scroll_to = st.scroll_to.take();
    let out = egui::ScrollArea::both()
        .id_salt("script_edit")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                // Line numbers.
                let numbers: String = (1..=rows).map(|n| format!("{n:>4}\n")).collect();
                ui.add(egui::Label::new(egui::RichText::new(numbers).font(font.clone()).color(Color32::from_gray(110))).selectable(false));
                let out = egui::TextEdit::multiline(text)
                    .id(id)
                    .code_editor()
                    .desired_width(f32::INFINITY)
                    .desired_rows(30)
                    .lock_focus(true)
                    .layouter(&mut layouter)
                    .show(ui);
                if let Some(ci) = scroll_to {
                    let c = out.galley.from_ccursor(egui::text::CCursor::new(ci));
                    let r = out.galley.pos_from_cursor(&c).translate(out.galley_pos.to_vec2());
                    ui.scroll_to_rect(r.expand2(egui::vec2(0.0, 2.0)), Some(egui::Align::TOP));
                }
                out
            })
            .inner
        })
        .inner;
    let mut changed = out.response.changed();
    let cursor = out.cursor_range.map(|r| r.primary.ccursor.index);

    // Typing helpers: auto-indent after Enter, closing pairs, overtyping closers.
    if changed {
        if let Some(ci) = cursor {
            let b = byte_of(text, ci);
            let typed_one = typed.last().filter(|t| t.chars().count() == 1).and_then(|t| t.chars().next());
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
                    st.set_cursor = Some(ci + indent.chars().count());
                }
            } else if let Some(c) = typed_one {
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
                } else if let Some(cl) = closer {
                    let line_before = text[..b].rsplit('\n').next().unwrap_or("");
                    let open_quote = c != '"' || line_before.matches('"').count() % 2 == 1;
                    let free = next.is_none_or(|n| n.is_whitespace() || matches!(n, ')' | ']' | '}' | ',' | ';'));
                    if open_quote && free {
                        text.insert(b, cl);
                        st.set_cursor = Some(ci);
                    }
                }
            }
        }
    }
    changed |= result.changed;
    result.changed = changed;

    // Completion: after typing (or Ctrl+Space), from the text before the cursor.
    if let Some(ci) = cursor {
        let typed_now = changed && !typed.is_empty();
        let force = force || st.comp.as_ref().is_some_and(|c| c.forced);
        if typed_now || force || st.comp.is_some() {
            let b = byte_of(text, ci);
            let (place, call) = analyze(&text[..b]);
            // Right after "(" or "," an id argument (item, map, sheet, function, ...) offers its
            // ids at once, inserted with their quotes.
            let id_arg = matches!(&place, Place::Code { partial, .. } if partial.is_empty())
                && text[..b].trim_end().ends_with(['(', ','])
                && call.as_ref().and_then(|c| arg_kind(&c.name, c.arg)).is_some_and(Arg::is_id);
            let start_b = match &place {
                Place::Code { start, .. } | Place::Str { start, .. } => *start,
                Place::Comment => b,
            };
            let show = match &place {
                Place::Str { .. } => typed_now || force || st.comp.is_some(),
                Place::Code { partial, .. } => !partial.is_empty() || force,
                Place::Comment => false,
            };
            let cands = if id_arg && (typed_now || force || st.comp.is_some()) {
                let ids = candidates(&Place::Str { partial: String::new(), start: b }, call.as_ref(), known, text, force);
                ids.into_iter().map(|c| Cand { insert: format!("\"{}\"", c.insert), ..c }).collect()
            } else if show {
                candidates(&place, call.as_ref(), known, text, force)
            } else {
                Vec::new()
            };
            let moved = st.comp.as_ref().is_some_and(|c| c.cursor != ci) && !typed_now;
            st.comp = if cands.is_empty() || moved {
                None
            } else {
                let sel = st.comp.as_ref().map_or(0, |c| c.sel).min(cands.len() - 1);
                Some(Completion { cands, sel, start: text[..start_b].chars().count(), cursor: ci, forced: force })
            };
        }
    } else {
        st.comp = None;
    }

    // While the popup is open, Esc closes it and the editor keeps the keyboard focus.
    let filter = egui::EventFilter { tab: true, horizontal_arrows: true, vertical_arrows: true, escape: st.comp.is_some() };
    ui.memory_mut(|m| m.set_focus_lock_filter(id, filter));

    // Ctrl+click: go to the definition of the function under the cursor.
    if out.response.clicked() && ui.input(|i| i.modifiers.command) {
        if let Some(ci) = cursor {
            let b = byte_of(text, ci);
            let start = text[..b].rfind(|c: char| !(c.is_alphanumeric() || c == '_')).map_or(0, |i| i + 1);
            let end = text[b..].find(|c: char| !(c.is_alphanumeric() || c == '_')).map_or(text.len(), |i| b + i);
            let word = &text[start..end];
            if known.functions.iter().any(|f| f == word) {
                result.goto = Some(word.to_string());
            }
        }
    }

    // The popup under the cursor.
    if let (Some(comp), Some(ci)) = (&st.comp, cursor) {
        let c = out.galley.from_ccursor(egui::text::CCursor::new(ci));
        let r = out.galley.pos_from_cursor(&c).translate(out.galley_pos.to_vec2());
        let mut clicked = None;
        egui::Area::new(egui::Id::new("script_completion"))
            .order(egui::Order::Foreground)
            .fixed_pos(r.left_bottom() + egui::vec2(0.0, 2.0))
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_max_width(520.0);
                    egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                        for (k, cand) in comp.cands.iter().enumerate() {
                            let mut job = LayoutJob::default();
                            job.append(&cand.label, 0.0, TextFormat { font_id: font.clone(), color: if k == comp.sel { Color32::WHITE } else { Color32::LIGHT_GRAY }, ..Default::default() });
                            if !cand.detail.is_empty() {
                                job.append(&format!("   {}", cand.detail), 0.0, TextFormat { font_id: FontId::proportional(12.0), color: Color32::GRAY, ..Default::default() });
                            }
                            let resp = ui.selectable_label(k == comp.sel, job);
                            if k == comp.sel {
                                resp.scroll_to_me(None);
                            }
                            if resp.clicked() {
                                clicked = Some(k);
                            }
                        }
                    });
                });
            });
        if let Some(k) = clicked {
            let comp = st.comp.take().unwrap();
            let prefix = text[..byte_of(text, comp.cursor)].to_string();
            let is_str = matches!(analyze(&prefix).0, Place::Str { .. });
            let at = accept(text, &comp, &comp.cands[k], is_str);
            st.set_cursor = Some(at);
            result.changed = true;
        }
    }
    result
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
        let known = Known { db: &db, maps: &maps, spawns: &spawns, targets: &none, flags: &none, vars: &none, functions: &funcs, signatures: &sigs };
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
        let hint = |src: &str| analyze(src).1.and_then(|c| signature_job(&c, FontId::monospace(12.0), &sigs)).map(|j| j.text);
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
        assert_eq!(code, vec!["total"]);
        assert!(get("stop_pa").starts_with(&["stop_patrol".to_string()]));
        assert!(get("guard").contains(&"guard_talk".to_string()));
    }


    #[test]
    fn highlights_and_finds_functions() {
        let src = "fn a() { say(\"x\"); b(1); } // c\nfn b(n) {}";
        assert_eq!(defined_functions(src), vec!["a", "b"]);
        let job = highlight(src, FontId::monospace(12.0), &[(13, 16)], &[]);
        let colored = |t: &str| job.sections.iter().find(|s| &job.text[s.byte_range.clone()] == t).map(|s| s.format.color);
        assert_eq!(colored("fn"), Some(C_KEYWORD));
        assert_eq!(colored("say"), Some(C_API));
        assert_eq!(colored("b"), Some(C_FUNC));
        assert_eq!(colored("// c"), Some(C_COMMENT));
        let bad = job.sections.iter().find(|s| &job.text[s.byte_range.clone()] == "\"x\"").unwrap();
        assert!(bad.format.underline.width > 0.0);
    }
}

