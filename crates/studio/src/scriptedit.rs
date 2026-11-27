//! A script in a tab: the text with its colours and line numbers, suggestions under the cursor
//! (the script API, the project's own functions, and inside a string argument the ids it can
//! name), the signature of the call being written, what is wrong at its line while typing, and
//! the file's functions as an outline. The thinking is `editor_core::scripts`; this is the
//! text field around it.

use std::path::PathBuf;

use eframe::egui::{self, text::LayoutJob, Color32, FontId, Key, Modifiers, Stroke, TextFormat};
use editor_core::scripts::{self, Completion, Issue, Known, Paint};
use editor_core::Workspace;

/// How long after the last key the text is checked (seconds).
const CHECK_AFTER: f64 = 0.4;

fn color(p: Paint) -> Color32 {
    match p {
        Paint::Comment => Color32::from_rgb(110, 150, 110),
        Paint::Str => Color32::from_rgb(230, 170, 110),
        Paint::Num => Color32::from_rgb(180, 210, 140),
        Paint::Keyword => Color32::from_rgb(200, 140, 230),
        Paint::Api => Color32::from_rgb(240, 210, 110),
        Paint::Func => Color32::from_rgb(120, 190, 250),
        Paint::Const => Color32::from_rgb(110, 220, 220),
        Paint::Text => Color32::from_rgb(215, 215, 215),
        Paint::Punct => Color32::from_rgb(170, 170, 170),
    }
}

/// The script as egui lays it out: its colours, what is wrong underlined in red.
fn highlight(src: &str, font: FontId, issues: &[Issue], user_fns: &[String]) -> LayoutJob {
    let mut job = LayoutJob::default();
    for (what, s, e) in scripts::paint(src, user_fns) {
        let bad = issues.iter().find(|i| s < i.span.1 && i.span.0 < e);
        let (color, underline) = match bad {
            Some(i) if i.warning => (color(what), Stroke::new(1.5_f32, Color32::from_rgb(230, 190, 60))),
            Some(_) => (Color32::from_rgb(255, 120, 110), Stroke::new(1.5_f32, Color32::RED)),
            None => (color(what), Stroke::NONE),
        };
        job.append(&src[s..e], 0.0, TextFormat { font_id: font.clone(), color, underline, ..Default::default() });
    }
    job
}

/// What a script tab asks of the window.
pub enum Ask {
    /// Go to the definition of this function (in whichever script it is).
    Function(String),
}

pub struct ScriptTab {
    pub path: PathBuf,
    pub text: String,
    saved: String,
    comp: Option<Completion>,
    /// Char index to put the cursor at with the next frame, and one to scroll to.
    set_cursor: Option<usize>,
    scroll_to: Option<usize>,
    /// What is wrong in the text as it was when it was checked last.
    issues: Vec<Issue>,
    checked: Option<String>,
    /// When the text last changed (`None`: nothing waits to be checked).
    changed_at: Option<f64>,
    /// The functions of the project's scripts, this one as it is being written.
    signatures: Vec<(String, Vec<String>)>,
    functions: Vec<String>,
    /// When the file was last looked at, and what it holds now when that isn't what was read
    /// or written here (changed by a rename, an agent, another program).
    looked: f64,
    on_disk: Option<String>,
}

impl ScriptTab {
    pub fn open(path: PathBuf) -> ScriptTab {
        let text = std::fs::read_to_string(&path).unwrap_or_default().replace("\r\n", "\n");
        ScriptTab { path, saved: text.clone(), text, comp: None, set_cursor: None, scroll_to: None, issues: Vec::new(), checked: None, changed_at: Some(0.0), signatures: Vec::new(), functions: Vec::new(), looked: 0.0, on_disk: None }
    }

    pub fn name(&self) -> String {
        self.path.file_name().unwrap_or_default().to_string_lossy().to_string()
    }

    pub fn dirty(&self) -> bool {
        self.text != self.saved
    }

    /// Writes the file. The project's functions and ids are read again by the caller.
    pub fn save(&mut self) -> Result<(), String> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        std::fs::write(&self.path, &self.text).map_err(|e| format!("{}: {e}", self.name()))?;
        self.saved = self.text.clone();
        self.on_disk = None;
        Ok(())
    }

    /// Shows char `at` of the text (a problem's place, a function's definition).
    pub fn reveal(&mut self, at: usize) {
        self.set_cursor = Some(at);
        self.scroll_to = Some(at);
    }

    /// Where function `name` is defined in this text (a char index).
    pub fn function_at(&self, name: &str) -> Option<usize> {
        rustpg2d_engine::script_refs::functions(&self.text).into_iter().find(|f| f.name == name).and_then(|_| self.text.find(&format!("fn {name}"))).map(|b| self.text[..b].chars().count())
    }

    /// `script=main.rhai:saved|changed:problems N:functions N[:suggestions a,b,c]` for the script's `status`.
    pub fn status(&self) -> String {
        let mut s = format!("{}:{}:problems{}:functions{}", self.name(), if self.dirty() { "changed" } else { "saved" }, self.issues.len(), rustpg2d_engine::script_refs::functions(&self.text).len());
        if let Some(c) = &self.comp {
            s += &format!(":suggestions {}", c.cands.iter().take(5).map(|c| c.insert.as_str()).collect::<Vec<_>>().join(","));
        }
        s
    }

    /// The file changed under the tab: taken when nothing was changed here, else asked about.
    fn look_at_file(&mut self, now: f64) {
        if now - self.looked < 1.0 {
            return;
        }
        self.looked = now;
        let Ok(disk) = std::fs::read_to_string(&self.path).map(|t| t.replace("\r\n", "\n")) else { return };
        if disk == self.saved {
            self.on_disk = None;
        } else if !self.dirty() {
            self.text = disk.clone();
            self.saved = disk;
            self.changed_at = Some(now - CHECK_AFTER);
            self.on_disk = None;
        } else {
            self.on_disk = Some(disk);
        }
    }

    /// Checks the text when it has been left alone for a moment.
    fn check(&mut self, ws: &Workspace, now: f64) {
        if self.changed_at.is_none_or(|t| now - t < CHECK_AFTER) && self.checked.is_some() {
            return;
        }
        if self.checked.as_deref() == Some(self.text.as_str()) {
            self.changed_at = None;
            return;
        }
        self.signatures = ws.script_signatures(Some((&self.path, &self.text)));
        self.functions = self.signatures.iter().map(|(n, _)| n.clone()).collect();
        self.issues = ws.script_issues(&self.text, &self.functions);
        self.checked = Some(self.text.clone());
        self.changed_at = None;
    }

    /// The tab. Returns what it asks of the window.
    pub fn show(&mut self, ui: &mut egui::Ui, ws: &mut Workspace) -> Option<Ask> {
        let now = ui.input(|i| i.time);
        self.look_at_file(now);
        self.check(ws, now);
        if self.changed_at.is_some() {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(150));
        }
        let mut ask = None;
        let id = egui::Id::unique(("script text", &self.path));
        let font = egui::TextStyle::Monospace.resolve(ui.style());

        // The bar: the file, what is wrong, saving.
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(self.name()).strong());
            if self.dirty() {
                if ui.button("Save").on_hover_text("Ctrl+S saves everything").clicked() {
                    match self.save() {
                        Ok(()) => {
                            ws.refresh_refs();
                            ws.set_status(format!("Saved {}", self.name()));
                        }
                        Err(e) => ws.set_status(e),
                    }
                }
                ui.weak("changed");
            }
            let (errors, warnings) = (self.issues.iter().filter(|i| !i.warning).count(), self.issues.iter().filter(|i| i.warning).count());
            match (errors, warnings) {
                (0, 0) if self.changed_at.is_some() => ui.weak("checking…"),
                (0, 0) => ui.weak("no problems"),
                (e, w) => ui.colored_label(if e > 0 { ui.visuals().error_fg_color } else { ui.visuals().warn_fg_color }, format!("{e} wrong, {w} to look at")),
            };
            if let Some(disk) = self.on_disk.clone() {
                ui.separator();
                ui.colored_label(ui.visuals().warn_fg_color, "The file changed outside this tab.");
                if ui.button("Take the file's").on_hover_text("What was typed here since it was saved is lost").clicked() {
                    self.text = disk.clone();
                    self.saved = disk;
                    self.on_disk = None;
                    self.changed_at = Some(now);
                } else if ui.button("Keep mine").on_hover_text("Saving writes this text over the file's").clicked() {
                    self.saved = disk;
                    self.on_disk = None;
                }
            }
        });

        // Keys for the open suggestions (taken before the text field sees them).
        let focused = ui.memory(|m| m.has_focus(id));
        let mut accept_now = None;
        if let Some(comp) = &mut self.comp {
            if focused {
                ui.input_mut(|i| {
                    let n = comp.cands.len().max(1);
                    if i.consume_key(Modifiers::NONE, Key::ArrowDown) {
                        comp.sel = (comp.sel + 1) % n;
                    }
                    if i.consume_key(Modifiers::NONE, Key::ArrowUp) {
                        comp.sel = (comp.sel + n - 1) % n;
                    }
                    if i.consume_key(Modifiers::NONE, Key::Tab) || i.consume_key(Modifiers::NONE, Key::Enter) {
                        accept_now = Some(comp.sel);
                    }
                });
            }
            if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
                self.comp = None;
            }
        }
        let force = focused && ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Space));
        let mut changed = false;
        if let Some(comp) = accept_now.and_then(|_| self.comp.take()) {
            if let Some(at) = accept_now.and_then(|k| scripts::accept_at(&mut self.text, &comp, k)) {
                self.set_cursor = Some(at);
                changed = true;
            }
        }
        if let Some(ci) = self.set_cursor.take() {
            let mut state = egui::text_edit::TextEditState::load(ui.ctx(), id).unwrap_or_default();
            state.cursor.set_char_range(Some(egui::text::CCursorRange::one(egui::text::CCursor::new(ci))));
            state.store(ui.ctx(), id);
            ui.memory_mut(|m| m.request_focus(id));
        }

        // The call around the cursor (as it was a frame ago): its parameters, the one being written.
        let cursor_before = egui::text_edit::TextEditState::load(ui.ctx(), id).and_then(|s| s.cursor.char_range()).map(|r| usize::from(r.primary.index));
        let call = cursor_before.and_then(|ci| scripts::analyze(&self.text[..scripts::byte_of(&self.text, ci)]).1);
        ui.horizontal(|ui| {
            ui.set_min_height(18.0);
            match call.as_ref().and_then(|c| scripts::signature_of(c, &self.signatures).map(|s| (c, s))) {
                Some((c, (params, doc))) => {
                    let mut job = LayoutJob::default();
                    let normal = TextFormat { font_id: font.clone(), color: Color32::LIGHT_GRAY, ..Default::default() };
                    let strong = TextFormat { font_id: font.clone(), color: Color32::YELLOW, underline: Stroke::new(1.0_f32, Color32::YELLOW), ..Default::default() };
                    job.append(&format!("{}(", c.name), 0.0, normal.clone());
                    for (k, p) in params.iter().enumerate() {
                        if k > 0 {
                            job.append(", ", 0.0, normal.clone());
                        }
                        job.append(p, 0.0, if k == c.arg { strong.clone() } else { normal.clone() });
                    }
                    job.append(")", 0.0, normal);
                    job.append(&format!("   {doc}"), 0.0, TextFormat { font_id: FontId::proportional(12.0), color: Color32::GRAY, ..Default::default() });
                    ui.label(job);
                }
                None => {
                    ui.weak("Ctrl+Space: suggestions · Tab or Enter takes one · Ctrl+click a function: go to it");
                }
            }
        });
        ui.separator();

        // The outline: the file's functions.
        let outline: Vec<(String, usize)> = rustpg2d_engine::script_refs::functions(&self.text).into_iter().filter_map(|f| self.text.find(&format!("fn {}", f.name)).map(|b| (f.name, self.text[..b].chars().count()))).collect();
        egui::Panel::left(egui::Id::unique(("script outline", &self.path))).default_size(170.0).resizable(true).show(ui, |ui| {
            ui.weak(format!("{} functions", outline.len()));
            egui::ScrollArea::vertical().id_salt("outline").auto_shrink([false, false]).show(ui, |ui| {
                for (name, at) in &outline {
                    if ui.add(egui::Button::selectable(false, format!("{name}()")).truncate()).clicked() {
                        self.set_cursor = Some(*at);
                        self.scroll_to = Some(*at);
                    }
                }
            });
        });
        // What is wrong, under the text: a click goes to the line.
        if !self.issues.is_empty() {
            egui::Panel::bottom(egui::Id::unique(("script issues", &self.path))).default_size(86.0).resizable(true).show(ui, |ui| {
                egui::ScrollArea::vertical().id_salt("issues").auto_shrink([false, false]).show(ui, |ui| {
                    for i in &self.issues {
                        let color = if i.warning { ui.visuals().warn_fg_color } else { ui.visuals().error_fg_color };
                        if ui.add(egui::Label::new(egui::RichText::new(format!("Line {}: {}", i.line, i.message)).color(color)).truncate().sense(egui::Sense::click())).on_hover_text(i.message.as_str()).clicked() {
                            let at = self.text[..i.span.0.min(self.text.len())].chars().count();
                            self.set_cursor = Some(at);
                            self.scroll_to = Some(at);
                        }
                    }
                });
            });
        }

        // The text.
        let typed: Vec<String> = ui.input(|i| i.events.iter().filter_map(|e| if let egui::Event::Text(t) = e { Some(t.clone()) } else { None }).collect());
        let enter = ui.input(|i| i.key_pressed(Key::Enter)) && accept_now.is_none();
        let (issues, user_fns) = (self.issues.clone(), self.functions.clone());
        let mut layouter = |ui: &egui::Ui, buf: &dyn egui::TextBuffer, _wrap: f32| {
            let mut job = highlight(buf.as_str(), font.clone(), &issues, &user_fns);
            job.wrap.max_width = f32::INFINITY;
            ui.fonts_mut(|f| f.layout_job(job))
        };
        let rows = self.text.lines().count().max(1) + usize::from(self.text.ends_with('\n'));
        let scroll_to = self.scroll_to.take();
        let text = &mut self.text;
        let out = egui::ScrollArea::both()
            .id_salt("script text")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    let numbers: String = (1..=rows).map(|n| format!("{n:>4}\n")).collect();
                    ui.add(egui::Label::new(egui::RichText::new(numbers).font(font.clone()).color(Color32::from_gray(110))).selectable(false));
                    let out = egui::TextEdit::multiline(text).id(id).code_editor().desired_width(f32::INFINITY).desired_rows(30).lock_focus(true).layouter(&mut layouter).show(ui);
                    out.response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::MultilineTextInput, true, "The script"));
                    if let Some(ci) = scroll_to {
                        let r = out.galley.pos_from_cursor(egui::text::CCursor::new(ci)).translate(out.galley_pos.to_vec2());
                        ui.scroll_to_rect(r.expand2(egui::vec2(0.0, 2.0)), Some(egui::Align::TOP));
                    }
                    out
                })
                .inner
            })
            .inner;
        changed |= out.response.changed();
        let cursor = out.cursor_range.map(|r| usize::from(r.primary.index));

        // What typing adds by itself: the indent of a new line, the closer of a bracket.
        if out.response.changed() {
            if let Some(ci) = cursor {
                let typed_one = typed.last().filter(|t| t.chars().count() == 1).and_then(|t| t.chars().next());
                if let Some(at) = scripts::after_typing(&mut self.text, ci, typed_one, enter) {
                    self.set_cursor = Some(at);
                }
            }
        }
        if changed {
            self.changed_at = Some(now);
        }

        // Suggestions: after typing (or Ctrl+Space), from the text before the cursor.
        match cursor {
            Some(ci) => {
                let typed_now = changed && !typed.is_empty();
                let force = force || self.comp.as_ref().is_some_and(|c| c.forced);
                if typed_now || force || self.comp.is_some() {
                    let known = Known { db: &ws.project.db, maps: &ws.refs.maps, spawns: &ws.refs.spawns, targets: &ws.refs.targets, flags: &ws.refs.all_flags, vars: &ws.refs.vars, functions: &self.functions, signatures: &self.signatures, factions: &ws.refs.factions };
                    self.comp = scripts::complete(&self.text, ci, &known, typed_now, force, self.comp.as_ref());
                }
            }
            None => self.comp = None,
        }
        // While the suggestions are open, Esc closes them and the text keeps the keyboard.
        ui.memory_mut(|m| m.set_focus_lock_filter(id, egui::EventFilter { tab: true, horizontal_arrows: true, vertical_arrows: true, escape: self.comp.is_some() }));

        // Ctrl+click: the definition of the function under the cursor.
        if out.response.clicked() && ui.input(|i| i.modifiers.command) {
            if let Some(ci) = cursor {
                let word = scripts::word_at(&self.text, ci).to_string();
                if self.functions.contains(&word) {
                    match self.function_at(&word) {
                        Some(at) => self.reveal(at),
                        None => ask = Some(Ask::Function(word)),
                    }
                }
            }
        }

        // The suggestions under the cursor.
        if let (Some(comp), Some(ci)) = (&self.comp, cursor) {
            let r = out.galley.pos_from_cursor(egui::text::CCursor::new(ci)).translate(out.galley_pos.to_vec2());
            let mut clicked = None;
            egui::Area::new(egui::Id::unique("script suggestions")).order(egui::Order::Foreground).fixed_pos(r.left_bottom() + egui::vec2(0.0, 2.0)).show(ui.ctx(), |ui| {
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
                let comp = self.comp.take().expect("checked above");
                if let Some(at) = scripts::accept_at(&mut self.text, &comp, k) {
                    self.set_cursor = Some(at);
                    self.changed_at = Some(now);
                }
            }
        }
        // What agents and the watcher of files see as the script being written.
        if ws.db.script_file.as_ref() != Some(&self.path) || ws.db.script_dirty != self.dirty() {
            ws.db.script_file = Some(self.path.clone());
            ws.db.script_dirty = self.dirty();
        }
        ask
    }
}

#[cfg(test)]
mod ui_tests {
    use super::*;
    use egui_kittest::kittest::Queryable;
    use egui_kittest::Harness;

    struct State {
        ws: Workspace,
        tab: ScriptTab,
        asked: Vec<String>,
        dir: PathBuf,
    }

    fn copy(from: &std::path::Path, to: &std::path::Path) {
        std::fs::create_dir_all(to).unwrap();
        for e in std::fs::read_dir(from).unwrap().flatten() {
            let (src, dst) = (e.path(), to.join(e.file_name()));
            if src.is_dir() {
                if e.file_name() != "saves" {
                    copy(&src, &dst);
                }
            } else {
                std::fs::copy(&src, &dst).unwrap();
            }
        }
    }

    /// A script of its own in a scratch copy of the example.
    fn script(test: &str, text: &str) -> Harness<'static, State> {
        let dir = std::env::temp_dir().join(format!("rustpg2d-script-{}-{test}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        copy(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../example_project"), &dir);
        let path = dir.join("scripts/test.rhai");
        std::fs::write(&path, text).unwrap();
        let mut ws = Workspace::new(dir.clone(), editor_core::Gpu::No).unwrap();
        ws.refresh_refs();
        let state = State { ws, tab: ScriptTab::open(path), asked: Vec::new(), dir };
        Harness::builder().with_size(egui::vec2(1000.0, 700.0)).build_ui_state(
            |ui, s: &mut State| {
                if let Some(Ask::Function(f)) = s.tab.show(ui, &mut s.ws) {
                    s.asked.push(f);
                }
            },
            state,
        )
    }

    #[test]
    fn a_script_shows_its_functions_and_what_is_wrong_and_is_saved() {
        let mut h = script("check", "fn greet() {\n    say(\"Hello\");\n}\n\nfn reward() {\n    give_item(\"no_such_item\", 1);\n    then_call(\"greet\");\n}\n");
        h.run();
        // Its functions are the outline; an id that isn't there is said with its line.
        assert!(h.query_all_by_label("greet()").next().is_some() && h.query_all_by_label("reward()").next().is_some());
        assert_eq!(h.state().tab.issues.len(), 1, "{:?}", h.state().tab.issues);
        assert_eq!(h.state().tab.issues[0].line, 6);
        assert!(h.query_all_by_label_contains("Line 6: ").next().is_some());
        assert!(h.query_all_by_label("Save").next().is_none(), "nothing to save yet");
        // The text put right: checked again a moment later, and saved with its button.
        let fixed = h.state().tab.text.replace("no_such_item", "potion");
        h.state_mut().tab.text = fixed.clone();
        h.state_mut().tab.changed_at = Some(-1.0);
        h.run();
        assert!(h.state().tab.issues.is_empty(), "{:?}", h.state().tab.issues);
        assert!(h.query_all_by_label("no problems").next().is_some());
        h.get_by_label("Save").click();
        h.run();
        let s = h.state();
        assert!(!s.tab.dirty());
        assert_eq!(std::fs::read_to_string(&s.tab.path).unwrap(), fixed);
        assert!(s.ws.refs.functions.contains(&"reward".to_string()), "the project knows the new functions");
        // Something that doesn't compile is said with the compiler's line.
        h.state_mut().tab.text = "fn broken() {\n    let x = ;\n}\n".into();
        h.state_mut().tab.changed_at = Some(-1.0);
        h.run();
        assert!(h.state().tab.issues.iter().any(|i| i.line == 2 && !i.warning), "{:?}", h.state().tab.issues);
        let dir = h.state().dir.clone();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_file_changed_outside_is_taken_or_asked_about() {
        let mut h = script("outside", "fn a() {\n}\n");
        h.run();
        // Nothing typed here: the file's new text is taken.
        std::fs::write(&h.state().tab.path, "fn a() {\n}\n\nfn b() {\n}\n").unwrap();
        h.state_mut().tab.looked = -10.0;
        h.run();
        assert!(h.state().tab.text.contains("fn b()") && !h.state().tab.dirty());
        // Typed here and changed there: asked, and what is kept is this text.
        h.state_mut().tab.text.push_str("// mine\n");
        std::fs::write(&h.state().tab.path, "fn c() {\n}\n").unwrap();
        h.state_mut().tab.looked = -10.0;
        h.run();
        assert!(h.query_all_by_label("The file changed outside this tab.").next().is_some());
        h.get_by_label("Keep mine").click();
        h.run();
        assert!(h.state().tab.text.ends_with("// mine\n") && h.state().tab.dirty());
        assert!(h.query_all_by_label("The file changed outside this tab.").next().is_none());
        let dir = h.state().dir.clone();
        let _ = std::fs::remove_dir_all(dir);
    }
}
