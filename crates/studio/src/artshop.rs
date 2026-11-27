//! Art looked at and listened to where it is defined: at the top of a sound's tab it is
//! played, looped and stopped, with its length; at the top of a sprite sheet's tab its
//! picture is shown with the grid its frames are cut by, cut again by saying how many frames
//! go across and down, and its animations play.

use eframe::egui::{self, pos2, Color32, Sense, Stroke};
use eframe::egui_wgpu::RenderState;
use editor_core::Workspace;
use rustpg2d_engine::defs::{SheetTag, SpriteRef};

// ---------------------------------------------------------------- sounds

/// A sound file decoded: the sound and its length in seconds, or what is wrong with the file.
type Decoded = Result<(gfx::audio::Sound, f32), String>;

/// A sound of the project, loaded to be heard.
#[derive(Default)]
pub struct SoundStage {
    /// The file that was loaded (a path under the project), and what came of it.
    loaded: Option<(String, Decoded)>,
    /// Playing since this time of the UI's clock; true: looped.
    playing: Option<(f64, bool)>,
}

/// Whether sound is switched off for this run (`RUSTPG2D_MUTE`, as for the game).
fn muted() -> bool {
    std::env::var_os("RUSTPG2D_MUTE").is_some()
}

impl SoundStage {
    fn load(&mut self, ws: &Workspace, file: &str) {
        if self.loaded.as_ref().is_some_and(|(f, _)| f == file) {
            return;
        }
        self.stop();
        self.loaded = Some((file.to_string(), rustpg2d_engine::audio::load_one(&ws.project.path(file))));
    }

    pub fn play(&mut self, ws: &Workspace, id: &str, looped: bool, now: f64) {
        let Some(def) = ws.project.db.sounds.iter().find(|s| s.id == id) else { return };
        self.load(ws, &def.file);
        self.stop();
        if let Some((_, Ok((sound, _)))) = &self.loaded {
            if !muted() {
                gfx::audio::play_sound(sound, gfx::audio::PlaySoundParams { looped, volume: def.volume.clamp(0.0, 1.0) });
            }
            self.playing = Some((now, looped));
        }
    }

    pub fn stop(&mut self) {
        if let (Some(_), Some((_, Ok((sound, _))))) = (self.playing.take(), &self.loaded) {
            gfx::audio::stop_sound(sound);
        }
    }

    /// `ID file=… length=… playing|looping|stopped[ muted]` or what is wrong with the file.
    pub fn status(&mut self, ws: &Workspace, id: &str) -> String {
        let Some(def) = ws.project.db.sounds.iter().find(|s| s.id == id) else { return "-".into() };
        self.load(ws, &def.file);
        match &self.loaded {
            Some((file, Ok((_, secs)))) => format!("{id} file={file} length={secs:.2} {}{}", match self.playing {
                Some((_, true)) => "looping",
                Some((_, false)) => "playing",
                None => "stopped",
            }, if muted() { " muted" } else { "" }),
            Some((file, Err(e))) => format!("{id} file={file} failed: {e}"),
            None => "-".into(),
        }
    }

    /// The row at the top of a sound's tab.
    pub fn show(&mut self, ui: &mut egui::Ui, ws: &Workspace, id: &str) {
        let Some(def) = ws.project.db.sounds.iter().find(|s| s.id == id) else { return };
        let (file, volume) = (def.file.clone(), def.volume);
        self.load(ws, &file);
        let now = ui.input(|i| i.time);
        let length = match &self.loaded {
            Some((_, Ok((_, secs)))) => Some(*secs),
            _ => None,
        };
        // (A sound played once is over when its length has passed.)
        if let (Some((since, false)), Some(secs)) = (self.playing, length) {
            if secs > 0.0 && now - since > secs as f64 {
                self.playing = None;
            }
        }
        ui.horizontal_wrapped(|ui| {
            ui.strong("Listen");
            let ok = length.is_some();
            if ui.add_enabled(ok, egui::Button::new("Play")).on_hover_text("Once, as loud as the definition says").clicked() {
                self.play(ws, id, false, now);
            }
            if ui.add_enabled(ok, egui::Button::new("Loop")).on_hover_text("Again and again, as music and ambience play: listen for the seam").clicked() {
                self.play(ws, id, true, now);
            }
            if ui.add_enabled(self.playing.is_some(), egui::Button::new("Stop")).clicked() {
                self.stop();
            }
            match (&self.loaded, length) {
                (_, Some(secs)) => {
                    let what = if secs <= 0.0 { "length unknown".to_string() } else if secs >= 60.0 { format!("{}:{:02} min", secs as u32 / 60, secs as u32 % 60) } else { format!("{secs:.1} s") };
                    ui.weak(format!("{what} · {file} · volume {:.0} %", volume * 100.0));
                    if let Some((since, looped)) = self.playing {
                        let at = if secs > 0.0 { ((now - since) as f32 % secs.max(0.01)) / secs } else { 0.0 };
                        ui.add(egui::ProgressBar::new(at).desired_width(160.0).text(if looped { "looping" } else { "playing" }));
                        ui.ctx().request_repaint_after(std::time::Duration::from_millis(50));
                    }
                    if muted() {
                        ui.colored_label(ui.visuals().warn_fg_color, "Sound is switched off for this run (RUSTPG2D_MUTE).");
                    }
                }
                (Some((_, Err(e))), _) => {
                    ui.colored_label(ui.visuals().error_fg_color, format!("{file}: {e}"));
                }
                _ => {}
            }
        });
    }
}

// ---------------------------------------------------------------- sprite sheets

/// A sprite sheet's picture with its frames.
#[derive(Default)]
pub struct SheetStage {
    /// The frame picked (shown framed; an animation added starts there).
    pub frame: Option<u32>,
    /// The animation that plays in the corner (its place in the sheet's list).
    pub tag: Option<usize>,
    /// The picture's size, read from its file when the art isn't loaded (image, width, height).
    size: Option<(String, u32, u32)>,
}

/// How many frames of `frame` pixels fit into `picture` pixels (at least one).
fn fit(picture: u32, frame: u32) -> u32 {
    (picture / frame.max(1)).max(1)
}

impl SheetStage {
    /// The picture's size in its pixels: of the loaded art, else read from the file.
    fn picture_size(&mut self, ws: &Workspace, id: &str, image: &str) -> Option<(u32, u32)> {
        if let Some(s) = ws.assets.sheets.get(id) {
            return Some((s.image.width as u32, s.image.height as u32));
        }
        if !self.size.as_ref().is_some_and(|(i, ..)| i == image) {
            let size = std::fs::read(ws.project.path(image)).ok().and_then(|b| rustpg2d_engine::imageio::Rgba::decode(&b).ok()).map_or((0, 0), |p| (p.w, p.h));
            self.size = Some((image.to_string(), size.0, size.1));
        }
        self.size.as_ref().map(|(_, w, h)| (*w, *h)).filter(|(w, h)| *w > 0 && *h > 0)
    }

    /// Cuts the sheet into `across` x `down` frames. False when it is cut so already.
    pub fn cut(&mut self, ws: &mut Workspace, id: &str, across: u32, down: u32) -> bool {
        let Some(pos) = ws.project.db.sheets.iter().position(|s| s.id == id) else { return false };
        let image = ws.project.db.sheets[pos].image.clone();
        let Some((w, h)) = self.picture_size(ws, id, &image) else { return false };
        let (fw, fh) = ((w / across.clamp(1, w)).max(1), (h / down.clamp(1, h)).max(1));
        let d = &ws.project.db.sheets[pos];
        if (d.frame_w, d.frame_h) == (fw, fh) {
            return false;
        }
        ws.begin(&format!("Cut the sheet {id} into {across} x {down} frames"));
        let d = &mut ws.project.db.sheets[pos];
        (d.frame_w, d.frame_h) = (fw, fh);
        ws.stale = true;
        true
    }

    /// `ID picture=WxH frames=AxD each=WxH frame=K tags=[name:first-last@fps …]`.
    pub fn status(&mut self, ws: &Workspace, id: &str) -> String {
        let Some(d) = ws.project.db.sheets.iter().find(|s| s.id == id).cloned() else { return "-".into() };
        let Some((w, h)) = self.picture_size(ws, id, &d.image) else { return format!("{id} picture=? (its file {} can't be read)", d.image) };
        let tags: Vec<String> = d.tags.iter().map(|t| format!("{}:{}-{}@{}", t.name, t.first, t.last, t.fps)).collect();
        format!("{id} picture={w}x{h} frames={}x{} each={}x{} frame={} tags=[{}]", fit(w, d.frame_w), fit(h, d.frame_h), d.frame_w, d.frame_h, self.frame.map_or("-".to_string(), |f| f.to_string()), tags.join(" "))
    }

    /// The workshop at the top of a sheet's tab. True when it changed the sheet.
    pub fn show(&mut self, ui: &mut egui::Ui, ws: &mut Workspace, rs: Option<&RenderState>, thumbs: &mut crate::thumbs::Thumbs, id: &str) -> bool {
        let Some(d) = ws.project.db.sheets.iter().find(|s| s.id == id).cloned() else { return false };
        let Some((w, h)) = self.picture_size(ws, id, &d.image) else {
            ui.colored_label(ui.visuals().warn_fg_color, format!("Its picture {} can't be read: name the file below.", d.image));
            return false;
        };
        let (mut across, mut down) = (fit(w, d.frame_w), fit(h, d.frame_h));
        let count = across * down;
        self.frame = self.frame.filter(|f| *f < count);
        let mut changed = false;
        ui.horizontal_wrapped(|ui| {
            ui.strong("Frames");
            let l = ui.label("across");
            ui.add(egui::DragValue::new(&mut across).range(1..=w.min(256)).speed(0.05)).labelled_by(l.id);
            let l = ui.label("down");
            ui.add(egui::DragValue::new(&mut down).range(1..=h.min(256)).speed(0.05)).labelled_by(l.id);
            ui.weak(format!("{count} frames of {} x {} pixels · the picture is {w} x {h}", d.frame_w, d.frame_h));
            let (rest_x, rest_y) = (w % d.frame_w.max(1), h % d.frame_h.max(1));
            if rest_x > 0 || rest_y > 0 {
                ui.colored_label(ui.visuals().warn_fg_color, format!("{rest_x} x {rest_y} pixels at the right and lower edge belong to no frame")).on_hover_text("The picture's size isn't a whole number of frames: frames may be cut in the wrong places");
            }
        });
        if (across, down) != (fit(w, d.frame_w), fit(h, d.frame_h)) {
            // (Dragged over many frames, it is one step.)
            crate::shell::hold_step(ui.ctx(), ("cut sheet", id));
            changed |= self.cut(ws, id, across, down);
        }
        ui.horizontal_top(|ui| {
            // The picture, as large as fits, with its grid.
            let room = egui::vec2((ui.available_width() - 260.0).clamp(160.0, 900.0), 440.0);
            let k = (room.x / w as f32).min(room.y / h as f32).min(8.0);
            let k = if k >= 1.0 { k.floor() } else { k };
            let (rect, resp) = ui.allocate_exact_size(egui::vec2(w as f32, h as f32) * k, Sense::click());
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Image, true, "The sheet's picture"));
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 0.0, Color32::from_rgb(38, 40, 46));
            if let Some(t) = rs.and_then(|rs| thumbs.sprite(ws, rs, &SpriteRef::new(id, 0))) {
                painter.image(t.texture, rect, egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            }
            let (cw, ch) = (d.frame_w as f32 * k, d.frame_h as f32 * k);
            let line = Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 70));
            for c in 0..=across {
                painter.line_segment([rect.min + egui::vec2(c as f32 * cw, 0.0), rect.min + egui::vec2(c as f32 * cw, down as f32 * ch)], line);
            }
            for r in 0..=down {
                painter.line_segment([rect.min + egui::vec2(0.0, r as f32 * ch), rect.min + egui::vec2(across as f32 * cw, r as f32 * ch)], line);
            }
            let cell = |f: u32| egui::Rect::from_min_size(rect.min + egui::vec2((f % across) as f32 * cw, (f / across) as f32 * ch), egui::vec2(cw, ch));
            if cw >= 26.0 && ch >= 18.0 {
                for f in 0..count.min(600) {
                    painter.text(cell(f).min + egui::vec2(2.0, 1.0), egui::Align2::LEFT_TOP, f.to_string(), egui::FontId::proportional(10.0), Color32::from_rgba_unmultiplied(255, 255, 255, 150));
                }
            }
            let at = |p: egui::Pos2| {
                let (c, r) = (((p.x - rect.min.x) / cw) as u32, ((p.y - rect.min.y) / ch) as u32);
                (c < across && r < down).then_some(r * across + c)
            };
            if let Some(f) = resp.hover_pos().and_then(at) {
                painter.rect_stroke(cell(f), 0.0, Stroke::new(1.0_f32, Color32::WHITE), egui::StrokeKind::Inside);
            }
            if let Some(f) = resp.interact_pointer_pos().filter(|_| resp.clicked()).and_then(at) {
                self.frame = Some(f);
            }
            if let Some(f) = self.frame {
                painter.rect_stroke(cell(f), 0.0, Stroke::new(2.0_f32, Color32::YELLOW), egui::StrokeKind::Inside);
            }
            // Beside it: the frame picked and the animations.
            ui.vertical(|ui| {
                match self.frame {
                    Some(f) => ui.label(format!("Frame {f} (column {}, row {})", f % across, f / across)),
                    None => ui.weak("Click a frame to pick it."),
                };
                ui.add_space(4.0);
                ui.strong("Animations");
                if d.tags.is_empty() {
                    ui.weak("None: a run of frames with a name, played by the Effect and Fly actions.");
                }
                for (k, t) in d.tags.iter().enumerate() {
                    let label = format!("{}  {}-{} at {} a second", t.name, t.first, t.last, t.fps);
                    if ui.selectable_label(self.tag == Some(k), label).on_hover_text("Plays it here").clicked() {
                        self.tag = if self.tag == Some(k) { None } else { Some(k) };
                    }
                }
                if let Some(first) = self.frame {
                    if ui.button("+ Animation from this frame").on_hover_text("From the frame picked to the end of its row, eight frames a second: its name, end and speed are changed in the form below").clicked() {
                        let last = (first / across) * across + across - 1;
                        let mut n = d.tags.len() + 1;
                        while d.tags.iter().any(|t| t.name == format!("animation{n}")) {
                            n += 1;
                        }
                        if let Some(sheet) = ws.project.db.sheets.iter_mut().find(|s| s.id == id) {
                            sheet.tags.push(SheetTag { name: format!("animation{n}"), first, last, fps: 8.0 });
                            self.tag = Some(sheet.tags.len() - 1);
                        }
                        ws.begin(&format!("Add an animation to the sheet {id}"));
                        ws.stale = true;
                        changed = true;
                    }
                }
                // The animation picked, playing.
                if let (Some(t), Some(rs)) = (self.tag.and_then(|k| d.tags.get(k)), rs) {
                    let frames = t.last.saturating_sub(t.first) + 1;
                    let f = t.first + ((ui.input(|i| i.time) * t.fps.max(0.1) as f64) as u32) % frames;
                    let (box_, _) = ui.allocate_exact_size(egui::vec2(120.0, 120.0), Sense::hover());
                    ui.painter().rect_filled(box_, 4.0, Color32::from_rgb(32, 34, 40));
                    if let Some(thumb) = thumbs.sprite(ws, rs, &SpriteRef::new(id, f)) {
                        thumb.paint(ui, box_.shrink(6.0));
                    }
                    ui.ctx().request_repaint_after(std::time::Duration::from_millis(33));
                }
            });
        });
        changed
    }
}

#[cfg(test)]
mod ui_tests {
    use super::*;
    use egui_kittest::kittest::Queryable;
    use egui_kittest::Harness;

    struct State {
        ws: Workspace,
        stage: SheetStage,
        thumbs: crate::thumbs::Thumbs,
        dir: std::path::PathBuf,
    }

    #[test]
    fn a_sheet_is_cut_into_frames_and_gets_an_animation() {
        let dir = std::env::temp_dir().join(format!("rustpg2d-artshop-{}", std::process::id())).join("game");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        editor_core::files::create_project(&dir).unwrap();
        // A picture of 96 x 64 pixels as one frame, as a dropped picture arrives.
        let picture = dir.parent().unwrap().join("flame.png");
        std::fs::write(&picture, rustpg2d_engine::imageio::Rgba::new(96, 64).encode().unwrap()).unwrap();
        let mut ws = Workspace::new(dir.clone(), editor_core::Gpu::No).unwrap();
        ws.history_frame(None);
        let id = ws.make_from_file(rustpg2d_engine::refactor::RefKind::Sheet, &picture, "Flame").unwrap();
        ws.history_frame(None);
        let steps = ws.timeline.done.len();
        let mut h = Harness::builder().with_size(egui::vec2(1100.0, 600.0)).build_ui_state(
            |ui, s: &mut State| {
                let held = ui.ctx().data_mut(|d| d.remove_temp::<u64>(crate::shell::held_step_id()));
                s.ws.history_frame(held.or(ui.ctx().memory(|m| m.focused()).map(|id| id.value())));
                s.stage.show(ui, &mut s.ws, None, &mut s.thumbs, "flame");
            },
            State { ws, stage: SheetStage::default(), thumbs: Default::default(), dir },
        );
        h.run();
        assert!(h.query_all_by_label_contains("1 frames of 96 x 64 pixels").next().is_some());
        // Three across, two down: frames of 32 x 32, as one step.
        let s = h.state_mut();
        assert!(!s.stage.cut(&mut s.ws, &id, 1, 1), "cut so already: nothing to do");
        let s = h.state_mut();
        assert!(s.stage.cut(&mut s.ws, &id, 3, 2));
        h.run();
        h.run();
        let s = h.state();
        let d = s.ws.project.db.sheets.iter().find(|d| d.id == "flame").unwrap();
        assert_eq!((d.frame_w, d.frame_h), (32, 32));
        assert_eq!(s.ws.timeline.done.len(), steps + 1);
        assert_eq!(s.ws.timeline.done.last().unwrap().label, "Cut the sheet flame into 3 x 2 frames");
        assert!(h.query_all_by_label_contains("6 frames of 32 x 32 pixels").next().is_some());
        // The fourth frame picked (the first of the second row): an animation from it runs to its row's end.
        let rect = h.get_by_label("The sheet's picture").rect();
        let cell = rect.width() / 3.0;
        h.hover_at(rect.min + egui::vec2(cell * 0.5, cell * 1.5));
        h.run();
        h.drag_at(rect.min + egui::vec2(cell * 0.5, cell * 1.5));
        h.run();
        h.event(egui::Event::PointerButton { pos: rect.min + egui::vec2(cell * 0.5, cell * 1.5), button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
        h.run();
        assert_eq!(h.state().stage.frame, Some(3));
        h.get_by_label("Frame 3 (column 0, row 1)");
        h.get_by_label("+ Animation from this frame").click();
        h.run();
        h.run();
        let s = h.state();
        let d = s.ws.project.db.sheets.iter().find(|d| d.id == "flame").unwrap();
        assert_eq!(d.tags, [SheetTag { name: "animation1".into(), first: 3, last: 5, fps: 8.0 }]);
        assert_eq!(s.stage.tag, Some(0));
        // A size that leaves pixels over says so.
        let s = h.state_mut();
        s.stage.cut(&mut s.ws, &id, 5, 2);
        h.run();
        assert!(h.query_all_by_label_contains("1 x 0 pixels at the right and lower edge belong to no frame").next().is_some());
        let dir = h.state().dir.clone();
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }
}
