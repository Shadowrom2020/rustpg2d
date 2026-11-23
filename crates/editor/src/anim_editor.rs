//! The Database's *Animations* form: an animation's cels, particle emitters, flashes and sounds
//! as rows on a timeline, the selected one's fields, and a preview on a straw target dummy that
//! plays it (or shows the moment the playhead is at) the way the game draws it: the engine's
//! `animations.rs` works out cels and particles from the time alone.

use egui_macroquad::egui::{self, pos2, vec2, Color32, Rect, Stroke, Ui};
use rustpg2d_engine::animations::{AnimFlash, AnimSound, AnimationDef, Cel, CelEnd, Emitter};
use rustpg2d_engine::render::Assets;

use crate::ui::{self as eui, ph};
use crate::widgets::{Refs, Textures};

/// What a row of the timeline is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Cel,
    Emitter,
    Flash,
    Sound,
}

/// The preview's playback and the selected row.
#[derive(Default)]
pub struct AnimPreview {
    /// The animation the state belongs to (another one starts over).
    pub id: String,
    pub time: f32,
    pub playing: bool,
    pub looping: bool,
    /// The editor's clock at the last frame (to advance while playing).
    last: f64,
    pub sel: Option<(Part, usize)>,
}

impl AnimPreview {
    /// A preview of animation `id` from its start, looping.
    pub fn for_id(id: &str) -> Self {
        AnimPreview { id: id.to_string(), looping: true, ..Default::default() }
    }

    /// For `status` (`anim=`): `ID@TIME/LENGTH[:playing]`.
    pub fn status(&self, len: f32) -> String {
        if self.id.is_empty() {
            return "-".into();
        }
        format!("{}@{:.2}/{:.2}{}", self.id, self.time, len, if self.playing { ":playing" } else { "" })
    }
}

/// The frame count of a sheet (1 when unknown).
fn frames_of(assets: &Assets, sheet: &str) -> u32 {
    assets.sheets.get(sheet).map_or(1, |s| s.count.max(1))
}

/// The form. True when the definition changed.
pub fn animation_form(ui: &mut Ui, d: &mut AnimationDef, tex: &Textures, assets: &Assets, refs: &Refs, audio: &mut rustpg2d_engine::audio::Audio, pv: &mut AnimPreview) {
    if pv.id != d.id {
        *pv = AnimPreview::for_id(&d.id);
    }
    let len = d.length(|s| frames_of(assets, s)).max(0.1);
    // Playback: the time runs on while playing; sounds start as the playhead passes them.
    let now = crate::ui::now();
    if pv.playing {
        let before = pv.time;
        pv.time += (now - pv.last).clamp(0.0, 0.1) as f32;
        for s in d.sounds.iter().filter(|s| s.at > before && s.at <= pv.time || (before == 0.0 && s.at == 0.0)) {
            audio.play(&s.sound);
        }
        if pv.time > len {
            if pv.looping {
                pv.time = 0.0;
            } else {
                pv.time = len;
                pv.playing = false;
            }
        }
        ui.ctx().request_repaint();
    }
    pv.last = now;
    ui.horizontal(|ui| {
        // The preview on the left, the timeline beside it.
        let (rect, _) = ui.allocate_exact_size(vec2(280.0, 240.0), egui::Sense::hover());
        paint_preview(ui, rect, d, pv.time, tex, assets);
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                let label = if pv.playing { eui::t(ph::PAUSE, "Pause") } else { eui::t(ph::PLAY, "Play") };
                if ui.button(label).clicked() {
                    if !pv.playing && pv.time >= len {
                        pv.time = 0.0;
                    }
                    pv.playing = !pv.playing;
                }
                ui.checkbox(&mut pv.looping, "Loop");
                ui.label(format!("{:.2} / {:.2} s", pv.time, len));
            });
            ui.add(egui::Slider::new(&mut pv.time, 0.0..=len).show_value(false)).on_hover_text("The moment the preview shows");
            timeline(ui, d, pv, len, assets);
            ui.horizontal_wrapped(|ui| {
                if ui.button(eui::t(ph::PLUS, "Cel")).on_hover_text("Frames of a sprite sheet at the playhead").clicked() {
                    d.cels.push(Cel { at: pv.time, sheet: refs.sheets.first().cloned().unwrap_or_default(), ..Default::default() });
                    pv.sel = Some((Part::Cel, d.cels.len() - 1));
                }
                if ui.button(eui::t(ph::PLUS, "Particles")).on_hover_text("An emitter of particles at the playhead").clicked() {
                    d.emitters.push(Emitter { at: pv.time, ..Default::default() });
                    pv.sel = Some((Part::Emitter, d.emitters.len() - 1));
                }
                if ui.button(eui::t(ph::PLUS, "Flash")).on_hover_text("A flash of the target or the screen at the playhead").clicked() {
                    d.flashes.push(AnimFlash { at: pv.time, ..Default::default() });
                    pv.sel = Some((Part::Flash, d.flashes.len() - 1));
                }
                if ui.button(eui::t(ph::PLUS, "Sound")).on_hover_text("A sound at the playhead").clicked() {
                    d.sounds.push(AnimSound { at: pv.time, sound: refs.sounds.first().cloned().unwrap_or_default() });
                    pv.sel = Some((Part::Sound, d.sounds.len() - 1));
                }
            });
        });
    });
    ui.separator();
    part_ui(ui, d, pv, refs, assets);
}

/// The rows: a block per cel (its frames' time), emitter (until its last particle is gone),
/// flash and sound, with the playhead; a click selects, a click on the ruler moves the playhead.
fn timeline(ui: &mut Ui, d: &AnimationDef, pv: &mut AnimPreview, len: f32, assets: &Assets) {
    let mut rows: Vec<(Part, usize, f32, f32, String)> = Vec::new();
    for (k, c) in d.cels.iter().enumerate() {
        rows.push((Part::Cel, k, c.at, c.at + c.seconds(frames_of(assets, &c.sheet)), format!("{} {}", ph::IMAGE, c.sheet)));
    }
    for (k, e) in d.emitters.iter().enumerate() {
        rows.push((Part::Emitter, k, e.at, e.end(), format!("{} {} particles", ph::SPARKLE, e.count)));
    }
    for (k, f) in d.flashes.iter().enumerate() {
        rows.push((Part::Flash, k, f.at, f.at + f.seconds, format!("{} {}", ph::LIGHTNING, if f.screen { "screen" } else { "target" })));
    }
    for (k, s) in d.sounds.iter().enumerate() {
        rows.push((Part::Sound, k, s.at, s.at + 0.25, format!("{} {}", ph::SPEAKER_HIGH, s.sound)));
    }
    let width = ui.available_width().clamp(240.0, 560.0);
    let row_h = 20.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(width, 16.0 + row_h * rows.len().max(1) as f32), egui::Sense::click_and_drag());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 3.0, Color32::from_gray(28));
    let x_of = |t: f32| rect.left() + 4.0 + (t / len) * (rect.width() - 8.0);
    // The ruler: a tick every tenth of a second, numbers every half.
    let mut t = 0.0;
    while t <= len + 1e-4 {
        let x = x_of(t);
        let big = ((t * 10.0).round() as i32) % 5 == 0;
        p.line_segment([pos2(x, rect.top()), pos2(x, rect.top() + if big { 8.0 } else { 4.0 })], Stroke::new(1.0_f32, Color32::GRAY));
        if big {
            p.text(pos2(x + 2.0, rect.top()), egui::Align2::LEFT_TOP, format!("{t:.1}"), egui::FontId::proportional(9.0), Color32::GRAY);
        }
        t += 0.1;
    }
    let mut picked = None;
    for (i, (part, k, a, b, label)) in rows.iter().enumerate() {
        let y = rect.top() + 16.0 + i as f32 * row_h;
        let r = Rect::from_min_max(pos2(x_of(*a), y + 2.0), pos2(x_of(*b).max(x_of(*a) + 6.0), y + row_h - 2.0));
        let selected = pv.sel == Some((*part, *k));
        let fill = match part {
            Part::Cel => Color32::from_rgb(70, 90, 140),
            Part::Emitter => Color32::from_rgb(140, 90, 50),
            Part::Flash => Color32::from_rgb(150, 140, 60),
            Part::Sound => Color32::from_rgb(60, 110, 80),
        };
        p.rect_filled(r, 3.0, if selected { Color32::from_rgb(200, 160, 60) } else { fill });
        p.text(pos2(r.left() + 3.0, r.center().y), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(10.0), Color32::WHITE);
        if let Some(pt) = resp.interact_pointer_pos().filter(|_| resp.clicked()) {
            if pt.y >= y && pt.y < y + row_h && r.expand2(vec2(4.0, 0.0)).contains(pt) {
                picked = Some((*part, *k));
            }
        }
    }
    let x = x_of(pv.time);
    p.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], Stroke::new(1.5_f32, Color32::from_rgb(255, 90, 90)));
    if let Some(pt) = resp.interact_pointer_pos() {
        if picked.is_some() {
            pv.sel = picked;
        } else if resp.clicked() || resp.dragged() {
            pv.time = ((pt.x - rect.left() - 4.0) / (rect.width() - 8.0) * len).clamp(0.0, len);
            pv.playing = false;
        }
    }
}

/// The selected row's fields.
fn part_ui(ui: &mut Ui, d: &mut AnimationDef, pv: &mut AnimPreview, refs: &Refs, assets: &Assets) {
    let Some((part, k)) = pv.sel else {
        ui.weak("Click a row of the timeline to edit it; the buttons add one at the playhead.");
        return;
    };
    let mut remove = false;
    let num = |ui: &mut Ui, label: &str, v: &mut f32, speed: f64, tip: &str| {
        ui.label(label).on_hover_text(tip);
        ui.add(egui::DragValue::new(v).speed(speed).max_decimals(2));
    };
    egui::Grid::new(("anim_part", part as u8, k)).num_columns(4).spacing([8.0, 4.0]).show(ui, |ui| match part {
        Part::Cel => {
            let Some(c) = d.cels.get_mut(k) else { return };
            ui.label("Sheet");
            crate::widgets::suggest(ui, "anim_cel_sheet", &mut c.sheet, &refs.sheets, 160.0);
            ui.label(format!("{} frames", frames_of(assets, &c.sheet)));
            ui.end_row();
            num(ui, "At (s)", &mut c.at, 0.01, "When it starts");
            num(ui, "Frames per second", &mut c.fps, 0.1, "How fast its frames play");
            ui.end_row();
            ui.label("First / last frame").on_hover_text("Both 0 = all of the sheet's frames");
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut c.first));
                ui.add(egui::DragValue::new(&mut c.last));
            });
            ui.label("Plays");
            ui.add(egui::DragValue::new(&mut c.loops).range(1..=50).suffix("x"));
            ui.end_row();
            num(ui, "Offset x", &mut c.offset[0], 0.5, "Pixels right of the point it plays at (the middle of the target)");
            num(ui, "Offset y", &mut c.offset[1], 0.5, "Pixels below the point (negative = above)");
            ui.end_row();
            num(ui, "Size", &mut c.scale, 0.02, "1 = the frame's size");
            num(ui, "Turn (degrees)", &mut c.rotation, 1.0, "Clockwise");
            ui.end_row();
            ui.label("Opacity");
            ui.add(egui::Slider::new(&mut c.opacity, 0.0..=1.0));
            ui.checkbox(&mut c.additive, "Additive").on_hover_text("Adds its light to what is under it: fire, sparks, magic");
            ui.end_row();
            num(ui, "Light (tiles)", &mut c.light, 0.1, "A light around it while it shows (0 = none)");
            ui.label("Light colour");
            let mut glow = c.glow;
            if ui.color_edit_button_srgb(&mut glow).changed() {
                c.glow = glow;
            }
            ui.end_row();
            let mut moves = c.end.is_some();
            if ui.checkbox(&mut moves, "Moves to").on_hover_text("Over its time it moves, grows, turns and fades to these values").changed() {
                c.end = moves.then_some(CelEnd { offset: c.offset, scale: c.scale, rotation: c.rotation, opacity: c.opacity });
            }
            ui.end_row();
            if let Some(e) = &mut c.end {
                num(ui, "  offset x", &mut e.offset[0], 0.5, "");
                num(ui, "  offset y", &mut e.offset[1], 0.5, "");
                ui.end_row();
                num(ui, "  size", &mut e.scale, 0.02, "");
                num(ui, "  turn", &mut e.rotation, 1.0, "");
                ui.end_row();
                ui.label("  opacity");
                ui.add(egui::Slider::new(&mut e.opacity, 0.0..=1.0));
                ui.end_row();
            }
        }
        Part::Emitter => {
            let Some(e) = d.emitters.get_mut(k) else { return };
            num(ui, "At (s)", &mut e.at, 0.01, "When it starts");
            num(ui, "Sends for (s)", &mut e.duration, 0.01, "Seconds it keeps sending particles, evenly (0 = all at once)");
            ui.end_row();
            ui.label("Particles");
            ui.add(egui::DragValue::new(&mut e.count).range(1..=2000));
            num(ui, "Life (s)", &mut e.lifetime, 0.01, "Seconds each one lives");
            ui.end_row();
            num(ui, "Offset x", &mut e.offset[0], 0.5, "Where they start: pixels from the point");
            num(ui, "Offset y", &mut e.offset[1], 0.5, "");
            ui.end_row();
            num(ui, "Area (px)", &mut e.area, 0.5, "They start anywhere within this radius");
            num(ui, "Gravity", &mut e.gravity, 1.0, "Pulls them down (pixels a second each second; negative = up, like flames)");
            ui.end_row();
            num(ui, "Direction", &mut e.direction, 1.0, "Degrees: 0 = right, 90 = down, -90 = up");
            num(ui, "Spread", &mut e.spread, 1.0, "How wide around the direction (360 = every way)");
            ui.end_row();
            num(ui, "Speed", &mut e.speed, 1.0, "Pixels a second");
            ui.label("Speed varies");
            ui.add(egui::Slider::new(&mut e.speed_var, 0.0..=1.0)).on_hover_text("How much slower some are, at random");
            ui.end_row();
            num(ui, "Size", &mut e.size, 0.1, "Pixels across at the start");
            num(ui, "Size at the end", &mut e.size_end, 0.1, "Pixels across at the end of its life");
            ui.end_row();
            ui.label("Colour");
            color4(ui, &mut e.color);
            ui.label("Colour at the end");
            color4(ui, &mut e.color_end);
            ui.end_row();
            ui.checkbox(&mut e.additive, "Additive").on_hover_text("Adds its light: glowing sparks");
            ui.end_row();
        }
        Part::Flash => {
            let Some(f) = d.flashes.get_mut(k) else { return };
            num(ui, "At (s)", &mut f.at, 0.01, "When it starts");
            num(ui, "Seconds", &mut f.seconds, 0.01, "How long it fades");
            ui.end_row();
            ui.radio_value(&mut f.screen, false, "The target").on_hover_text("The target's figure shines in the colour");
            ui.radio_value(&mut f.screen, true, "The screen").on_hover_text("The whole screen (players can turn screen effects off)");
            ui.label("Colour");
            color4(ui, &mut f.color).on_hover_text("Alpha = how strong");
            ui.end_row();
        }
        Part::Sound => {
            let Some(s) = d.sounds.get_mut(k) else { return };
            num(ui, "At (s)", &mut s.at, 0.01, "When it starts");
            ui.label("Sound");
            crate::widgets::suggest(ui, "anim_sound", &mut s.sound, &refs.sounds, 160.0);
            ui.end_row();
        }
    });
    if ui.button(eui::t(ph::TRASH, "Remove")).clicked() {
        remove = true;
    }
    if remove {
        match part {
            Part::Cel if k < d.cels.len() => drop(d.cels.remove(k)),
            Part::Emitter if k < d.emitters.len() => drop(d.emitters.remove(k)),
            Part::Flash if k < d.flashes.len() => drop(d.flashes.remove(k)),
            Part::Sound if k < d.sounds.len() => drop(d.sounds.remove(k)),
            _ => {}
        }
        pv.sel = None;
    }
}

/// A colour button for RGBA that only writes when the person changes it (egui's round trip
/// through HSV would shift a fully transparent colour every frame).
fn color4(ui: &mut Ui, c: &mut [u8; 4]) -> egui::Response {
    let mut v = *c;
    let r = ui.color_edit_button_srgba_unmultiplied(&mut v);
    if r.changed() {
        *c = v;
    }
    r
}

/// The preview: grass, a straw target dummy and the animation at `t` on it, at twice the game's
/// size (32 px tiles shown as 64).
fn paint_preview(ui: &Ui, rect: Rect, d: &AnimationDef, t: f32, tex: &Textures, assets: &Assets) {
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(52, 84, 44));
    let k = 2.0;
    // The tile grid.
    let tile = 32.0 * k;
    let mut x = rect.center().x - tile * 2.5;
    while x < rect.right() {
        p.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(0, 0, 0, 30)));
        x += tile;
    }
    // The dummy stands with its feet a tile below the middle; the animation plays at its body's
    // middle (as the game plays it on a character: 0.6 tiles above the feet).
    let feet = pos2(rect.center().x, rect.center().y + tile * 0.9);
    let point = feet - vec2(0.0, tile * 0.6);
    // A target flash tints the dummy.
    let mut tint: Option<Color32> = None;
    for f in d.flashes.iter().filter(|f| !f.screen && t >= f.at && t < f.at + f.seconds) {
        let a = 1.0 - (t - f.at) / f.seconds.max(0.01);
        tint = Some(Color32::from_rgba_unmultiplied(f.color[0], f.color[1], f.color[2], (f.color[3] as f32 * a) as u8));
    }
    paint_dummy(&p, feet, k, tint);
    for c in &d.cels {
        let Some(pose) = c.pose(t, frames_of(assets, &c.sheet)) else { continue };
        let (Some(handle), Some(s)) = (tex.sheets.get(&c.sheet), assets.sheets.get(&c.sheet)) else { continue };
        let src = s.src_rect(pose.frame);
        let (w, h) = (s.image.width as f32, s.image.height as f32);
        let uv = Rect::from_min_max(pos2(src.x / w, src.y / h), pos2((src.x + src.w) / w, (src.y + src.h) / h));
        let size = vec2(s.world_size().x, s.world_size().y) * pose.scale * k;
        let center = point + vec2(pose.offset[0], pose.offset[1]) * k;
        let color = Color32::from_white_alpha((pose.opacity * 255.0) as u8);
        let mut mesh = egui::Mesh::with_texture(handle.id());
        let (sin, cos) = pose.rotation.to_radians().sin_cos();
        let corner = |dx: f32, dy: f32| center + vec2(dx * cos - dy * sin, dx * sin + dy * cos);
        let (hw, hh) = (size.x / 2.0, size.y / 2.0);
        let idx = mesh.vertices.len() as u32;
        for (pt, u) in [(corner(-hw, -hh), uv.left_top()), (corner(hw, -hh), uv.right_top()), (corner(hw, hh), uv.right_bottom()), (corner(-hw, hh), uv.left_bottom())] {
            mesh.vertices.push(egui::epaint::Vertex { pos: pt, uv: u, color });
        }
        mesh.indices.extend([idx, idx + 1, idx + 2, idx, idx + 2, idx + 3]);
        p.add(egui::Shape::mesh(mesh));
    }
    for e in &d.emitters {
        for part in e.particles(t) {
            let c = Color32::from_rgba_unmultiplied((part.color[0] * 255.0) as u8, (part.color[1] * 255.0) as u8, (part.color[2] * 255.0) as u8, (part.color[3] * 255.0) as u8);
            p.circle_filled(point + vec2(part.pos[0], part.pos[1]) * k, (part.size * k / 2.0).max(0.8), c);
        }
    }
    // A screen flash covers the preview.
    for f in d.flashes.iter().filter(|f| f.screen && t >= f.at && t < f.at + f.seconds) {
        let a = 1.0 - (t - f.at) / f.seconds.max(0.01);
        p.rect_filled(rect, 4.0, Color32::from_rgba_unmultiplied(f.color[0], f.color[1], f.color[2], (f.color[3] as f32 * a) as u8));
    }
}

/// A straw training dummy on a post, its feet at `feet` (`k` = the preview's zoom).
fn paint_dummy(p: &egui::Painter, feet: egui::Pos2, k: f32, tint: Option<Color32>) {
    let mix = |c: Color32| match tint {
        Some(t) => {
            let a = t.a() as f32 / 255.0;
            let m = |x: u8, y: u8| (x as f32 * (1.0 - a) + y as f32 * a) as u8;
            Color32::from_rgb(m(c.r(), t.r()), m(c.g(), t.g()), m(c.b(), t.b()))
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
