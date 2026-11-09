//! Languages: a game is written in one language (`ProjectConfig::language`) and translated by
//! files `lang/<code>.ron` in the project, keyed by the original text:
//!
//! ```text
//! (
//!     name: "Deutsch",
//!     texts: { "Welcome, {player}!": "Willkommen, {player}!", "Apple": "Apfel" },
//!     ui: { "New game": "Neues Spiel" },
//! )
//! ```
//!
//! - **`texts`**: the game's content — dialog lines, choices, names and descriptions of items,
//!   props, characters, quests, skills, spells, recipes and achievements, signs, script `say`
//!   texts. Looked up whole ([`tr`]) where content reaches the player: `WorldState::expand_text`
//!   (before `{player}` etc. are filled in) and the translated database ([`translate_db`]).
//! - **`ui`**: the runtime's own texts (menus, windows, HUD, messages). R(ust)PG2D ships them for
//!   German (`engine/lang/de.ron`); a project's `ui` entries add to or replace those. They are
//!   looked up when text is drawn ([`tr_ui`]), so keys are the finished English strings; `{}`
//!   stands for a value (`"Lv {}": "Stufe {}"`, `"{} coins": "{} Münzen"`), `{0}`, `{1}` in a
//!   translation reorder them.
//!
//! A missing entry shows the original text. The current language is global (one game per
//! process); [`set`] switches it.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::rc::Rc;

use serde::{Deserialize, Serialize};

/// A translation file `lang/<code>.ron`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LangFile {
    /// The language's own name ("Deutsch"), shown in *Options › Language*.
    pub name: String,
    /// Content: original text → translation.
    pub texts: BTreeMap<String, String>,
    /// Runtime texts: English → translation (adds to / replaces the built-in table).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub ui: BTreeMap<String, String>,
}

/// Built-in translations of the runtime's texts (the `ui` part of a [`LangFile`]).
const BUILTIN: [(&str, &str); 1] = [("de", include_str!("../lang/de.ron"))];

/// Names of common languages by code (the original language's name in the Options menu).
pub fn language_name(code: &str) -> String {
    match code {
        "en" => "English",
        "de" => "Deutsch",
        "fr" => "Français",
        "es" => "Español",
        "it" => "Italiano",
        "nl" => "Nederlands",
        "pt" => "Português",
        "pl" => "Polski",
        "sv" => "Svenska",
        "da" => "Dansk",
        "fi" => "Suomi",
        "no" | "nb" => "Norsk",
        _ => code,
    }
    .to_string()
}

/// A `ui` key with `{}` placeholders, split into its literal pieces.
#[derive(Debug)]
struct Template {
    pieces: Vec<String>,
    to: String,
}

impl Template {
    /// The values in `text` if it has this template's shape (every value non-empty).
    fn captures(&self, text: &str) -> Option<Vec<String>> {
        let (first, last) = (self.pieces.first()?, self.pieces.last()?);
        let mut rest = text.strip_prefix(first.as_str())?;
        rest = rest.strip_suffix(last.as_str())?;
        let mut out = Vec::new();
        let middle = &self.pieces[1..self.pieces.len() - 1];
        for p in middle {
            // The value runs to the next piece (the first occurrence after a non-empty value).
            let at = rest.get(1..)?.find(p.as_str())? + 1;
            out.push(rest[..at].to_string());
            rest = &rest[at + p.len()..];
        }
        if rest.is_empty() {
            return None;
        }
        out.push(rest.to_string());
        Some(out)
    }
}

/// Fills `{}` (in order) and `{0}`, `{1}`… in a translation.
fn fill(to: &str, values: &[String]) -> String {
    let mut out = String::with_capacity(to.len() + 16);
    let mut next = 0;
    let mut rest = to;
    while let Some(i) = rest.find('{') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        match after.find('}') {
            Some(0) => {
                out.push_str(values.get(next).map_or("", |s| s.as_str()));
                next += 1;
                rest = &after[1..];
            }
            Some(j) if after[..j].chars().all(|c| c.is_ascii_digit()) => {
                let k: usize = after[..j].parse().unwrap_or(0);
                out.push_str(values.get(k).map_or("", |s| s.as_str()));
                rest = &after[j + 1..];
            }
            _ => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// The translations of one language, ready for lookups.
#[derive(Debug, Default)]
pub struct Translations {
    pub code: String,
    pub name: String,
    texts: HashMap<String, String>,
    ui: HashMap<String, String>,
    templates: Vec<Template>,
    /// Results of template lookups (texts with values repeat every frame).
    cache: RefCell<HashMap<String, Option<String>>>,
}

impl Translations {
    /// The built-in runtime texts of `code` plus a project's file for it (either may be missing).
    pub fn new(code: &str, project: Option<LangFile>) -> Translations {
        let mut ui: BTreeMap<String, String> = BTreeMap::new();
        if let Some((_, src)) = BUILTIN.iter().find(|(c, _)| *c == code) {
            match ron::from_str::<LangFile>(src) {
                Ok(f) => ui.extend(f.ui),
                Err(e) => eprintln!("[lang] built-in {code}: {e}"),
            }
        }
        let project = project.unwrap_or_default();
        ui.extend(project.ui);
        let name = if project.name.is_empty() { language_name(code) } else { project.name };
        let mut t = Translations { code: code.to_string(), name, texts: project.texts.into_iter().filter(|(_, v)| !v.is_empty()).collect(), ..Default::default() };
        for (k, v) in ui.into_iter().filter(|(_, v)| !v.is_empty()) {
            if k.contains("{}") {
                let pieces: Vec<String> = k.split("{}").map(str::to_string).collect();
                // A key needs some words of its own ("{}" or "{}: {}" alone would match anything).
                if pieces.iter().map(|p| p.chars().filter(|c| c.is_alphabetic()).count()).sum::<usize>() >= 2 {
                    t.templates.push(Template { pieces, to: v });
                }
            } else {
                t.ui.insert(k, v);
            }
        }
        // Longer keys first: "{} of {} unlocked" before "{} of {}".
        t.templates.sort_by_key(|tpl| std::cmp::Reverse(tpl.pieces.iter().map(String::len).sum::<usize>()));
        t
    }

    /// Content translation of `text` (whole), if there is one.
    pub fn text(&self, text: &str) -> Option<&str> {
        self.texts.get(text).map(String::as_str)
    }

    /// Runtime-text translation of `text`, if there is one.
    pub fn ui(&self, text: &str) -> Option<String> {
        if let Some(t) = self.ui.get(text) {
            return Some(t.clone());
        }
        if self.templates.is_empty() || text.is_empty() {
            return None;
        }
        if let Some(hit) = self.cache.borrow().get(text) {
            return hit.clone();
        }
        let found = self.templates.iter().find_map(|tpl| tpl.captures(text).map(|v| fill(&tpl.to, &v)));
        let mut cache = self.cache.borrow_mut();
        if cache.len() > 4096 {
            cache.clear();
        }
        cache.insert(text.to_string(), found.clone());
        found
    }
}

thread_local! {
    static CURRENT: RefCell<Option<Rc<Translations>>> = const { RefCell::new(None) };
}

/// Switches the language (`None` = the game's original language).
pub fn set(t: Option<Translations>) {
    CURRENT.with(|c| *c.borrow_mut() = t.map(Rc::new));
}

/// The current language's code, if a translation is active.
pub fn current() -> Option<String> {
    CURRENT.with(|c| c.borrow().as_ref().map(|t| t.code.clone()))
}

/// Content text in the current language (the original if there is no translation for it).
pub fn tr(text: &str) -> String {
    CURRENT.with(|c| c.borrow().as_ref().and_then(|t| t.text(text).map(str::to_string))).unwrap_or_else(|| text.to_string())
}

/// A runtime text (menus, HUD, messages) in the current language.
pub fn tr_ui(text: &str) -> std::borrow::Cow<'_, str> {
    match CURRENT.with(|c| c.borrow().as_ref().and_then(|t| t.ui(text))) {
        Some(t) => std::borrow::Cow::Owned(t),
        None => std::borrow::Cow::Borrowed(text), // (English, or no translation: nothing copied)
    }
}

/// Reads `lang/<code>.ron` of a project (None if it has none or it can't be read).
pub fn load_file(root: &Path, code: &str) -> Option<LangFile> {
    let path = root.join("lang").join(format!("{code}.ron"));
    let src = crate::sys::read_to_string(&path).ok()?;
    match ron::from_str(&src) {
        Ok(f) => Some(f),
        Err(e) => {
            eprintln!("[lang] {}: {e}", path.display());
            None
        }
    }
}

/// The languages a project offers: its original (`config.language`) and every `lang/<code>.ron`
/// listed in `config.languages`, as (code, name).
pub fn offered(root: &Path, config: &crate::project::ProjectConfig) -> Vec<(String, String)> {
    let mut out = vec![(config.language.clone(), language_name(&config.language))];
    for code in &config.languages {
        if *code == config.language || out.iter().any(|(c, _)| c == code) {
            continue;
        }
        let name = load_file(root, code).map(|f| f.name).filter(|n| !n.is_empty()).unwrap_or_else(|| language_name(code));
        out.push((code.clone(), name));
    }
    out
}

/// The player's system language (two-letter code): `LANGUAGE` / `LC_ALL` / `LC_MESSAGES` /
/// `LANG` natively, the browser's language on the web.
pub fn system_language() -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    let raw = Some(crate::sys::web::browser_language());
    #[cfg(not(target_arch = "wasm32"))]
    let raw = ["LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"].iter().find_map(|v| std::env::var(v).ok().filter(|s| !s.is_empty() && s != "C" && s != "POSIX"));
    let code: String = raw?.chars().take_while(|c| c.is_ascii_alphabetic()).collect::<String>().to_lowercase();
    (code.len() >= 2).then_some(code)
}

/// The database with names and descriptions in the current language (the game keeps the
/// original to switch back): items, props, characters, quests and their stages' texts, skills,
/// spells, achievements. Texts inside triggers, locks and containers are translated where they
/// are shown (`WorldState::expand_text`, `tr`).
pub fn translate_db(db: &crate::defs::Database) -> crate::defs::Database {
    let mut d = db.clone();
    if current().is_none() {
        return d;
    }
    let t = |s: &mut String| {
        if !s.is_empty() {
            *s = tr(s);
        }
    };
    for i in &mut d.items {
        t(&mut i.name);
        t(&mut i.description);
    }
    for p in &mut d.props {
        t(&mut p.name);
    }
    for c in &mut d.characters {
        t(&mut c.name);
    }
    for q in &mut d.quests {
        t(&mut q.title);
        t(&mut q.description);
        for s in &mut q.stages {
            t(s);
        }
    }
    for s in &mut d.skills {
        t(&mut s.name);
        t(&mut s.description);
    }
    for s in &mut d.spells {
        t(&mut s.name);
        t(&mut s.description);
    }
    for a in &mut d.achievements {
        t(&mut a.name);
        t(&mut a.description);
    }
    d.reindex();
    d
}

/// Every player-facing text of a project, for a translation template: names and descriptions
/// of the database, quest stages, the dialog / choice texts of all triggers (definitions, maps,
/// map objects, zones) and the texts scripts pass to `say` / `choice`.
pub fn content_texts(project: &crate::project::Project) -> std::collections::BTreeSet<String> {
    use crate::events::{Action, Trigger};
    let mut out = std::collections::BTreeSet::new();
    let mut add = |s: &str| {
        // Texts with letters of their own (not "{self}" or "...").
        let words = s.replace('{', " {").split_whitespace().filter(|w| !w.starts_with('{')).any(|w| w.chars().any(char::is_alphabetic));
        if words {
            out.insert(s.to_string());
        }
    };
    fn actions(list: &[Action], add: &mut dyn FnMut(&str)) {
        crate::events::walk_actions(list, &mut |a| match a {
            Action::Dialog { speaker, text } => {
                add(speaker);
                add(text);
            }
            Action::Choice { speaker, text, options, .. } => {
                add(speaker);
                add(text);
                options.iter().for_each(|o| add(o));
            }
            Action::OpenCrafting { title, .. } => add(title),
            _ => {}
        });
    }
    let triggers = |list: &[Trigger], add: &mut dyn FnMut(&str)| {
        for t in list {
            actions(&t.actions, add);
        }
    };
    let db = &project.db;
    // The game's name (title screen).
    add(&project.config.name);
    for i in &db.items {
        add(&i.name);
        add(&i.description);
        triggers(&i.on_use, &mut add);
    }
    let lock_texts = |lock: Option<&crate::defs::LockSpec>, container: Option<&crate::defs::ContainerSpec>, add: &mut dyn FnMut(&str)| {
        if let Some(l) = lock {
            add(&l.locked_text);
            add(&l.unlock_text);
        }
        if let Some(c) = container {
            add(&c.locked_text);
        }
    };
    for p in &db.props {
        add(&p.name);
        lock_texts(p.lock.as_ref(), p.container.as_ref(), &mut add);
        triggers(&p.triggers, &mut add);
    }
    for c in &db.characters {
        add(&c.name);
        triggers(&c.triggers, &mut add);
    }
    for q in &db.quests {
        add(&q.title);
        add(&q.description);
        q.stages.iter().for_each(|s| add(s));
    }
    for s in &db.skills {
        add(&s.name);
        add(&s.description);
    }
    for s in &db.spells {
        add(&s.name);
        add(&s.description);
    }
    for a in &db.achievements {
        add(&a.name);
        add(&a.description);
    }
    for name in project.list_maps() {
        let Ok(m) = project.load_map(&name) else { continue };
        triggers(&m.triggers, &mut add);
        for e in &m.entities {
            lock_texts(e.lock.as_ref(), e.container.as_ref(), &mut add);
            triggers(&e.triggers, &mut add);
        }
        for z in &m.zones {
            triggers(&z.triggers, &mut add);
        }
    }
    for f in crate::sys::files_in(&project.scripts_dir()).unwrap_or_default() {
        let Ok(src) = crate::sys::read_to_string(&f) else { continue };
        for a in crate::script_refs::string_args(&src) {
            // say(text) / say(speaker, text) / choice(text, [options], var): not choice's variable.
            if a.func == "say" || (a.func == "choice" && a.arg == 0) {
                add(&a.value);
            }
        }
        for l in crate::script_refs::list_args(&src) {
            if l.func == "choice" {
                l.values.iter().for_each(|v| add(v));
            }
        }
    }
    out
}

/// A translation file for `code` with every content text of the project: `existing`
/// translations are kept, new texts get an empty translation (shown in the original until
/// filled in). Returns the file and the keys of `existing` no longer used (kept, so nothing
/// typed is lost; delete them by hand).
pub fn template(project: &crate::project::Project, code: &str, existing: Option<LangFile>) -> (LangFile, Vec<String>) {
    let texts = content_texts(project);
    let mut file = existing.unwrap_or_default();
    if file.name.is_empty() {
        file.name = language_name(code);
    }
    let unused: Vec<String> = file.texts.keys().filter(|k| !texts.contains(*k)).cloned().collect();
    for t in texts {
        file.texts.entry(t).or_default();
    }
    (file, unused)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn de() -> Translations {
        let file = LangFile {
            name: "Deutsch".into(),
            texts: [("Apple".to_string(), "Apfel".to_string()), ("Hello, {player}!".into(), "Hallo, {player}!".into()), ("Unfinished".into(), String::new())].into(),
            ui: [("{} of {} unlocked".to_string(), "{} von {} freigeschaltet".to_string()), ("Lv {}".into(), "Stufe {}".into()), ("{} sold {}".into(), "{1} von {0} verkauft".into()), ("{}: {}".into(), "nope".into())].into(),
        };
        Translations::new("de", Some(file))
    }

    #[test]
    fn content_is_looked_up_whole() {
        let t = de();
        assert_eq!(t.text("Apple"), Some("Apfel"));
        assert_eq!(t.text("Apples"), None);
        assert_eq!(t.text("Unfinished"), None, "empty translations count as missing");
        set(Some(de()));
        assert_eq!(tr("Hello, {player}!"), "Hallo, {player}!");
        assert_eq!(tr("Pear"), "Pear");
        set(None);
        assert_eq!(tr("Apple"), "Apple");
    }

    #[test]
    fn ui_texts_with_values() {
        let t = de();
        // Built in (engine/lang/de.ron) and from the project.
        assert_eq!(t.ui("New game").as_deref(), Some("Neues Spiel"));
        assert_eq!(t.ui("3 of 6 unlocked").as_deref(), Some("3 von 6 freigeschaltet"));
        assert_eq!(t.ui("Lv 12").as_deref(), Some("Stufe 12"));
        assert_eq!(t.ui("Tilda sold Apple").as_deref(), Some("Apple von Tilda verkauft"), "reordered");
        assert_eq!(t.ui("Lv "), None, "values aren't empty");
        assert_eq!(t.ui("a: b"), None, "keys without words of their own are ignored");
        assert_eq!(t.ui("Something else"), None);
        assert_eq!(fill("{0}-{}-{1}{x}", &["a".into(), "b".into()]), "a-a-b{x}");
    }

    #[test]
    fn template_collects_every_kind_of_text() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../example_project");
        let project = crate::project::Project::load(&root).unwrap();
        let texts = content_texts(&project);
        // Database (item, quest stage), a map's dialog, a script's say(), the game's name.
        for t in ["Apple", "Return to Elder Rowan.", "Halt, traveller! ... {player}? Thank the stars, you came!", "Ta-da!", "R(ust)PG2D example"] {
            assert!(texts.contains(t), "{t}");
        }
        assert!(!texts.iter().any(|t| t.is_empty() || t == "{self}"));
        // Existing translations stay, unknown keys are reported (and kept).
        let old = LangFile { name: String::new(), texts: [("Apple".to_string(), "Apfel".to_string()), ("Gone".to_string(), "Weg".to_string())].into(), ui: Default::default() };
        let (file, unused) = template(&project, "de", Some(old));
        assert_eq!(file.name, "Deutsch");
        assert_eq!(file.texts.get("Apple").map(String::as_str), Some("Apfel"));
        assert_eq!(file.texts.get("Ta-da!").map(String::as_str), Some(""));
        assert_eq!(unused, vec!["Gone".to_string()]);
        // The example's German file covers everything.
        let de = load_file(&root, "de").unwrap();
        assert!(texts.iter().all(|t| de.texts.get(t).is_some_and(|v| !v.is_empty())));
    }

    /// The example's German file has no leftovers: every text it translates is still in the game
    /// (edit the translation along with the content, e.g. with the AI agent server's
    /// `write_translation`).
    #[test]
    fn example_translation_has_no_unused_entries() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../example_project");
        let project = crate::project::Project::load(&root).unwrap();
        let (_, unused) = template(&project, "de", Some(load_file(&root, "de").unwrap()));
        assert!(unused.is_empty(), "example_project/lang/de.ron translates texts the example no longer has: {unused:?}");
    }

    #[test]
    fn builtin_german_parses_and_has_its_templates_right() {
        let f: LangFile = ron::from_str(BUILTIN[0].1).unwrap();
        assert!(f.ui.len() > 100, "{}", f.ui.len());
        for (k, v) in &f.ui {
            assert_eq!(k.matches("{}").count(), v.matches("{}").count() + (0..10).filter(|i| v.contains(&format!("{{{i}}}"))).count(), "placeholders of {k:?} → {v:?}");
        }
    }
}
