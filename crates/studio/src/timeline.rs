//! A cut-scene's timeline, in its tab: a row per track with its cues as boxes on a time axis.
//! A click on the axis (or a drag along it) moves the playhead; the map's tab, when it is in
//! view beside this one, shows where everyone is at that moment and what the camera frames. A
//! cue is picked with a click, moved in time by dragging, and changed below with its action's
//! fields. *Play from here* plays the cut-scene in the Play tab from the playhead.
//!
//! What is where at a moment is `editor_core::cutscene`'s; this is the widget.

use eframe::egui::{self, Color32};
use editor_core::cutscene::{cue_label, cue_width, lanes, offered};
use editor_core::schema::{self, Shape};
use rustpg2d_engine::cutscenes::{Cue, CutsceneDef, CutsceneTrack, TrackKind};
use rustpg2d_engine::events::Action;

use crate::form::{self, Cx};

/// Pixels per second of the time axis at the start.
const PPS: f32 = 60.0;

/// What a timeline asks of the window.
pub enum Ask {
    /// Play the cut-scene from this second.
    Play(f32),
    /// Bring its map's tab forward.
    ShowMap,
}

pub struct Timeline {
    /// The playhead (seconds).
    pub time: f32,
    /// The cue picked: track, cue.
    pub sel: Option<(usize, usize)>,
    /// Pixels per second.
    pps: f32,
    /// The *Add a track* row's choice.
    new_kind: TrackKind,
    new_target: String,
    /// A cue being dragged in time: track, cue, its time at the press, the pointer's x then.
    drag: Option<(usize, usize, f32, f32)>,
}

impl Default for Timeline {
    fn default() -> Timeline {
        Timeline { time: 0.0, sel: None, pps: PPS, new_kind: TrackKind::Actor, new_target: "player".into(), drag: None }
    }
}

/// What an action looks like to the forms (a cue's action is changed with its fields).
fn action_node() -> Option<schema::Node<'static>> {
    let Shape::Struct(fields) = schema::trigger().shape() else { return None };
    match fields.iter().find(|f| f.key == "actions")?.node.shape() {
        Shape::List(inner) => Some(inner),
        _ => None,
    }
}

impl Timeline {
    /// `@seconds/length Nt Nc [sel=T:C]` for the script's `status`.
    pub fn status(&self, d: &CutsceneDef) -> String {
        let cues: usize = d.tracks.iter().map(|t| t.cues.len()).sum();
        format!("@{:.1}/{:.1} {}t {cues}c{}", self.time, d.length(), d.tracks.len(), self.sel.map_or(String::new(), |(t, c)| format!(" sel={t}:{c}")))
    }

    /// A new cue of the action called `label` on track `track`, at the playhead. False when
    /// the track offers no such action.
    pub fn add_cue(&mut self, d: &mut CutsceneDef, track: usize, label: &str) -> bool {
        let Some(tr) = d.tracks.get_mut(track) else { return false };
        let Some(action) = offered(tr.kind, &tr.target).into_iter().find(|a| a.label().eq_ignore_ascii_case(label)) else { return false };
        let wait = tr.kind.waits_by_default(&action);
        tr.cues.push(Cue { at: (self.time * 10.0).round() / 10.0, wait, action });
        self.sel = Some((track, tr.cues.len() - 1));
        true
    }

    /// The timeline. `targets`: the names a track's character is picked from. Returns whether
    /// the cut-scene changed, and what is asked of the window.
    pub fn show(&mut self, ui: &mut egui::Ui, cx: &mut Cx, d: &mut CutsceneDef, targets: &[String]) -> (bool, Option<Ask>) {
        let (mut changed, mut ask) = (false, None);
        let len = d.length().max(5.0) + 3.0;
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new("Timeline").strong());
            if ui.button("Play from here").on_hover_text("Plays the cut-scene in the Play tab from the playhead: what came before is done at once (walkers stand where they were going, the camera is there; dialogs and sounds are left out)").clicked() {
                ask = Some(Ask::Play(self.time));
            }
            if ui.button("Show its map").on_hover_text("The map's tab shows where everyone is at the playhead, and the camera's frame, while this tab is in view beside it").clicked() {
                ask = Some(Ask::ShowMap);
            }
            ui.label(format!("Playhead {:.1} s", self.time));
            let zoom = ui.label("Zoom");
            ui.add(egui::Slider::new(&mut self.pps, 15.0..=200.0).show_value(false)).labelled_by(zoom.id);
        });
        ui.weak("A cue that waits holds the scene until it is done (a dialog until it is closed, a walk until the walker arrives): the cues after it come that much later in the game. One that doesn't wait runs alongside.");

        // The time axis with the playhead, then a row per track.
        let label_w = 170.0;
        let width = label_w + len * self.pps + 20.0;
        let (mut remove_track, mut add_cue): (Option<usize>, Option<(usize, Action)>) = (None, None);
        let pps = self.pps;
        egui::ScrollArea::horizontal().id_salt("timeline rows").show(ui, |ui| {
            let (axis, resp) = ui.allocate_exact_size(egui::vec2(width, 22.0), egui::Sense::click_and_drag());
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Slider, true, "The time axis"));
            let painter = ui.painter_at(axis);
            let step = if pps >= 40.0 { 1.0 } else { 5.0 };
            let mut s = 0.0;
            while s <= len {
                let x = axis.left() + label_w + s * pps;
                painter.line_segment([egui::pos2(x, axis.bottom() - 6.0), egui::pos2(x, axis.bottom())], egui::Stroke::new(1.0_f32, Color32::GRAY));
                if (s as i32) % if step >= 5.0 { 5 } else { 1 } == 0 {
                    painter.text(egui::pos2(x + 2.0, axis.top()), egui::Align2::LEFT_TOP, format!("{s:.0}s"), egui::FontId::proportional(11.0), Color32::GRAY);
                }
                s += step;
            }
            if let Some(p) = resp.interact_pointer_pos() {
                if resp.clicked() || resp.dragged() {
                    self.time = ((p.x - axis.left() - label_w) / pps).clamp(0.0, len);
                }
            }
            let mut rows = Vec::new();
            for (ti, tr) in d.tracks.iter_mut().enumerate() {
                // A row: the track's name in a column of fixed width, then its strip (so every
                // strip starts where the axis does).
                let (lane, n) = lanes(&tr.cues, pps);
                let (row, _) = ui.allocate_exact_size(egui::vec2(width, 6.0 + 22.0 * n as f32), egui::Sense::hover());
                let label_rect = egui::Rect::from_min_size(row.min, egui::vec2(label_w - 6.0, 28.0));
                let strip = egui::Rect::from_min_max(egui::pos2(row.left() + label_w, row.top() + 1.0), egui::pos2(row.right(), row.bottom() - 1.0));
                let mut lab = ui.new_child(egui::UiBuilder::new().id_salt(("track", ti)).max_rect(label_rect).layout(egui::Layout::left_to_right(egui::Align::Center)));
                lab.set_clip_rect(label_rect.intersect(ui.clip_rect()));
                if lab.small_button("x").on_hover_text("Remove this track and its cues").clicked() {
                    remove_track = Some(ti);
                }
                let name = if tr.kind == TrackKind::Actor { if tr.target.is_empty() { "?".to_string() } else { tr.target.clone() } } else { tr.kind.label().to_string() };
                lab.menu_button(name, |ui| {
                    if tr.kind == TrackKind::Actor {
                        ui.horizontal(|ui| {
                            let l = ui.label("Character");
                            changed |= ui.add(egui::TextEdit::singleline(&mut tr.target).desired_width(120.0)).labelled_by(l.id).changed();
                            ui.menu_button("…", |ui| {
                                egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                                    for t in targets.iter().filter(|t| *t != "self") {
                                        if ui.button(t.as_str()).clicked() {
                                            tr.target = t.clone();
                                            changed = true;
                                            ui.close();
                                        }
                                    }
                                });
                            });
                        });
                        ui.separator();
                    }
                    ui.label(format!("Add a cue at {:.1} s:", self.time));
                    for a in offered(tr.kind, &tr.target) {
                        if ui.button(a.label()).clicked() {
                            add_cue = Some((ti, a));
                            ui.close();
                        }
                    }
                })
                .response
                .on_hover_text("The track's cues are added from here, at the playhead");
                let sresp = ui.interact(strip, ui.scope_id().with(("strip", ti)), egui::Sense::click());
                sresp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, format!("Track {}", ti + 1)));
                rows.push((ti, strip, sresp, lane));
            }
            // The cues of every row (after the rows, so drags work across them).
            let mut press = None;
            for (ti, strip, sresp, lane) in &rows {
                let painter = ui.painter_at(*strip);
                painter.rect_filled(*strip, 3.0, Color32::from_gray(30));
                let tr = &d.tracks[*ti];
                for (ci, c) in tr.cues.iter().enumerate() {
                    let x = strip.left() + c.at * pps;
                    let r = egui::Rect::from_min_size(egui::pos2(x, strip.top() + 2.0 + 22.0 * lane[ci] as f32), egui::vec2(cue_width(pps), 20.0));
                    let picked = self.sel == Some((*ti, ci));
                    let fill = if c.wait { Color32::from_rgb(70, 90, 140) } else { Color32::from_rgb(60, 110, 80) };
                    painter.rect_filled(r, 3.0, if picked { Color32::from_rgb(180, 140, 50) } else { fill });
                    let label = cue_label(&c.action);
                    ui.painter_at(r.intersect(*strip)).text(r.left_center() + egui::vec2(3.0, 0.0), egui::Align2::LEFT_CENTER, label.as_str(), egui::FontId::proportional(11.0), Color32::WHITE);
                    let cr = ui.interact(r, ui.scope_id().with(("cue", *ti, ci)), egui::Sense::click_and_drag());
                    cr.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, format!("Cue {label} at {:.1} s", c.at)));
                    if cr.clicked() || cr.drag_started() {
                        press = Some((*ti, ci, c.at, cr.interact_pointer_pos().map_or(0.0, |p| p.x)));
                    }
                    cr.on_hover_text(format!("{} at {:.2} s{}", c.action.label(), c.at, if c.wait { ", waits" } else { ", alongside" }));
                }
                if sresp.clicked() {
                    if let Some(p) = sresp.interact_pointer_pos() {
                        self.time = ((p.x - strip.left()) / pps).clamp(0.0, len);
                    }
                }
                // The playhead.
                let x = strip.left() + self.time * pps;
                painter.line_segment([egui::pos2(x, strip.top()), egui::pos2(x, strip.bottom())], egui::Stroke::new(1.5_f32, Color32::from_rgb(255, 90, 90)));
            }
            if let Some((t, c, at, x)) = press {
                self.sel = Some((t, c));
                self.drag = Some((t, c, at, x));
            }
            // A cue dragged moves in time (to a tenth of a second).
            if let Some((t, c, at0, x0)) = self.drag {
                if ui.input(|i| i.pointer.primary_down()) {
                    if let Some(p) = ui.input(|i| i.pointer.interact_pos()) {
                        let at = (((at0 + (p.x - x0) / pps) * 10.0).round() / 10.0).max(0.0);
                        if let Some(cue) = d.tracks.get_mut(t).and_then(|tr| tr.cues.get_mut(c)) {
                            if (cue.at - at).abs() > 1e-4 {
                                cue.at = at;
                                changed = true;
                            }
                        }
                    }
                } else {
                    self.drag = None;
                }
            }
        });
        if let Some(t) = remove_track {
            d.tracks.remove(t);
            self.sel = None;
            changed = true;
        }
        if let Some((t, a)) = add_cue {
            let wait = d.tracks[t].kind.waits_by_default(&a);
            d.tracks[t].cues.push(Cue { at: (self.time * 10.0).round() / 10.0, wait, action: a });
            self.sel = Some((t, d.tracks[t].cues.len() - 1));
            changed = true;
        }
        ui.horizontal(|ui| {
            let l = ui.label("Add a track");
            egui::ComboBox::from_id_salt("timeline track kind").selected_text(self.new_kind.label()).show_ui(ui, |ui| {
                for k in TrackKind::ALL {
                    ui.selectable_value(&mut self.new_kind, k, k.label());
                }
            })
            .response
            .labelled_by(l.id);
            if self.new_kind == TrackKind::Actor {
                ui.add(egui::TextEdit::singleline(&mut self.new_target).hint_text("who").desired_width(120.0)).widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, "The new track's character"));
            }
            if ui.button("+ Track").clicked() {
                let target = if self.new_kind == TrackKind::Actor { self.new_target.trim().to_string() } else { String::new() };
                d.tracks.push(CutsceneTrack { kind: self.new_kind, target, cues: Vec::new() });
                changed = true;
            }
        });
        ui.separator();

        // The cue picked: when it comes, whether it waits, its action's fields.
        let mut delete = false;
        match self.sel.and_then(|(t, c)| d.tracks.get_mut(t).and_then(|tr| tr.cues.get_mut(c))) {
            Some(cue) => {
                let before = cue.clone();
                ui.horizontal(|ui| {
                    ui.strong(cue.action.label());
                    let l = ui.label("at");
                    ui.add(egui::DragValue::new(&mut cue.at).range(0.0..=3600.0).speed(0.05).suffix(" s")).labelled_by(l.id);
                    ui.checkbox(&mut cue.wait, "Waits").on_hover_text("Holds the scene until it is done; off: it runs alongside");
                    if ui.button("Delete the cue").clicked() {
                        delete = true;
                    }
                });
                if let (Some(node), Ok(mut v)) = (action_node(), serde_json::to_value(&cue.action)) {
                    ui.push_id(("cue fields", self.sel), |ui| {
                        if form::body(ui, cx, node, "Cue.action", &mut v, 1) {
                            if let Ok(a) = serde_json::from_value(v) {
                                cue.action = a;
                            }
                        }
                    });
                }
                changed |= *cue != before;
            }
            None => {
                self.sel = None;
                ui.weak("Click a cue to change it; a track's name adds cues (they go in at the playhead).");
            }
        }
        if delete {
            if let Some((t, c)) = self.sel.take() {
                d.tracks[t].cues.remove(c);
                changed = true;
            }
        }
        (changed, ask)
    }
}

#[cfg(test)]
mod ui_tests {
    use super::*;
    use egui_kittest::kittest::Queryable;
    use egui_kittest::Harness;

    struct State {
        tl: Timeline,
        def: CutsceneDef,
        asked: Vec<f32>,
    }

    #[test]
    fn cues_are_picked_changed_added_and_removed_on_the_timeline() {
        let def = CutsceneDef {
            id: "intro".into(),
            tracks: vec![CutsceneTrack { kind: TrackKind::Actor, target: "player".into(), cues: vec![Cue { at: 1.0, wait: true, action: Action::WalkTo { target: "player".into(), x: 5, y: 30 } }] }],
            ..Default::default()
        };
        let targets = vec!["player".to_string(), "guard".to_string()];
        let mut h = Harness::builder().with_size(egui::vec2(1100.0, 700.0)).build_ui_state(
            move |ui, s: &mut State| {
                let mut cx = Cx { lists: &[], maps: &[], show_all: false, compact: false, pictures: None, refs: None, asks: Vec::new() };
                if let (_, Some(Ask::Play(at))) = s.tl.show(ui, &mut cx, &mut s.def, &targets) {
                    s.asked.push(at);
                }
            },
            State { tl: Timeline::default(), def, asked: Vec::new() },
        );
        h.run();
        // A cue is a box that says what it does and when; picked, its fields are under the rows.
        assert!(h.query_all_by_label("Waits").next().is_none());
        h.get_by_label("Cue walk 5,30 at 1.0 s").click();
        h.run();
        assert_eq!(h.state().tl.sel, Some((0, 0)));
        h.get_by_label("Waits").click();
        h.run();
        assert!(!h.state().def.tracks[0].cues[0].wait);
        // A second track, for the guard; a cue for him at the playhead from the track's menu.
        let who = h.get_by_role_and_label(egui::accesskit::Role::TextInput, "The new track's character");
        who.focus();
        who.type_text("guard");
        h.run();
        h.state_mut().tl.new_target = "guard".into();
        h.get_by_label("+ Track").click();
        h.run();
        assert_eq!(h.state().def.tracks.len(), 2);
        assert_eq!(h.state().def.tracks[1].target, "guard");
        h.state_mut().tl.time = 2.5;
        h.get_by_role_and_label(egui::accesskit::Role::Button, "guard").click();
        h.run();
        h.get_by_label("Face").click();
        h.run();
        let cue = &h.state().def.tracks[1].cues[0];
        assert!((cue.at - 2.5).abs() < 1e-4 && matches!(&cue.action, Action::Face { target, .. } if target == "guard"), "{cue:?}");
        assert_eq!(h.state().tl.sel, Some((1, 0)), "the new cue is the one picked");
        // It is taken away again; the scene is played from the playhead.
        h.get_by_label("Delete the cue").click();
        h.run();
        assert!(h.state().def.tracks[1].cues.is_empty() && h.state().tl.sel.is_none());
        h.get_by_label("Play from here").click();
        h.run();
        assert_eq!(h.state().asked, [2.5]);
    }
}
