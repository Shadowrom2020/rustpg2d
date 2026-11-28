//! Translations, at the top of the *Languages* page: the languages the game is offered in
//! with how far each is, a language added, and a language's table: every text players read
//! with its translation typed beside it. Play-tests can start in one of them.
//!
//! The logic is `editor_core::langs`.

use eframe::egui::{self, RichText};
use editor_core::langs::{LangTable, Language};
use editor_core::manual::drawable;
use editor_core::Workspace;
use rustpg2d_engine::lang;

/// Rows of the table drawn at once (searching finds the rest).
const ROWS: usize = 120;

#[derive(Default)]
pub struct LangPage {
    /// The language whose table is open.
    pub table: Option<LangTable>,
    /// The languages with their counts, and what they were counted for (counted again when
    /// the project or a table was saved).
    counted: Option<(String, Vec<Language>)>,
    saves: u32,
    new_code: String,
    filter: String,
    missing_only: bool,
    pub said: Option<Result<String, String>>,
}

/// What the page asks the window for.
pub enum Want {
    /// Play-tests start in this language from now on (`None`: as the player's settings say),
    /// and one starts now when `true`.
    PlayIn(Option<String>, bool),
}

/// A language's own name with its code, in letters the editor can draw.
fn named(code: &str, name: &str) -> String {
    let name = drawable(name);
    if name == code || name.trim_matches('…').is_empty() {
        code.to_string()
    } else {
        format!("{name} ({code})")
    }
}

impl LangPage {
    /// A table has translations that aren't written yet.
    pub fn dirty(&self) -> bool {
        self.table.as_ref().is_some_and(|t| t.dirty)
    }

    /// Writes the open table when something was typed. True when a file was written.
    pub fn save(&mut self, ws: &mut Workspace) -> bool {
        let Some(table) = self.table.as_mut().filter(|t| t.dirty) else { return false };
        self.said = Some(ws.save_lang_table(table));
        self.saves += 1;
        true
    }

    fn languages(&mut self, ws: &Workspace) -> Vec<Language> {
        let key = format!("{}|{}|{}|{}", ws.timeline.status(), ws.is_dirty(), ws.project.config.languages.join(","), self.saves);
        if self.counted.as_ref().is_none_or(|(k, _)| *k != key) {
            self.counted = Some((key, ws.languages()));
        }
        self.counted.as_ref().map(|(_, l)| l.clone()).unwrap_or_default()
    }

    fn open(&mut self, ws: &mut Workspace, code: &str) {
        match ws.lang_table(code) {
            Ok(t) => {
                self.table = Some(t);
                self.filter.clear();
            }
            Err(e) => self.said = Some(Err(e)),
        }
    }

    fn add(&mut self, ws: &mut Workspace, code: &str) {
        let code = code.trim().to_lowercase();
        self.said = Some(ws.add_language(&code));
        self.saves += 1;
        if self.said.as_ref().is_some_and(|r| r.is_ok()) {
            self.new_code.clear();
            self.open(ws, &code);
        }
    }

    /// The script's line: `own=en langs=[de:1/130 …] table=de:missing=3/130:changed|- said=…`.
    pub fn status(&mut self, ws: &Workspace) -> String {
        let langs: Vec<String> = self.languages(ws).iter().map(|l| format!("{}:{}/{}", l.code, l.done, l.total)).collect();
        let said = match &self.said {
            Some(Ok(m)) => m.clone(),
            Some(Err(e)) => format!("failed: {e}"),
            None => "-".into(),
        };
        format!("own={} langs=[{}] table={} said={said}", ws.project.config.language, langs.join(" "), self.table.as_ref().map_or("-".to_string(), LangTable::status))
    }

    /// The script's words: `add CODE`, `open CODE`, `set K TRANSLATION…` (the K-th text shown
    /// without a translation), `update` (the table again from the game), `clean` (translations
    /// of texts the game no longer has), `save`, `close`, `remove CODE`, `play CODE|-`.
    pub fn word(&mut self, ws: &mut Workspace, args: &[String]) -> Result<Option<Want>, String> {
        let arg = |i: usize| args.get(i).map(String::as_str).unwrap_or("");
        match arg(0) {
            "" => {}
            "add" => self.add(ws, arg(1)),
            "open" => self.open(ws, arg(1)),
            "set" => {
                let table = self.table.as_mut().ok_or("no language's table is open")?;
                let text = table.shown("", true).get(arg(1).parse::<usize>().unwrap_or(0)).map(|t| t.to_string()).ok_or("no such text without a translation")?;
                table.set(&text, &args[2.min(args.len())..].join(" "));
            }
            "update" => {
                let code = self.table.as_ref().map(|t| t.code.clone()).ok_or("no language's table is open")?;
                self.save(ws);
                self.add(ws, &code);
            }
            "clean" => {
                self.table.as_mut().ok_or("no language's table is open")?.drop_unused();
            }
            "save" => {
                self.save(ws);
            }
            "close" => self.table = None,
            "remove" => {
                self.said = Some(ws.remove_language(arg(1)));
                if self.table.as_ref().is_some_and(|t| t.code == arg(1)) {
                    self.table = None;
                }
            }
            "play" => return Ok(Some(Want::PlayIn((arg(1) != "-" && !arg(1).is_empty()).then(|| arg(1).to_string()), false))),
            other => return Err(format!("lang {other}: add, open, set, update, clean, save, close, remove, play")),
        }
        Ok(None)
    }

    /// `play_lang`: the language play-tests start in.
    pub fn show(&mut self, ui: &mut egui::Ui, ws: &mut Workspace, play_lang: &Option<String>) -> Option<Want> {
        let mut want = None;
        let own = ws.project.config.language.clone();
        ui.strong("Translations");
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("The game is written in {}: the texts typed into the editor are in that language.", named(&own, &lang::language_name(&own))));
            ui.weak("A translation is a list of those texts with what players of another language read instead.");
        });
        if ws.is_dirty() {
            ui.colored_label(ui.visuals().warn_fg_color, "Texts typed since the last save aren't counted or listed yet: Ctrl+S.");
        }
        let langs = self.languages(ws);
        let (mut open, mut remove, mut add) = (None, None, None);
        if langs.is_empty() {
            ui.weak("The game isn't translated yet. Add a language below: its table opens with every text of the game to translate.");
        }
        egui::Grid::new("languages").num_columns(5).spacing([14.0, 4.0]).show(ui, |ui| {
            for l in &langs {
                ui.label(RichText::new(named(&l.code, &l.name)).strong());
                let left = l.total.saturating_sub(l.done);
                if left == 0 {
                    ui.label(format!("all {} texts", l.total));
                } else {
                    ui.colored_label(ui.visuals().warn_fg_color, format!("{} of {} texts, {left} to do", l.done, l.total)).on_hover_text("A text without a translation is shown as it was written");
                }
                if ui.button("Translate…").on_hover_text("Its table: every text with its translation").widget_info_named(format!("Translate {}", l.code)).clicked() {
                    open = Some(l.code.clone());
                }
                if ui.button("Play in it").on_hover_text("Starts the game from its title screen in this language; play-tests keep starting in it until another is picked below").widget_info_named(format!("Play in {}", l.code)).clicked() {
                    want = Some(Want::PlayIn(Some(l.code.clone()), true));
                }
                if ui.small_button("Take off the list").on_hover_text("The game is no longer offered in it. Its file stays, with what was translated.").widget_info_named(format!("Take {} off the list", l.code)).clicked() {
                    remove = Some(l.code.clone());
                }
                ui.end_row();
                if !l.builtin_ui {
                    ui.label("");
                    ui.weak("The game's own menus and windows aren't translated into it by the engine: they stay English.");
                    ui.end_row();
                }
            }
        });
        // A language more.
        ui.horizontal_wrapped(|ui| {
            let by = ui.label("Add a language").id;
            ui.add(egui::TextEdit::singleline(&mut self.new_code).hint_text("its code: de, fr, pt-br").desired_width(130.0)).labelled_by(by);
            let code = self.new_code.trim().to_lowercase();
            let ok = editor_core::langs::valid_code(&code) && code != own && !langs.iter().any(|l| l.code == code);
            if ui.add_enabled(ok, egui::Button::new("Add")).on_hover_text("Saves the project, writes lang/CODE.ron with every text of the game and opens its table").clicked() {
                add = Some(code);
            }
            // (Those the engine has its own menus in: a click away.)
            for code in lang::builtin_codes().filter(|c| *c != own && !langs.iter().any(|l| l.code == *c)) {
                // (A name in a writing the editor's font lacks: the code says which.)
                let name = drawable(&lang::language_name(code));
                let name = name.trim_matches('…').trim();
                if ui.small_button(if name.is_empty() { code } else { name }).on_hover_text(format!("{code}: the game's own menus and windows are translated already")).widget_info_named(format!("Add {code}")).clicked() {
                    add = Some(code.to_string());
                }
            }
        });
        // What play-tests start in.
        ui.horizontal(|ui| {
            let by = ui.label("Play-tests start in").id;
            let shown = |code: &Option<String>| match code {
                None => "what the player's settings say".to_string(),
                Some(c) => named(c, &langs.iter().find(|l| l.code == *c).map_or_else(|| lang::language_name(c), |l| l.name.clone())),
            };
            egui::ComboBox::from_id_salt("play language").selected_text(shown(play_lang)).width(240.0).show_ui(ui, |ui| {
                let mut pick = |ui: &mut egui::Ui, code: Option<String>| {
                    if ui.selectable_label(*play_lang == code, shown(&code)).clicked() {
                        want = Some(Want::PlayIn(code, false));
                    }
                };
                pick(ui, None);
                pick(ui, Some(own.clone()));
                for l in &langs {
                    pick(ui, Some(l.code.clone()));
                }
            })
            .response
            .labelled_by(by);
        });
        match &self.said {
            Some(Ok(m)) => {
                ui.weak(m);
            }
            Some(Err(e)) => {
                ui.colored_label(ui.visuals().error_fg_color, e);
            }
            None => {}
        }
        if let Some(code) = add {
            self.add(ws, &code);
        }
        if let Some(code) = open {
            // (What was typed into the table that is open is written before another opens.)
            self.save(ws);
            self.open(ws, &code);
        }
        if let Some(code) = remove {
            self.said = Some(ws.remove_language(&code));
            if self.table.as_ref().is_some_and(|t| t.code == code) {
                self.table = None;
            }
        }
        self.table_ui(ui, ws);
        ui.add_space(6.0);
        ui.separator();
        want
    }

    /// The table of the language that is open.
    fn table_ui(&mut self, ui: &mut egui::Ui, ws: &mut Workspace) {
        let Some(table) = self.table.as_mut() else { return };
        ui.add_space(6.0);
        let (mut save, mut update, mut close) = (false, false, false);
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(named(&table.code, &table.file.name)).strong());
                let missing = table.missing();
                ui.label(if missing == 0 { format!("all {} texts are translated", table.texts.len()) } else { format!("{missing} of {} texts aren't translated yet", table.texts.len()) });
                ui.add(egui::TextEdit::singleline(&mut self.filter).hint_text("Search the texts and the translations").desired_width(240.0)).widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, "Search the texts"));
                ui.checkbox(&mut self.missing_only, "Only what isn't translated");
                save = ui.add_enabled(table.dirty, egui::Button::new("Save")).on_hover_text("Writes the language's file (Ctrl+S does too)").widget_info_named("Save the translations").clicked();
                update = ui.button("Update from the game").on_hover_text("Saves the project and takes in the texts written since the table was opened").clicked();
                close = ui.button("Close").widget_info_named("Close the table").clicked();
            });
            ui.horizontal(|ui| {
                let by = ui.label("The language's own name").on_hover_text("As players see it in Options, Language").id;
                if ui.add(egui::TextEdit::singleline(&mut table.file.name).desired_width(200.0)).labelled_by(by).changed() {
                    table.dirty = true;
                }
                if ["ja", "zh", "ko"].iter().any(|c| table.code.starts_with(c)) {
                    ui.weak("The editor's font has no letters of this writing: they show as boxes here. In the game they are drawn with the font the game has for them (Look & feel).");
                }
            });
            if !table.unused.is_empty() {
                ui.horizontal_wrapped(|ui| {
                    ui.weak(format!("{} translations are of texts the game no longer has (a text that was changed needs its translation again):", table.unused.len()));
                    if ui.small_button("Take them out").clicked() {
                        table.drop_unused();
                    }
                });
            }
            ui.separator();
            let shown: Vec<String> = table.shown(&self.filter, self.missing_only).into_iter().map(str::to_string).collect();
            if shown.is_empty() {
                ui.weak(if table.texts.is_empty() { "The game has no texts yet: names, descriptions, what characters say and quests' stages are listed here once they are written and saved." } else { "Nothing fits what is searched." });
            }
            let half = ((ui.available_width() - 24.0) / 2.0).max(160.0);
            egui::Grid::new("translations").num_columns(2).spacing([12.0, 6.0]).striped(true).show(ui, |ui| {
                for (k, text) in shown.iter().take(ROWS).enumerate() {
                    ui.vertical(|ui| {
                        ui.set_max_width(half);
                        ui.add(egui::Label::new(drawable(text)).wrap());
                    });
                    let mut translation = table.translation(text).to_string();
                    let long = text.contains('\n') || text.chars().count() > 70;
                    let edit = if long { egui::TextEdit::multiline(&mut translation).desired_rows(2) } else { egui::TextEdit::singleline(&mut translation) };
                    let label = format!("Translation {}", k + 1);
                    let r = ui.add(edit.desired_width(half).hint_text("as it was written")).widget_info_named_as(egui::Role::TextInput, label);
                    if r.changed() {
                        table.set(text, &translation);
                    }
                    ui.end_row();
                }
            });
            if shown.len() > ROWS {
                ui.weak(format!("… and {} more: search for them, or tick Only what isn't translated.", shown.len() - ROWS));
            }
        });
        if save {
            self.save(ws);
        }
        if update {
            let code = self.table.as_ref().map(|t| t.code.clone()).unwrap_or_default();
            self.save(ws);
            self.add(ws, &code);
        }
        if close {
            self.save(ws);
            self.table = None;
        }
    }
}

/// A control named for screen readers and tests when several share their text.
trait Named: Sized {
    fn widget_info_named_as(self, role: egui::Role, name: impl Into<String>) -> Self;
    fn widget_info_named(self, name: impl Into<String>) -> Self {
        self.widget_info_named_as(egui::Role::Button, name)
    }
}

impl Named for egui::Response {
    fn widget_info_named_as(self, role: egui::Role, name: impl Into<String>) -> Self {
        let name = name.into();
        self.widget_info(|| egui::WidgetInfo::labeled(role, true, name.as_str()));
        self
    }
}

#[cfg(test)]
mod ui_tests {
    use super::*;
    use egui_kittest::kittest::Queryable;
    use egui_kittest::Harness;
    use rustpg2d_engine::refactor::RefKind;

    struct State {
        ws: Workspace,
        page: LangPage,
        play: Option<String>,
        started: usize,
    }

    #[test]
    fn a_language_is_added_and_its_texts_translated_in_the_table() {
        let dir = std::env::temp_dir().join(format!("rustpg2d-langpage-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        editor_core::files::create_project(&dir).unwrap();
        let mut ws = Workspace::new(dir.clone(), editor_core::Gpu::No).unwrap();
        ws.history_frame(None);
        ws.make_def(RefKind::Item, "Golden key", None).unwrap();
        ws.history_frame(None);
        ws.save_all();
        let mut h = Harness::builder().with_size(egui::vec2(1100.0, 900.0)).build_ui_state(
            |ui, s: &mut State| {
                if let Some(Want::PlayIn(code, now)) = s.page.show(ui, &mut s.ws, &s.play) {
                    s.play = code;
                    s.started += usize::from(now);
                }
            },
            State { ws, page: LangPage::default(), play: None, started: 0 },
        );
        h.run();
        // Not translated yet: the languages the engine has its menus in are a click away.
        assert!(h.query_all_by_label_contains("isn't translated yet").next().is_some());
        h.get_by_label("Add de").click();
        h.run_steps(3);
        assert_eq!(h.state().ws.project.config.languages, ["de"]);
        assert!(dir.join("lang/de.ron").is_file());
        // Its table is open with the game's texts; one is translated and saved.
        assert!(h.query_all_by_label_contains("Deutsch (de)").next().is_some());
        let golden = h.state().page.table.as_ref().unwrap().shown("", false).iter().position(|t| *t == "Golden key").expect("the item's name is a text of the game");
        let label = format!("Translation {}", golden + 1);
        let field = h.get_by_role_and_label(egui::accesskit::Role::TextInput, label.as_str());
        field.focus();
        field.type_text("Goldener Schluessel");
        h.run_steps(3);
        assert!(h.state().page.dirty());
        h.get_by_label("Save the translations").click();
        h.run_steps(3);
        assert!(!h.state().page.dirty());
        assert_eq!(rustpg2d_engine::lang::load_file(&dir, "de").unwrap().texts.get("Golden key").map(String::as_str), Some("Goldener Schluessel"));
        let status = h.state().ws.languages().iter().map(|l| (l.code.clone(), l.done)).collect::<Vec<_>>();
        assert_eq!(status, [("de".to_string(), 1)]);
        // Play in it: play-tests start in German from now on, and one starts.
        h.get_by_label("Play in de").click();
        h.run_steps(3);
        assert_eq!((h.state().play.as_deref(), h.state().started), (Some("de"), 1));
        // Closed and taken off the list, the file stays.
        h.get_by_label("Close the table").click();
        h.run_steps(3);
        assert!(h.state().page.table.is_none());
        h.get_by_label("Take de off the list").click();
        h.run_steps(3);
        assert!(h.state().ws.project.config.languages.is_empty() && dir.join("lang/de.ron").is_file());
        let _ = std::fs::remove_dir_all(dir);
    }
}
