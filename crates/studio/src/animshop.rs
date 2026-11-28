//! An animation seen and played, at the top of its tab: a stage with a straw dummy and the
//! animation drawn over it at the playhead the way the game draws it (each cel's frame at its
//! place, size, turn and opacity; the particles; a flash as a tint), and under it a strip of
//! time with a row for each kind of part. The parts themselves are edited in the form below;
//! the stage reads the definition every frame, so a change shows at once. Sounds are not
//! played: their marks carry their names.
//!
//! The engine's `animations.rs` works out cels and particles from the time alone (0.35's
//! `anim_editor.rs` drew its preview the same way).

use eframe::egui::{self, pos2, vec2, Color32, Rect, Sense, Stroke};
use eframe::egui_wgpu::RenderState;
use editor_core::Workspace;
use rustpg2d_engine::animations::AnimationDef;
use rustpg2d_engine::defs::SpriteRef;

/// The stage's size.
const STAGE: egui::Vec2 = vec2(260.0, 220.0);
/// The stage shows the game's pixels twice as big.
const ZOOM: f32 = 2.0;
/// The strip's rows: what each holds.
const ROWS: [&str; 4] = ["Pictures", "Particles", "Flashes", "Sounds"];

pub struct AnimStage {
    /// The moment shown, in seconds.
    pub time: f32,
    pub playing: bool,
    pub looping: bool,
    /// The window's clock at the last frame (the time runs on by what passed since).
    last: Option<f64>,
}

impl Default for AnimStage {
    fn default() -> AnimStage {
        AnimStage { time: 0.0, playing: false, looping: true, last: None }
    }
}

/// How many frames a sheet has (1 when it isn't loaded).
fn frames_of(ws: &Workspace, sheet: &str) -> u32 {
    ws.assets.sheets.get(sheet).map_or(1, |s| s.count.max(1))
}

/// How long the animation lasts (a tenth of a second at least: an empty one still has a strip).
pub fn length(ws: &Workspace, d: &AnimationDef) -> f32 {
    d.length(|s| frames_of(ws, s)).max(0.1)
}

/// The strip's marks: for each row, (start, end, what it is); a moment has start == end.
fn marks(ws: &Workspace, d: &AnimationDef) -> [Vec<(f32, f32, String)>; 4] {
    [
        d.cels.iter().map(|c| (c.at, c.at + c.seconds(frames_of(ws, &c.sheet)), c.sheet.clone())).collect(),
        d.emitters.iter().map(|e| (e.at, e.end(), format!("{} particles", e.count))).collect(),
        d.flashes.iter().map(|f| (f.at, f.at + f.seconds, if f.screen { "the screen".to_string() } else { "the target".to_string() })).collect(),
        d.sounds.iter().map(|s| (s.at, s.at, s.sound.clone())).collect(),
    ]
}

impl AnimStage {
    /// `ID length=S at=S playing|paused cels=N emitters=N flashes=N sounds=N` for scripts.
    pub fn status(&self, ws: &Workspace, id: &str) -> String {
        let Some(d) = ws.project.db.animations.iter().find(|a| a.id == id) else { return "-".into() };
        format!(
            "{id} length={:.2} at={:.2} {}{} cels={} emitters={} flashes={} sounds={}",
            length(ws, d),
            self.time,
            if self.playing { "playing" } else { "paused" },
            if self.looping { " loop" } else { "" },
            d.cels.len(),
            d.emitters.len(),
            d.flashes.len(),
            d.sounds.len()
        )
    }

    /// The script's words: `time SECS`, `play`, `pause`, `loop on|off`.
    pub fn word(&mut self, ws: &Workspace, id: &str, args: &[String]) -> Result<(), String> {
        let Some(d) = ws.project.db.animations.iter().find(|a| a.id == id) else { return Err(format!("no animation {id}")) };
        let arg = |i: usize| args.get(i).map(String::as_str).unwrap_or("");
        match arg(0) {
            "" => {}
            "time" => {
                self.time = arg(1).parse::<f32>().map_err(|_| "time SECONDS")?.clamp(0.0, length(ws, d));
                self.playing = false;
            }
            "play" => self.play(ws, d),
            "pause" => self.playing = false,
            "loop" => self.looping = arg(1) != "off",
            other => return Err(format!("anim {other}: time, play, pause, loop")),
        }
        Ok(())
    }

    fn play(&mut self, ws: &Workspace, d: &AnimationDef) {
        if self.time >= length(ws, d) {
            self.time = 0.0;
        }
        self.playing = true;
        self.last = None;
    }

    pub fn show(&mut self, ui: &mut egui::Ui, ws: &Workspace, rs: Option<&RenderState>, thumbs: &mut crate::thumbs::Thumbs, id: &str) {
        let Some(d) = ws.project.db.animations.iter().find(|a| a.id == id) else { return };
        let len = length(ws, d);
        // The time runs on while it plays.
        let now = ui.input(|i| i.time);
        if self.playing {
            self.time += self.last.map_or(0.0, |l| (now - l).clamp(0.0, 0.1) as f32);
            if self.time > len {
                if self.looping {
                    self.time = 0.0;
                } else {
                    self.time = len;
                    self.playing = false;
                }
            }
            ui.ctx().request_repaint();
        }
        self.last = Some(now);
        self.time = self.time.clamp(0.0, len);
        ui.horizontal_top(|ui| {
            let (rect, _) = ui.allocate_exact_size(STAGE, Sense::hover());
            paint_stage(ui, rect, ws, rs, thumbs, d, self.time);
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Animation").strong());
                    if self.playing {
                        if ui.button("Pause").clicked() {
                            self.playing = false;
                        }
                    } else if ui.button("Play").on_hover_text("Plays it on the stage (its sounds aren't played here)").clicked() {
                        self.play(ws, d);
                    }
                    if ui.button("From the start").clicked() {
                        self.time = 0.0;
                        self.last = None;
                    }
                    ui.checkbox(&mut self.looping, "Loop");
                    ui.label(format!("{:.2} / {:.2} s", self.time, len));
                });
                self.strip(ui, ws, d, len);
                if d.cels.is_empty() && d.emitters.is_empty() && d.flashes.is_empty() && d.sounds.is_empty() {
                    ui.weak("It has nothing yet: add pictures (cels), particles, flashes or sounds in the form below.");
                } else {
                    ui.weak("Drag on the strip to look at a moment. Its parts are changed in the form below.");
                }
            });
        });
    }

    /// The strip of time: a ruler, a row a kind with a bar for what lasts and a mark for a
    /// moment, the playhead (dragged to look at a moment).
    fn strip(&mut self, ui: &mut egui::Ui, ws: &Workspace, d: &AnimationDef, len: f32) {
        let width = ui.available_width().clamp(260.0, 640.0);
        let (label_w, row_h, ruler) = (70.0, 22.0, 16.0);
        let (rect, resp) = ui.allocate_exact_size(vec2(width, ruler + row_h * ROWS.len() as f32), Sense::click_and_drag());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Slider, true, "The animation's time"));
        let p = ui.painter_at(rect);
        p.rect_filled(rect, 3.0, Color32::from_gray(28));
        let left = rect.left() + label_w;
        let x_of = |t: f32| left + 4.0 + (t / len) * (rect.right() - left - 8.0);
        // A tick every tenth of a second, a number every half (every tenth when it is short).
        let every = if len < 1.0 { 1 } else { 5 };
        let mut k = 0;
        while k as f32 * 0.1 <= len + 1e-4 {
            let x = x_of(k as f32 * 0.1);
            let big = k % every == 0;
            p.line_segment([pos2(x, rect.top()), pos2(x, rect.top() + if big { 8.0 } else { 4.0 })], Stroke::new(1.0, Color32::GRAY));
            if big {
                p.text(pos2(x + 2.0, rect.top()), egui::Align2::LEFT_TOP, format!("{:.1}", k as f32 * 0.1), egui::FontId::proportional(9.0), Color32::GRAY);
            }
            k += 1;
        }
        let colors = [Color32::from_rgb(70, 90, 140), Color32::from_rgb(140, 90, 50), Color32::from_rgb(150, 140, 60), Color32::from_rgb(60, 110, 80)];
        for (row, (name, list)) in ROWS.iter().zip(marks(ws, d)).enumerate() {
            let y = rect.top() + ruler + row as f32 * row_h;
            p.text(pos2(rect.left() + 6.0, y + row_h / 2.0), egui::Align2::LEFT_CENTER, *name, egui::FontId::proportional(11.0), Color32::from_gray(180));
            for (a, b, what) in list {
                if b > a {
                    let r = Rect::from_min_max(pos2(x_of(a), y + 3.0), pos2(x_of(b).max(x_of(a) + 5.0), y + row_h - 3.0));
                    p.rect_filled(r, 3.0, colors[row]);
                    p.text(pos2(r.left() + 3.0, r.center().y), egui::Align2::LEFT_CENTER, what, egui::FontId::proportional(10.0), Color32::WHITE);
                } else {
                    // A moment: a mark with its name beside it.
                    let x = x_of(a);
                    p.line_segment([pos2(x, y + 3.0), pos2(x, y + row_h - 3.0)], Stroke::new(3.0, colors[row]));
                    p.text(pos2(x + 4.0, y + row_h / 2.0), egui::Align2::LEFT_CENTER, what, egui::FontId::proportional(10.0), Color32::from_gray(220));
                }
            }
        }
        let x = x_of(self.time);
        p.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], Stroke::new(1.5, Color32::from_rgb(255, 90, 90)));
        if let Some(pt) = resp.interact_pointer_pos().filter(|_| resp.clicked() || resp.dragged()) {
            self.time = ((pt.x - left - 4.0) / (rect.right() - left - 8.0) * len).clamp(0.0, len);
            self.playing = false;
        }
    }
}

/// The stage: grass, a straw target dummy and the animation at `t` on it, at twice the game's
/// size (0.35's preview). The animation plays at the dummy's middle, as the game plays it on a
/// character: 0.6 tiles above the feet.
fn paint_stage(ui: &egui::Ui, rect: Rect, ws: &Workspace, rs: Option<&RenderState>, thumbs: &mut crate::thumbs::Thumbs, d: &AnimationDef, t: f32) {
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(52, 84, 44));
    let tile = 32.0 * ZOOM;
    let mut x = rect.center().x - tile * 2.5;
    while x < rect.right() {
        p.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], Stroke::new(1.0, Color32::from_rgba_unmultiplied(0, 0, 0, 30)));
        x += tile;
    }
    let feet = pos2(rect.center().x, rect.center().y + tile * 0.9);
    let point = feet - vec2(0.0, tile * 0.6);
    // A flash of the target: the dummy shines in its colour, fading (as the game draws the
    // figure again in the colour over itself).
    let mut tint = None;
    for f in d.flashes.iter().filter(|f| !f.screen && t >= f.at && t < f.at + f.seconds) {
        let fade = 1.0 - (t - f.at) / f.seconds.max(0.01);
        tint = Some(([f.color[0], f.color[1], f.color[2]], f.color[3] as f32 / 255.0 * fade));
    }
    paint_dummy(&p, feet, ZOOM, tint);
    for c in &d.cels {
        let Some(pose) = c.pose(t, frames_of(ws, &c.sheet)) else { continue };
        let (Some(rs), Some(sheet)) = (rs, ws.assets.sheets.get(&c.sheet)) else { continue };
        let Some(thumb) = thumbs.sprite(ws, rs, &SpriteRef::new(&c.sheet, pose.frame)) else { continue };
        let size = vec2(sheet.world_size().x, sheet.world_size().y) * pose.scale * ZOOM;
        let center = point + vec2(pose.offset[0], pose.offset[1]) * ZOOM;
        let color = blend(Color32::from_white_alpha((pose.opacity.clamp(0.0, 1.0) * 255.0) as u8), c.additive);
        let mut mesh = egui::Mesh::with_texture(thumb.texture);
        let (sin, cos) = pose.rotation.to_radians().sin_cos();
        let corner = |dx: f32, dy: f32| center + vec2(dx * cos - dy * sin, dx * sin + dy * cos);
        let (hw, hh) = (size.x / 2.0, size.y / 2.0);
        let uv = thumb.uv;
        for (pt, u) in [(corner(-hw, -hh), uv.left_top()), (corner(hw, -hh), uv.right_top()), (corner(hw, hh), uv.right_bottom()), (corner(-hw, hh), uv.left_bottom())] {
            mesh.vertices.push(egui::epaint::Vertex { pos: pt, uv: u, color });
        }
        mesh.indices.extend([0, 1, 2, 0, 2, 3]);
        p.add(egui::Shape::mesh(mesh));
    }
    for e in &d.emitters {
        for part in e.particles(t) {
            let c = Color32::from_rgba_unmultiplied((part.color[0] * 255.0) as u8, (part.color[1] * 255.0) as u8, (part.color[2] * 255.0) as u8, (part.color[3] * 255.0) as u8);
            // (Round, as the game draws them.)
            p.circle_filled(point + vec2(part.pos[0], part.pos[1]) * ZOOM, (part.size * ZOOM / 2.0).max(0.8), blend(c, e.additive));
        }
    }
    // A flash of the screen covers the stage.
    for f in d.flashes.iter().filter(|f| f.screen && t >= f.at && t < f.at + f.seconds) {
        let a = 1.0 - (t - f.at) / f.seconds.max(0.01);
        p.rect_filled(rect, 4.0, Color32::from_rgba_unmultiplied(f.color[0], f.color[1], f.color[2], (f.color[3] as f32 * a) as u8));
    }
}

/// A colour as drawn: additive adds its light to what is under it (egui's colours are
/// premultiplied: one without alpha adds).
fn blend(c: Color32, additive: bool) -> Color32 {
    if additive {
        Color32::from_rgba_premultiplied(c.r(), c.g(), c.b(), 0)
    } else {
        c
    }
}

/// A straw training dummy on a post, its feet at `feet` (`k` = the stage's zoom).
/// `tint`: a colour it shines in, and how strongly (0 to 1).
fn paint_dummy(p: &egui::Painter, feet: egui::Pos2, k: f32, tint: Option<([u8; 3], f32)>) {
    let mix = |c: Color32| match tint {
        Some((t, a)) => {
            let m = |x: u8, y: u8| (x as f32 * (1.0 - a) + y as f32 * a) as u8;
            Color32::from_rgb(m(c.r(), t[0]), m(c.g(), t[1]), m(c.b(), t[2]))
        }
        None => c,
    };
    let wood = mix(Color32::from_rgb(110, 76, 44));
    let straw = mix(Color32::from_rgb(214, 186, 110));
    let rope = mix(Color32::from_rgb(150, 110, 60));
    p.add(egui::Shape::ellipse_filled(feet, vec2(9.0 * k, 3.0 * k), Color32::from_rgba_unmultiplied(0, 0, 0, 70)));
    p.rect_filled(Rect::from_center_size(feet - vec2(0.0, 10.0 * k), vec2(3.0 * k, 20.0 * k)), 1.0, wood);
    p.rect_filled(Rect::from_center_size(feet - vec2(0.0, 22.0 * k), vec2(22.0 * k, 3.0 * k)), 1.0, wood);
    p.rect_filled(Rect::from_center_size(feet - vec2(0.0, 22.0 * k), vec2(12.0 * k, 18.0 * k)), 4.0 * k, straw);
    p.rect_filled(Rect::from_center_size(feet - vec2(0.0, 18.0 * k), vec2(12.0 * k, 1.5 * k)), 0.0, rope);
    p.rect_filled(Rect::from_center_size(feet - vec2(0.0, 26.0 * k), vec2(12.0 * k, 1.5 * k)), 0.0, rope);
    p.circle_filled(feet - vec2(0.0, 36.0 * k), 5.5 * k, straw);
}

#[cfg(test)]
mod ui_tests {
    use super::*;
    use egui_kittest::kittest::Queryable;
    use egui_kittest::Harness;
    use rustpg2d_engine::animations::{AnimFlash, AnimSound, Emitter};

    struct State {
        ws: Workspace,
        stage: AnimStage,
        thumbs: crate::thumbs::Thumbs,
    }

    #[test]
    fn the_strip_shows_every_part_and_the_playhead_moves_with_the_time() {
        let dir = std::env::temp_dir().join(format!("rustpg2d-animshop-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        editor_core::files::create_project(&dir).unwrap();
        let mut ws = Workspace::new(dir.clone(), editor_core::Gpu::No).unwrap();
        // Sparks for half a second, a flash of the target at 0.1, a sound at 0.2.
        let mut d = AnimationDef::fresh("sparks");
        d.emitters = vec![Emitter { at: 0.0, duration: 0.2, lifetime: 0.3, ..Default::default() }];
        d.flashes = vec![AnimFlash { at: 0.1, seconds: 0.2, ..Default::default() }];
        d.sounds = vec![AnimSound { at: 0.2, sound: "bell".into() }];
        ws.project.db.animations.push(d);
        let mut h = Harness::builder().with_size(egui::vec2(1000.0, 400.0)).build_ui_state(|ui, s: &mut State| s.stage.show(ui, &s.ws, None, &mut s.thumbs, "sparks"), State { ws, stage: AnimStage::default(), thumbs: Default::default() });
        h.run();
        assert_eq!(h.state().stage.status(&h.state().ws, "sparks"), "sparks length=0.50 at=0.00 paused loop cels=0 emitters=1 flashes=1 sounds=1");
        // The time set by the script's word: the playhead is there, playing stops.
        let s = h.state_mut();
        s.stage.word(&s.ws, "sparks", &["time".into(), "0.2".into()]).unwrap();
        h.run();
        assert!(h.state().stage.status(&h.state().ws, "sparks").contains("at=0.20 paused"));
        // Play and Pause.
        h.get_by_label("Play").click();
        // (Playing asks for every next frame.)
        h.run_steps(3);
        assert!(h.state().stage.playing);
        h.get_by_label("Pause").click();
        h.run();
        assert!(!h.state().stage.playing);
        // A click at the strip's right end: its last moment.
        let strip = h.get_by_label("The animation's time").rect();
        let at = egui::pos2(strip.right() - 2.0, strip.center().y);
        h.hover_at(at);
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
            h.run();
        }
        assert!((h.state().stage.time - 0.5).abs() < 0.02, "{}", h.state().stage.time);
        // From the start.
        h.get_by_label("From the start").click();
        h.run();
        assert_eq!(h.state().stage.time, 0.0);
        let _ = std::fs::remove_dir_all(dir);
    }
}
