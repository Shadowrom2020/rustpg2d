//! The prop workshop, at the top of a prop's tab: the prop large, standing on a few tiles as
//! the game draws it, with what is otherwise typed as numbers dragged on the picture: the
//! hitbox by its corners, the spot it is used from, how far its light reaches. Its states as a
//! strip above (the one picked is the one shown), a quarter turn for props that turn, and
//! *In the dark* to see the light it gives.

use eframe::egui::{self, pos2, Color32, Pos2, Sense, Stroke};
use eframe::egui_wgpu::RenderState;
use editor_core::Workspace;
use gfx::input::Input;
use gfx::prelude::{vec2, Vec2};
use rustpg2d_engine::defs::{LightSpec, PropDef, PropState};
use rustpg2d_engine::entity::{EntityInstance, EntityKind, Facing};
use rustpg2d_engine::map::{LightMode, Map, MapLighting};
use rustpg2d_engine::render::{self, lighting::LightPass};

use crate::scene::{Look, Scene};

/// The stage: this many tiles a side, the prop on the middle one.
const TILES: u32 = 21;

/// What is dragged on the picture.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Handle {
    /// A corner of the hitbox: east (else west), north (else south).
    Corner(bool, bool),
    Spot,
    Reach,
    /// The middle of the light (where it sits on the ground).
    LightAt,
    /// The foot of the picture (where it stands, in the picture's own pixels).
    Picture,
}

impl Handle {
    fn label(self) -> &'static str {
        match self {
            Handle::Corner(false, true) => "The hitbox's north-west corner",
            Handle::Corner(true, true) => "The hitbox's north-east corner",
            Handle::Corner(true, false) => "The hitbox's south-east corner",
            Handle::Corner(false, false) => "The hitbox's south-west corner",
            Handle::Spot => "The spot it is used from",
            Handle::Reach => "How far the light reaches",
            Handle::LightAt => "Where the light sits",
            Handle::Picture => "Where the picture stands",
        }
    }
}

pub struct PropStage {
    screen: Option<gfx::Screen>,
    input: Input,
    texture: Option<(egui::TextureId, (u32, u32))>,
    light_pass: Option<LightPass>,
    /// The state shown (0 = the first) and the way the prop is turned.
    pub state: usize,
    pub facing: Facing,
    /// Shown at night, lit by its own light only.
    pub dark: bool,
    /// Where the picture is in the window, and its camera (for the handles and the script).
    rect: egui::Rect,
    look: Option<Look>,
    ppp: f32,
    /// A handle is being dragged: the camera stays as it is (else what is dragged would move
    /// the picture under the pointer).
    dragging: bool,
    /// How many drags there were: each is a step of the history of its own.
    drags: u32,
}

impl Default for PropStage {
    fn default() -> PropStage {
        PropStage { screen: None, input: Input::default(), texture: None, light_pass: None, state: 0, facing: Facing::Down, dark: false, rect: egui::Rect::NOTHING, look: None, ppp: 1.0, dragging: false, drags: 0 }
    }
}

fn feet(ts: f32) -> Vec2 {
    // (The middle of a tile's southern edge: a hitbox one tile wide then lies on that tile.)
    vec2((TILES / 2) as f32 * ts + ts / 2.0, (TILES / 2 + 1) as f32 * ts)
}

fn quarter(f: Facing) -> Facing {
    match f {
        Facing::Down => Facing::Left,
        Facing::Left => Facing::Up,
        Facing::Up => Facing::Right,
        Facing::Right => Facing::Down,
    }
}

fn snapped(v: f32, step: f32) -> f32 {
    (v / step).round() * step
}

impl PropStage {
    fn light_of(def: &PropDef, state: usize) -> Option<&LightSpec> {
        def.state_light(state)
    }

    fn light_mut(def: &mut PropDef, state: usize) -> Option<&mut LightSpec> {
        match state.checked_sub(1) {
            None => def.light.as_mut(),
            Some(i) => def.states.get_mut(i).and_then(|s| s.light.as_mut()),
        }
    }

    /// Where the light of the state shown sits (on the ground, as the game's light pass has it).
    fn light_at(&self, ws: &Workspace, def: &PropDef) -> Vec2 {
        let ts = ws.ts();
        let lift = ws.assets.sprite_size(def.state_look(self.state).0, ts).y * 0.65;
        feet(ts) + Vec2::from(PropStage::light_of(def, self.state).map_or([0.0, -lift], |l| l.offset_or(lift)))
    }

    /// Whether the state shown is drawn as a picture (a 3D model, a building and a block
    /// stand on the hitbox by themselves).
    fn pictured(&self, def: &PropDef) -> bool {
        def.model_at(self.state).is_empty() && def.building.is_none() && def.block == 0.0
    }

    /// The camera: the prop in the middle, as large as the picture allows (in the dark: far
    /// enough away to see how far the light reaches).
    fn camera(&self, ws: &Workspace, def: &PropDef, size: (u32, u32)) -> Look {
        let ts = ws.ts();
        let at = feet(ts);
        let screen = vec2(size.0 as f32, size.1 as f32);
        let frame = ws.assets.prop_rect(def, self.state, self.facing, at, ts);
        let [w, h] = def.hitbox_facing(self.facing);
        // What has to be seen: the prop's picture and its hitbox on the ground, with room around.
        let (mut wide, mut high) = (frame.w.max((w + h) / render::iso::UPRIGHT) + 2.0 * ts, frame.h.max(ts) + 1.5 * ts);
        if let (true, Some(l)) = (self.dark, PropStage::light_of(def, self.state)) {
            wide = wide.max(2.0 * l.radius * ts * 1.5 + ts);
            high = high.max(l.radius * ts * 1.5 + ts);
        }
        let zoom = (screen.x / wide).min(screen.y / high).clamp(0.4, 4.0);
        let look = Look { center: at, zoom, screen };
        // (The feet a little under the middle: what stands rises above them.)
        let lift = look.pan(vec2(0.0, (frame.h * zoom * 0.5).min(screen.y * 0.3)));
        Look { center: at - lift, ..look }
    }

    /// Where a handle is (world, on the ground).
    fn handle_at(&self, ws: &Workspace, def: &PropDef, h: Handle) -> Option<Vec2> {
        let at = feet(ws.ts());
        let [w, d] = def.hitbox_facing(self.facing);
        match h {
            Handle::Corner(east, north) => Some(vec2(at.x + if east { w / 2.0 } else { -w / 2.0 }, at.y - if north { d } else { 0.0 })),
            Handle::Spot => def.use_spot.as_ref().map(|s| at + Vec2::from(s.offset)),
            Handle::Reach => PropStage::light_of(def, self.state).map(|l| self.light_at(ws, def) + vec2(l.radius * ws.ts(), 0.0)),
            Handle::LightAt => PropStage::light_of(def, self.state).map(|_| self.light_at(ws, def)),
            // (Not a point of the ground: see `handle_window`.)
            Handle::Picture => Some(at),
        }
    }

    /// Where a handle is in the window. The picture's foot is moved in the picture's own
    /// pixels (right and down, as it is drawn standing), the others lie on the ground.
    fn handle_window(&self, ws: &Workspace, def: &PropDef, h: Handle) -> Option<Pos2> {
        let p = self.window_pos(self.handle_at(ws, def, h)?)?;
        match h {
            Handle::Picture => Some(p + egui::vec2(def.picture_offset[0], def.picture_offset[1]) * self.look?.zoom / self.ppp),
            _ => Some(p),
        }
    }

    fn handles(&self, def: &PropDef) -> Vec<Handle> {
        let mut v = vec![Handle::Corner(false, true), Handle::Corner(true, true), Handle::Corner(true, false), Handle::Corner(false, false)];
        if def.use_spot.is_some() {
            v.push(Handle::Spot);
        }
        if PropStage::light_of(def, self.state).is_some() {
            v.extend([Handle::Reach, Handle::LightAt]);
        }
        if self.pictured(def) {
            v.push(Handle::Picture);
        }
        v
    }

    /// A handle dragged to the ground point `p` (the picture's foot: by `moved` pixels of the
    /// picture from the prop's place). `fine`: whole pixels instead of steps of four.
    fn drag_to(&self, ws: &Workspace, def: &mut PropDef, h: Handle, p: Vec2, moved: Vec2, fine: bool) {
        let ts = ws.ts();
        let at = feet(ts);
        let step = if fine { 1.0 } else { 4.0 };
        match h {
            Handle::Corner(_, north) => {
                let [_, d0] = def.hitbox_facing(self.facing);
                let w = snapped(2.0 * (p.x - at.x).abs(), step).clamp(2.0, 16.0 * ts);
                let d = if north { snapped(at.y - p.y, step).clamp(2.0, 16.0 * ts) } else { d0 };
                // (A prop turned a quarter stands across: its own width is what lies north to south.)
                let turned = matches!(self.facing, Facing::Left | Facing::Right) && def.turns();
                def.hitbox = if turned { [d, w] } else { [w, d] };
            }
            Handle::Spot => {
                if let Some(s) = &mut def.use_spot {
                    let o = p - at;
                    s.offset = [snapped(o.x, step / 2.0), snapped(o.y, step / 2.0)];
                }
            }
            Handle::Reach => {
                let from = self.light_at(ws, def);
                if let Some(l) = PropStage::light_mut(def, self.state) {
                    l.radius = snapped(p.distance(from) / ts, if fine { 0.05 } else { 0.25 }).clamp(0.25, 30.0);
                }
            }
            Handle::LightAt => {
                if let Some(l) = PropStage::light_mut(def, self.state) {
                    let o = p - at;
                    l.offset = Some([snapped(o.x, step), snapped(o.y, step)]);
                }
            }
            // (A picture is moved by its own pixels: one at a time is the coarse step too.)
            Handle::Picture => def.picture_offset = [snapped(moved.x, if fine { 0.5 } else { 1.0 }).clamp(-8.0 * ts, 8.0 * ts), snapped(moved.y, if fine { 0.5 } else { 1.0 }).clamp(-8.0 * ts, 8.0 * ts)],
        }
    }

    /// A point of the ground as a place in the window.
    fn window_pos(&self, world: Vec2) -> Option<Pos2> {
        let s = self.look?.to_screen(world);
        Some(self.rect.min + egui::vec2(s.x, s.y) / self.ppp)
    }

    /// The script's line: `state=NAME facing=… dark=… hitbox=WxH [spot=X,Y] [light=R] HANDLE@X,Y…`
    /// (where each handle is in the window, for `drag`).
    pub fn status(&self, ws: &Workspace, id: &str) -> String {
        let Some(def) = ws.project.db.prop(id) else { return "-".into() };
        let mut out = format!("{id} state={} facing={:?} dark={} hitbox={}x{}", def.state_names().get(self.state).cloned().unwrap_or_default(), self.facing, self.dark, def.hitbox[0], def.hitbox[1]);
        if let Some(s) = &def.use_spot {
            out += &format!(" spot={},{}", s.offset[0], s.offset[1]);
        }
        if let Some(l) = PropStage::light_of(def, self.state) {
            out += &format!(" light={}", l.radius);
            if let Some([x, y]) = l.offset {
                out += &format!(" lightat={x},{y}");
            }
        }
        if def.picture_offset != [0.0, 0.0] {
            out += &format!(" picture={},{}", def.picture_offset[0], def.picture_offset[1]);
        }
        for h in self.handles(def) {
            let name = match h {
                Handle::Corner(e, n) => format!("{}{}", if n { "n" } else { "s" }, if e { "e" } else { "w" }),
                Handle::Spot => "spot".into(),
                Handle::Reach => "reach".into(),
                Handle::LightAt => "lightat".into(),
                Handle::Picture => "picture".into(),
            };
            if let Some(p) = self.handle_window(ws, def, h) {
                out += &format!(" {name}@{:.0},{:.0}", p.x, p.y);
            }
        }
        out
    }

    /// The script's words: `state NAME`, `turn`, `dark on|off`, `usual light|picture` (the
    /// light, the picture back where the engine puts it).
    pub fn word(&mut self, ws: &mut Workspace, id: &str, args: &[String]) {
        let Some(def) = ws.project.db.prop(id) else { return };
        let arg = |k: usize| args.get(k).map(String::as_str).unwrap_or("");
        match arg(0) {
            "state" => self.state = def.state_index(arg(1)).unwrap_or(0),
            "turn" => self.facing = quarter(self.facing),
            "dark" => self.dark = arg(1) != "off",
            "usual" => {
                let mut def = def.clone();
                match arg(1) {
                    "light" => {
                        if let Some(l) = PropStage::light_mut(&mut def, self.state) {
                            l.offset = None;
                        }
                    }
                    _ => def.picture_offset = [0.0, 0.0],
                }
                PropStage::put(ws, id, def);
            }
            _ => {}
        }
    }

    /// The prop changed in the workshop: a step of the history.
    fn put(ws: &mut Workspace, id: &str, def: PropDef) -> bool {
        let Some(pos) = ws.project.db.props.iter().position(|p| p.id == id) else { return false };
        if ws.project.db.props[pos] == def {
            return false;
        }
        ws.begin(&format!("Change the prop {}", if def.name.is_empty() { id } else { &def.name }));
        ws.project.db.props[pos] = def;
        ws.stale = true;
        true
    }

    /// The workshop. True when it changed the prop.
    pub fn show(&mut self, ui: &mut egui::Ui, ws: &mut Workspace, rs: Option<&RenderState>, thumbs: &mut crate::thumbs::Thumbs, id: &str) -> bool {
        let Some(pos) = ws.project.db.props.iter().position(|p| p.id == id) else { return false };
        let mut def = ws.project.db.props[pos].clone();
        let before = def.clone();
        let names = def.state_names();
        self.state = self.state.min(names.len() - 1);
        if !def.turns() {
            self.facing = Facing::Down;
        }
        // The states as a strip; the one picked is the one shown.
        ui.horizontal_wrapped(|ui| {
            ui.strong("Workshop");
            for (i, name) in names.iter().enumerate() {
                let (sprite, _) = def.state_look(i);
                let sprite = sprite.clone();
                ui.vertical(|ui| {
                    if let Some(rs) = rs {
                        crate::thumbs::boxed(ui, 40.0, || if i == 0 { thumbs.of(ws, rs, rustpg2d_engine::refactor::RefKind::Prop, id) } else { thumbs.sprite(ws, rs, &sprite) });
                    }
                    let r = ui.selectable_label(self.state == i, name.as_str()).on_hover_text("Shows the prop in this state");
                    if r.clicked() {
                        self.state = i;
                    }
                });
            }
            if ui.button("+ State").on_hover_text("Another state of the prop (open, lit, broken…): it starts as a copy of the first, with its own picture, light and whether it is in the way set below").clicked() {
                let mut n = def.states.len() + 2;
                while names.iter().any(|s| *s == format!("state{n}")) {
                    n += 1;
                }
                def.states.push(PropState { name: format!("state{n}"), sprite: def.sprite.clone(), solid: def.solid, light: None, model: String::new() });
                self.state = def.states.len();
            }
            ui.separator();
            if def.turns() && ui.button("Turn").on_hover_text("Shows it turned a quarter further, as R does for a placed one").clicked() {
                self.facing = quarter(self.facing);
            }
            if PropStage::light_of(&def, self.state).is_some() {
                ui.toggle_value(&mut self.dark, "In the dark").on_hover_text("Shows it at night, lit by its own light, from far enough away to see how far the light reaches");
            } else {
                self.dark = false;
            }
        });
        ui.weak(match (def.use_spot.is_some(), PropStage::light_of(&def, self.state).is_some()) {
            (false, false) => "Drag the corners of the hitbox: what is in the way of someone walking, and what E and a click reach. (Shift: pixel by pixel.)",
            (true, false) => "Drag the corners of the hitbox, and the ring to where someone using it sits, lies or stands. (Shift: pixel by pixel.)",
            (false, true) => "Drag the corners of the hitbox, and the point on the circle to how far the light reaches. (Shift: pixel by pixel.)",
            (true, true) => "Drag the corners of the hitbox, the ring to where someone using it is, and the point on the circle to how far the light reaches. (Shift: pixel by pixel.)",
        });
        // What was moved from where the engine puts it goes back with a click. (The row is
        // there before anything is moved: appearing during a drag, it would push the picture
        // down under the pointer.)
        let has_light = PropStage::light_of(&def, self.state).is_some();
        let light_moved = PropStage::light_of(&def, self.state).is_some_and(|l| l.offset.is_some());
        if has_light || self.pictured(&def) {
            ui.horizontal(|ui| {
                if self.pictured(&def) && ui.add_enabled(def.picture_offset != [0.0, 0.0], egui::Button::new("Picture back to its usual place").small()).on_hover_text(format!("It is moved {} pixels right and {} down from standing with the middle of its lower edge on the prop's place", def.picture_offset[0], def.picture_offset[1])).on_disabled_hover_text("Drag the cross at the picture's foot to move the picture on its hitbox").clicked() {
                    def.picture_offset = [0.0, 0.0];
                }
                if has_light && ui.add_enabled(light_moved, egui::Button::new("Light back to its usual place").small()).on_hover_text("North of the prop by two thirds of its picture's height").on_disabled_hover_text("Drag the dot in the ring to where the light sits").clicked() {
                    if let Some(l) = PropStage::light_mut(&mut def, self.state) {
                        l.offset = None;
                    }
                }
            });
        }
        // The picture.
        let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width().min(900.0), 300.0), Sense::hover());
        let ppp = ui.ctx().pixels_per_point();
        let size = ((rect.width() * ppp).round().max(16.0) as u32, (rect.height() * ppp).round().max(16.0) as u32);
        let look = match self.look {
            Some(l) if self.dragging && l.screen == vec2(size.0 as f32, size.1 as f32) => l,
            _ => self.camera(ws, &def, size),
        };
        (self.rect, self.look, self.ppp) = (rect, Some(look), ppp);
        ui.painter().rect_filled(rect, 4.0, Color32::from_rgb(32, 34, 40));
        self.draw(ws, rs, &def, id, look, size);
        if let Some((tex, _)) = self.texture {
            ui.painter().image(tex, rect, egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }
        let painter = ui.painter_at(rect);
        let on_window = |w: Vec2| {
            let s = look.to_screen(w);
            rect.min + egui::vec2(s.x, s.y) / ppp
        };
        let ts = ws.ts();
        let at = feet(ts);
        // The hitbox on the ground.
        let [w, d] = def.hitbox_facing(self.facing);
        let corners = [vec2(at.x - w / 2.0, at.y - d), vec2(at.x + w / 2.0, at.y - d), vec2(at.x + w / 2.0, at.y), vec2(at.x - w / 2.0, at.y)].map(on_window);
        let hitbox = if def.state_look(self.state).1 { Color32::from_rgb(255, 120, 90) } else { Color32::from_rgb(120, 220, 140) };
        painter.add(egui::Shape::closed_line(corners.to_vec(), Stroke::new(1.5_f32, hitbox)));
        // The light's reach.
        if let Some(l) = PropStage::light_of(&def, self.state) {
            let from = self.light_at(ws, &def);
            let ring: Vec<Pos2> = (0..64).map(|k| k as f32 / 64.0 * std::f32::consts::TAU).map(|a| on_window(from + vec2(a.cos(), a.sin()) * l.radius * ts)).collect();
            painter.add(egui::Shape::closed_line(ring, Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(l.color[0], l.color[1], l.color[2], 200))));
        }
        // The spot it is used from, with the way its user looks.
        if let Some(s) = &def.use_spot {
            let spot = at + Vec2::from(s.offset);
            let c = on_window(spot);
            let ahead = match s.facing {
                Facing::Down => vec2(0.0, 1.0),
                Facing::Up => vec2(0.0, -1.0),
                Facing::Left => vec2(-1.0, 0.0),
                Facing::Right => vec2(1.0, 0.0),
            };
            painter.circle_stroke(c, 9.0, Stroke::new(1.5_f32, Color32::from_rgb(120, 190, 255)));
            painter.line_segment([c, on_window(spot + ahead * ts * 0.6)], Stroke::new(1.5_f32, Color32::from_rgb(120, 190, 255)));
        }
        // The handles.
        let fine = ui.input(|i| i.modifiers.shift);
        let mut dragging = false;
        for h in self.handles(&def) {
            let Some(c) = self.handle_window(ws, &def, h) else { continue };
            let r = ui.interact(egui::Rect::from_center_size(c, egui::vec2(14.0, 14.0)), egui::Id::unique(("prop workshop", id, h.label())), Sense::drag());
            r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, h.label()));
            let color = match h {
                Handle::Corner(..) => hitbox,
                Handle::Spot => Color32::from_rgb(120, 190, 255),
                Handle::Reach | Handle::LightAt => Color32::from_rgb(255, 220, 150),
                Handle::Picture => Color32::from_rgb(210, 160, 255),
            };
            let color = if r.hovered() || r.dragged() { Color32::WHITE } else { color };
            match h {
                // (A dot in a ring for the light's middle, a cross for the picture's foot: they
                // may lie on a corner of the hitbox and must be told from it.)
                Handle::LightAt => {
                    painter.circle_filled(c, 3.0, color);
                    painter.circle_stroke(c, 6.0, Stroke::new(1.5_f32, color));
                }
                Handle::Picture => {
                    painter.line_segment([c - egui::vec2(6.0, 0.0), c + egui::vec2(6.0, 0.0)], Stroke::new(2.0_f32, color));
                    painter.line_segment([c - egui::vec2(0.0, 6.0), c + egui::vec2(0.0, 6.0)], Stroke::new(2.0_f32, color));
                }
                _ => {
                    painter.rect_filled(egui::Rect::from_center_size(c, egui::vec2(8.0, 8.0)), 2.0, color);
                }
            }
            if r.drag_started() {
                self.drags += 1;
            }
            if r.dragged() {
                dragging = true;
                if let Some(p) = r.interact_pointer_pos() {
                    let s = (p - rect.min) * ppp;
                    let moved = (p - on_window(at)) * ppp / look.zoom;
                    self.drag_to(ws, &mut def, h, look.to_world(vec2(s.x, s.y)), vec2(moved.x, moved.y), fine);
                }
            }
            let r = r.on_hover_cursor(egui::CursorIcon::Grab);
            r.on_hover_text(match h {
                Handle::Corner(..) => format!("{}: the hitbox is {} x {} pixels ({:.2} x {:.2} tiles)", h.label(), def.hitbox[0], def.hitbox[1], def.hitbox[0] / ts, def.hitbox[1] / ts),
                Handle::Spot => h.label().to_string(),
                Handle::Reach => format!("{}: {} tiles", h.label(), PropStage::light_of(&def, self.state).map_or(0.0, |l| l.radius)),
                Handle::LightAt => format!("{}: drag it to where the lamp's flame, the fire or the window is, on the ground", h.label()),
                Handle::Picture => format!("{}: drag the picture until it stands right on its hitbox ({} right, {} down)", h.label(), def.picture_offset[0], def.picture_offset[1]),
            });
        }
        painter.text(rect.left_bottom() + egui::vec2(8.0, -6.0), egui::Align2::LEFT_BOTTOM, format!("Hitbox {} x {} px", def.hitbox[0], def.hitbox[1]), egui::FontId::proportional(12.0), Color32::from_gray(170));
        self.dragging = dragging;
        if def == before {
            return false;
        }
        if dragging {
            // (A drag is one step of the history.)
            crate::shell::hold_step(ui.ctx(), ("prop workshop", id, self.drags));
        }
        ws.begin(&format!("Change the prop {}", if def.name.is_empty() { id } else { &def.name }));
        ws.project.db.props[pos] = def;
        ws.stale = true;
        true
    }

    /// Draws the stage with the game's renderer: a few tiles, the prop on them, at night its light.
    fn draw(&mut self, ws: &Workspace, rs: Option<&RenderState>, def: &PropDef, id: &str, look: Look, size: (u32, u32)) {
        let Some(rs) = rs else { return };
        if ws.gpu == editor_core::Gpu::No {
            return;
        }
        use gfx::prelude::*;
        let ts = ws.ts();
        let at = feet(ts);
        let mut map = Map::new("@prop", TILES, TILES);
        let mut e = EntityInstance::new(1, EntityKind::Prop, id, at.x, at.y);
        e.state = if self.state == 0 { String::new() } else { def.state_names()[self.state].clone() };
        e.facing = self.facing;
        map.entities.push(e);
        let scene = Scene { db: &ws.project.db, assets: &ws.assets, look };
        let mut screen = self.screen.take().unwrap_or_else(|| gfx::Screen::new(size.0, size.1, 1.0));
        screen.resize(size.0, size.1, 1.0);
        let mut input = std::mem::take(&mut self.input);
        let (dark, mut light_pass) = (self.dark, self.light_pass.take());
        gfx::with_screen(&mut screen, &mut input, || {
            let cam = look.ground();
            clear_background(Color::from_rgba(32, 34, 40, 255));
            set_camera(&cam);
            // The ground: tiles to measure by, the prop's own one a little lighter.
            for y in 0..TILES {
                for x in 0..TILES {
                    let own = x == TILES / 2 && y == TILES / 2;
                    let shade = if own { 74 } else if (x + y) % 2 == 0 { 58 } else { 52 };
                    draw_rectangle(x as f32 * ts, y as f32 * ts, ts, ts, Color::from_rgba(shade, shade + 6, shade, 255));
                }
            }
            scene.draw_objects(&map, look.visible(), false, WHITE, false, (true, true));
            if dark {
                if let Some(ambient) = render::lighting::ambient(MapLighting { mode: LightMode::Dark, darkness: 0.85 }, 0.0) {
                    light_pass.get_or_insert_with(LightPass::new).draw(&cam, ambient, &scene.lights(&map), ts, get_time());
                    set_camera(&cam);
                }
            }
        });
        (self.screen, self.input, self.light_pass) = (Some(screen), input, light_pass);
        // egui shows the picture; a new size is a new texture.
        if let Some(screen) = &self.screen {
            let view = screen.texture().view();
            match self.texture {
                Some((_, was)) if was == size => {}
                Some((tex, _)) => {
                    rs.renderer.write().update_egui_texture_from_wgpu_texture(&rs.device, view, eframe::wgpu::FilterMode::Linear, tex);
                    self.texture = Some((tex, size));
                }
                None => self.texture = Some((rs.renderer.write().register_native_texture(&rs.device, view, eframe::wgpu::FilterMode::Linear), size)),
            }
        }
    }
}

#[cfg(test)]
mod ui_tests {
    use super::*;
    use egui_kittest::kittest::Queryable;
    use egui_kittest::Harness;

    struct State {
        ws: Workspace,
        stage: PropStage,
        thumbs: crate::thumbs::Thumbs,
        dir: std::path::PathBuf,
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

    fn drag(h: &mut Harness<State>, label: &str, by: egui::Vec2) {
        let from = h.get_by_label(label).rect().center();
        h.hover_at(from);
        h.run();
        h.drag_at(from);
        h.run();
        for k in 1..=4 {
            h.hover_at(from + by * (k as f32 / 4.0));
            h.run();
        }
        h.drop_at(from + by);
        h.run();
        h.run();
    }

    #[test]
    fn a_props_hitbox_use_spot_and_light_are_dragged_on_its_picture() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir = std::env::temp_dir().join(format!("rustpg2d-propshop-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        copy(&root.join("example_project"), &dir);
        let mut ws = Workspace::new(dir.clone(), editor_core::Gpu::No).unwrap();
        ws.history_frame(None);
        let chair = ws.project.db.prop("chair").expect("the example has a chair").clone();
        assert!(chair.use_spot.is_some() && chair.light.is_none());
        let mut h = Harness::builder().with_size(egui::vec2(1000.0, 600.0)).build_ui_state(
            |ui, s: &mut State| {
                let held = ui.ctx().data_mut(|d| d.remove_temp::<u64>(crate::shell::held_step_id()));
                s.ws.history_frame(held.or(ui.ctx().memory(|m| m.focused()).map(|id| id.value())));
                s.stage.show(ui, &mut s.ws, None, &mut s.thumbs, "chair");
            },
            State { ws, stage: PropStage::default(), thumbs: Default::default(), dir },
        );
        h.run();
        // The north-east corner dragged east along the ground (two to the right, one down on the
        // screen): the hitbox gets wider on both sides and keeps its depth; one step.
        assert!(h.query_all_by_label("How far the light reaches").next().is_none());
        drag(&mut h, "The hitbox's north-east corner", egui::vec2(40.0, 20.0));
        let s = h.state();
        let now = s.ws.project.db.prop("chair").unwrap().hitbox;
        assert!(now[0] > chair.hitbox[0] + 8.0 && now[1] == chair.hitbox[1], "{:?} -> {now:?}", chair.hitbox);
        assert_eq!(now[0] % 4.0, 0.0, "in steps of four pixels");
        assert_eq!(s.ws.timeline.done.len(), 1, "{:?}", s.ws.timeline.done.iter().map(|c| &c.label).collect::<Vec<_>>());
        assert_eq!(s.ws.timeline.done[0].label, "Change the prop Chair");
        // The spot it is used from, dragged to the south (to the left and down on the screen).
        let was = chair.use_spot.as_ref().unwrap().offset;
        drag(&mut h, "The spot it is used from", egui::vec2(-30.0, 15.0));
        let spot = h.state().ws.project.db.prop("chair").unwrap().use_spot.as_ref().unwrap().offset;
        assert!(spot[1] > was[1] + 8.0 && (spot[0] - was[0]).abs() < 6.0, "{was:?} -> {spot:?}");
        assert_eq!(h.state().ws.timeline.done.len(), 2);
        // Another state: it starts as the first and is the one shown.
        h.get_by_label("+ State").click();
        h.run();
        h.run();
        let s = h.state();
        let def = s.ws.project.db.prop("chair").unwrap();
        assert_eq!(def.states.len(), chair.states.len() + 1);
        assert_eq!(s.stage.state, def.states.len());
        assert_eq!(def.states.last().unwrap().sprite, def.sprite);
        h.get_by_label(chair.default_state.as_str()).click();
        h.run();
        assert_eq!(h.state().stage.state, 0);
        // All of it undone.
        for _ in 0..3 {
            h.state_mut().ws.undo().unwrap();
        }
        assert_eq!(h.state().ws.project.db.prop("chair").unwrap(), &chair);
        let dir = h.state().dir.clone();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_lights_reach_is_dragged_and_the_camera_holds_still_meanwhile() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir = std::env::temp_dir().join(format!("rustpg2d-propshop-light-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        copy(&root.join("example_project"), &dir);
        let mut ws = Workspace::new(dir.clone(), editor_core::Gpu::No).unwrap();
        ws.history_frame(None);
        let fire = ws.project.db.prop("campfire").expect("the example has a campfire").clone();
        let lit = fire.state_index("lit").expect("with a state lit");
        let was = fire.state_light(lit).expect("that gives light").radius;
        let mut h = Harness::builder().with_size(egui::vec2(1000.0, 600.0)).build_ui_state(
            |ui, s: &mut State| {
                let held = ui.ctx().data_mut(|d| d.remove_temp::<u64>(crate::shell::held_step_id()));
                s.ws.history_frame(held.or(ui.ctx().memory(|m| m.focused()).map(|id| id.value())));
                s.stage.show(ui, &mut s.ws, None, &mut s.thumbs, "campfire");
            },
            State { ws, stage: PropStage { state: lit, dark: true, ..Default::default() }, thumbs: Default::default(), dir },
        );
        h.run();
        // Dragged outwards by a known stretch of the screen: the reach grows by what that is on
        // the ground at the zoom the drag began with (the picture doesn't zoom out under it).
        let zoom = h.state().stage.look.unwrap().zoom;
        drag(&mut h, "How far the light reaches", egui::vec2(60.0, 30.0));
        let s = h.state();
        let now = s.ws.project.db.prop("campfire").unwrap().state_light(lit).unwrap().radius;
        let east = 60.0 / (zoom / render::iso::UPRIGHT) / s.ws.ts();
        assert!(now > was && (now - was) < east * 1.5 + 0.5, "{was} -> {now} (a drag of {east:.2} tiles east)");
        assert_eq!(now % 0.25, 0.0, "in quarters of a tile");
        assert_eq!(s.ws.timeline.done.len(), 1);
        let dir = h.state().dir.clone();
        let _ = std::fs::remove_dir_all(dir);
    }
    #[test]
    fn a_lights_place_and_a_pictures_foot_are_dragged_and_put_back() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir = std::env::temp_dir().join(format!("rustpg2d-propshop-place-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        copy(&root.join("example_project"), &dir);
        let mut ws = Workspace::new(dir.clone(), editor_core::Gpu::No).unwrap();
        ws.history_frame(None);
        let lamp = ws.project.db.prop("street_lamp").expect("the example has a street lamp").clone();
        assert!(lamp.light.as_ref().is_some_and(|l| l.offset.is_none()) && lamp.picture_offset == [0.0, 0.0] && lamp.model.is_empty());
        let mut h = Harness::builder().with_size(egui::vec2(1000.0, 600.0)).build_ui_state(
            |ui, s: &mut State| {
                let held = ui.ctx().data_mut(|d| d.remove_temp::<u64>(crate::shell::held_step_id()));
                s.ws.history_frame(held.or(ui.ctx().memory(|m| m.focused()).map(|id| id.value())));
                s.stage.show(ui, &mut s.ws, None, &mut s.thumbs, "street_lamp");
            },
            State { ws, stage: PropStage::default(), thumbs: Default::default(), dir },
        );
        h.run();
        // The picture's foot dragged right and up by a stretch of the screen: the picture is
        // moved by that many of its own pixels, whole ones, as one step.
        let zoom = h.state().stage.look.unwrap().zoom;
        drag(&mut h, "Where the picture stands", egui::vec2(10.0 * zoom, -6.0 * zoom));
        let now = h.state().ws.project.db.prop("street_lamp").unwrap().picture_offset;
        assert!((now[0] - 10.0).abs() <= 1.0 && (now[1] + 6.0).abs() <= 1.0 && now[0].fract() == 0.0, "{now:?}");
        assert_eq!(h.state().ws.timeline.done.len(), 1);
        // The light's middle dragged on the ground: it has a place of its own now, in steps of
        // four pixels, and the point its reach is dragged by went with it.
        let reach = h.state().stage.handle_window(&h.state().ws, h.state().ws.project.db.prop("street_lamp").unwrap(), Handle::Reach).unwrap();
        drag(&mut h, "Where the light sits", egui::vec2(-40.0, -20.0));
        let s = h.state();
        let def = s.ws.project.db.prop("street_lamp").unwrap();
        let at = def.light.as_ref().unwrap().offset.expect("the light has its own place");
        assert!(at[0] % 4.0 == 0.0 && at[1] % 4.0 == 0.0, "{at:?}");
        assert_ne!(s.stage.handle_window(&s.ws, def, Handle::Reach).unwrap(), reach);
        assert_eq!(s.ws.timeline.done.len(), 2);
        // Each goes back to where the engine puts it with a click.
        h.get_by_label("Light back to its usual place").click();
        h.run();
        h.get_by_label("Picture back to its usual place").click();
        h.run();
        let def = h.state().ws.project.db.prop("street_lamp").unwrap();
        assert_eq!(def, &lamp, "as it was");
        let dir = h.state().dir.clone();
        let _ = std::fs::remove_dir_all(dir);
    }

}
