//! The cut-scene timeline (`defs/cutscenes.ron`): a window over the map view with a row per
//! track and the cues as boxes on a time axis. Dragging the playhead (or a click on the axis)
//! scrubs: the map shows where each actor is at that moment and the camera's frame, worked out
//! from the cues (walks at the walker's speed, pans over their seconds; no animation). A cue is
//! selected with a click, moved in time by dragging, and edited below with the usual action
//! fields. *Play from here* play-tests the cut-scene from the playhead.
//!
//! Script commands: `timeline ID` (open), `timelinetime SECS`, `timelinecue TRACK CUE`,
//! `timelineplay`; `status` prints `timeline=`.

use egui_macroquad::egui::{self, Color32, Ui};
use macroquad::prelude::*;
use rustpg2d_engine::camera::CameraAim;
use rustpg2d_engine::cutscenes::{Cue, CutsceneDef, CutsceneTrack, TrackKind};
use rustpg2d_engine::events::Action;
use rustpg2d_engine::map::Map;

use crate::app::Editor;
use crate::ui::{self as eui, ph};

/// Pixels per second of the time axis at the start.
const PPS: f32 = 60.0;
/// The game's window at view scale 1 (`release` runner): the camera frame's size.
const GAME_VIEW: (f32, f32) = (1280.0, 720.0);
/// Walking speed when the walker's definition is unknown (world px per second).
const WALK: f32 = 120.0;

/// The open timeline.
pub struct Timeline {
    pub id: String,
    /// The playhead (seconds).
    pub time: f32,
    /// Selected cue: track, cue.
    pub sel: Option<(usize, usize)>,
    /// Pixels per second.
    pub pps: f32,
    /// The *Add track* row's choice.
    new_kind: TrackKind,
    new_target: String,
    /// A cue being dragged in time: track, cue, its time at the press, the pointer's x then.
    drag: Option<(usize, usize, f32, f32)>,
}

impl Timeline {
    pub fn new(id: &str) -> Self {
        Timeline { id: id.to_string(), time: 0.0, sel: None, pps: PPS, new_kind: TrackKind::Actor, new_target: "player".into(), drag: None }
    }
}

thread_local! {
    /// The Cut-scenes form's *Edit timeline* button: open this cut-scene's timeline.
    static OPEN: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// Asks the editor to open the timeline of cut-scene `id` (on its map).
pub fn request_open(id: &str) {
    OPEN.with(|o| *o.borrow_mut() = Some(id.to_string()));
}

/// Where things are at a moment of a cut-scene, as the editor previews it.
#[derive(Debug, Default, PartialEq)]
pub struct Preview {
    /// Each actor track's character: its name, where it stands (tile, fractions while walking),
    /// whether it is walking.
    pub actors: Vec<(String, Vec2, bool)>,
    /// The camera's middle (tile) and zoom; None = on the player as usual.
    pub camera: Option<(Vec2, f32)>,
}

/// The tile `target` starts on: a character of that name on the map (the player: the map's
/// `start` spawn, else none until a cue places them).
fn start_of(map: &Map, target: &str) -> Option<Vec2> {
    if target == "player" {
        return map.zones.iter().find(|z| z.name == "start").map(|z| vec2(z.x as f32, z.y as f32));
    }
    let ts = rustpg2d_engine::project::TILE_SIZE as f32;
    map.entities.iter().find(|e| e.name == target).map(|e| vec2((e.x / ts).floor(), ((e.y - 1.0) / ts).floor()))
}

/// Where a cut-scene has everyone at `t` seconds. The cues are taken at their times (a dialog
/// that holds the scene in the game takes no time here): walks go at `speed(target)` world
/// pixels per second, pans over their seconds.
pub fn preview_at(d: &CutsceneDef, map: &Map, t: f32, speed: &dyn Fn(&str) -> f32) -> Preview {
    let ts = rustpg2d_engine::project::TILE_SIZE as f32;
    let mut out = Preview::default();
    let pos_of = |name: &str, p: &Preview| p.actors.iter().find(|a| a.0 == name).map(|a| a.1);
    for (_, c) in d.cues().into_iter().filter(|(_, c)| c.at <= t + 1e-4) {
        let since = t - c.at;
        match &c.action {
            Action::Place { target, x, y } => set(&mut out.actors, target, vec2(*x as f32, *y as f32), false),
            Action::WalkTo { target, x, y } => {
                let to = vec2(*x as f32, *y as f32);
                match pos_of(target, &out).or_else(|| start_of(map, target)) {
                    Some(from) => {
                        let secs = from.distance(to) * ts / speed(target).max(1.0);
                        let k = if secs <= 0.0 { 1.0 } else { (since / secs).min(1.0) };
                        set(&mut out.actors, target, from.lerp(to, k), k < 1.0);
                    }
                    None => set(&mut out.actors, target, to, false),
                }
            }
            Action::Approach { target, to } => {
                if let (Some(a), Some(b)) = (pos_of(target, &out).or_else(|| start_of(map, target)), pos_of(to, &out).or_else(|| start_of(map, to))) {
                    let off = if (a - b).x.abs() >= (a - b).y.abs() { vec2((a - b).x.signum(), 0.0) } else { vec2(0.0, (a - b).y.signum()) };
                    let dest = b + if off == Vec2::ZERO { vec2(0.0, 1.0) } else { off };
                    let secs = a.distance(dest) * ts / speed(target).max(1.0);
                    let k = if secs <= 0.0 { 1.0 } else { (since / secs).min(1.0) };
                    set(&mut out.actors, target, a.lerp(dest, k), k < 1.0);
                }
            }
            Action::Camera { aim, seconds, zoom, .. } => {
                let to = match aim {
                    CameraAim::Tile { x, y } => Some(vec2(*x as f32, *y as f32)),
                    CameraAim::Player => pos_of("player", &out).or_else(|| start_of(map, "player")),
                    CameraAim::Follow { target } => pos_of(target, &out).or_else(|| start_of(map, target)),
                };
                if let Some(to) = to {
                    let from = out.camera.map(|c| c.0).or_else(|| pos_of("player", &out)).or_else(|| start_of(map, "player")).unwrap_or(to);
                    let k = if *seconds <= 0.0 { 1.0 } else { (since / seconds).min(1.0) };
                    let z0 = out.camera.map_or(1.0, |c| c.1);
                    out.camera = Some((from.lerp(to, k), z0 + (zoom.max(0.1) - z0) * k));
                }
            }
            _ => {}
        }
    }
    // Actor tracks whose character hasn't moved yet stand where the map has them.
    for tr in d.tracks.iter().filter(|t| t.kind == TrackKind::Actor && !t.target.is_empty()) {
        if pos_of(&tr.target, &out).is_none() {
            if let Some(p) = start_of(map, &tr.target) {
                set(&mut out.actors, &tr.target, p, false);
            }
        }
    }
    out
}

fn set(actors: &mut Vec<(String, Vec2, bool)>, name: &str, p: Vec2, walking: bool) {
    match actors.iter_mut().find(|a| a.0 == name) {
        Some(a) => (a.1, a.2) = (p, walking),
        None => actors.push((name.to_string(), p, walking)),
    }
}

/// A short line for a cue's box.
fn cue_label(a: &Action) -> String {
    let short = |s: &str| if s.chars().count() > 18 { format!("{}…", s.chars().take(17).collect::<String>()) } else { s.to_string() };
    match a {
        Action::WalkTo { x, y, .. } => format!("walk {x},{y}"),
        Action::Place { x, y, .. } => format!("at {x},{y}"),
        Action::Approach { to, .. } => format!("to {to}"),
        Action::Face { facing, .. } => format!("face {facing:?}").to_lowercase(),
        Action::Emote { emotion, .. } => emotion.clone(),
        Action::Dialog { speaker, text } => short(&if speaker.is_empty() { text.clone() } else { format!("{speaker}: {text}") }),
        Action::Camera { aim, .. } => match aim {
            CameraAim::Player => "camera: player".into(),
            CameraAim::Tile { x, y } => format!("camera {x},{y}"),
            CameraAim::Follow { target } => format!("follow {target}"),
        },
        Action::PlaySound { sound, .. } | Action::PlayMusic { sound, .. } | Action::PlayMusicEffect { sound, .. } => sound.clone(),
        Action::ShowPicture { layer, sheet, .. } => format!("{layer}: {sheet}"),
        Action::MovePicture { layer, .. } => format!("move {layer}"),
        Action::ErasePicture { layer } => format!("erase {layer}"),
        Action::Wait { seconds } => format!("wait {seconds:.1}"),
        other => other.label().to_lowercase(),
    }
}

/// The actions a track of `kind` offers for new cues (with the actor filled in).
fn offered(kind: TrackKind, target: &str) -> Vec<Action> {
    Action::templates()
        .into_iter()
        .filter(|a| kind.fits(a))
        .map(|mut a| {
            if kind == TrackKind::Actor {
                if let Action::WalkTo { target: t, .. } | Action::Place { target: t, .. } | Action::Face { target: t, .. } | Action::Emote { target: t, .. } | Action::Bark { target: t, .. } | Action::MoveRoute { target: t, .. } | Action::Approach { target: t, .. } | Action::Mood { target: t, .. } = &mut a {
                    *t = target.to_string();
                }
            }
            a
        })
        .collect()
}

impl Editor {
    /// Opens the timeline of cut-scene `id`: its map in the map view, the window over it.
    pub fn open_timeline(&mut self, id: &str) -> bool {
        let Some(d) = self.project.db.cutscene(id) else { return false };
        let map = if d.map.is_empty() { self.project.config.start_map.clone() } else { d.map.clone() };
        self.mode = crate::app::Mode::Map;
        if self.doc.as_ref().is_none_or(|doc| doc.map.name != map) && self.maps.contains(&map) {
            self.open_map(&map);
        }
        self.timeline = Some(Timeline::new(id));
        true
    }

    /// The Cut-scenes form asked to open a timeline (once a frame).
    pub fn take_timeline_request(&mut self) {
        if let Some(id) = OPEN.with(|o| o.borrow_mut().take()) {
            self.open_timeline(&id);
        }
    }

    /// Play-tests the open cut-scene from the playhead.
    pub fn timeline_play(&mut self) {
        let Some(tl) = &self.timeline else { return };
        let (id, t) = (tl.id.clone(), tl.time);
        self.play_cutscene_at(&id, t);
    }

    /// `timeline=` for `status`: id@seconds/length tracks cues [selected track:cue] or `-`.
    pub fn timeline_status(&self) -> String {
        let Some(tl) = &self.timeline else { return "-".into() };
        let Some(d) = self.project.db.cutscene(&tl.id) else { return format!("{}:missing", tl.id) };
        let cues: usize = d.tracks.iter().map(|t| t.cues.len()).sum();
        let sel = tl.sel.map_or(String::new(), |(t, c)| format!(" sel={t}:{c}"));
        let p = self.doc.as_ref().and_then(|doc| self.timeline_preview(&doc.map));
        let actors: Vec<String> = p.as_ref().map_or(Vec::new(), |p| p.actors.iter().map(|(n, v, w)| format!("{n}@{:.1},{:.1}{}", v.x, v.y, if *w { "~" } else { "" })).collect());
        let cam = p.and_then(|p| p.camera).map_or("-".into(), |(c, z)| format!("{:.1},{:.1}x{z:.1}", c.x, c.y));
        format!("{}@{:.1}/{:.1}:{}t:{cues}c{sel}[{}]cam={cam}", tl.id, tl.time, d.length(), d.tracks.len(), actors.join(" "))
    }

    /// The preview on `map` (the map view takes the document out while it draws).
    fn timeline_preview(&self, map: &Map) -> Option<Preview> {
        let tl = self.timeline.as_ref()?;
        let d = self.project.db.cutscene(&tl.id)?;
        let db = &self.project.db;
        let speed = |name: &str| {
            let def = if name == "player" { self.project.config.player.clone() } else { map.entities.iter().find(|e| e.name == name).map(|e| e.def.clone()).unwrap_or_default() };
            db.character(&def).map_or(WALK, |c| c.speed)
        };
        Some(preview_at(d, map, tl.time, &speed))
    }

    /// The preview on the map (world space, in the map view's camera): each actor at its spot
    /// with its name, a walker's ring dashed, the camera's frame.
    pub fn draw_timeline_preview(&self, map: &Map, px: f32) {
        let Some(p) = self.timeline_preview(map) else { return };
        let ts = rustpg2d_engine::project::TILE_SIZE as f32;
        let col = Color::new(1.0, 0.55, 0.85, 1.0);
        for (name, t, walking) in &p.actors {
            let c = vec2((t.x + 0.5) * ts, (t.y + 0.5) * ts);
            draw_circle(c.x, c.y, 16.0 * px, Color::new(0.3, 0.05, 0.25, 0.55));
            draw_circle_lines(c.x, c.y, 16.0 * px, if *walking { 2.0 } else { 3.0 } * px, col);
            let fs = (14.0 * px).max(8.0);
            let w = measure_text(name, None, fs as u16, 1.0).width;
            draw_text(name, c.x - w / 2.0, c.y - 14.0 * px, fs, WHITE);
        }
        if let Some((c, zoom)) = p.camera {
            let scale = self.project.config.scale.max(0.5) * zoom.max(0.1);
            let (w, h) = (GAME_VIEW.0 / scale, GAME_VIEW.1 / scale);
            let mid = vec2((c.x + 0.5) * ts, (c.y + 0.5) * ts);
            draw_rectangle_lines(mid.x - w / 2.0, mid.y - h / 2.0, w, h, 2.0 * px, Color::new(0.4, 0.85, 1.0, 0.9));
        }
    }

    /// The timeline window (while one is open).
    pub fn timeline_window(&mut self, ctx: &egui::Context) {
        self.take_timeline_request();
        let Some(mut tl) = self.timeline.take() else { return };
        let Some(pos) = self.project.db.cutscenes.iter().position(|d| d.id == tl.id) else { return };
        let mut open = true;
        let mut play = false;
        let mut changed = false;
        let maps = self.maps.clone();
        let refs = &self.refs;
        let d = &mut self.project.db.cutscenes[pos];
        let title = format!("Cut-scene: {}", if d.name.is_empty() { &d.id } else { &d.name });
        egui::Window::new(title).id(egui::Id::new("timeline")).open(&mut open).default_width(760.0).default_height(420.0).anchor(egui::Align2::CENTER_BOTTOM, [0.0, -40.0]).show(ctx, |ui| {
            changed |= timeline_ui(ui, &mut tl, d, &maps, refs, &mut play);
        });
        if changed {
            self.defs_dirty = true;
        }
        if open {
            self.timeline = Some(tl);
        }
        if play {
            self.timeline_play();
        }
    }
}

/// The width of a cue's block on the timeline (pixels).
fn cue_width(pps: f32) -> f32 {
    96.0_f32.min(pps * 2.0).max(34.0)
}

/// Each cue's lane within its track, so blocks that would overlap stack instead (cues close
/// together, like dialog lines that wait for each other), and the number of lanes.
fn lanes(cues: &[Cue], pps: f32) -> (Vec<usize>, usize) {
    let w = cue_width(pps);
    let mut order: Vec<usize> = (0..cues.len()).collect();
    order.sort_by(|a, b| cues[*a].at.total_cmp(&cues[*b].at).then(a.cmp(b)));
    let mut ends: Vec<f32> = Vec::new();
    let mut lane = vec![0; cues.len()];
    for k in order {
        let x = cues[k].at * pps;
        let l = ends.iter().position(|e| *e <= x + 0.5).unwrap_or(ends.len());
        if l == ends.len() {
            ends.push(0.0);
        }
        ends[l] = x + w + 2.0;
        lane[k] = l;
    }
    (lane, ends.len().max(1))
}

/// The window's contents; true when the cut-scene changed.
fn timeline_ui(ui: &mut Ui, tl: &mut Timeline, d: &mut CutsceneDef, maps: &[String], refs: &crate::widgets::Refs, play: &mut bool) -> bool {
    let mut changed = false;
    let len = d.length().max(5.0) + 3.0;
    ui.horizontal(|ui| {
        ui.label("Map");
        egui::ComboBox::from_id_salt("tl_map").selected_text(if d.map.is_empty() { "(the start map)" } else { d.map.as_str() }).show_ui(ui, |ui| {
            for m in maps {
                changed |= ui.selectable_value(&mut d.map, m.clone(), m).changed();
            }
        });
        ui.separator();
        if ui.button(eui::t(ph::PLAY, "Play from here")).on_hover_text("Play-tests the cut-scene from the playhead: what came before is done at once (walkers stand where they were going, the camera is there; dialogs and sounds are left out).").clicked() {
            *play = true;
        }
        ui.label("Zoom");
        ui.add(egui::Slider::new(&mut tl.pps, 15.0..=200.0).show_value(false));
    });
    ui.weak("A cue that waits holds the scene until it is done (a dialog until it is closed, a walk until the walker arrives): the cues after it come that much later in the game. One that doesn't wait runs alongside.");
    // The time axis with the playhead, then a row per track.
    let label_w = 150.0;
    let width = label_w + len * tl.pps + 20.0;
    let mut remove_track = None;
    let mut add_cue: Option<(usize, Action)> = None;
    egui::ScrollArea::both().id_salt("tl_rows").max_height(300.0).show(ui, |ui| {
        let (axis, resp) = ui.allocate_exact_size(egui::vec2(width, 22.0), egui::Sense::click_and_drag());
        let painter = ui.painter_at(axis);
        let x_of = |t: f32| axis.left() + label_w + t * tl.pps;
        let step = if tl.pps >= 40.0 { 1.0 } else { 5.0 };
        let mut s = 0.0;
        while s <= len {
            let x = x_of(s);
            painter.line_segment([egui::pos2(x, axis.bottom() - 6.0), egui::pos2(x, axis.bottom())], egui::Stroke::new(1.0_f32, Color32::GRAY));
            if (s as i32) % if step >= 5.0 { 5 } else { 1 } == 0 {
                painter.text(egui::pos2(x + 2.0, axis.top()), egui::Align2::LEFT_TOP, format!("{s:.0}s"), egui::FontId::proportional(11.0), Color32::GRAY);
            }
            s += step;
        }
        if let Some(p) = resp.interact_pointer_pos() {
            if resp.clicked() || resp.dragged() {
                tl.time = ((p.x - axis.left() - label_w) / tl.pps).clamp(0.0, len);
            }
        }
        let mut rows = Vec::new();
        for (ti, tr) in d.tracks.iter_mut().enumerate() {
            // A row: the track's name in a column of fixed width, then its strip (so every
            // strip starts at the same x as the axis).
            let (lane, n) = lanes(&tr.cues, tl.pps);
            let (row, _) = ui.allocate_exact_size(egui::vec2(width, 6.0 + 22.0 * n as f32), egui::Sense::hover());
            let label_rect = egui::Rect::from_min_size(row.min, egui::vec2(label_w - 6.0, 28.0));
            let strip = egui::Rect::from_min_max(egui::pos2(row.left() + label_w, row.top() + 1.0), egui::pos2(row.right(), row.bottom() - 1.0));
            let mut lab = ui.new_child(egui::UiBuilder::new().max_rect(label_rect).layout(egui::Layout::left_to_right(egui::Align::Center)));
            lab.set_clip_rect(label_rect.intersect(ui.clip_rect()));
            if lab.small_button(ph::TRASH).on_hover_text("Remove this track and its cues").clicked() {
                remove_track = Some(ti);
            }
            let name = if tr.kind == TrackKind::Actor { format!("{} {}", ph::USER, if tr.target.is_empty() { "?" } else { &tr.target }) } else { tr.kind.label().to_string() };
            lab.menu_button(name, |ui| {
                if tr.kind == TrackKind::Actor {
                    ui.horizontal(|ui| {
                        ui.label("Character");
                        let before = tr.target.clone();
                        crate::widgets::suggest(ui, &format!("tl_target{ti}"), &mut tr.target, &refs.targets, 110.0);
                        changed |= tr.target != before;
                    });
                    ui.separator();
                }
                ui.label(format!("Add a cue at {:.1} s:", tl.time));
                for a in offered(tr.kind, &tr.target) {
                    if ui.button(a.label()).clicked() {
                        add_cue = Some((ti, a));
                        ui.close_menu();
                    }
                }
            });
            let sresp = ui.interact(strip, ui.id().with(("strip", ti)), egui::Sense::click());
            rows.push((ti, strip, sresp, lane));
        }
        // The cues of every row (drawn after the rows so drags work across them).
        let mut press = None;
        for (ti, strip, sresp, lane) in &rows {
            let painter = ui.painter_at(*strip);
            painter.rect_filled(*strip, 3.0, Color32::from_gray(30));
            let tr = &d.tracks[*ti];
            for (ci, c) in tr.cues.iter().enumerate() {
                let x = strip.left() + c.at * tl.pps;
                let r = egui::Rect::from_min_size(egui::pos2(x, strip.top() + 2.0 + 22.0 * lane[ci] as f32), egui::vec2(cue_width(tl.pps), 20.0));
                let selected = tl.sel == Some((*ti, ci));
                let fill = if c.wait { Color32::from_rgb(70, 90, 140) } else { Color32::from_rgb(60, 110, 80) };
                painter.rect_filled(r, 3.0, if selected { Color32::from_rgb(180, 140, 50) } else { fill });
                ui.painter_at(r.intersect(*strip)).text(r.left_center() + egui::vec2(3.0, 0.0), egui::Align2::LEFT_CENTER, cue_label(&c.action), egui::FontId::proportional(11.0), Color32::WHITE);
                let id = ui.id().with(("cue", *ti, ci));
                let cr = ui.interact(r, id, egui::Sense::click_and_drag());
                if cr.clicked() || cr.drag_started() {
                    press = Some((*ti, ci, c.at, cr.interact_pointer_pos().map_or(0.0, |p| p.x)));
                }
                cr.on_hover_text(format!("{} at {:.2} s{}", c.action.label(), c.at, if c.wait { ", waits" } else { ", alongside" }));
            }
            if sresp.clicked() {
                if let Some(p) = sresp.interact_pointer_pos() {
                    tl.time = ((p.x - strip.left()) / tl.pps).clamp(0.0, len);
                }
            }
            // The playhead.
            let x = strip.left() + tl.time * tl.pps;
            painter.line_segment([egui::pos2(x, strip.top()), egui::pos2(x, strip.bottom())], egui::Stroke::new(1.5_f32, Color32::from_rgb(255, 90, 90)));
        }
        if let Some((t, c, at, x)) = press {
            tl.sel = Some((t, c));
            tl.drag = Some((t, c, at, x));
        }
        // Dragging a cue moves it in time (snapped to a tenth of a second).
        if let Some((t, c, at0, x0)) = tl.drag {
            if ui.input(|i| i.pointer.primary_down()) {
                if let Some(p) = ui.input(|i| i.pointer.interact_pos()) {
                    let at = ((at0 + (p.x - x0) / tl.pps) * 10.0).round() / 10.0;
                    if let Some(cue) = d.tracks.get_mut(t).and_then(|tr| tr.cues.get_mut(c)) {
                        if (cue.at - at.max(0.0)).abs() > 1e-4 {
                            cue.at = at.max(0.0);
                            changed = true;
                        }
                    }
                }
            } else {
                tl.drag = None;
            }
        }
    });
    if let Some(t) = remove_track {
        d.tracks.remove(t);
        tl.sel = None;
        changed = true;
    }
    if let Some((t, a)) = add_cue {
        let kind = d.tracks[t].kind;
        let wait = kind.waits_by_default(&a);
        d.tracks[t].cues.push(Cue { at: (tl.time * 10.0).round() / 10.0, wait, action: a });
        tl.sel = Some((t, d.tracks[t].cues.len() - 1));
        changed = true;
    }
    ui.horizontal(|ui| {
        ui.label("Add track");
        egui::ComboBox::from_id_salt("tl_kind").selected_text(tl.new_kind.label()).show_ui(ui, |ui| {
            for k in TrackKind::ALL {
                ui.selectable_value(&mut tl.new_kind, k, k.label());
            }
        });
        if tl.new_kind == TrackKind::Actor {
            crate::widgets::suggest(ui, "tl_new_target", &mut tl.new_target, &refs.targets, 110.0);
        }
        if ui.button(eui::t(ph::PLUS, "Add")).clicked() {
            let target = if tl.new_kind == TrackKind::Actor { tl.new_target.trim().to_string() } else { String::new() };
            d.tracks.push(CutsceneTrack { kind: tl.new_kind, target, cues: Vec::new() });
            changed = true;
        }
        ui.separator();
        ui.label(format!("Playhead {:.1} s", tl.time));
    });
    ui.separator();
    // The selected cue.
    let mut delete = false;
    if let Some((t, c)) = tl.sel {
        if let Some(cue) = d.tracks.get_mut(t).and_then(|tr| tr.cues.get_mut(c)) {
            egui::ScrollArea::vertical().id_salt("tl_cue").max_height(160.0).show(ui, |ui| {
                let before = cue.clone();
                ui.horizontal(|ui| {
                    ui.strong(cue.action.label());
                    ui.label("at");
                    ui.add(egui::DragValue::new(&mut cue.at).range(0.0..=3600.0).speed(0.05).suffix(" s"));
                    ui.checkbox(&mut cue.wait, "Waits").on_hover_text("Holds the scene until it is done; off = it runs alongside.");
                    if ui.button(eui::t(ph::TRASH, "Delete")).clicked() {
                        delete = true;
                    }
                });
                crate::widgets::action_fields(ui, &format!("tl/{t}/{c}"), &mut cue.action, refs);
                changed |= *cue != before;
            });
        } else {
            tl.sel = None;
        }
    } else {
        ui.weak("Click a cue to edit it; add cues with a track's menu (they go in at the playhead).");
    }
    if delete {
        if let Some((t, c)) = tl.sel.take() {
            d.tracks[t].cues.remove(c);
            changed = true;
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cues_close_together_stack_in_lanes() {
        let at = |t: f32| Cue { at: t, wait: true, action: Action::Wait { seconds: 0.0 } };
        // 50 px a second, blocks of 96 px: 7.5 / 7.6 / 7.7 overlap, 10.0 is clear of the first.
        let (lane, n) = lanes(&[at(7.6), at(7.5), at(10.0), at(7.7)], 50.0);
        assert_eq!((lane, n), (vec![1, 0, 0, 2], 3));
        assert_eq!(lanes(&[], 50.0).1, 1);
    }

    fn map() -> Map {
        let mut m = Map::new("village", 40, 30);
        let mut guard = rustpg2d_engine::entity::EntityInstance::new(1, rustpg2d_engine::entity::EntityKind::Character, "guard", 10.0 * 32.0 + 16.0, 5.0 * 32.0 + 31.0);
        guard.name = "guard".into();
        m.entities.push(guard);
        m
    }

    #[test]
    fn the_preview_walks_actors_and_pans_the_camera() {
        let d = CutsceneDef {
            id: "intro".into(),
            tracks: vec![
                CutsceneTrack { kind: TrackKind::Actor, target: "player".into(), cues: vec![Cue { at: 0.0, wait: true, action: Action::Place { target: "player".into(), x: 0, y: 5 } }, Cue { at: 1.0, wait: true, action: Action::WalkTo { target: "player".into(), x: 4, y: 5 } }] },
                CutsceneTrack { kind: TrackKind::Actor, target: "guard".into(), cues: vec![] },
                CutsceneTrack { kind: TrackKind::Camera, target: String::new(), cues: vec![Cue { at: 0.0, wait: false, action: Action::Camera { aim: CameraAim::Tile { x: 20, y: 5 }, seconds: 2.0, easing: Default::default(), zoom: 1.0, wait: true } }] },
            ],
            ..Default::default()
        };
        // 4 tiles at 64 px/s = 2 s; at 2 s the player is halfway, the camera there.
        let p = preview_at(&d, &map(), 2.0, &|_| 64.0);
        let player = p.actors.iter().find(|a| a.0 == "player").unwrap();
        assert!((player.1.x - 2.0).abs() < 1e-3 && player.2, "{player:?}");
        assert_eq!(p.actors.iter().find(|a| a.0 == "guard").map(|a| a.1), Some(vec2(10.0, 5.0)));
        let (cam, zoom) = p.camera.unwrap();
        assert!((cam.x - 20.0).abs() < 1e-3 && (zoom - 1.0).abs() < 1e-3);
        let early = preview_at(&d, &map(), 1.0, &|_| 64.0);
        assert!((early.camera.unwrap().0.x - 10.0).abs() < 1e-3, "halfway from the player's spot to tile 20");
    }
}
