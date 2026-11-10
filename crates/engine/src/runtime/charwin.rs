//! The character window: one window with tabs for the character sheet + inventory, the skills
//! and the spells (opened by the HUD button or C / I, K, L; Tab cycles the tabs). The tabs' contents
//! are drawn by `inventory_ui.rs` and `skills.rs` inside the rect from [`Game::window_content`].

use macroquad::prelude::*;

use super::inventory_ui::Side;
use super::input::{self, Act};
use super::{ui::panel, Game};

/// Height of the tab strip above the content.
const TAB_H: f32 = 34.0;
const TAB_W: f32 = 170.0;
/// Inner margin of the content panel.
pub(super) const MARGIN: f32 = 10.0;
/// Space kept free at the bottom for the quick bar.
const QUICKBAR_SPACE: f32 = 64.0;
/// The skills / spells tabs need at least this much room.
const MIN_W: f32 = 820.0;
const MIN_H: f32 = 520.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum WinTab {
    Character,
    Skills,
    Spells,
}

impl WinTab {
    fn label(self) -> &'static str {
        match self {
            WinTab::Character => "Character ({key:character})",
            WinTab::Skills => "Skills ({key:skills})",
            WinTab::Spells => "Spells ({key:spells})",
        }
    }
    fn parse(s: &str) -> Option<WinTab> {
        Some(match s {
            "character" | "inventory" | "bag" => WinTab::Character,
            "skills" => WinTab::Skills,
            "spells" | "magic" => WinTab::Spells,
            _ => return None,
        })
    }
}

impl Game {
    /// The tabs this game has (skills and spells only when the project enables them).
    fn win_tabs(&self) -> Vec<WinTab> {
        let mut v = vec![WinTab::Character];
        if self.skills_on() {
            v.push(WinTab::Skills);
        }
        if self.magic_on() {
            v.push(WinTab::Spells);
        }
        v
    }

    /// The open tab, if the window is open (a container's loot window is not part of it).
    pub(super) fn win_tab(&self) -> Option<WinTab> {
        if self.skills_open() {
            Some(if self.skills_tab() == 0 { WinTab::Skills } else { WinTab::Spells })
        } else if self.inv.is_open() && !self.looting() {
            Some(WinTab::Character)
        } else {
            None
        }
    }

    /// Opens the window at tab `t` (switching from another tab).
    pub(super) fn open_tab(&mut self, t: WinTab) {
        match t {
            WinTab::Character => {
                self.close_skills();
                self.open_inventory(Side::Player);
            }
            WinTab::Skills | WinTab::Spells => {
                self.inv.close();
                self.show_skills(if t == WinTab::Skills { 0 } else { 1 });
            }
        }
    }

    pub(super) fn close_window(&mut self) {
        self.close_skills();
        if !self.looting() {
            self.inv.close();
        }
    }

    /// The window's content area (below the tabs): big enough for every tab, centered in the
    /// space above the quick bar.
    pub(super) fn window_content(&self) -> Rect {
        let (sw, sh) = (screen_width(), screen_height());
        let inv = self.character_content_size();
        let w = (inv.x + 2.0 * MARGIN).max(MIN_W);
        let h = (inv.y + 2.0 * MARGIN).max(MIN_H);
        let top = ((sh - QUICKBAR_SPACE - TAB_H - h) / 2.0).max(4.0) + TAB_H;
        Rect::new(((sw - w) / 2.0).max(4.0).round(), top.round(), w, h)
    }

    /// The close button (right end of the tab strip).
    pub(super) fn window_close(&self) -> Rect {
        let c = self.window_content();
        Rect::new(c.x + c.w - 28.0, c.y - TAB_H + 6.0, 24.0, 24.0)
    }

    /// The whole window (tabs + content).
    pub(super) fn window_rect(&self) -> Rect {
        let c = self.window_content();
        Rect::new(c.x, c.y - TAB_H, c.w, c.h + TAB_H)
    }

    fn tab_rects(&self) -> Vec<(WinTab, Rect)> {
        let c = self.window_content();
        self.win_tabs().into_iter().enumerate().map(|(k, t)| (t, Rect::new(c.x + k as f32 * (TAB_W + 4.0), c.y - TAB_H, TAB_W, TAB_H))).collect()
    }

    /// Screen center of a tab (automated play-throughs): "character", "skills", "spells".
    pub fn window_tab_pos(&self, name: &str) -> Option<Vec2> {
        // The open journal's tabs (quests / bestiary).
        if self.journal_open() {
            return self.journal_tab_pos(name);
        }
        let t = WinTab::parse(name)?;
        self.tab_rects().into_iter().find(|(x, _)| *x == t).map(|(_, r)| r.center())
    }

    /// Keys C / I (character), K (skills), L (spells) and Tab open the window, switch tabs or -
    /// the key of the open tab - close it; a click on a tab switches to it. Returns (key
    /// consumed, mouse consumed).
    pub(super) fn update_window_tabs(&mut self, allow: bool, key_consumed: bool) -> (bool, bool) {
        if !allow || self.looting() || self.shop_open() || self.journal_open() {
            return (false, false);
        }
        let open = self.win_tab();
        let tabs = self.win_tabs();
        if !key_consumed {
            let mut want = None;
            // Tab / RB: next tab, LB: previous one (both open the window when it is closed).
            let step = if input::act_pressed(Act::NextTab) {
                1
            } else if open.is_some() && input::act_pressed(Act::PrevTab) {
                tabs.len() - 1
            } else {
                0
            };
            if input::act_pressed(Act::Character) {
                want = Some(WinTab::Character);
            } else if input::act_pressed(Act::Skills) && tabs.contains(&WinTab::Skills) {
                want = Some(WinTab::Skills);
            } else if input::act_pressed(Act::Spells) && tabs.contains(&WinTab::Spells) {
                want = Some(WinTab::Spells);
            } else if step > 0 {
                want = Some(match open {
                    None => WinTab::Character,
                    Some(t) => tabs[(tabs.iter().position(|x| *x == t).unwrap_or(0) + step) % tabs.len()],
                });
            }
            if let Some(t) = want {
                if open == Some(t) && step == 0 {
                    self.close_window();
                } else {
                    self.open_tab(t);
                    // C opens the character sheet at the equipment, I at the inventory grid.
                    if t == WinTab::Character && input::pressed(KeyCode::C) {
                        self.inv_select(Side::Equip, 0);
                    }
                }
                return (true, false);
            }
        }
        if open.is_some() && input::mouse_pressed(MouseButton::Left) {
            let m = input::mouse_pos();
            if let Some((t, _)) = self.tab_rects().into_iter().find(|(_, r)| r.contains(m)) {
                if open != Some(t) {
                    self.open_tab(t);
                }
                return (true, true);
            }
            if self.window_close().contains(m) {
                self.close_window();
                return (true, true);
            }
        }
        (false, false)
    }

    /// The window's frame: dimmed background, the content panel and the tabs (the open one
    /// joined to the panel), the close button.
    pub(super) fn draw_window_frame(&self) {
        let Some(open) = self.win_tab() else { return };
        let m = input::mouse_pos();
        draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(0.0, 0.0, 0.0, 0.35));
        let c = self.window_content();
        panel(c.x, c.y, c.w, c.h);
        let border = Color::new(0.85, 0.8, 0.6, 1.0);
        for (t, r) in self.tab_rects() {
            let on = t == open;
            let fill = if on {
                Color::new(0.06, 0.06, 0.12, 0.97)
            } else if r.contains(m) {
                Color::new(0.16, 0.15, 0.22, 0.95)
            } else {
                Color::new(0.04, 0.04, 0.08, 0.85)
            };
            draw_rectangle(r.x, r.y, r.w, r.h, fill);
            draw_rectangle_lines(r.x, r.y, r.w, r.h, if on { 3.0 } else { 1.5 }, if on { border } else { Color::new(0.85, 0.8, 0.6, 0.5) });
            if on {
                // Open tab: no line between it and the panel.
                draw_rectangle(r.x + 3.0, r.y + r.h - 3.0, r.w - 6.0, 6.0, Color::new(0.06, 0.06, 0.12, 1.0));
            }
            let label = t.label();
            let tw = super::glyphs::measure_hint(label, 22.0);
            super::glyphs::draw_hint(label, r.x + (r.w - tw) / 2.0, r.y + 23.0, 22.0, if on { GOLD } else { LIGHTGRAY });
        }
        let x = self.window_close();
        let hover = x.contains(m);
        draw_rectangle(x.x, x.y, x.w, x.h, Color::new(0.06, 0.06, 0.12, if hover { 1.0 } else { 0.8 }));
        draw_rectangle_lines(x.x, x.y, x.w, x.h, 1.5, border);
        let col = if hover { WHITE } else { LIGHTGRAY };
        draw_line(x.x + 6.0, x.y + 6.0, x.x + x.w - 6.0, x.y + x.h - 6.0, 2.0, col);
        draw_line(x.x + x.w - 6.0, x.y + 6.0, x.x + 6.0, x.y + x.h - 6.0, 2.0, col);
    }

    /// The window state for `Game::status()`: "tab" or "closed".
    pub(super) fn window_status(&self) -> String {
        self.win_tab().map_or("closed".into(), |t| format!("{t:?}").to_lowercase())
    }
}
