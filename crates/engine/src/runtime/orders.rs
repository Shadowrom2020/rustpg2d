//! The companions' orders menu (G / pad LB): a bar at the bottom that pauses the game, with the
//! companion to address (or all of them, when there are several) and the orders: Attack,
//! Defend, Passive (how they fight), Stay here, Follow me. A click on an order gives it at once;
//! keys: Up / Down between the rows, Left / Right in a row, E gives the order, Esc / G close.

use macroquad::prelude::*;

use super::input::{self, Act};
use super::Game;
use crate::state::Order;
use super::text::{draw_text, measure_text};

/// The open menu.
#[derive(Clone, Debug)]
pub(super) struct OrderMenu {
    /// 0 = all companions (when there are several), else companion `who - 1`.
    who: usize,
    /// The order chosen with the keys (index into `CHOICES`).
    sel: usize,
    /// Keys move in the companion row (true) or the orders row.
    on_who: bool,
    mouse: Vec2,
}

/// What the menu offers: a fighting order, or staying / following.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Choice {
    Fight(Order),
    Stay(bool),
    /// Opens the companion window (gear and bag).
    Gear,
}

const CHOICES: [Choice; 6] = [Choice::Fight(Order::Attack), Choice::Fight(Order::Defend), Choice::Fight(Order::Passive), Choice::Stay(true), Choice::Stay(false), Choice::Gear];

impl Choice {
    fn label(self) -> &'static str {
        match self {
            Choice::Fight(o) => o.label(),
            Choice::Stay(true) => "Stay here",
            Choice::Stay(false) => "Follow me",
            Choice::Gear => "Gear...",
        }
    }
}

/// Where the bar's parts are (screen pixels).
struct Layout {
    panel: Rect,
    /// The companion row (empty with a single companion).
    who: Vec<(String, Rect)>,
    orders: Vec<Rect>,
}

const HINT: &str = "Click an order or Left / Right + E   Esc: close";
/// The hint with several companions (the "who" row).
const HINT_MANY: &str = "Click an order or Left / Right + E   Up / Down: who   Esc: close";

impl Game {
    /// Companions that can take orders (on this map, not knocked down).
    fn order_targets(&self) -> Vec<usize> {
        (0..self.loaded.entities.len()).filter(|&i| self.loaded.entities[i].active && self.loaded.entities[i].companion.is_some() && !self.is_down(i)).collect()
    }

    pub(super) fn order_menu_open(&self) -> bool {
        self.order_menu.is_some()
    }

    /// G / LB while playing opens the menu (`allow`: no window, dialog or event running).
    pub(super) fn maybe_open_order_menu(&mut self, allow: bool) -> bool {
        if !allow || !input::act_pressed(Act::Command) {
            return false;
        }
        if self.order_targets().is_empty() {
            self.ui.toast(if self.healable_companions().is_empty() { "No companions to give orders to.".into() } else { "Your companions can't hear you now.".into() });
            return true;
        }
        let several = self.order_targets().len() > 1;
        // Start on the order the (first) companion has.
        let first = self.order_targets()[0];
        let cur = self.loaded.entities[first].companion.as_ref().map_or(Order::Attack, |l| l.order);
        let sel = CHOICES.iter().position(|c| *c == Choice::Fight(cur)).unwrap_or(0);
        self.order_menu = Some(OrderMenu { who: if several { 0 } else { 1 }, sel, on_who: false, mouse: input::mouse_pos() });
        true
    }

    /// The menu's input (it pauses the world). Returns true while it is open.
    pub(super) fn update_order_menu(&mut self) -> bool {
        let Some(m) = self.order_menu.clone() else { return false };
        let targets = self.order_targets();
        if targets.is_empty() {
            self.order_menu = None;
            return false;
        }
        if input::act_pressed(Act::Cancel) || input::act_pressed(Act::Command) || input::mouse_pressed(MouseButton::Right) {
            self.order_menu = None;
            return true;
        }
        let several = targets.len() > 1;
        let mut m = OrderMenu { who: m.who.min(targets.len()), ..m };
        if several && (input::act_pressed(Act::Up) || input::act_pressed(Act::Down)) {
            m.on_who = !m.on_who;
        }
        let step = if input::act_pressed(Act::Left) { -1 } else if input::act_pressed(Act::Right) { 1 } else { 0 };
        if step != 0 {
            if m.on_who {
                let n = targets.len() + 1;
                m.who = ((m.who as i32 + step).rem_euclid(n as i32)) as usize;
            } else {
                m.sel = ((m.sel as i32 + step).rem_euclid(CHOICES.len() as i32)) as usize;
            }
        }
        let mut give = input::act_pressed(Act::Confirm) && !m.on_who;
        if input::act_pressed(Act::Confirm) && m.on_who {
            m.on_who = false;
        }
        // The mouse: pointing selects, a click on an order gives it.
        let p = input::mouse_pos();
        let moved = p != m.mouse;
        let lay = self.order_layout(&m, &targets);
        let clicked = input::mouse_pressed(MouseButton::Left);
        if moved || clicked {
            if let Some(k) = lay.orders.iter().position(|r| r.contains(p)) {
                m.sel = k;
                m.on_who = false;
                give |= clicked;
            } else if let Some(k) = lay.who.iter().position(|(_, r)| r.contains(p)) {
                if clicked {
                    m.who = k;
                    m.on_who = true;
                }
            }
        }
        m.mouse = p;
        if give {
            let who: Vec<usize> = if m.who == 0 { targets.clone() } else { vec![targets[m.who - 1]] };
            let choice = CHOICES[m.sel];
            if choice == Choice::Gear {
                if who.len() == 1 {
                    self.order_menu = None;
                    self.open_companion_window(who[0]);
                } else {
                    self.ui.toast("Choose one companion for its gear.".into());
                    self.order_menu = Some(m);
                }
                return true;
            }
            for &i in &who {
                match choice {
                    Choice::Fight(o) => self.give_order(i, Some(o), None),
                    Choice::Stay(s) => self.give_order(i, None, Some(s)),
                    Choice::Gear => {}
                }
            }
            let name = if who.len() == 1 { self.entity_display_name(self.loaded.entities[who[0]].uid).unwrap_or_default() } else { "All of them".into() };
            let (one, many) = match choice {
                Choice::Fight(Order::Attack) => ("attacks enemies near you", "attack enemies near you"),
                Choice::Fight(Order::Defend) => ("only fights what attacks you", "only fight what attacks you"),
                Choice::Fight(Order::Passive) => ("stays out of fights", "stay out of fights"),
                Choice::Stay(true) => ("waits here", "wait here"),
                Choice::Stay(false) => ("follows you", "follow you"),
                Choice::Gear => ("", ""),
            };
            self.ui.toast(format!("{name} {}.", if who.len() == 1 { one } else { many }));
            self.order_menu = None;
            return true;
        }
        self.order_menu = Some(m);
        true
    }

    fn order_layout(&self, m: &OrderMenu, targets: &[usize]) -> Layout {
        let mut who = Vec::new();
        let font = 20.0;
        let bw = |t: &str| measure_text(t, None, font as u16, 1.0).width + 24.0;
        let orders_w: Vec<f32> = CHOICES.iter().map(|c| bw(c.label())).collect();
        let row = |ws: &[f32]| ws.iter().sum::<f32>() + 8.0 * (ws.len() as f32 - 1.0);
        let mut names = Vec::new();
        if targets.len() > 1 {
            names.push("All".to_string());
            for &i in targets {
                names.push(self.entity_display_name(self.loaded.entities[i].uid).unwrap_or_else(|| self.loaded.entities[i].name.clone()));
            }
        }
        let who_w: Vec<f32> = names.iter().map(|n| bw(n)).collect();
        let w = row(&orders_w).max(if names.is_empty() { 0.0 } else { row(&who_w) }).max(measure_text(HINT_MANY, None, 16, 1.0).width).max(measure_text(self.order_title(m, targets).as_str(), None, 22, 1.0).width) + 32.0;
        let h = if names.is_empty() { 96.0 } else { 132.0 };
        let panel = Rect::new((screen_width() - w) / 2.0, screen_height() - h - 24.0, w, h);
        let mut y = panel.y + 36.0;
        if !names.is_empty() {
            let mut x = panel.x + (w - row(&who_w)) / 2.0;
            for (n, bw) in names.into_iter().zip(&who_w) {
                who.push((n, Rect::new(x, y, *bw, 28.0)));
                x += bw + 8.0;
            }
            y += 36.0;
        }
        let mut x = panel.x + (w - row(&orders_w)) / 2.0;
        let orders = orders_w
            .iter()
            .map(|bw| {
                let r = Rect::new(x, y, *bw, 28.0);
                x += bw + 8.0;
                r
            })
            .collect();
        Layout { panel, who, orders }
    }

    fn order_title(&self, m: &OrderMenu, targets: &[usize]) -> String {
        if m.who == 0 {
            "Orders for everyone".into()
        } else {
            let i = targets[(m.who - 1).min(targets.len() - 1)];
            format!("Orders for {}", self.entity_display_name(self.loaded.entities[i].uid).unwrap_or_else(|| self.loaded.entities[i].name.clone()))
        }
    }

    /// The menu bar (after the HUD and windows).
    pub(super) fn draw_order_menu(&self) {
        let Some(m) = &self.order_menu else { return };
        let targets = self.order_targets();
        if targets.is_empty() {
            return;
        }
        let lay = self.order_layout(m, &targets);
        let p = lay.panel;
        let gold = Color::new(0.85, 0.72, 0.38, 1.0);
        draw_rectangle(p.x, p.y, p.w, p.h, Color::new(0.05, 0.05, 0.1, 0.88));
        draw_rectangle_lines(p.x, p.y, p.w, p.h, 2.5, gold);
        let title = self.order_title(m, &targets);
        let tw = measure_text(&title, None, 22, 1.0).width;
        draw_text(&title, p.x + (p.w - tw) / 2.0, p.y + 26.0, 22.0, Color::new(1.0, 0.86, 0.45, 1.0));
        // What the addressed companion(s) do now: their order and staying are marked.
        let addressed: Vec<usize> = if m.who == 0 { targets.clone() } else { vec![targets[m.who - 1]] };
        let has = |c: Choice| {
            addressed.iter().all(|&i| {
                let Some(l) = &self.loaded.entities[i].companion else { return false };
                match c {
                    Choice::Fight(o) => l.order == o,
                    Choice::Stay(s) => l.post.is_some() == s,
                    Choice::Gear => false,
                }
            })
        };
        let button = |r: &Rect, text: &str, lit: bool, focus: bool, current: bool| {
            draw_rectangle(r.x, r.y, r.w, r.h, if lit { Color::new(0.2, 0.45, 0.25, 0.95) } else { Color::new(0.15, 0.15, 0.2, 0.9) });
            if current {
                draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.5, gold);
            }
            if lit && focus {
                draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, Color::new(0.5, 1.0, 0.55, 1.0));
            }
            draw_text(text, r.x + 12.0, r.y + 20.0, 20.0, if current { Color::new(1.0, 0.86, 0.45, 1.0) } else { WHITE });
        };
        for (k, (n, r)) in lay.who.iter().enumerate() {
            button(r, n, k == m.who, m.on_who, false);
        }
        for (k, r) in lay.orders.iter().enumerate() {
            button(r, CHOICES[k].label(), k == m.sel, !m.on_who, has(CHOICES[k]));
        }
        let hint = if lay.who.is_empty() { HINT } else { HINT_MANY };
        let hw = measure_text(hint, None, 16, 1.0).width;
        draw_text(hint, p.x + (p.w - hw) / 2.0, p.bottom() - 10.0, 16.0, LIGHTGRAY);
    }

    /// `status()`: the open menu's choice (`orders=<who>:<order>`), else `-`.
    pub(super) fn order_menu_status(&self) -> String {
        let Some(m) = &self.order_menu else { return "-".into() };
        let who = if m.who == 0 { "all".to_string() } else { self.order_targets().get(m.who - 1).map_or("?".into(), |&i| self.loaded.entities[i].name.clone()) };
        format!("{who}:{}", CHOICES[m.sel].label().to_lowercase().replace(' ', "_"))
    }
}
