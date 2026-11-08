//! Crafting window (*Open crafting* action / `open_crafting(station)`): the station's recipes,
//! one per row with the result, the ingredients (what the player has of each) and a *Make*
//! button, greyed when something is missing.
//!
//! Click a row to select it, click *Make* (or double-click the row, or press Enter / E) to make
//! one; hold Shift for up to ten. Up / Down or the mouse wheel select, Esc closes.

use macroquad::prelude::*;

use super::input::{self, Act};
use super::{ui::panel, Game};
use crate::crafting::{Missing, RecipeDef};
use crate::render::wrap_text;
use super::text::{draw_text, measure_text};

const ROW: f32 = 58.0;
const ROWS: usize = 7;
const W: f32 = 640.0;
const PAD: f32 = 14.0;
const TITLE_H: f32 = 44.0;
const FOOT_H: f32 = 84.0;
const BTN_W: f32 = 92.0;
const DOUBLE_CLICK: f64 = 0.35;

#[derive(Default)]
pub(super) struct CraftingUi {
    /// Open station and the window's title.
    open: Option<(String, String)>,
    /// Station to open once the current event sequence has finished.
    pub pending: Option<(String, String)>,
    /// The station's prop (the trigger's owner; `RecipeDef::station_state` looks at it).
    prop: Option<u32>,
    pending_prop: Option<u32>,
    sel: usize,
    scroll: usize,
    last_click: Option<(usize, f64)>,
    /// Result of the last attempt (shown in the footer) and when it happened.
    msg: Option<(String, bool, f64)>,
}

impl CraftingUi {
    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }
    pub fn close(&mut self) {
        self.open = None;
        self.pending = None;
    }
}

struct Layout {
    panel: Rect,
    list: Rect,
    close: Rect,
    foot: Rect,
}

fn layout() -> Layout {
    let (sw, sh) = (screen_width(), screen_height());
    let h = TITLE_H + ROWS as f32 * ROW + FOOT_H + 3.0 * PAD;
    let x = ((sw - W) / 2.0).max(4.0).round();
    let y = ((sh - h) / 2.0).max(4.0).round();
    Layout {
        panel: Rect::new(x, y, W, h),
        list: Rect::new(x + PAD, y + TITLE_H + PAD, W - 2.0 * PAD, ROWS as f32 * ROW),
        close: Rect::new(x + W - 30.0, y + 8.0, 22.0, 22.0),
        foot: Rect::new(x + PAD, y + TITLE_H + 2.0 * PAD + ROWS as f32 * ROW, W - 2.0 * PAD, FOOT_H),
    }
}

fn row_rect(list: Rect, k: usize) -> Rect {
    Rect::new(list.x, list.y + k as f32 * ROW, list.w, ROW - 4.0)
}

fn button_rect(row: Rect) -> Rect {
    Rect::new(row.x + row.w - BTN_W - 6.0, row.y + 10.0, BTN_W, row.h - 20.0)
}

impl Game {
    pub(super) fn crafting_open(&self) -> bool {
        self.crafting.is_open()
    }

    /// Queues the crafting window of `station` (opens once the current events have finished).
    pub(super) fn open_crafting(&mut self, station: &str, title: &str, owner: Option<u32>) {
        let title = if title.is_empty() { "Crafting".to_string() } else { title.to_string() };
        self.crafting.pending = Some((station.to_string(), title));
        self.crafting.pending_prop = owner;
    }

    /// What is missing to make recipe `r` here: the station's state, then skill, tools, items.
    fn recipe_missing(&self, r: &RecipeDef) -> Option<Missing> {
        if !r.station_state.is_empty() {
            let state = self.crafting.prop.and_then(|uid| self.loaded.entities.iter().find(|e| e.uid == uid)).and_then(|e| self.project.db.prop(&e.def).map(|d| d.state_name(e.state).to_string()));
            if state.as_deref() != Some(r.station_state.as_str()) {
                return Some(Missing::StationState(r.station_state.clone()));
            }
        }
        r.missing(&self.world, &self.project.db.tool_kinds())
    }

    fn crafting_recipes(&self) -> Vec<RecipeDef> {
        let Some((station, _)) = &self.crafting.open else { return Vec::new() };
        self.project.db.recipes_at(station).filter(|r| !r.result.is_empty()).cloned().collect()
    }

    fn crafting_msg(&mut self, text: String, ok: bool) {
        self.crafting.msg = Some((text, ok, get_time()));
    }

    /// What a missing requirement reads like.
    fn missing_text(&self, m: &Missing) -> String {
        match m {
            Missing::Items(list) => {
                let names: Vec<String> = list.iter().map(|(item, have, need)| format!("{} {have}/{need}", self.item_name(item))).collect();
                format!("Missing: {}", names.join(", "))
            }
            Missing::Skill(skill, _, need) => {
                let name = self.project.db.skill(skill).map_or(skill.clone(), |d| d.name.clone());
                format!("Needs {name} {need}")
            }
            Missing::Tools(kinds) => format!("Needs a {}", kinds.join(" and a ")),
            Missing::StationState(state) => format!("Needs it {state}"),
        }
    }

    /// Makes up to `n` of recipe row `k`. Returns how many were made.
    fn craft_row(&mut self, k: usize, n: u32) -> u32 {
        let Some(r) = self.crafting_recipes().into_iter().nth(k) else { return 0 };
        let mut made = 0;
        while made < n {
            if let Some(m) = self.recipe_missing(&r) {
                if made == 0 {
                    let text = self.missing_text(&m);
                    self.crafting_msg(text, false);
                }
                break;
            }
            if !r.fits(&self.world) {
                if made == 0 {
                    self.crafting_msg("Your inventory is full.".into(), false);
                }
                break;
            }
            if r.craft(&mut self.world, &self.project.db.tool_kinds()).is_err() {
                break;
            }
            made += 1;
        }
        if made > 0 {
            self.audio.play(&self.project.config.sfx.pickup);
            // (The quality the skill reached when it was made.)
            let name = self.item_name(r.result_for(&self.world)).to_string();
            let total = made * r.count.max(1);
            self.crafting_msg(if total > 1 { format!("Made {total} {name}") } else { format!("Made {name}") }, true);
        }
        made
    }

    fn clamp_crafting(&mut self) {
        let n = self.crafting_recipes().len();
        let c = &mut self.crafting;
        c.sel = c.sel.min(n.saturating_sub(1));
        c.scroll = c.scroll.min(n.saturating_sub(ROWS));
        if c.sel < c.scroll {
            c.scroll = c.sel;
        } else if c.sel >= c.scroll + ROWS {
            c.scroll = c.sel + 1 - ROWS;
        }
    }

    /// The open crafting window (for automated play-throughs): "station [ids] sel=K msg=…".
    pub(super) fn crafting_status(&self) -> Option<String> {
        let (station, _) = self.crafting.open.as_ref()?;
        let ids: Vec<String> = self.crafting_recipes().iter().map(|r| if self.recipe_missing(r).is_none() { r.id.clone() } else { format!("{}(missing)", r.id) }).collect();
        let msg = self.crafting.msg.as_ref().map_or("", |m| m.0.as_str());
        Some(format!("{station} [{}] sel={} msg={msg:?}", ids.join(" "), self.crafting.sel))
    }

    /// Screen position of recipe row `k`'s *Make* button (for automated play-throughs).
    pub fn crafting_button_pos(&self, k: usize) -> Option<Vec2> {
        let vis = k.checked_sub(self.crafting.scroll).filter(|v| *v < ROWS)?;
        (k < self.crafting_recipes().len()).then(|| button_rect(row_rect(layout().list, vis)).center())
    }

    /// Handles the crafting window. Returns (key consumed, mouse consumed).
    pub(super) fn update_crafting_ui(&mut self, allow: bool) -> (bool, bool) {
        if !allow {
            return (false, false);
        }
        if let Some(open) = self.crafting.pending.take() {
            self.inv.close();
            self.crafting.open = Some(open);
            self.crafting.prop = self.crafting.pending_prop.take();
            self.crafting.sel = 0;
            self.crafting.scroll = 0;
            self.crafting.msg = None;
            self.audio.play(&self.project.config.sfx.open_container);
        }
        if !self.crafting.is_open() {
            return (false, false);
        }
        let l = layout();
        let m = input::mouse_pos();
        if input::any_act_pressed(&[Act::Cancel, Act::Character]) || (input::mouse_pressed(MouseButton::Left) && l.close.contains(m)) {
            self.crafting.close();
            return (true, true);
        }
        let n = if input::act_down(Act::Run) { 10 } else { 1 };
        if input::act_pressed(Act::Down) {
            self.crafting.sel += 1;
        }
        if input::act_pressed(Act::Up) {
            self.crafting.sel = self.crafting.sel.saturating_sub(1);
        }
        let wheel = input::wheel();
        if wheel != 0.0 {
            let sel = &mut self.crafting.sel;
            *sel = if wheel > 0.0 { sel.saturating_sub(1) } else { *sel + 1 };
        }
        self.clamp_crafting();
        let count = self.crafting_recipes().len();
        if input::act_pressed(Act::Confirm) && count > 0 {
            self.craft_row(self.crafting.sel, n);
        }
        if input::mouse_pressed(MouseButton::Left) {
            for vis in 0..ROWS {
                let k = self.crafting.scroll + vis;
                let r = row_rect(l.list, vis);
                if k >= count || !r.contains(m) {
                    continue;
                }
                self.crafting.sel = k;
                let now = get_time();
                let double = self.crafting.last_click.is_some_and(|(i, t)| i == k && now - t < DOUBLE_CLICK);
                self.crafting.last_click = Some((k, now));
                if button_rect(r).contains(m) || double {
                    self.crafting.last_click = None;
                    self.craft_row(k, n);
                }
            }
        }
        (true, l.panel.contains(m) || input::mouse_pressed(MouseButton::Left))
    }

    pub(super) fn draw_crafting_ui(&self) {
        let Some((_, title)) = &self.crafting.open else { return };
        let l = layout();
        let m = input::mouse_pos();
        let p = l.panel;
        draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(0.0, 0.0, 0.0, 0.35));
        panel(p.x, p.y, p.w, p.h);
        draw_text(title, l.list.x, p.y + 32.0, 28.0, GOLD);
        let list = l.list;
        draw_rectangle(list.x, list.y, list.w, list.h, Color::new(0.0, 0.0, 0.0, 0.25));
        let recipes = self.crafting_recipes();
        if recipes.is_empty() {
            draw_text("Nothing can be made here.", list.x + 12.0, list.y + 28.0, 22.0, GRAY);
        }
        let db = &self.project.db;
        for (vis, r) in recipes.iter().skip(self.crafting.scroll).take(ROWS).enumerate() {
            let k = self.crafting.scroll + vis;
            let rr = row_rect(list, vis);
            let selected = self.crafting.sel == k;
            let missing = self.recipe_missing(r);
            let bg = if selected {
                Color::new(0.85, 0.7, 0.3, 0.22)
            } else if rr.contains(m) {
                Color::new(1.0, 1.0, 1.0, 0.1)
            } else {
                Color::new(0.0, 0.0, 0.0, 0.25)
            };
            draw_rectangle(rr.x, rr.y, rr.w, rr.h, bg);
            if selected {
                draw_rectangle_lines(rr.x, rr.y, rr.w, rr.h, 2.0, GOLD);
            }
            let dim = if missing.is_some() { Color::new(1.0, 1.0, 1.0, 0.45) } else { WHITE };
            // What the player's skill makes of it (quality steps).
            let result = r.result_for(&self.world);
            if let Some(d) = db.item(result) {
                self.assets.draw_frame_in(&d.icon.sheet, d.icon.frame, Rect::new(rr.x + 6.0, rr.y + 9.0, 36.0, 36.0), dim);
            }
            let name = if r.count > 1 { format!("{} x{}", self.item_name(result), r.count) } else { self.item_name(result).to_string() };
            // The next quality step, as a hint after the name.
            let have = self.world.skill_level(&r.skill);
            if let Some(q) = r.quality.iter().filter(|q| q.level > have && !r.skill.is_empty()).min_by_key(|q| q.level) {
                let skill = db.skill(&r.skill).map_or(r.skill.clone(), |d| d.name.clone());
                let hint = format!("{skill} {}: {}", q.level, self.item_name(&q.result));
                let nw = measure_text(&name, None, 21, 1.0).width;
                draw_text(&hint, rr.x + 58.0 + nw, rr.y + 20.0, 16.0, Color::new(0.75, 0.75, 0.6, 1.0));
            }
            draw_text(&name, rr.x + 50.0, rr.y + 21.0, 21.0, if missing.is_some() { LIGHTGRAY } else { WHITE });
            // Ingredients: small icons with have / need, red where short.
            let mut x = rr.x + 50.0;
            for s in r.ingredients.iter().filter(|s| !s.item.is_empty()) {
                let have = self.world.item_count(&s.item);
                if let Some(d) = db.item(&s.item) {
                    self.assets.draw_frame_in(&d.icon.sheet, d.icon.frame, Rect::new(x, rr.y + 28.0, 20.0, 20.0), WHITE);
                }
                let t = format!("{have}/{}", s.count);
                let c = if have >= s.count { Color::new(0.7, 0.95, 0.7, 1.0) } else { Color::new(1.0, 0.55, 0.5, 1.0) };
                draw_text(&t, x + 22.0, rr.y + 44.0, 17.0, c);
                x += 30.0 + measure_text(&t, None, 17, 1.0).width;
            }
            // Tools: an icon of such a tool and its kind, red if the player has none.
            let kinds = db.tool_kinds();
            for kind in r.tools.iter().filter(|t| !t.trim().is_empty()) {
                let have = crate::crafting::has_tool(&self.world, &kinds, kind);
                if let Some(d) = db.items.iter().find(|d| d.tool.eq_ignore_ascii_case(kind)) {
                    self.assets.draw_frame_in(&d.icon.sheet, d.icon.frame, Rect::new(x, rr.y + 28.0, 20.0, 20.0), WHITE);
                }
                let c = if have { Color::new(0.7, 0.95, 0.7, 1.0) } else { Color::new(1.0, 0.55, 0.5, 1.0) };
                draw_text(kind, x + 22.0, rr.y + 44.0, 17.0, c);
                x += 30.0 + measure_text(kind, None, 17, 1.0).width;
            }
            if let Some(Missing::Skill(..) | Missing::StationState(..)) = &missing {
                draw_text(self.missing_text(missing.as_ref().unwrap()), x + 6.0, rr.y + 44.0, 17.0, Color::new(1.0, 0.55, 0.5, 1.0));
            }
            // Make button.
            let b = button_rect(rr);
            let ok = missing.is_none();
            let fill = if !ok {
                Color::new(0.25, 0.1, 0.1, 0.8)
            } else if b.contains(m) {
                Color::new(0.45, 0.35, 0.12, 1.0)
            } else {
                Color::new(0.3, 0.24, 0.1, 0.9)
            };
            draw_rectangle(b.x, b.y, b.w, b.h, fill);
            draw_rectangle_lines(b.x, b.y, b.w, b.h, 1.0, Color::new(0.85, 0.8, 0.6, 0.8));
            let tw = measure_text("Make", None, 18, 1.0).width;
            draw_text("Make", b.x + (b.w - tw) / 2.0, b.y + b.h / 2.0 + 6.0, 18.0, if ok { WHITE } else { Color::new(0.9, 0.5, 0.5, 1.0) });
        }
        if self.crafting.scroll > 0 {
            draw_text("more above", list.x + list.w - 80.0, list.y - 2.0, 14.0, LIGHTGRAY);
        }
        if recipes.len() > self.crafting.scroll + ROWS {
            draw_text("more below", list.x + list.w - 80.0, list.y + list.h + 12.0, 14.0, LIGHTGRAY);
        }
        let c = l.close;
        draw_rectangle_lines(c.x, c.y, c.w, c.h, 1.5, Color::new(0.85, 0.8, 0.6, 1.0));
        draw_line(c.x + 6.0, c.y + 6.0, c.x + c.w - 6.0, c.y + c.h - 6.0, 2.0, WHITE);
        draw_line(c.x + c.w - 6.0, c.y + 6.0, c.x + 6.0, c.y + c.h - 6.0, 2.0, WHITE);

        // Footer: the selected result's description, the last message, help.
        let f = l.foot;
        draw_rectangle(f.x, f.y, f.w, f.h, Color::new(0.0, 0.0, 0.0, 0.25));
        if let Some(d) = recipes.get(self.crafting.sel).and_then(|r| db.item(r.result_for(&self.world))) {
            draw_text(&d.name, f.x + 10.0, f.y + 22.0, 21.0, GOLD);
            for (k, line) in wrap_text(&d.description, 17, f.w - 20.0).iter().take(1).enumerate() {
                draw_text(line, f.x + 10.0, f.y + 42.0 + k as f32 * 17.0, 17.0, LIGHTGRAY);
            }
        }
        if let Some((text, ok, t)) = &self.crafting.msg {
            let age = (get_time() - t) as f32;
            if age < 4.0 {
                let a = (4.0 - age).min(1.0);
                let c = if *ok { Color::new(0.6, 0.95, 0.6, a) } else { Color::new(1.0, 0.55, 0.5, a) };
                let tw = measure_text(text, None, 20, 1.0).width;
                draw_text(text, f.x + f.w - tw - 10.0, f.y + 22.0, 20.0, c);
            }
        }
        draw_text("Make / Enter: make one   Shift: ten   Wheel / Up / Down: select   Esc: close", f.x + 10.0, f.y + f.h - 8.0, 16.0, GRAY);
    }
}
