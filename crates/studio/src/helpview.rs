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
}

impl HelpView {
    /// Opens chapter `chapter` at heading `anchor` ("" = its top).
    pub fn open(&mut self, chapter: usize, anchor: &str) {
        let chapter = chapter.min(manual::chapters().len() - 1);
        if chapter != self.chapter {
            self.back.push(self.chapter);
        }
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

    pub fn search(&mut self, query: &str) {
        self.query = query.to_string();
        self.hits = manual::search(query, 60);
    }

    /// The script's line: `chapter=FILE at=ANCHOR hits=N [first=FILE#ANCHOR]`.
    pub fn status(&self) -> String {
        let all = manual::chapters();
        let first = self.hits.first().map_or(String::new(), |h| format!(" first={}#{}", all[h.chapter].file, h.anchor));
        format!("chapter={} at={} hits={}{first}", all[self.chapter].file, if self.at.is_empty() { "-" } else { &self.at }, self.hits.len())
    }

    /// The script's words: `open FILE [ANCHOR]`, `search TEXT…`, `hit K` (opens the K-th hit), `back`.
    pub fn word(&mut self, args: &[String]) -> Result<(), String> {
        let arg = |i: usize| args.get(i).map(String::as_str).unwrap_or("");
        match arg(0) {
            "" => {}
            "open" => self.open(manual::chapter_named(arg(1)).ok_or_else(|| format!("the manual has no chapter {}", arg(1)))?, arg(2)),
            "search" => self.search(&args[1..].join(" ")),
            "hit" => {
                let h = self.hits.get(arg(1).parse::<usize>().unwrap_or(0)).cloned().ok_or("no such hit")?;
                self.open(h.chapter, &h.anchor);
            }
            "back" => {
                if let Some(k) = self.back.pop() {
                    (self.chapter, self.jump, self.at) = (k, None, String::new());
                }
            }
            other => return Err(format!("help {other}: open, search, hit, back")),
        }
        Ok(())
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        let all = manual::chapters();
        // The chapters and the search.
        egui::Panel::left(egui::Id::unique("help chapters")).default_size(230.0).resizable(true).show(ui, |ui| {
            let r = ui.add(egui::TextEdit::singleline(&mut self.query).hint_text("Search the manual").desired_width(f32::INFINITY));
            r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, "Search the manual"));
            if r.changed() {
                self.hits = manual::search(&self.query, 60);
            }
            ui.separator();
            egui::ScrollArea::vertical().id_salt("help list").auto_shrink([false, false]).show(ui, |ui| {
                if self.query.trim().is_empty() {
                    for (k, c) in all.iter().enumerate() {
                        let title = if k == 0 { "Contents" } else { c.title.as_str() };
                        if ui.selectable_label(self.chapter == k, title).clicked() {
                            self.open(k, "");
                        }
                    }
                } else if self.hits.is_empty() {
                    ui.weak("Nothing in the manual has these words.");
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
        // The chapter.
        let chapter = &all[self.chapter];
        ui.horizontal(|ui| {
            if ui.add_enabled(!self.back.is_empty(), egui::Button::new("Back")).on_hover_text("The chapter read before").clicked() {
                if let Some(k) = self.back.pop() {
                    (self.chapter, self.jump, self.at) = (k, None, String::new());
                }
            }
            // The chapter's sections, to jump to.
            egui::ComboBox::from_id_salt("help sections").selected_text("In this chapter").width(260.0).show_ui(ui, |ui| {
                for s in chapter.sections.iter().filter(|s| s.level <= 3) {
                    let indent = "    ".repeat(s.level.saturating_sub(1) as usize);
                    if ui.selectable_label(false, format!("{indent}{}", s.title)).clicked() {
                        self.jump = Some(s.anchor.clone());
                        self.at = s.anchor.clone();
                    }
                }
            })
            .response
            .widget_info(|| egui::WidgetInfo::labeled(egui::Role::ComboBox, true, "In this chapter"));
            ui.weak("The manual still describes the editor of 0.35 in places: the menus it names are commands here (Ctrl+Shift+P finds them).");
        });
        ui.separator();
        let chapter = &all[self.chapter];
        let mut follow: Option<String> = None;
        let jump = self.jump.take();
        egui::ScrollArea::vertical().id_salt(("help text", self.chapter)).auto_shrink([false, false]).show(ui, |ui| {
            ui.set_max_width(ui.available_width().min(860.0));
            for (n, block) in chapter.blocks.iter().enumerate() {
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
}
