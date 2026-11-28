//! A guided tour while it is followed: a small window with its steps (done, to do, the one at
//! hand with how it is done), and a frame around the control to use next. Controls say where
//! they are with [`mark`]; a tour's steps name them (`editor_core::tours`).

use std::collections::HashMap;

use eframe::egui::{self, Color32, RichText};
use editor_core::tours::{TourRun, TOURS};
use editor_core::Workspace;

fn marks_id() -> egui::Id {
    egui::Id::unique("tour marks")
}

/// A control a tour may point at says where it is this frame (`tool:Terrain`, `menu:Play`…).
/// One that is scrolled out of view says nothing: there is nothing to point at.
pub fn mark(ui: &egui::Ui, name: &str, rect: egui::Rect) {
    if !ui.is_rect_visible(rect) {
        return;
    }
    let ctx = ui.ctx();
    let frame = ctx.cumulative_frame_nr();
    ctx.data_mut(|d| {
        let marks: &mut HashMap<String, (egui::Rect, u64)> = d.get_temp_mut_or_default(marks_id());
        marks.insert(name.to_string(), (rect, frame));
    });
}

/// Whether a control said where it is in this very frame (a list that is folded away lets
/// what holds it stand in: the group's header for a kind's).
pub fn marked_now(ctx: &egui::Context, name: &str) -> bool {
    let frame = ctx.cumulative_frame_nr();
    ctx.data(|d| d.get_temp::<HashMap<String, (egui::Rect, u64)>>(marks_id())).is_some_and(|m| m.get(name).is_some_and(|(_, f)| *f == frame))
}

/// Where a marked control was in this frame or the one before.
fn marked(ctx: &egui::Context, name: &str) -> Option<egui::Rect> {
    let frame = ctx.cumulative_frame_nr();
    ctx.data(|d| d.get_temp::<HashMap<String, (egui::Rect, u64)>>(marks_id())).and_then(|m| m.get(name).copied()).filter(|(_, f)| frame.saturating_sub(*f) <= 1).map(|(r, _)| r)
}

/// What is shown from here on belongs to this (`thing:` = the Inspector of a thing placed
/// on a map; "" = nothing in particular): its controls say where they are under that name
/// ([`mark_here`]), so a tour can point at the *Add an event* of the thing and not of the map.
pub fn set_scope(ctx: &egui::Context, scope: &'static str) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::unique("tour scope"), scope));
}

/// [`mark`] under the scope set by [`set_scope`] (`add:event` in the scope `thing:` is
/// `thing:add:event`).
pub fn mark_here(ui: &egui::Ui, name: &str, rect: egui::Rect) {
    let scope: &'static str = ui.ctx().data(|d| d.get_temp(egui::Id::unique("tour scope"))).unwrap_or("");
    mark(ui, &format!("{scope}{name}"), rect);
}

/// What the panel asks the shell for.
pub enum Want {
    /// The manual at this chapter's section.
    Help(&'static str, &'static [&'static str]),
    /// This tab in front (its script name: `map:forest`).
    Show(String),
}

/// The first of a step's controls that is in view: its name and where it is.
fn pointed(ctx: &egui::Context, anchors: &[&'static str]) -> Option<(&'static str, egui::Rect)> {
    anchors.iter().find_map(|a| marked(ctx, a).map(|r| (*a, r)))
}

#[derive(Default)]
pub struct TourPanel {
    pub run: Option<TourRun>,
}

impl TourPanel {
    pub fn start(&mut self, ws: &Workspace, tour: usize) {
        self.run = Some(TourRun::start(ws, tour));
    }

    /// The game was started from the editor.
    pub fn played(&mut self) {
        if let Some(run) = &mut self.run {
            run.played = true;
        }
    }

    /// The script's line: `-`, or the tour's (`first-map 2/5 [x x . . .] now=…`) with where the
    /// control it points at is (`at=X,Y`).
    pub fn status(&self, ctx: &egui::Context, ws: &Workspace) -> String {
        let Some(run) = &self.run else { return "-".into() };
        let at = run.current(ws).and_then(|k| pointed(ctx, run.tour().steps[k].anchors)).map(|(a, r)| format!(" at={:.0},{:.0} ({a})", r.center().x, r.center().y)).unwrap_or_default();
        // (The step's tab isn't in view: the window leads there first.)
        let wait = run.tab(ws).filter(|t| marked(ctx, &format!("tab:{t}")).is_none()).map(|t| format!(" wait={t}")).unwrap_or_default();
        format!("{}{at}{wait}", run.status(ws))
    }

    /// The script's words: `start ID`, `skip` (the step at hand), `end`.
    pub fn word(&mut self, ws: &Workspace, args: &[String]) -> Result<(), String> {
        let arg = |i: usize| args.get(i).map(String::as_str).unwrap_or("");
        match arg(0) {
            "" => {}
            "start" => self.start(ws, editor_core::tours::tour_named(arg(1)).ok_or_else(|| format!("tours: {}", TOURS.iter().map(|t| t.id).collect::<Vec<_>>().join(", ")))?),
            "skip" => {
                let run = self.run.as_mut().ok_or("no tour is followed")?;
                if let Some(k) = run.current(ws) {
                    run.skipped.insert(k);
                }
            }
            "end" => self.run = None,
            other => return Err(format!("tour {other}: start, skip, end")),
        }
        Ok(())
    }

    /// The tours to choose from (the Overview's *Learn by doing*). Returns the one clicked.
    pub fn offer(&self, ui: &mut egui::Ui) -> Option<usize> {
        let mut picked = None;
        for (k, t) in TOURS.iter().enumerate() {
            ui.horizontal(|ui| {
                let running = self.run.as_ref().is_some_and(|r| r.tour == k);
                if ui.add_enabled(!running, egui::Button::new(t.title)).on_hover_text("The editor shows each step and points at what to click; nothing is done for you").on_disabled_hover_text("This tour is being followed").clicked() {
                    picked = Some(k);
                }
                ui.weak(t.about);
            });
        }
        picked
    }

    /// The tour's window and the frame around the control to use. Nothing while no tour is followed.
    pub fn show(&mut self, ctx: &egui::Context, ws: &Workspace) -> Option<Want> {
        let run = self.run.as_mut()?;
        let tour = run.tour();
        // (A map's tab is in view: a map was opened where the creator sees it.)
        if ws.doc.as_ref().is_some_and(|d| marked(ctx, &format!("tab:map:{}", d.map.name)).is_some()) {
            run.map_seen = true;
        }
        let done = run.done(ws);
        let current = run.current(ws);
        // The tab the step is done in, when it isn't in view.
        let away = run.tab(ws).filter(|t| marked(ctx, &format!("tab:{t}")).is_none());
        let (mut want, mut end, mut skip, mut next) = (None, false, false, None);
        // (It starts at the bottom, left of where a map's Inspector is, and is moved by its title.)
        let corner = ctx.content_rect().right_bottom() - egui::vec2(372.0, 34.0);
        egui::Window::new(format!("Tour: {}", tour.title)).id(egui::Id::unique("tour window")).collapsible(true).resizable(false).pivot(egui::Align2::RIGHT_BOTTOM).default_pos(corner).show(ctx, |ui| {
            ui.set_width(330.0);
            for (k, step) in tour.steps.iter().enumerate() {
                let state = if done[k] { "[x]" } else if run.skipped.contains(&k) { "[-]" } else { "[ ]" };
                if current == Some(k) {
                    ui.horizontal(|ui| {
                        ui.label(state);
                        ui.label(RichText::new(step.title).strong());
                    });
                    ui.label(step.how);
                    if let Some(tab) = &away {
                        let what = match tab.split_once(':') {
                            Some(("map", name)) => format!("the map {name}"),
                            Some(("def", rest)) => format!("the {}", rest.replacen(':', " ", 1)),
                            _ => tab.clone(),
                        };
                        ui.horizontal_wrapped(|ui| {
                            ui.colored_label(ui.visuals().warn_fg_color, "This is done in another tab.");
                            if ui.button(format!("Show {what}")).clicked() {
                                want = Some(Want::Show(tab.clone()));
                            }
                        });
                    }
                    ui.horizontal(|ui| {
                        if ui.small_button("Read about it").on_hover_text("Opens the manual at this (F1 does that anywhere)").clicked() {
                            want = Some(Want::Help(step.help.0, step.help.1));
                        }
                        if ui.small_button("Skip this step").clicked() {
                            skip = true;
                        }
                    });
                    ui.add_space(4.0);
                } else {
                    ui.horizontal(|ui| {
                        ui.label(state);
                        ui.weak(step.title);
                    });
                }
            }
            ui.separator();
            if current.is_none() {
                ui.label(RichText::new("That was it.").strong());
                ui.horizontal_wrapped(|ui| {
                    for (k, t) in TOURS.iter().enumerate().filter(|(k, _)| *k != run.tour) {
                        if ui.button(format!("Next: {}", t.title)).clicked() {
                            next = Some(k);
                        }
                    }
                });
            }
            if ui.small_button(if current.is_none() { "Close" } else { "End the tour" }).clicked() {
                end = true;
            }
        });
        // A frame around the control the step at hand uses (while it is in view).
        if let Some((_, rect)) = current.and_then(|k| pointed(ctx, tour.steps[k].anchors)) {
            let pulse = ((ctx.input(|i| i.time) * 3.0).sin() * 0.5 + 0.5) as f32;
            let color = Color32::from_rgb(255, 210, 90).gamma_multiply(0.55 + 0.45 * pulse);
            let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("tour pointer")));
            painter.rect_stroke(rect.expand(3.0 + 2.0 * pulse), 5.0, egui::Stroke::new(2.5_f32, color), egui::StrokeKind::Outside);
            ctx.request_repaint_after(std::time::Duration::from_millis(60));
        }
        if skip {
            if let Some(k) = current {
                run.skipped.insert(k);
            }
        }
        if let Some(k) = next {
            self.start(ws, k);
        } else if end {
            self.run = None;
        }
        want
    }
}

#[cfg(test)]
mod ui_tests {
    use super::*;
    use egui_kittest::kittest::Queryable;
    use egui_kittest::Harness;

    struct State {
        ws: Workspace,
        panel: TourPanel,
        asked: Vec<String>,
        map_in_view: bool,
    }

    #[test]
    fn a_tour_shows_its_steps_points_at_the_control_and_ticks_what_is_done() {
        let dir = std::env::temp_dir().join(format!("rustpg2d-tourpanel-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        editor_core::files::create_project(&dir).unwrap();
        let mut ws = Workspace::new(dir.clone(), editor_core::Gpu::No).unwrap();
        ws.close_map("start");
        ws.history_frame(None);
        let mut h = Harness::builder().with_size(egui::vec2(900.0, 700.0)).build_ui_state(
            |ui, s: &mut State| {
                // (A control that says where it is, as the map's tool bar does.)
                let r = ui.button("Terrain");
                mark(ui, "tool:Terrain", r.rect);
                if let Some(picked) = s.panel.offer(ui) {
                    s.panel.start(&s.ws, picked);
                }
                // (And a tab that says it is in view, as the window's tabs do.)
                if s.map_in_view {
                    mark(ui, "tab:map:start", ui.max_rect());
                }
                match s.panel.show(ui.ctx(), &s.ws) {
                    Some(Want::Help(file, _)) => s.asked.push(file.to_string()),
                    Some(Want::Show(tab)) => s.asked.push(tab),
                    None => {}
                }
            },
            State { ws, panel: TourPanel::default(), asked: Vec::new(), map_in_view: true },
        );
        h.run();
        // The three tours are offered; one is begun.
        h.get_by_label("A character who talks");
        h.get_by_label("Your first map").click();
        h.run_steps(3);
        assert!(h.state().panel.run.is_some());
        h.get_by_label("Open a map");
        assert!(h.query_all_by_label_contains("In the Project browser at the left").next().is_some(), "the step at hand says how");
        // Its manual page is a click away.
        h.get_by_label("Read about it").click();
        h.run_steps(3);
        assert_eq!(h.state().asked, ["03-maps.md"]);
        // The map opened: the step is ticked, the next is at hand, and its control is pointed at.
        h.state_mut().ws.open_map("start");
        h.run_steps(3);
        assert!(h.query_all_by_label_contains("Click Terrain in the bar above the map").next().is_some());
        let s = h.state();
        let status = s.panel.status(&h.ctx, &s.ws);
        assert!(status.starts_with("first-map 1/5 [x . . . .] now=Paint some ground at=") && status.ends_with("(tool:Terrain)"), "{status}");
        // The map's tab out of view: the window says so and leads back to it.
        h.state_mut().map_in_view = false;
        h.run_steps(3);
        h.get_by_label("This is done in another tab.");
        assert!(h.state().panel.status(&h.ctx, &h.state().ws).ends_with("wait=map:start"));
        h.get_by_label("Show the map start").click();
        h.run_steps(3);
        assert_eq!(h.state().asked.last().map(String::as_str), Some("map:start"));
        h.state_mut().map_in_view = true;
        h.run_steps(3);
        assert!(h.query_all_by_label("This is done in another tab.").next().is_none());
        // Skipped steps are marked, the tour goes on; ended, the window is gone.
        h.get_by_label("Skip this step").click();
        h.run_steps(3);
        assert!(h.state().panel.status(&h.ctx, &h.state().ws).contains("[x - . . .] now=Put something on the map"));
        h.get_by_label("End the tour").click();
        h.run_steps(3);
        assert!(h.state().panel.run.is_none() && h.query_all_by_label("End the tour").next().is_none());
        let _ = std::fs::remove_dir_all(dir);
    }
}
