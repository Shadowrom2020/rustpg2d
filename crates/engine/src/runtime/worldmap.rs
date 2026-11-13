//! The map screen (M or the map button in the HUD): a picture of the current map with the
//! player and the exits to other maps. It pauses the game while open. Scroll with the arrow keys
//! or WASD, by dragging the map or the scrollbars, or with the mouse wheel (Shift: sideways);
//! the + and - keys zoom, Space centers on the player, M or Esc close. On maps with fog of war
//! only the explored tiles (`WorldState::explored`) are shown.

use macroquad::prelude::*;

use super::input::{self, Act};
use super::{ui::panel, Game, RtEntity};
use crate::entity::EntityKind;
use crate::map::{Edge, ZoneKind};
use crate::render;
use super::text::{draw_text, measure_text};

/// Zoom levels in screen pixels per tile.
const ZOOMS: [f32; 7] = [4.0, 6.0, 8.0, 12.0, 16.0, 24.0, 32.0];
/// The map opens at the largest zoom that shows all of it, but not smaller than this (larger
/// maps scroll instead of turning into a few pixels per tile).
const MIN_OPEN_ZOOM: f32 = 8.0;
const MAX_OPEN_ZOOM: f32 = 24.0;
/// The rendered picture is at most this many pixels wide / high.
const MAX_TEXTURE: f32 = 4096.0;
/// Keyboard scroll speed in screen pixels per second.
const KEY_SPEED: f32 = 700.0;
const WHEEL_STEP: f32 = 80.0;
const MARGIN: f32 = 24.0;
const HEAD: f32 = 52.0;
const FOOT: f32 = 34.0;
const BAR: f32 = 10.0;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Drag {
    /// Dragging the map: mouse start and view center at the start.
    Map(Vec2, Vec2),
    Horizontal,
    Vertical,
}

#[derive(Default)]
pub(super) struct WorldMap {
    open: bool,
    /// Rendered picture of the map and its pixels per tile; rebuilt every time the map opens.
    picture: Option<(RenderTarget, f32)>,
    redraw: bool,
    /// Screen pixels per tile.
    zoom: f32,
    /// Map position (in tiles) shown in the middle of the view.
    center: Vec2,
    drag: Option<Drag>,
    /// Fog of war: one pixel per tile, opaque where unexplored (`None` on maps without fog).
    fog: Option<Texture2D>,
}

/// Buttons in the map screen's header.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Button {
    Close,
    ZoomIn,
    ZoomOut,
    Center,
}

const BUTTONS: [Button; 4] = [Button::Close, Button::ZoomIn, Button::ZoomOut, Button::Center];

fn panel_rect() -> Rect {
    Rect::new(MARGIN, MARGIN, (screen_width() - 2.0 * MARGIN).max(200.0), (screen_height() - 2.0 * MARGIN).max(200.0))
}

/// The area the map is drawn in.
fn view_rect() -> Rect {
    let p = panel_rect();
    Rect::new(p.x + 12.0, p.y + HEAD, p.w - 24.0 - BAR - 4.0, p.h - HEAD - FOOT - BAR - 4.0)
}

fn button_rect(b: Button) -> Rect {
    let p = panel_rect();
    let k = BUTTONS.iter().position(|x| *x == b).unwrap_or(0) as f32;
    let w = if b == Button::Center { 92.0 } else { 30.0 };
    // Right to left: close, +, -, Center.
    let right = p.right() - 12.0 - k * 38.0 - if k >= 3.0 { 62.0 } else { 0.0 };
    Rect::new(right - w, p.y + 12.0, w, 30.0)
}

/// Scrollbar tracks below and right of the view.
fn bar_tracks() -> (Rect, Rect) {
    let v = view_rect();
    (Rect::new(v.x, v.bottom() + 4.0, v.w, BAR), Rect::new(v.right() + 4.0, v.y, BAR, v.h))
}

impl Game {
    pub(super) fn world_map_open(&self) -> bool {
        self.worldmap.open
    }

    /// Opens the map screen centered on the player (M, the HUD button).
    pub(super) fn open_world_map(&mut self) {
        let (mw, mh) = (self.loaded.map.width as f32, self.loaded.map.height as f32);
        let v = view_rect();
        let fit = (v.w / mw).min(v.h / mh);
        // Exactly fitting (+ / - then step through the zoom levels).
        let zoom = fit.floor().clamp(MIN_OPEN_ZOOM, MAX_OPEN_ZOOM);
        self.reveal_fog();
        let center = self.player.pos / self.ts();
        let wm = &mut self.worldmap;
        wm.open = true;
        wm.redraw = true;
        wm.zoom = zoom;
        wm.drag = None;
        wm.center = center;
        self.clamp_map_center();
        self.audio.play(&self.project.config.sfx.open_container);
    }

    /// Fog of war: marks the tiles around the player as explored (every frame on fog maps).
    pub(super) fn reveal_fog(&mut self) {
        let m = &self.loaded.map;
        if !m.fog || self.on_title() {
            return;
        }
        let (w, h) = (m.width, m.height);
        let p = self.player.pos / self.ts();
        let e = self.world.explored.entry(m.name.clone()).or_insert_with(|| crate::fog::Explored::new(w, h));
        if e.size() != (w, h) {
            *e = e.resized(w, h);
        }
        e.reveal(p.x, p.y - 0.3, crate::fog::REVEAL_RADIUS);
    }

    /// Middle (in tiles) of the explored part of `r` (in tiles); `None` if none of it is
    /// explored. On maps without fog of war: the middle of `r`.
    fn explored_center(&self, r: Rect) -> Option<Vec2> {
        if !self.loaded.map.fog {
            return Some(r.center());
        }
        let e = self.world.explored.get(&self.loaded.map.name)?;
        let (x0, y0) = (r.x.floor() as i32, r.y.floor() as i32);
        let (x1, y1) = ((r.x + r.w).ceil() as i32, (r.y + r.h).ceil() as i32);
        let (mut sum, mut n) = (Vec2::ZERO, 0.0);
        for y in y0..y1.max(y0 + 1) {
            for x in x0..x1.max(x0 + 1) {
                if e.get(x, y) {
                    sum += vec2(x as f32 + 0.5, y as f32 + 0.5);
                    n += 1.0;
                }
            }
        }
        (n > 0.0).then(|| sum / n)
    }

    /// The fog texture of the current map (`None` without fog of war).
    fn build_fog(&self) -> Option<Texture2D> {
        let m = &self.loaded.map;
        if !m.fog {
            return None;
        }
        let explored = self.world.explored.get(&m.name);
        let mut img = Image::gen_image_color(m.width as u16, m.height as u16, Color::new(0.0, 0.0, 0.0, 0.0));
        for y in 0..m.height {
            for x in 0..m.width {
                if explored.is_some_and(|e| e.get(x as i32, y as i32)) {
                    continue;
                }
                // Slightly mottled, like clouds on a map.
                let n = ((x.wrapping_mul(73856093) ^ y.wrapping_mul(19349663)) % 1000) as f32 / 1000.0;
                let v = 0.13 + n * 0.035;
                img.set_pixel(x, y, Color::new(v, v, v + 0.03, 1.0));
            }
        }
        let t = Texture2D::from_image(&img);
        // Soft edges between explored and unexplored tiles.
        t.set_filter(FilterMode::Linear);
        Some(t)
    }

    /// Keeps the view on the map; an axis that fits completely is centered.
    fn clamp_map_center(&mut self) {
        let (mw, mh) = (self.loaded.map.width as f32, self.loaded.map.height as f32);
        let v = view_rect();
        let wm = &mut self.worldmap;
        let axis = |c: f32, size: f32, view: f32| {
            let half = view / wm.zoom / 2.0;
            if size <= 2.0 * half {
                size / 2.0
            } else {
                c.clamp(half, size - half)
            }
        };
        wm.center = vec2(axis(wm.center.x, mw, v.w), axis(wm.center.y, mh, v.h));
    }

    fn zoom_map(&mut self, dir: i32) {
        let wm = &mut self.worldmap;
        // The opening zoom may lie between two levels: step to the next one in that direction.
        wm.zoom = if dir > 0 {
            ZOOMS.iter().copied().find(|z| *z > wm.zoom + 0.01).unwrap_or(ZOOMS[ZOOMS.len() - 1])
        } else {
            ZOOMS.iter().rev().copied().find(|z| *z < wm.zoom - 0.01).unwrap_or(ZOOMS[0])
        };
        self.clamp_map_center();
    }

    /// Screen position of a map tile in the map screen.
    fn map_to_screen(&self, tile: Vec2) -> Vec2 {
        let v = view_rect();
        v.center() + (tile - self.worldmap.center) * self.worldmap.zoom
    }

    /// Scrollbar thumbs (horizontal, vertical); `None` where everything is visible.
    fn bar_thumbs(&self) -> (Option<Rect>, Option<Rect>) {
        let (mw, mh) = (self.loaded.map.width as f32, self.loaded.map.height as f32);
        let v = view_rect();
        let (th, tv) = bar_tracks();
        let z = self.worldmap.zoom;
        let c = self.worldmap.center;
        let hor = (mw * z > v.w).then(|| {
            let len = th.w * v.w / (mw * z);
            let start = (c.x - v.w / z / 2.0) / mw * th.w;
            Rect::new(th.x + start, th.y, len, th.h)
        });
        let ver = (mh * z > v.h).then(|| {
            let len = tv.h * v.h / (mh * z);
            let start = (c.y - v.h / z / 2.0) / mh * tv.h;
            Rect::new(tv.x, tv.y + start, tv.w, len)
        });
        (hor, ver)
    }

    /// Handles M (open / close) and the open map screen. Returns true while the map screen is
    /// open (the game is paused then).
    pub(super) fn update_world_map(&mut self, dt: f32) -> bool {
        if !self.worldmap.open {
            let can_open = !self.on_title()
                && !self.runner.busy()
                && matches!(self.fade, super::Fade::None)
                && !self.ui.dialog_open()
                && !self.inventory_open()
                && self.game_over.is_none()
                && !self.aiming();
            if can_open && input::act_pressed(Act::Map) {
                self.open_world_map();
                return true;
            }
            return false;
        }
        let m = input::mouse_pos();
        let click = input::mouse_pressed(MouseButton::Left);
        if input::any_act_pressed(&[Act::Map, Act::Cancel]) || (click && button_rect(Button::Close).contains(m)) {
            self.worldmap.open = false;
            self.worldmap.drag = None;
            return true;
        }
        // Zoom: + / - keys (RB / LB) and buttons.
        if input::act_pressed(Act::ZoomIn) || (click && button_rect(Button::ZoomIn).contains(m)) {
            self.zoom_map(1);
        }
        if input::act_pressed(Act::ZoomOut) || (click && button_rect(Button::ZoomOut).contains(m)) {
            self.zoom_map(-1);
        }
        if input::act_pressed(Act::MapCenter) || (click && button_rect(Button::Center).contains(m)) {
            self.worldmap.center = self.player.pos / self.ts();
        }
        // Keyboard scrolling.
        let mut dir = Vec2::ZERO;
        if input::act_down(Act::Left) {
            dir.x -= 1.0;
        }
        if input::act_down(Act::Right) {
            dir.x += 1.0;
        }
        if input::act_down(Act::Up) {
            dir.y -= 1.0;
        }
        if input::act_down(Act::Down) {
            dir.y += 1.0;
        }
        let zoom = self.worldmap.zoom;
        self.worldmap.center += dir * KEY_SPEED * dt / zoom;
        // Mouse wheel: up / down, with Shift sideways.
        let wheel = input::wheel();
        if wheel != 0.0 {
            let step = -wheel.signum() * WHEEL_STEP / zoom;
            if input::act_down(Act::Run) {
                self.worldmap.center.x += step;
            } else {
                self.worldmap.center.y += step;
            }
        }
        // Dragging the map or a scrollbar.
        let (th, tv) = bar_tracks();
        if click {
            let (hor, ver) = self.bar_thumbs();
            self.worldmap.drag = if hor.is_some() && th.contains(m) {
                Some(Drag::Horizontal)
            } else if ver.is_some() && tv.contains(m) {
                Some(Drag::Vertical)
            } else if view_rect().contains(m) {
                Some(Drag::Map(m, self.worldmap.center))
            } else {
                None
            };
        }
        if !input::mouse_down(MouseButton::Left) {
            self.worldmap.drag = None;
        }
        let (mw, mh) = (self.loaded.map.width as f32, self.loaded.map.height as f32);
        match self.worldmap.drag {
            Some(Drag::Map(start, c)) => self.worldmap.center = c - (m - start) / zoom,
            // A scrollbar puts the thumb's middle under the mouse.
            Some(Drag::Horizontal) => self.worldmap.center.x = (m.x - th.x) / th.w * mw,
            Some(Drag::Vertical) => self.worldmap.center.y = (m.y - tv.y) / tv.h * mh,
            None => {}
        }
        self.clamp_map_center();
        true
    }

    /// Renders the current map (tiles, props, roofs) into the picture texture.
    fn render_map_picture(&mut self) {
        let ts = self.ts();
        let m = &self.loaded.map;
        let (mw, mh) = (m.width.max(1) as f32, m.height.max(1) as f32);
        // Pixels per tile: the tile size, halved until the picture fits. Only exact halvings
        // keep the samples inside each tile (other ratios pick up the neighbouring tile of the
        // tileset at tile edges: thin lines between rows).
        let mut ppt = ts;
        while ppt > 1.0 && mw.max(mh) * ppt > MAX_TEXTURE {
            ppt /= 2.0;
        }
        let (w, h) = ((mw * ppt) as u32, (mh * ppt) as u32);
        let rt = match self.worldmap.picture.take() {
            Some((rt, p)) if p == ppt && rt.texture.width() as u32 == w && rt.texture.height() as u32 == h => rt,
            _ => render_target(w, h),
        };
        let world = vec2(mw * ts, mh * ts);
        let cam = Camera2D { target: world / 2.0, zoom: vec2(2.0 / world.x, 2.0 / world.y), render_target: Some(rt.clone()), ..Default::default() };
        set_camera(&cam);
        let bg = m.background;
        clear_background(Color::from_rgba(bg[0], bg[1], bg[2], 255));
        let view = Rect::new(0.0, 0.0, world.x, world.y);
        let sets = self.assets.map_tilesets(m);
        for layer in m.layers.iter().filter(|l| l.visible && !l.above_entities) {
            render::draw_layer(m, layer, &sets, ts, view, WHITE);
        }
        // Props belong to the map's picture (houses' signs, trees, wells); people and loose
        // items don't.
        let mut props: Vec<&RtEntity> = self.loaded.entities.iter().filter(|e| e.active && e.kind == EntityKind::Prop).collect();
        props.sort_by(|a, b| a.body.pos.y.total_cmp(&b.body.pos.y));
        for e in props {
            if let Some(d) = self.project.db.prop(&e.def) {
                self.assets.draw_sprite(d.state_look(e.state).0, e.frame, e.body.pos, WHITE);
            }
        }
        for layer in m.layers.iter().filter(|l| l.visible && l.above_entities) {
            render::draw_layer(m, layer, &sets, ts, view, WHITE);
        }
        set_default_camera();
        self.worldmap.picture = Some((rt, ppt));
    }

    /// Exits to other maps: (rect in tiles, target map).
    fn map_exits(&self) -> Vec<(Rect, String)> {
        let m = &self.loaded.map;
        let mut out: Vec<(Rect, String)> = m
            .zones
            .iter()
            .filter_map(|z| match &z.kind {
                ZoneKind::Portal { target_map, .. } | ZoneKind::Transition { target_map, .. } => Some((z.pixel_rect(1.0), target_map.clone())),
                _ => None,
            })
            .collect();
        for e in Edge::ALL {
            if let (Some(link), Some(r)) = (m.edges.get(e), m.edges.strip_rect(e, m.width, m.height, 1.0)) {
                out.push((r, link.target_map.clone()));
            }
        }
        out
    }

    pub(super) fn draw_world_map(&mut self) {
        if !self.worldmap.open {
            return;
        }
        if self.worldmap.redraw || self.worldmap.picture.is_none() {
            self.render_map_picture();
            self.worldmap.fog = self.build_fog();
            self.worldmap.redraw = false;
        }
        let (sw, sh) = (screen_width(), screen_height());
        draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.6));
        let p = panel_rect();
        panel(p.x, p.y, p.w, p.h);
        let title = format!("Map - {}", self.map_title(&self.loaded.map.name));
        draw_text(&title, p.x + 16.0, p.y + 36.0, 30.0, GOLD);
        let mouse = input::mouse_pos();

        // The visible part of the picture.
        let v = view_rect();
        draw_rectangle(v.x, v.y, v.w, v.h, Color::new(0.0, 0.0, 0.0, 0.5));
        let (mw, mh) = (self.loaded.map.width as f32, self.loaded.map.height as f32);
        let zoom = self.worldmap.zoom;
        let origin = self.map_to_screen(Vec2::ZERO);
        let full = Rect::new(origin.x, origin.y, mw * zoom, mh * zoom);
        if let (Some((rt, ppt)), Some(vis)) = (&self.worldmap.picture, full.intersect(v)) {
            // Pixel art stays crisp when enlarged and smooth when shrunk.
            rt.texture.set_filter(if zoom >= *ppt { FilterMode::Nearest } else { FilterMode::Linear });
            let src = Rect::new((vis.x - origin.x) / zoom * ppt, (vis.y - origin.y) / zoom * ppt, vis.w / zoom * ppt, vis.h / zoom * ppt);
            draw_texture_ex(&rt.texture, vis.x, vis.y, WHITE, DrawTextureParams { dest_size: Some(vis.size()), source: Some(src), ..Default::default() });
            // Exits to other maps (under the fog: only their explored part shows).
            let exits: Vec<(Rect, String, Vec2)> = self.map_exits().into_iter().filter_map(|(r, t)| Some((r, t, self.explored_center(r)?))).collect();
            for (r, _, _) in &exits {
                let a = self.map_to_screen(vec2(r.x, r.y));
                let sr = Rect::new(a.x, a.y, (r.w * zoom).max(4.0), (r.h * zoom).max(4.0));
                if let Some(on) = sr.intersect(v) {
                    draw_rectangle(on.x, on.y, on.w, on.h, Color::new(1.0, 0.85, 0.3, 0.3));
                    draw_rectangle_lines(on.x, on.y, on.w, on.h, 2.0, GOLD);
                }
            }
            if let Some(fog) = &self.worldmap.fog {
                let src = Rect::new((vis.x - origin.x) / zoom, (vis.y - origin.y) / zoom, vis.w / zoom, vis.h / zoom);
                draw_texture_ex(fog, vis.x, vis.y, WHITE, DrawTextureParams { dest_size: Some(vis.size()), source: Some(src), ..Default::default() });
            }
            draw_rectangle_lines(vis.x, vis.y, vis.w, vis.h, 1.0, Color::new(0.85, 0.8, 0.6, 0.5));
            // Their labels: the map they lead to, at the explored part.
            for (_, target, c) in exits {
                let c = self.map_to_screen(c);
                if !v.contains(c) {
                    continue;
                }
                let label = format!("to {}", self.map_title(&target));
                let tw = measure_text(&label, None, 18, 1.0).width;
                let lx = (c.x - tw / 2.0).clamp(v.x + 2.0, v.right() - tw - 2.0);
                let ly = (c.y + 6.0).clamp(v.y + 16.0, v.bottom() - 4.0);
                draw_rectangle(lx - 4.0, ly - 15.0, tw + 8.0, 20.0, Color::new(0.0, 0.0, 0.0, 0.7));
                draw_text(&label, lx, ly, 18.0, WHITE);
            }
        }

        // Where the active quests lead.
        self.draw_quest_pins(v, |t| self.map_to_screen(t));

        // The player: a pulsing marker with an arrow pointing where they face.
        let pp = self.map_to_screen(self.player.pos / self.ts() - vec2(0.0, 0.3));
        if v.contains(pp) {
            let pulse = 1.0 + 0.25 * ((get_time() * 4.0).sin() as f32);
            draw_circle(pp.x, pp.y, 11.0 * pulse, Color::new(1.0, 0.3, 0.2, 0.35));
            draw_circle(pp.x, pp.y, 7.0, Color::new(0.95, 0.2, 0.15, 1.0));
            draw_circle_lines(pp.x, pp.y, 7.0, 2.0, WHITE);
            let d = match self.player.facing {
                crate::entity::Facing::Up => vec2(0.0, -1.0),
                crate::entity::Facing::Down => vec2(0.0, 1.0),
                crate::entity::Facing::Left => vec2(-1.0, 0.0),
                crate::entity::Facing::Right => vec2(1.0, 0.0),
            };
            let n = vec2(-d.y, d.x);
            draw_triangle(pp + d * 17.0, pp + d * 9.0 + n * 5.0, pp + d * 9.0 - n * 5.0, WHITE);
        } else {
            // Off screen: an arrow at the edge of the view pointing to the player.
            let c = v.center();
            let dir = (pp - c).normalize_or_zero();
            let t = ((v.w / 2.0 - 14.0) / dir.x.abs().max(1e-3)).min((v.h / 2.0 - 14.0) / dir.y.abs().max(1e-3));
            let tip = c + dir * t;
            let n = vec2(-dir.y, dir.x);
            draw_triangle(tip, tip - dir * 14.0 + n * 8.0, tip - dir * 14.0 - n * 8.0, Color::new(0.95, 0.2, 0.15, 1.0));
        }

        // Scrollbars.
        let (th, tv) = bar_tracks();
        let (hor, ver) = self.bar_thumbs();
        for (track, thumb, axis) in [(th, hor, Drag::Horizontal), (tv, ver, Drag::Vertical)] {
            let Some(t) = thumb else { continue };
            draw_rectangle(track.x, track.y, track.w, track.h, Color::new(1.0, 1.0, 1.0, 0.08));
            let hot = self.worldmap.drag == Some(axis) || track.contains(mouse);
            draw_rectangle(t.x, t.y, t.w, t.h, if hot { GOLD } else { Color::new(0.85, 0.8, 0.6, 0.7) });
        }

        // Header buttons.
        for b in BUTTONS {
            let r = button_rect(b);
            let hot = r.contains(mouse);
            draw_rectangle(r.x, r.y, r.w, r.h, Color::new(1.0, 1.0, 1.0, if hot { 0.18 } else { 0.06 }));
            draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.5, Color::new(0.85, 0.8, 0.6, 1.0));
            let c = r.center();
            match b {
                Button::Close => {
                    draw_line(c.x - 7.0, c.y - 7.0, c.x + 7.0, c.y + 7.0, 2.0, WHITE);
                    draw_line(c.x + 7.0, c.y - 7.0, c.x - 7.0, c.y + 7.0, 2.0, WHITE);
                }
                Button::ZoomIn | Button::ZoomOut => {
                    draw_line(c.x - 7.0, c.y, c.x + 7.0, c.y, 2.0, WHITE);
                    if b == Button::ZoomIn {
                        draw_line(c.x, c.y - 7.0, c.x, c.y + 7.0, 2.0, WHITE);
                    }
                }
                Button::Center => {
                    let tw = measure_text("Center", None, 20, 1.0).width;
                    draw_text("Center", c.x - tw / 2.0, c.y + 6.0, 20.0, WHITE);
                }
            }
        }
        super::glyphs::draw_hint(
            "Arrows / WASD, drag or wheel ({key:run}: sideways): scroll   {key:zoom_in} / {key:zoom_out}: zoom   {key:map_center}: center on you   {key:map} / {key:cancel}: close",
            p.x + 16.0,
            p.bottom() - 12.0,
            16.0,
            GRAY,
        );
    }

    /// A map's name for the player: "dark_forest" -> "Dark forest".
    fn map_title(&self, name: &str) -> String {
        let s = name.replace(['_', '-'], " ");
        let mut c = s.chars();
        c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
    }

    /// Screen position of a map screen button ("close", "zoomin", "zoomout", "center") for
    /// automated play-throughs; `None` while the map screen is closed.
    pub fn world_map_button_pos(&self, name: &str) -> Option<Vec2> {
        let b = match name {
            "close" => Button::Close,
            "zoomin" => Button::ZoomIn,
            "zoomout" => Button::ZoomOut,
            "center" => Button::Center,
            _ => return None,
        };
        self.worldmap.open.then(|| button_rect(b).center())
    }

    /// Map screen state for `Game::status()`.
    pub(super) fn world_map_status(&self) -> String {
        let wm = &self.worldmap;
        if !wm.open {
            return "closed".into();
        }
        let explored = self.world.explored.get(&self.loaded.map.name).map_or(0, |e| e.count());
        format!("open zoom={} center={:.1},{:.1} fog={} explored={explored}", wm.zoom, wm.center.x, wm.center.y, self.loaded.map.fog)
    }
}
