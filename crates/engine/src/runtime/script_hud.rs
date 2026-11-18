//! Drawing what scripts put on the screen (`crate::script_ui`): HUD elements (`WorldState::hud`,
//! drawn every frame with the HUD) and the script window (`window_open`: a modal panel of text,
//! pictures and buttons that run functions). Both use the skin (`runtime/skin.rs`).

use macroquad::prelude::*;

use super::input::{self, Act};
use super::skin;
use super::text::{draw_plain, measure_plain, role, Role};
use super::Game;
use crate::script_ui::{HudKind, ScriptWindow, Source, WindowRequest, WindowRow};

/// The open script window and its selected button.
pub(super) struct WindowUi {
    win: ScriptWindow,
    /// Index into the window's buttons.
    sel: usize,
}

const WIN_W: f32 = 520.0;
const ROW_H: f32 = 30.0;
const BUTTON_H: f32 = 36.0;
const TITLE_H: f32 = 56.0;
const HUD_TEXT: f32 = 20.0;

/// A colour name (the dialog codes' and the skin's) or `#rrggbb`; `None` for empty / unknown.
pub(super) fn color_of(s: &str) -> Option<Color> {
    (!s.trim().is_empty()).then(|| crate::dialog_codes::color(s)).flatten().map(|[r, g, b]| Color::from_rgba(r, g, b, 255))
}

/// A minimap's side in pixels when its element has no size.
const MINIMAP_SIZE: f32 = 160.0;

impl Game {
    /// Takes what scripts asked of the window (after they ran).
    pub(super) fn take_window_request(&mut self) {
        match self.world.window_request.take() {
            Some(WindowRequest::Open(win)) => {
                // The same window opened again (a refresh) keeps its selection.
                let sel = self.script_window.as_ref().filter(|w| w.win.id == win.id).map_or(0, |w| w.sel);
                let buttons = win.rows.iter().filter(|r| matches!(r, WindowRow::Button { .. })).count();
                self.script_window = Some(WindowUi { win, sel: sel.min(buttons.saturating_sub(1)) });
            }
            Some(WindowRequest::Close) => self.script_window = None,
            None => {}
        }
    }

    pub(super) fn script_window_open(&self) -> bool {
        self.script_window.is_some()
    }

    /// A bar's value or maximum now.
    fn source_value(&self, s: &str) -> f32 {
        let find = |name: &str| self.loaded.entities.iter().find(|e| e.name == name && e.max_hp > 0);
        match Source::parse(s) {
            Source::Number(n) => n,
            Source::Var(v) => self.world.var(&v) as f32,
            Source::Hp(n) if n == "player" => self.hp() as f32,
            Source::MaxHp(n) if n == "player" => self.player_max_hp() as f32,
            Source::Hp(n) => find(&n).filter(|e| e.active).map_or(0.0, |e| e.hp.max(0) as f32),
            Source::MaxHp(n) => find(&n).map_or(0.0, |e| e.max_hp as f32),
        }
    }

    /// The window's rects: the panel, then per row its rect (buttons included).
    fn window_layout(&self, w: &ScriptWindow) -> (Rect, Vec<Rect>) {
        let (sw, sh) = (screen_width(), screen_height());
        let heights: Vec<f32> = w
            .rows
            .iter()
            .map(|r| match r {
                WindowRow::Text(_) => ROW_H,
                WindowRow::Button { .. } => BUTTON_H + 6.0,
                WindowRow::Image { sheet, .. } => self.assets.sheets.get(sheet).map_or(ROW_H, |s| (s.frame_h / s.scale).min(160.0) * 2.0 + 8.0),
            })
            .collect();
        let h = TITLE_H + heights.iter().sum::<f32>() + 40.0;
        let p = Rect::new(((sw - WIN_W) / 2.0).round(), ((sh - h) / 2.0).max(16.0).round(), WIN_W, h);
        let mut y = p.y + TITLE_H;
        let rows = heights
            .iter()
            .map(|hh| {
                let r = Rect::new(p.x + 20.0, y, WIN_W - 40.0, *hh);
                y += hh;
                r
            })
            .collect();
        (p, rows)
    }

    /// Input of the open script window: Up / Down / the pointer choose a button, E / Enter / a
    /// click / A press it (its function runs; the window stays), Esc / B close it. Returns true
    /// while the window is open (it pauses the game).
    pub(super) fn update_script_window(&mut self) -> bool {
        let Some(ui) = &self.script_window else { return false };
        let win = ui.win.clone();
        let (p, rects) = self.window_layout(&win);
        let buttons: Vec<(usize, &WindowRow)> = win.rows.iter().enumerate().filter(|(_, r)| matches!(r, WindowRow::Button { .. })).collect();
        let n = buttons.len();
        let m = input::mouse_pos();
        let mut sel = ui.sel;
        if input::act_pressed(Act::Cancel) || (input::mouse_pressed(MouseButton::Left) && !p.contains(m)) {
            self.script_window = None;
            return true;
        }
        if n > 0 {
            if input::act_pressed(Act::Up) {
                sel = (sel + n - 1) % n;
            }
            if input::act_pressed(Act::Down) {
                sel = (sel + 1) % n;
            }
            let hovered = buttons.iter().position(|(k, _)| rects[*k].contains(m));
            if let Some(h) = hovered {
                if mouse_delta_position() != Vec2::ZERO {
                    sel = h;
                }
            }
            let press = if input::act_pressed(Act::Confirm) { Some(sel) } else if input::mouse_pressed(MouseButton::Left) { hovered } else { None };
            if let Some(ui) = &mut self.script_window {
                ui.sel = sel;
            }
            if let Some(k) = press {
                if let WindowRow::Button { call, .. } = buttons[k].1 {
                    if !call.is_empty() {
                        // (Run now, so a refreshed window shows at once; waiting actions stay queued.)
                        self.runner.push(&[crate::events::Action::RunScript { function: call.clone() }], &Default::default());
                        let mut runner = std::mem::take(&mut self.runner);
                        runner.update(self, 0.0);
                        let queued = std::mem::replace(&mut self.runner, runner);
                        self.runner.append(queued);
                        self.take_window_request();
                    }
                }
            }
        }
        true
    }

    /// The HUD elements scripts set and the enabled plugins bring (with the HUD: hidden with it).
    pub(super) fn draw_script_hud(&self) {
        if self.world.hud.is_empty() && self.plugin_hud.is_empty() {
            return;
        }
        let _font = role(Role::Hud);
        let (sw, sh) = (screen_width(), screen_height());
        for e in self.plugin_hud.values().chain(self.world.hud.values()) {
            match &e.kind {
                HudKind::Text { text } => {
                    let size = if e.size > 0.0 { e.size } else { HUD_TEXT };
                    let text = self.world.expand_text(text);
                    let d = measure_plain(&text, size);
                    let (x, y) = e.anchor.place(sw, sh, d.width, size, e.x, e.y);
                    let col = color_of(&e.color).unwrap_or_else(skin::normal);
                    draw_plain(&text, x + 1.0, y + size * 0.8 + 1.0, size, Color::new(0.0, 0.0, 0.0, 0.7));
                    draw_plain(&text, x, y + size * 0.8, size, col);
                }
                HudKind::Minimap { tiles } => {
                    let side = if e.size > 0.0 { e.size } else { MINIMAP_SIZE };
                    let (x, y) = e.anchor.place(sw, sh, side, side, e.x, e.y);
                    self.draw_minimap(Rect::new(x, y, side, side), *tiles);
                }
                HudKind::Image { sheet, frame } => {
                    let Some(s) = self.assets.sheets.get(sheet) else { continue };
                    let k = if e.size > 0.0 { e.size } else { 2.0 };
                    let (w, h) = (s.frame_w / s.scale * k, s.frame_h / s.scale * k);
                    let (x, y) = e.anchor.place(sw, sh, w, h, e.x, e.y);
                    self.assets.draw_frame_in(sheet, *frame, Rect::new(x, y, w, h), WHITE);
                }
                HudKind::Bar { value, max, w, label } => {
                    let h = 10.0;
                    let label_h = if label.is_empty() { 0.0 } else { 20.0 };
                    let (x, y) = e.anchor.place(sw, sh, *w, h + label_h, e.x, e.y);
                    if !label.is_empty() {
                        let l = crate::lang::tr(label);
                        let lw = measure_plain(&l, 18.0).width;
                        draw_plain(&l, x + (w - lw) / 2.0 + 1.0, y + 15.0, 18.0, Color::new(0.0, 0.0, 0.0, 0.7));
                        draw_plain(&l, x + (w - lw) / 2.0, y + 14.0, 18.0, skin::title());
                    }
                    let (v, m) = (self.source_value(value), self.source_value(max));
                    let frac = if m > 0.0 { v / m } else { 0.0 };
                    skin::color_bar(x, y + label_h, *w, h, frac, color_of(&e.color));
                }
            }
        }
    }

    /// The open script window (over everything else).
    pub(super) fn draw_script_window(&self) {
        let Some(ui) = &self.script_window else { return };
        let _font = role(Role::Ui);
        let (sw, sh) = (screen_width(), screen_height());
        draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.45));
        let (p, rects) = self.window_layout(&ui.win);
        skin::panel(p.x, p.y, p.w, p.h);
        let title = crate::lang::tr(&ui.win.title);
        let tw = measure_plain(&title, 30.0).width;
        draw_plain(&title, p.x + (p.w - tw) / 2.0, p.y + 38.0, 30.0, skin::title());
        let mut b = 0;
        for (row, r) in ui.win.rows.iter().zip(&rects) {
            match row {
                WindowRow::Text(t) => {
                    draw_plain(&self.world.expand_text(t), r.x, r.y + 21.0, 20.0, skin::normal());
                }
                WindowRow::Image { sheet, frame } => {
                    if let Some(s) = self.assets.sheets.get(sheet) {
                        let k = 2.0_f32.min(160.0 * 2.0 / (s.frame_h / s.scale)).max(0.25);
                        let (w, h) = (s.frame_w / s.scale * k, s.frame_h / s.scale * k);
                        self.assets.draw_frame_in(sheet, *frame, Rect::new(r.x, r.y + 4.0, w, h), WHITE);
                    }
                }
                WindowRow::Button { label, .. } => {
                    let br = Rect::new(r.x, r.y + 3.0, r.w, BUTTON_H);
                    let on = b == ui.sel;
                    if !skin::button_picture(br, on) {
                        draw_rectangle(br.x, br.y, br.w, br.h, skin::button_fill(on, Color::new(0.3, 0.25, 0.1, if on { 1.0 } else { 0.8 })));
                        draw_rectangle_lines(br.x, br.y, br.w, br.h, if on { 2.0 } else { 1.0 }, if on { skin::highlight() } else { skin::line(0.8) });
                    }
                    let l = self.world.expand_text(label);
                    let lw = measure_plain(&l, 22.0).width;
                    draw_plain(&l, br.x + (br.w - lw) / 2.0, br.y + 25.0, 22.0, if on { skin::highlight() } else { skin::normal() });
                    b += 1;
                }
            }
        }
        super::glyphs::draw_hint_in("{key:up}/{key:down} + {key:confirm} or click · {key:cancel}: close", p.x + 20.0, p.bottom() - 12.0, 16.0, p.w - 40.0, skin::disabled());
    }

    /// For `Game::status()`: `hud=[id:kind@anchor+x,y …]` (bars with value/max) and the open
    /// window (`id[rows]`, the selected button starting with `>`, or `-`).
    pub(super) fn script_ui_status(&self) -> (Vec<String>, String) {
        let hud = self
            .plugin_hud
            .iter()
            .chain(self.world.hud.iter())
            .map(|(id, e)| {
                let extra = match &e.kind {
                    HudKind::Bar { value, max, .. } => format!(":{}/{}", self.source_value(value), self.source_value(max)),
                    HudKind::Text { text } => format!(":{:?}", self.world.expand_text(text)),
                    HudKind::Image { sheet, frame } => format!(":{sheet}#{frame}"),
                    HudKind::Minimap { tiles } => format!(":{}x{tiles}", if e.size > 0.0 { e.size } else { MINIMAP_SIZE }),
                };
                format!("{id}:{}{extra}@{}+{},{}", e.kind_name(), e.anchor.name(), e.x, e.y)
            })
            .collect();
        let win = match &self.script_window {
            None => "-".to_string(),
            Some(ui) => {
                let mut b = 0;
                let rows: Vec<String> = ui
                    .win
                    .rows
                    .iter()
                    .map(|r| match r {
                        WindowRow::Text(t) => self.world.expand_text(t),
                        WindowRow::Image { sheet, .. } => format!("[{sheet}]"),
                        WindowRow::Button { label, .. } => {
                            let s = format!("{}<{}>", if b == ui.sel { ">" } else { "" }, self.world.expand_text(label));
                            b += 1;
                            s
                        }
                    })
                    .collect();
                format!("{}{:?}", ui.win.id, rows)
            }
        };
        (hud, win)
    }
}
