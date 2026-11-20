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
//!   props, characters, quests, skills, spells, recipes, achievements and races, signs, script `say`
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
const BUILTIN: [(&str, &str); 9] = [
    ("de", include_str!("../lang/de.ron")),
    ("fr", include_str!("../lang/fr.ron")),
    ("es", include_str!("../lang/es.ron")),
    ("pt-br", include_str!("../lang/pt-br.ron")),
    ("it", include_str!("../lang/it.ron")),
    ("pl", include_str!("../lang/pl.ron")),
    ("ru", include_str!("../lang/ru.ron")),
    ("ja", include_str!("../lang/ja.ron")),
    ("zh-hans", include_str!("../lang/zh-hans.ron")),
];

/// The built-in runtime file for a project's language code (case does not matter; `zh` and
/// `zh-cn` use Simplified Chinese, `pt` Brazilian Portuguese).
fn builtin_src(code: &str) -> Option<&'static str> {
    let code = match code.to_lowercase().as_str() {
        "zh" | "zh-cn" | "zh-sg" => "zh-hans".to_string(),
        "pt" => "pt-br".to_string(),
        c => c.to_string(),
    };
    BUILTIN.iter().find(|(c, _)| *c == code).map(|(_, src)| *src)
}

/// The language codes whose runtime texts (menus, HUD, windows) are built in.
pub fn builtin_codes() -> impl Iterator<Item = &'static str> {
    BUILTIN.iter().map(|(c, _)| *c)
}

/// The built-in runtime texts of a language (English → translation), if it has them.
pub fn builtin_ui(code: &str) -> BTreeMap<String, String> {
    builtin_src(code).and_then(|src| ron::from_str::<LangFile>(src).ok()).map(|f| f.ui).unwrap_or_default()
}

/// Names of common languages by code, each in its own language and script (the Options menu,
/// the editor's language list); codes in any case (`pt-BR`).
pub fn language_name(code: &str) -> String {
    match code.to_lowercase().as_str() {
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
        "pt-br" => "Português (Brasil)",
        "cs" => "Čeština",
        "hu" => "Magyar",
        "ro" => "Română",
        "tr" => "Türkçe",
        "el" => "Ελληνικά",
        "ru" => "Русский",
        "uk" => "Українська",
        "ja" => "日本語",
        "ko" => "한국어",
        "zh" | "zh-hans" | "zh-cn" => "简体中文",
        "zh-hant" | "zh-tw" | "zh-hk" => "繁體中文",
        _ => return code.to_string(),
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
            // The value runs to the next piece (the first occurrence after a non-empty value;
            // its first character may take several bytes: "Überzeugen").
            let skip = rest.chars().next()?.len_utf8();
            let at = rest[skip..].find(p.as_str())? + skip;
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
        if let Some(src) = builtin_src(code) {
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
        t(&mut c.description);
    }
    for q in &mut d.quests {
        t(&mut q.title);
        t(&mut q.description);
        for s in &mut q.stages {
            t(s);
        }
    }
    for s in &mut d.spells {
        t(&mut s.name);
        t(&mut s.description);
    }
    for a in &mut d.achievements {
        t(&mut a.name);
        t(&mut a.description);
    }
    for r in &mut d.races {
        t(&mut r.name);
        t(&mut r.description);
        for tr in &mut r.traits {
            t(&mut tr.name);
            t(&mut tr.description);
        }
        for c in &mut r.choices {
            t(&mut c.label);
            for n in c.names.values_mut() {
                t(n);
            }
        }
    }
    for b in &mut d.backgrounds {
        t(&mut b.name);
        t(&mut b.description);
        t(&mut b.feature);
        t(&mut b.feature_text);
    }
    for x in &mut d.diseases {
        t(&mut x.name);
        t(&mut x.description);
    }
    for s in &mut d.sounds {
        t(&mut s.caption);
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
        let words = crate::dialog_codes::plain(s).replace('{', " {").split_whitespace().filter(|w| !w.starts_with('{')).any(|w| w.chars().any(char::is_alphabetic));
        if words {
            out.insert(s.to_string());
        }
    };
    fn actions(list: &[Action], add: &mut dyn FnMut(&str)) {
        crate::events::walk_actions(list, &mut |a| match a {
            Action::Dialog { speaker, text } => {
                add(speaker);
                add(crate::dialog_codes::split_box_at(text).1);
            }
            Action::InputNumber { text, .. } | Action::InputText { text, .. } | Action::ScrollText { text, .. } => add(text),
            Action::Choice { speaker, text, options, .. } => {
                add(speaker);
                add(crate::dialog_codes::split_box_at(text).1);
                options.iter().for_each(|o| add(o));
            }
            Action::OpenCrafting { title, .. } => add(title),
            Action::Bark { lines, .. } => lines.iter().for_each(|l| add(l)),
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
    // The menus' own entries and the Credits page.
    for e in project.config.title.entries.iter().chain(&project.config.menu.entries) {
        if let crate::menus::MenuEntry::Custom { label, detail, .. } = e {
            add(label);
            add(detail);
        }
    }
    // (The action bar's custom entries.)
    for e in &project.config.action_bar {
        if let crate::menus::BarEntry::Custom { label, detail, .. } = e {
            add(label);
            add(detail);
        }
    }
    add(&project.config.credits);
    for i in &db.items {
        add(&i.name);
        add(&i.description);
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
        if let Some(spot) = &p.use_spot {
            add(&spot.refuse_text);
        }
    }
    for c in &db.characters {
        add(&c.name);
        add(&c.description);
    }
    // The maps' titles (the map screen, its exits, the save list).
    for m in project.list_maps() {
        add(&project.config.map_title(&m));
    }
    for q in &db.quests {
        add(&q.title);
        add(&q.description);
        q.stages.iter().for_each(|s| add(s));
        // What the quest editor describes: the offer, the steps' lines and actions, the rewards.
        add(&q.giver_name);
        let o = &q.offer;
        for l in o.lines.iter().chain(&o.accepted).chain(&o.declined).chain([&o.question, &o.accept, &o.decline]) {
            add(l);
        }
        for s in &q.steps {
            add(&s.giver_line);
            if let crate::quest::StepGoal::Talk { lines, speaker, .. } = &s.done {
                add(speaker);
                lines.iter().for_each(|l| add(l));
            }
            actions(&s.on_start, &mut add);
            actions(&s.on_done, &mut add);
        }
        actions(&q.rewards.actions, &mut add);
    }
    for c in &db.common_events {
        actions(&c.actions, &mut add);
    }
    for s in &db.spells {
        add(&s.name);
        add(&s.description);
    }
    for a in &db.achievements {
        add(&a.name);
        add(&a.description);
    }
    for r in &db.races {
        add(&r.name);
        add(&r.description);
        for tr in &r.traits {
            add(&tr.name);
            add(&tr.description);
        }
        for c in &r.choices {
            add(&c.label);
            for n in c.names.values() {
                add(n);
            }
        }
    }
    for b in &db.backgrounds {
        add(&b.name);
        add(&b.description);
        add(&b.feature);
        add(&b.feature_text);
    }
    for x in &db.diseases {
        add(&x.name);
        add(&x.description);
    }
    for s in &db.sounds {
        add(&s.caption);
    }
    // The character creator's colour rows.
    for f in db.models.iter().flat_map(|m| &m.features).filter(|f| !f.colors.is_empty()) {
        add(&f.label());
    }
    // Every trigger of the definitions and the maps, and the lock / container texts of props and
    // placed entities.
    project.for_each_trigger(&mut |site, ts| {
        match site {
            crate::events::TriggerSite::Prop(p) => {
                lock_texts(p.lock.as_ref(), p.container.as_ref(), &mut add);
                // (A trap's line when it springs.)
                if let Some(t) = &p.trap {
                    add(&t.text);
                }
            }
            crate::events::TriggerSite::Zone { zone, .. } => {
                if let crate::map::ZoneKind::Trap { trap } = &zone.kind {
                    add(&trap.text);
                }
            }
            crate::events::TriggerSite::Entity { entity, .. } => lock_texts(entity.lock.as_ref(), entity.container.as_ref(), &mut add),
            _ => {}
        }
        triggers(ts, &mut add);
    });
    for f in crate::sys::files_in(&project.scripts_dir()).unwrap_or_default() {
        let Ok(src) = crate::sys::read_to_string(&f) else { continue };
        for a in crate::script_refs::string_args(&src) {
            // say(text) / say(speaker, text) / choice(text, [options], var): the text arguments.
            if crate::script_api::is_text_arg(&a.func, a.arg) {
                add(&a.value);
            }
        }
        for l in crate::script_refs::list_args(&src) {
            if l.func == "choice" {
                l.values.iter().for_each(|v| add(v));
            }
        }
        // Script windows' lines and buttons, HUD bars' labels.
        crate::script_refs::ui_texts(&src).iter().for_each(|t| add(t));
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

    #[test]
    fn languages_are_named_in_their_own_script() {
        assert_eq!(language_name("ja"), "日本語");
        assert_eq!(language_name("pt-BR"), "Português (Brasil)");
        assert_eq!(language_name("ZH-hant"), "繁體中文");
        assert_eq!(language_name("xx"), "xx");
    }

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
        // The bed's question, filled in by the engine (runtime/furniture.rs).
        assert_eq!(t.ui("Sleep until 07:00?").as_deref(), Some("Bis 07:00 schlafen?"));
        assert_eq!(t.ui("Not now").as_deref(), Some("Jetzt nicht"));
        assert_eq!(t.ui("a: b"), None, "keys without words of their own are ignored");
        assert_eq!(t.ui("Something else"), None);
        // A value may start with a letter of several bytes (an already translated skill name).
        assert_eq!(t.ui("Überzeugen: 3 vs DC 5 - failure").as_deref(), Some("Überzeugen: 3 gegen SG 5 - Fehlschlag"));
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
    fn builtin_languages_parse_and_have_their_templates_right() {
        let german: LangFile = ron::from_str(BUILTIN[0].1).unwrap();
        assert!(german.ui.len() > 100, "{}", german.ui.len());
        for (code, src) in BUILTIN {
            let f: LangFile = ron::from_str(src).unwrap_or_else(|e| panic!("{code}: {e}"));
            assert!(!f.name.is_empty(), "{code}");
            // Every language translates the same runtime texts as German (the reference).
            let missing: Vec<&String> = german.ui.keys().filter(|k| f.ui.get(*k).is_none_or(|v| v.is_empty())).collect();
            let extra: Vec<&String> = f.ui.keys().filter(|k| !german.ui.contains_key(*k)).collect();
            assert!(missing.is_empty() && extra.is_empty(), "{code}: missing {missing:?}, not in de.ron {extra:?}");
            for (k, v) in &f.ui {
                let values = k.matches("{}").count();
                let indexed: Vec<usize> = (0..10).filter(|i| v.contains(&format!("{{{i}}}"))).collect();
                if indexed.is_empty() {
                    assert_eq!(values, v.matches("{}").count(), "{code}: placeholders of {k:?} → {v:?}");
                } else {
                    // ({0}, {1} pick values; one may be left out, e.g. an English plural "s".)
                    assert!(v.matches("{}").count() == 0 && indexed.iter().all(|i| *i < values), "{code}: placeholders of {k:?} → {v:?}");
                }
                let keys = |s: &str| {
                    let mut t: Vec<String> = s.match_indices("{key:").chain(s.match_indices("{pad:")).map(|(i, _)| s[i..].split_inclusive('}').next().unwrap_or("").to_string()).collect();
                    t.sort();
                    t
                };
                assert_eq!(keys(k), keys(v), "{code}: key glyphs of {k:?} → {v:?}");
            }
        }
        assert_eq!(builtin_ui("ZH").get("New game").map(String::as_str), Some("新游戏"));
        assert_eq!(builtin_ui("pt-BR").get("New game").map(String::as_str), Some("Novo jogo"));
        assert!(builtin_ui("xx").is_empty());
    }
}
