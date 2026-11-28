//! The Help tab: the Creator's Manual, read inside the editor. Its chapters at the left, a
//! search over all of them, the chapter itself with its headings, lists, tables and code;
//! links between chapters lead on, *Back* leads back. F1 opens it at the chapter for whatever
//! is being worked on.
//!
//! The manual's text, its parsing and its search are `editor_core::manual`.

use eframe::egui::{self, Color32, RichText};
use editor_core::manual::{self, Block, Span};

#[derive(Default)]
pub struct HelpView {
    pub chapter: usize,
    /// The heading to bring into view (once).
    jump: Option<String>,
    pub query: String,
    hits: Vec<manual::Hit>,
    /// Where the reader was before each jump.
    back: Vec<usize>,
    /// The heading last jumped to (for scripts).
    pub at: String,
    /// *What's new* is shown instead of a chapter: this release of `manual::releases`.
    pub news: Option<usize>,
    /// The field that was asked about (F1 with the pointer on it): its label and what it is
    /// for, shown above what the manual says about it.
    pub about: Option<(String, String)>,
    /// Where it is is kept in the editor's settings (the window's own Help tab; a view made
    /// elsewhere, in a test, never writes them): the place last written.
    kept: Option<(usize, String)>,
}

impl HelpView {
    /// The window's Help tab, where it was left the last time (the editor's settings).
    pub fn restored() -> HelpView {
        let (file, anchor) = editor_core::files::load_settings().help_at;
        let mut v = HelpView::default();
        if let Some(k) = manual::chapters().iter().position(|c| c.file == file) {
            v.open(k, &anchor);
            v.back.clear();
        }
        v.kept = Some((v.chapter, v.at.clone()));
        v
    }

    /// Writes where it is into the editor's settings when that changed (a chapter, a heading;
    /// not *What's new*).
    pub fn remember(&mut self) {
        let Some(kept) = &self.kept else { return };
        if self.news.is_some() || (kept.0 == self.chapter && kept.1 == self.at) {
            return;
        }
        self.kept = Some((self.chapter, self.at.clone()));
        let mut s = editor_core::files::load_settings();
        s.help_at = (manual::chapters()[self.chapter].file.to_string(), self.at.clone());
        editor_core::files::save_settings(&s);
    }

    /// Opens chapter `chapter` at heading `anchor` ("" = its top).
    pub fn open(&mut self, chapter: usize, anchor: &str) {
        let chapter = chapter.min(manual::chapters().len() - 1);
        if chapter != self.chapter && self.news.is_none() {
            self.back.push(self.chapter);
        }
        self.news = None;
        self.chapter = chapter;
        self.at = anchor.to_string();
        self.jump = Some(if anchor.is_empty() { manual::chapters()[chapter].sections.first().map_or(String::new(), |s| s.anchor.clone()) } else { anchor.to_string() });
    }

    /// Follows a link of the chapter shown: another chapter, a heading. Returns a web address
    /// it couldn't open itself.
    fn follow(&mut self, target: &str) -> Option<String> {
        if target.contains("://") {
            return Some(target.to_string());
        }
        let (file, anchor) = target.split_once('#').unwrap_or((target, ""));
        let chapter = if file.is_empty() { Some(self.chapter) } else { manual::chapter_named(file) };
        if let Some(k) = chapter {
            self.open(k, anchor);
        }
        None
    }

    /// *What's new*: release `k` of the changelog (0 = the newest).
    pub fn open_news(&mut self, k: usize) {
        self.news = Some(k.min(manual::releases().len().saturating_sub(1)));
        self.jump = None;
        self.at = String::new();
        self.query.clear();
        self.hits.clear();
        self.about = None;
    }

    /// A field of a form was asked about: what it is for, and where the manual names it.
    /// `file`: the chapter about the form it is in (what that says comes first).
    pub fn open_field(&mut self, label: &str, help: &str, file: &str) {
        self.about = Some((label.to_string(), help.to_string()));
        self.search(label);
        let home = manual::chapter_named(file);
        self.hits.sort_by_key(|h| Some(h.chapter) != home);
        // (The best place the manual has is opened at once.)
        if let Some(h) = self.hits.first().cloned() {
            self.open(h.chapter, &h.anchor);
        }
    }

    pub fn search(&mut self, query: &str) {
        self.query = query.to_string();
        self.hits = manual::search(query, 60);
    }

    /// The script's line: `chapter=FILE at=ANCHOR hits=N [first=FILE#ANCHOR]`.
    pub fn status(&self) -> String {
        let all = manual::chapters();
        let first = self.hits.first().map_or(String::new(), |h| format!(" first={}#{}", all[h.chapter].file, h.anchor));
        let shown = match self.news {
            Some(k) => format!("news:{}", manual::releases().get(k).map_or("-", |r| r.title.split_whitespace().next().unwrap_or("-"))),
            None => all[self.chapter].file.to_string(),
        };
        let about = self.about.as_ref().map_or(String::new(), |(label, _)| format!(" field={}", label.replace(' ', "_")));
        format!("chapter={shown} at={} hits={}{first}{about}", if self.at.is_empty() { "-" } else { &self.at }, self.hits.len())
    }

    /// The script's words: `open FILE [ANCHOR]`, `search TEXT…`, `hit K` (opens the K-th hit),
    /// `back`, `news [K]` (*What's new*, release K).
    pub fn word(&mut self, args: &[String]) -> Result<(), String> {
        let arg = |i: usize| args.get(i).map(String::as_str).unwrap_or("");
        match arg(0) {
            "" => {}
            "open" => self.open(manual::chapter_named(arg(1)).ok_or_else(|| format!("the manual has no chapter {}", arg(1)))?, arg(2)),
            "search" => {
                self.about = None;
                self.search(&args[1..].join(" "));
            }
            "news" => self.open_news(arg(1).parse().unwrap_or(0)),
            "hit" => {
                let h = self.hits.get(arg(1).parse::<usize>().unwrap_or(0)).cloned().ok_or("no such hit")?;
                self.open(h.chapter, &h.anchor);
            }
            "back" => {
                if let Some(k) = self.back.pop() {
                    (self.chapter, self.jump, self.at) = (k, None, String::new());
                }
            }
            other => return Err(format!("help {other}: open, search, hit, back, news")),
        }
        Ok(())
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        self.remember();
        let all = manual::chapters();
        // The chapters and the search.
        egui::Panel::left(egui::Id::unique("help chapters")).default_size(230.0).resizable(true).show(ui, |ui| {
            let r = ui.add(egui::TextEdit::singleline(&mut self.query).hint_text("Search the manual").desired_width(f32::INFINITY));
            r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, "Search the manual"));
            if r.changed() {
                self.hits = manual::search(&self.query, 60);
                self.about = None;
            }
            ui.separator();
            // The field that was asked about, before what the manual has on it.
            let mut put_away = false;
            if let Some((label, help)) = &self.about {
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(label).strong());
                        ui.weak("a field");
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let r = ui.small_button("x").on_hover_text("Put this away");
                            r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "Put this away"));
                            put_away = r.clicked();
                        });
                    });
                    ui.label(if help.is_empty() { "Nothing more is written about this field." } else { help.as_str() });
                });
                ui.weak(if self.hits.is_empty() { "The manual doesn't name it." } else { "Where the manual names it:" });
                ui.add_space(2.0);
            }
            if put_away {
                self.about = None;
            }
            egui::ScrollArea::vertical().id_salt("help list").auto_shrink([false, false]).show(ui, |ui| {
                if self.query.trim().is_empty() {
                    for (k, c) in all.iter().enumerate() {
                        let title = if k == 0 { "Contents" } else { c.title.as_str() };
                        if ui.selectable_label(self.news.is_none() && self.chapter == k, title).clicked() {
                            self.open(k, "");
                        }
                    }
                    ui.separator();
                    if ui.selectable_label(self.news.is_some(), "What's new").on_hover_text("What changed in this version of the editor, and in the ones before").clicked() {
                        self.open_news(0);
                    }
                } else if self.hits.is_empty() {
                    if self.about.is_none() {
                        ui.weak("Nothing in the manual has these words.");
                    }
                } else {
                    let mut go = None;
                    for (k, h) in self.hits.iter().enumerate() {
                        let chapter = &all[h.chapter];
                        let title = if h.line == chapter.title { h.section.clone() } else { format!("{} · {}", chapter.title, h.section) };
                        if ui.link(RichText::new(title).strong()).clicked() {
                            go = Some(k);
                        }
                        if h.line != chapter.title {
                            let line: String = h.line.chars().take(140).collect();
                            ui.weak(line);
                        }
                        ui.add_space(4.0);
                    }
                    if let Some(k) = go {
                        let h = self.hits[k].clone();
                        self.open(h.chapter, &h.anchor);
                    }
                }
            });
        });
        // The chapter, or a release's news.
        let releases = manual::releases();
        let (blocks, sections): (&[Block], &[manual::Section]) = match self.news.and_then(|k| releases.get(k)) {
            Some(r) => (&r.blocks, &r.sections),
            None => (&all[self.chapter].blocks, &all[self.chapter].sections),
        };
        ui.horizontal(|ui| {
            if ui.add_enabled(!self.back.is_empty() || self.news.is_some(), egui::Button::new("Back")).on_hover_text("The chapter read before").clicked() {
                if self.news.take().is_none() {
                    if let Some(k) = self.back.pop() {
                        self.chapter = k;
                    }
                }
                (self.jump, self.at) = (None, String::new());
            }
            // Which version's news.
            if let Some(now) = self.news {
                let mut pick = None;
                egui::ComboBox::from_id_salt("help releases").selected_text(release_title(&releases[now.min(releases.len() - 1)])).width(220.0).show_ui(ui, |ui| {
                    for (k, r) in releases.iter().enumerate() {
                        if ui.selectable_label(k == now, release_title(r)).clicked() {
                            pick = Some(k);
                        }
                    }
                })
                .response
                .widget_info(|| egui::WidgetInfo::labeled(egui::Role::ComboBox, true, "Version"));
                if let Some(k) = pick {
                    self.open_news(k);
                }
            }
            // The chapter's sections, to jump to.
            egui::ComboBox::from_id_salt("help sections").selected_text(if self.news.is_some() { "In this version" } else { "In this chapter" }).width(260.0).show_ui(ui, |ui| {
                for s in sections.iter().filter(|s| s.level <= 3) {
                    let indent = "    ".repeat(s.level.saturating_sub(if self.news.is_some() { 3 } else { 1 }) as usize);
                    if ui.selectable_label(false, format!("{indent}{}", s.title)).clicked() {
                        self.jump = Some(s.anchor.clone());
                        self.at = s.anchor.clone();
                    }
                }
            })
            .response
            .widget_info(|| egui::WidgetInfo::labeled(egui::Role::ComboBox, true, "In this chapter"));
            if self.news.is_none() {
                ui.weak("The manual still describes the editor of 0.35 in places: the menus it names are commands here (Ctrl+Shift+P finds them).");
            }
        });
        ui.separator();
        let mut follow: Option<String> = None;
        let jump = self.jump.take();
        egui::ScrollArea::vertical().id_salt(("help text", self.chapter, self.news)).auto_shrink([false, false]).show(ui, |ui| {
            ui.set_max_width(ui.available_width().min(860.0));
            if let Some(r) = self.news.and_then(|k| releases.get(k)) {
                ui.label(RichText::new(format!("What's new: {}", release_title(r))).heading().strong().size(22.0));
                ui.add_space(4.0);
            }
            for (n, block) in blocks.iter().enumerate() {
                match block {
                    Block::Heading(level, title, anchor) => {
                        ui.add_space(if *level <= 2 { 10.0 } else { 6.0 });
                        let text = match level {
                            1 => RichText::new(title).heading().strong().size(22.0),
                            2 => RichText::new(title).heading(),
                            _ => RichText::new(title).strong(),
                        };
                        let r = ui.label(text);
                        if jump.as_deref() == Some(anchor.as_str()) {
                            r.scroll_to_me(Some(egui::Align::TOP));
                        }
                        ui.add_space(2.0);
                    }
                    Block::Para(spans) => {
                        line(ui, spans, &mut follow);
                        ui.add_space(4.0);
                    }
                    Block::List(ordered, items) => {
                        let mut number = 0;
                        for (depth, spans) in items {
                            ui.horizontal_top(|ui| {
                                ui.add_space(8.0 + *depth as f32 * 16.0);
                                if *ordered && *depth == 0 {
                                    number += 1;
                                    ui.label(format!("{number}."));
                                } else {
                                    ui.label("•");
                                }
                                ui.vertical(|ui| line(ui, spans, &mut follow));
                            });
                        }
                        ui.add_space(4.0);
                    }
                    Block::Code(code) => {
                        egui::Frame::group(ui.style()).fill(Color32::from_rgb(28, 30, 34)).show(ui, |ui| {
                            ui.add(egui::Label::new(RichText::new(code).monospace()).wrap_mode(egui::TextWrapMode::Extend).selectable(true));
                        });
                        ui.add_space(4.0);
                    }
                    Block::Table(rows) => {
                        let columns = rows.iter().map(Vec::len).max().unwrap_or(1).max(1);
                        // (The first column is a name, the last the explanation: it gets the room.)
                        let width = ui.available_width();
                        let first = (width * 0.26).min(220.0);
                        let rest = ((width - first - 12.0 * columns as f32) / (columns.max(2) - 1) as f32).max(80.0);
                        egui::Grid::new(("help table", self.chapter, n)).num_columns(columns).striped(true).spacing([12.0, 6.0]).show(ui, |ui| {
                            for (r, row) in rows.iter().enumerate() {
                                for (c, cell) in row.iter().enumerate() {
                                    ui.vertical(|ui| {
                                        ui.set_max_width(if c == 0 && columns > 1 { first } else { rest });
                                        if r == 0 {
                                            ui.label(RichText::new(manual::plain(&plain_of(cell))).strong());
                                        } else {
                                            line(ui, cell, &mut follow);
                                        }
                                    });
                                }
                                ui.end_row();
                            }
                        });
                        ui.add_space(6.0);
                    }
                    Block::Picture(alt) => {
                        ui.weak(format!("(Picture: {alt})"));
                        ui.add_space(4.0);
                    }
                    Block::Rule => {
                        ui.separator();
                    }
                }
            }
            ui.add_space(40.0);
        });
        if let Some(target) = follow {
            if let Some(web) = self.follow(&target) {
                ui.ctx().open_url(egui::OpenUrl::new_tab(web));
            }
        }
    }
}

/// A release as its list names it: its heading, or what isn't released yet.
fn release_title(r: &manual::Release) -> String {
    if r.version.is_empty() {
        format!("This version ({})", manual::VERSION)
    } else {
        r.title.clone()
    }
}

/// The spans of a cell as one text (a table's header is shown strong).
fn plain_of(spans: &[Span]) -> String {
    spans
        .iter()
        .map(|s| match s {
            Span::Text(t) | Span::Bold(t) | Span::Italic(t) | Span::Code(t) | Span::Link(t, _) => t.as_str(),
        })
        .collect()
}

/// A line of text with its bold, italic, code and links, wrapped.
fn line(ui: &mut egui::Ui, spans: &[Span], follow: &mut Option<String>) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for s in spans {
            match s {
                Span::Text(t) => {
                    ui.label(t.as_str());
                }
                Span::Bold(t) => {
                    ui.label(RichText::new(t).strong());
                }
                Span::Italic(t) => {
                    ui.label(RichText::new(t).italics());
                }
                Span::Code(t) => {
                    ui.label(RichText::new(t).monospace().background_color(Color32::from_rgb(40, 42, 48)));
                }
                Span::Link(text, target) => {
                    let leads = !target.starts_with("../");
                    if leads && ui.link(text.as_str()).on_hover_text(if target.contains("://") { target.as_str() } else { "Opens that part of the manual" }).clicked() {
                        *follow = Some(target.clone());
                    } else if !leads {
                        ui.label(text.as_str());
                    }
                }
            }
        }
    });
}

#[cfg(test)]
mod ui_tests {
    use super::*;
    use egui_kittest::kittest::Queryable;
    use egui_kittest::Harness;

    #[test]
    fn the_manual_is_read_searched_and_its_links_followed() {
        let mut h = Harness::builder().with_size(egui::vec2(1200.0, 800.0)).build_ui_state(|ui, v: &mut HelpView| v.show(ui), HelpView::default());
        h.run();
        // It opens on the contents; a chapter is a click away.
        h.get_by_label("The R(ust)PG2D Creator's Manual");
        h.get_by_label("3. Maps").click();
        h.run();
        assert_eq!(manual::chapters()[h.state().chapter].file, "03-maps.md");
        h.get_by_label("Painted ground");
        // Searching finds a section by its title and opens it there.
        let field = h.get_by_role_and_label(egui::accesskit::Role::TextInput, "Search the manual");
        field.focus();
        field.type_text("lighting");
        h.run();
        assert!(!h.state().hits.is_empty());
        let first = h.state().hits[0].clone();
        h.state_mut().open(first.chapter, &first.anchor);
        h.run();
        assert_eq!(h.state().at, first.anchor);
        // Back leads to the chapter read before (when the hit was in another one).
        h.state_mut().open(1, "");
        h.run();
        h.get_by_label("Back").click();
        h.run();
        assert_eq!(h.state().chapter, first.chapter);
        // A link inside the text leads to its chapter and heading.
        h.state_mut().open(0, "");
        h.state_mut().query.clear();
        h.run();
        assert_eq!(h.state_mut().follow("07-events.md#triggers"), None);
        h.run();
        assert_eq!((manual::chapters()[h.state().chapter].file, h.state().at.as_str()), ("07-events.md", "triggers"));
        assert_eq!(h.state_mut().follow("https://example.org"), Some("https://example.org".to_string()));
    }

    #[test]
    fn every_chapter_and_every_release_is_drawn_in_characters_the_font_has() {
        // (The harness stops at a character without a glyph: what the manual or the changelog
        // gains must be drawable, or `manual::drawable` learns it.)
        let mut h = Harness::builder().with_size(egui::vec2(1200.0, 800.0)).build_ui_state(|ui, v: &mut HelpView| v.show(ui), HelpView::default());
        for k in 0..manual::chapters().len() {
            h.state_mut().open(k, "");
            h.run_steps(2);
        }
        for k in 0..manual::releases().len() {
            h.state_mut().open_news(k);
            h.run_steps(2);
        }
        // Searching shows lines of any chapter.
        for words in ["the", "choose", "language", "pad"] {
            h.state_mut().search(words);
            h.run_steps(2);
        }
    }

    #[test]
    fn whats_new_is_a_chapter_of_its_own_and_a_field_is_explained() {
        let mut h = Harness::builder().with_size(egui::vec2(1200.0, 800.0)).build_ui_state(|ui, v: &mut HelpView| v.show(ui), HelpView::default());
        h.run();
        // What's new: at the end of the chapters, the newest version first, the others to pick.
        h.get_by_label("What's new").click();
        h.run();
        assert_eq!(h.state().news, Some(0));
        let newest = release_title(&manual::releases()[0]);
        assert!(h.query_all_by_label_contains(&format!("What's new: {newest}")).next().is_some());
        assert!(h.state().status().starts_with("chapter=news:"), "{}", h.state().status());
        if manual::releases().len() > 1 {
            h.state_mut().open_news(1);
            h.run();
            assert!(h.query_all_by_label_contains(&format!("What's new: {}", release_title(&manual::releases()[1]))).next().is_some());
        }
        // Back leads to the chapter that was read; a chapter picked leaves the news too.
        h.get_by_label("Back").click();
        h.run();
        assert_eq!((h.state().news, h.state().chapter), (None, 0));
        // A field asked about: what it is for stands above where the manual names it, and the
        // chapter about its form comes first.
        h.state_mut().open_field("Stack size", "How many fit in one inventory slot.", "05-database.md");
        h.run();
        assert!(h.query_all_by_label("Stack size").next().is_some());
        h.get_by_label("How many fit in one inventory slot.");
        h.get_by_label("Where the manual names it:");
        assert_eq!(manual::chapters()[h.state().chapter].file, "05-database.md");
        assert!(h.state().status().ends_with("field=Stack_size"), "{}", h.state().status());
        // One the manual doesn't name says so; put away, the search is a search again.
        h.state_mut().open_field("Zzyzx", "", "05-database.md");
        h.run();
        h.get_by_label("The manual doesn't name it.");
        h.get_by_label("Nothing more is written about this field.");
        h.get_by_label("Put this away").click();
        h.run();
        assert!(h.state().about.is_none());
    }

    #[test]
    fn the_help_tab_opens_where_it_was_left() {
        let dir = std::env::temp_dir().join(format!("rustpg2d-helpat-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        editor_core::files::config_dir_here(Some(dir.clone()));
        // Never opened: the first chapter.
        let mut v = HelpView::restored();
        assert_eq!(v.chapter, 0);
        // A chapter at a heading: written once it changed.
        let k = manual::chapters().iter().position(|c| c.file == "03-maps.md").unwrap();
        v.open(k, "painted-ground");
        v.remember();
        assert_eq!(editor_core::files::load_settings().help_at, ("03-maps.md".to_string(), "painted-ground".to_string()));
        // What's new isn't a place to come back to.
        v.open_news(0);
        v.remember();
        assert_eq!(editor_core::files::load_settings().help_at.0, "03-maps.md");
        // A new editor: there again.
        let again = HelpView::restored();
        assert_eq!((again.chapter, again.at.as_str()), (k, "painted-ground"));
        assert!(again.back.is_empty());
        // A Help view that isn't the window's writes nothing.
        let mut other = HelpView::default();
        other.open(0, "");
        other.remember();
        assert_eq!(editor_core::files::load_settings().help_at.0, "03-maps.md");
        editor_core::files::config_dir_here(None);
        let _ = std::fs::remove_dir_all(dir);
    }
}
