//! The action bar (0.32): one bar at the bottom of the screen in two modes. In a fight it is the
//! turn's bar (`turns/ui.rs`); outside fights it shows the project's entries (`BarEntry`: Use,
//! Cast, Class, Rest, Sneak, Party, custom script entries), each opening a list of what can be
//! done now or acting at once. It replaced the quick bar of 0.31 and older. F / RB focuses the bar
//! for keys and gamepads; a list pauses the world like the inventory.

use macroquad::prelude::*;

use super::input::{self, Act};
use super::skin;
use super::text::{self, draw_plain, draw_text, measure_plain, measure_text, Role};
use super::turns::Who;
use super::Game;
use crate::menus::BarEntry;

const BAR_H: f32 = 28.0;
const GAP: f32 = 4.0;
const FONT: f32 = 18.0;
const ROW_H: f32 = 24.0;
/// The key help under a list.
const LIST_HINT: &str = "{key:confirm}: use   {key:cancel}: close";

/// Where the keyboard / gamepad focus is: an entry of the bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Focus {
    Bar(usize),
}

/// What a list row does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum BarPick {
    Item(String),
    Spell(String),
    Class(String),
    /// `short_rest`, `camp`.
    Act(String),
    /// A beast's shape to take (Wild Shape, Polymorph's window).
    Shape(String),
}

#[derive(Clone, Debug)]
pub(super) struct BarRow {
    pub label: String,
    /// Dimmer, on the right: a count, a slot, uses left.
    pub detail: String,
    /// Why it can't be picked now (greyed, instead of the detail).
    pub why: Option<&'static str>,
    pub pick: BarPick,
}

/// An open list of an entry.
#[derive(Clone, Debug)]
pub(super) struct BarList {
    pub entry: BarEntry,
    pub rows: Vec<BarRow>,
    pub sel: usize,
    mouse: Vec2,
}

/// The bar's state outside fights.
#[derive(Default)]
pub(super) struct ActionBar {
    pub focus: Option<Focus>,
    pub list: Option<BarList>,
}

impl Game {
    /// The entries the bar shows now: the project's (or the built-in) ones, without Cast, Class
    /// and Party when the hero has nothing there.
    pub(super) fn bar_entries(&self) -> Vec<BarEntry> {
        let list: Vec<BarEntry> = if self.project.config.action_bar.is_empty() { BarEntry::DEFAULT.to_vec() } else { self.project.config.action_bar.clone() };
        list.into_iter()
            .filter(|e| match e {
                BarEntry::Cast => !self.cast_rows(Who::Player).is_empty(),
                BarEntry::Class => !self.class_acts(Who::Player).is_empty(),
                BarEntry::Party => self.loaded.entities.iter().any(|e| e.active && e.companion.is_some()),
                _ => true,
            })
            .collect()
    }

    fn entry_text(&self, e: &BarEntry) -> String {
        match e {
            BarEntry::Sneak if self.sneaking() => format!("{}: {}", crate::lang::tr_ui("Sneak"), crate::lang::tr_ui("on")),
            BarEntry::Custom { label, .. } => crate::lang::tr(label).to_string(),
            e => crate::lang::tr_ui(e.label()).into_owned(),
        }
    }

    /// The exploration bar's buttons (screen rectangles), centred at the bottom.
    pub(super) fn explore_bar_rects(&self) -> Vec<Rect> {
        let _font = text::role(Role::Hud);
        let entries = self.bar_entries();
        let widths: Vec<f32> = entries.iter().map(|e| measure_text(&self.entry_text(e), None, FONT as u16, 1.0).width + 18.0).collect();
        let total = widths.iter().sum::<f32>() + GAP * (widths.len().max(1) as f32 - 1.0);
        let mut x = (screen_width() - total) / 2.0;
        let y = screen_height() - 12.0 - BAR_H;
        widths
            .iter()
            .map(|w| {
                let r = Rect::new(x, y, *w, BAR_H);
                x += w + GAP;
                r
            })
            .collect()
    }

    /// The top of the bar: the fight's turn bar, else the exploration bar.
    pub(super) fn bar_top(&self) -> f32 {
        let rects = if self.fight.is_some() { self.bar_layout() } else { self.explore_bar_rects() };
        rects.iter().map(|r| r.y).fold(screen_height() - 12.0, f32::min)
    }

    // ------------------------------------------------------------------ input

    /// The bar's keys and clicks while nothing else is open (outside fights). Returns (key
    /// consumed, mouse consumed).
    pub(super) fn action_bar_input(&mut self, key_consumed: bool, click: bool) -> (bool, bool) {
        let m = input::mouse_pos();
        if self.fight.is_some() {
            return (false, false);
        }
        if !key_consumed && input::act_pressed(Act::Actions) {
            self.bar.focus = Some(Focus::Bar(0));
            return (true, false);
        }
        if click && !self.ui_hidden {
            if let Some(k) = self.explore_bar_rects().iter().position(|r| r.contains(m)) {
                self.bar.focus = None;
                self.activate_entry(k);
                return (false, true);
            }
        }
        (false, false)
    }

    /// The focused bar and an open list take all input and pause the world. True while one is.
    pub(super) fn update_action_bar_modal(&mut self) -> bool {
        if self.bar.list.is_some() {
            self.list_input();
            return true;
        }
        let Some(focus) = self.bar.focus else { return false };
        if self.fight.is_some() || self.ui.dialog_open() {
            self.bar.focus = None;
            return false;
        }
        let n = self.bar_entries().len().max(1);
        let Focus::Bar(k) = focus;
        let k = k.min(n - 1);
        let step = |d: i32| ((k as i32 + d).rem_euclid(n as i32)) as usize;
        let mut next = k;
        if input::act_pressed(Act::Left) {
            next = step(-1);
        }
        if input::act_pressed(Act::Right) {
            next = step(1);
        }
        self.bar.focus = Some(Focus::Bar(next));
        if input::act_pressed(Act::Confirm) {
            self.activate_entry(next);
        } else if input::act_pressed(Act::Cancel) || input::act_pressed(Act::Actions) || input::mouse_pressed(MouseButton::Right) {
            self.bar.focus = None;
        }
        true
    }

    /// Entry `k` of the bar: a list opens, or it acts at once (Sneak, Party, custom entries).
    pub(super) fn activate_entry(&mut self, k: usize) {
        let Some(e) = self.bar_entries().get(k).cloned() else { return };
        match &e {
            BarEntry::Sneak => {
                self.bar.focus = None;
                if !self.player_stunned() {
                    self.combat.sneak = !self.combat.sneak;
                }
            }
            BarEntry::Search => {
                self.bar.focus = None;
                self.search();
            }
            BarEntry::Party => {
                self.bar.focus = None;
                self.open_order_menu();
            }
            BarEntry::Custom { function, .. } => {
                self.bar.focus = None;
                self.run_script(function);
            }
            _ => {
                let rows = self.entry_rows(&e);
                if rows.is_empty() {
                    self.ui.toast(crate::lang::tr_ui("Nothing to use now").into_owned());
                    return;
                }
                let sel = rows.iter().position(|r| r.why.is_none()).unwrap_or(0);
                self.bar.list = Some(BarList { entry: e, rows, sel, mouse: input::mouse_pos() });
            }
        }
    }

    /// Opens a list of `rows` under `entry`'s title (the beasts of Wild Shape).
    pub(super) fn open_bar_list(&mut self, entry: BarEntry, rows: Vec<BarRow>) {
        let sel = rows.iter().position(|r| r.why.is_none()).unwrap_or(0);
        self.bar.list = Some(BarList { entry, rows, sel, mouse: input::mouse_pos() });
    }

    /// The rows of a list entry.
    fn entry_rows(&self, e: &BarEntry) -> Vec<BarRow> {
        match e {
            BarEntry::Use => {
                let mut seen: Vec<String> = Vec::new();
                for s in self.world.inventory.slots.iter().flatten() {
                    if !seen.contains(&s.item) && self.usable_from_bar(&s.item) {
                        seen.push(s.item.clone());
                    }
                }
                seen.into_iter()
                    .map(|id| {
                        let name = self.project.db.item(&id).map_or(id.clone(), |d| crate::lang::tr(&d.name).to_string());
                        BarRow { label: name, detail: format!("x{}", self.world.inventory.count(&id)), why: None, pick: BarPick::Item(id) }
                    })
                    .collect()
            }
            BarEntry::Cast => {
                let mut out: Vec<BarRow> = Vec::new();
                for r in self.cast_rows(Who::Player) {
                    let pick = match &r.pick {
                        super::turns::menu::MenuPick::Cast(id, _) => BarPick::Spell(id.clone()),
                        super::turns::menu::MenuPick::ItemCast { item, .. } => BarPick::Item(item.clone()),
                        _ => continue,
                    };
                    // (One row per spell: outside fights the lowest slot is spent.)
                    if out.iter().any(|o| o.pick == pick) {
                        continue;
                    }
                    // (A combat spell starts a fight with an enemy in sight: without one it waits.)
                    let attack = matches!(&pick, BarPick::Spell(id) if self.project.db.spell(id).is_some_and(|d| d.offensive() && !d.heals()));
                    let why = r.why.or((attack && !self.hostile_in_sight(super::combat::ATTACK_SIGHT)).then_some("No enemy in sight"));
                    out.push(BarRow { label: r.label, detail: r.detail, why, pick });
                }
                out
            }
            BarEntry::Class => self
                .class_acts_outside()
                .into_iter()
                .map(|(a, why)| {
                    let detail = if a.resource.is_empty() { String::new() } else { self.uses(Who::Player, &a.resource).map_or(String::new(), |(l, m)| format!("{l}/{m}")) };
                    BarRow { label: crate::lang::tr(&a.name).to_string(), detail, why, pick: BarPick::Class(a.id) }
                })
                .collect(),
            BarEntry::Rest => {
                let camp = self.project.config.rules.camp_item.trim().to_string();
                let camp_name = self.project.db.item(&camp).map_or(camp.clone(), |d| crate::lang::tr(&d.name).to_string());
                vec![
                    BarRow { label: crate::lang::tr_ui("Short rest").into_owned(), detail: crate::lang::tr_ui("1 hour, hit dice").into_owned(), why: self.cant_rest(), pick: BarPick::Act("short_rest".into()) },
                    BarRow {
                        label: crate::lang::tr_ui("Make camp").into_owned(),
                        detail: if camp.is_empty() { crate::lang::tr_ui("8 hours").into_owned() } else { format!("{} - {camp_name}", crate::lang::tr_ui("8 hours")) },
                        why: self.cant_camp(),
                        pick: BarPick::Act("camp".into()),
                    },
                ]
            }
            _ => Vec::new(),
        }
    }

    /// An item the Use list offers: consumables, items with *Use* triggers, scrolls, items
    /// that cast spells and poisons that coat a weapon (not gear to wear: the inventory equips that).
    fn usable_from_bar(&self, id: &str) -> bool {
        let Some(d) = self.project.db.item(id) else { return false };
        let casts = d.magic.as_ref().is_some_and(|m| !m.spells.is_empty());
        let coats = d.poison.as_ref().is_some_and(|p| p.kind.coats());
        (d.consumable || !d.on_use.is_empty() || !d.teaches.is_empty() || casts || coats) && (self.project.db.slot_of(id).is_none() || casts) && d.coin_cp == 0
    }

    fn list_rects(&self, list: &BarList) -> (Rect, Vec<Rect>) {
        let _font = text::role(Role::Hud);
        let title = self.entry_text(&list.entry);
        let inset = skin::inset();
        let width = list.rows.iter().map(|r| measure_plain(&format!("{}    {}", r.label, row_tail(r)), FONT).width).fold(measure_plain(&title, FONT).width, f32::max).max(super::glyphs::measure_hint(LIST_HINT, 14.0)) + 2.0 * inset + 12.0;
        let w = width.min(screen_width() - 32.0);
        // (Title, rows, the hint line; the frame's border around them.)
        let head = (inset + 22.0).max(34.0);
        let h = head + list.rows.len() as f32 * ROW_H + 18.0 + inset;
        let bottom = self.bar_top() - 10.0;
        let panel = Rect::new((screen_width() - w) / 2.0, (bottom - h).max(40.0), w, h);
        let rows = (0..list.rows.len()).map(|k| Rect::new(panel.x + inset.max(8.0), panel.y + head + k as f32 * ROW_H, panel.w - 2.0 * inset.max(8.0), ROW_H)).collect();
        (panel, rows)
    }

    fn list_input(&mut self) {
        let Some(list) = self.bar.list.clone() else { return };
        let n = list.rows.len().max(1);
        let mut sel = list.sel.min(n - 1);
        if input::act_pressed(Act::Up) {
            sel = (sel + n - 1) % n;
        }
        if input::act_pressed(Act::Down) {
            sel = (sel + 1) % n;
        }
        let (_, rects) = self.list_rects(&list);
        let mouse = input::mouse_pos();
        let hovered = rects.iter().position(|r| r.contains(mouse));
        if let (Some(h), true) = (hovered, input::mouse_pressed(MouseButton::Left) || mouse != list.mouse) {
            sel = h;
        }
        if let Some(l) = &mut self.bar.list {
            l.sel = sel;
            l.mouse = mouse;
        }
        if input::act_pressed(Act::Confirm) || (input::mouse_pressed(MouseButton::Left) && hovered.is_some()) {
            self.pick_bar_row(sel);
        } else if input::act_pressed(Act::Cancel) || input::mouse_pressed(MouseButton::Right) || (input::mouse_pressed(MouseButton::Left) && hovered.is_none()) {
            self.bar.list = None;
        }
    }

    /// Row `k` of the open list: what it does happens (the list closes); a greyed row says why.
    pub(super) fn pick_bar_row(&mut self, k: usize) {
        let Some(row) = self.bar.list.as_ref().and_then(|l| l.rows.get(k)).cloned() else { return };
        if let Some(why) = row.why {
            self.ui.toast(crate::lang::tr_ui(why).into_owned());
            return;
        }
        self.bar.list = None;
        self.bar.focus = None;
        self.do_pick(&row.pick);
    }

    fn do_pick(&mut self, pick: &BarPick) {
        match pick {
            BarPick::Item(id) => {
                if self.world.inventory.count(id) == 0 {
                    let name = self.item_name(id);
                    self.ui.toast(format!("No {name} left"));
                    return;
                }
                self.use_item(id);
            }
            BarPick::Spell(id) => {
                self.cast_spell(id);
            }
            BarPick::Class(id) => {
                if let Err(why) = self.use_class_act_outside(id) {
                    self.ui.toast(crate::lang::tr_ui(why).into_owned());
                }
            }
            BarPick::Act(a) => self.bar_act(a),
            BarPick::Shape(form) => self.pick_shape(form),
        }
    }

    /// A rest (a list row).
    fn bar_act(&mut self, a: &str) {
        match a {
            "short_rest" => self.open_short_rest(),
            "camp" => self.make_camp(),
            "search" => self.search(),
            "sneak" => {
                if !self.player_stunned() {
                    self.combat.sneak = !self.combat.sneak;
                }
            }
            other => eprintln!("[game] action bar: unknown action '{other}'"),
        }
    }

    // ------------------------------------------------------------------ camping

    /// Why the hero can't make camp now (None = they can).
    pub(super) fn cant_camp(&self) -> Option<&'static str> {
        if self.fight.is_some() {
            return Some("Not in a fight");
        }
        if let Some(why) = self.cant_rest() {
            return Some(why);
        }
        let need = self.project.config.rules.camp_item.trim();
        if !need.is_empty() {
            let has = self.world.inventory.slots.iter().flatten().any(|s| s.item == need || self.project.db.item(&s.item).is_some_and(|d| d.srd == need));
            if !has {
                return Some("You need a bedroll");
            }
        }
        None
    }

    /// Make camp: a long rest anywhere (the sleep sequence for 8 hours) with the camp item.
    pub(super) fn make_camp(&mut self) {
        if let Some(why) = self.cant_camp() {
            self.ui.toast(crate::lang::tr_ui(why).into_owned());
            return;
        }
        self.start_sleep(8.0);
    }

    // ------------------------------------------------------------------ drawing

    /// The bar outside fights and an open list.
    pub(super) fn draw_action_bar(&self) {
        let _font = text::role(Role::Hud);
        if self.fight.is_none() {
            let entries = self.bar_entries();
            let rects = self.explore_bar_rects();
            let m = input::mouse_pos();
            for (k, (e, r)) in entries.iter().zip(&rects).enumerate() {
                let focused = self.bar.focus == Some(Focus::Bar(k));
                let open = self.bar.list.as_ref().is_some_and(|l| &l.entry == e);
                let on = matches!(e, BarEntry::Sneak) && self.sneaking();
                let hot = focused || open || on || r.contains(m);
                if !skin::button_picture(*r, hot) {
                    draw_rectangle(r.x, r.y, r.w, r.h, skin::button_fill(hot, Color::new(0.12, 0.12, 0.18, 0.92)));
                }
                let label = self.entry_text(e);
                let w = measure_text(&label, None, FONT as u16, 1.0).width;
                draw_text(&label, r.x + (r.w - w) / 2.0, r.y + 19.0, FONT, if focused || open { skin::highlight() } else { skin::normal() });
                if focused {
                    skin::cursor(*r, Color::new(1.0, 0.86, 0.45, 1.0));
                }
            }
            // The key that focuses the bar, left of it.
            if let (Some(first), Some(key)) = (rects.first(), super::glyphs::key_text("actions")) {
                let t = format!("[{key}]");
                let w = measure_text(&t, None, 15, 1.0).width;
                draw_text(&t, first.x - w - 6.0, first.y + 19.0, 15.0, skin::dim());
            }
        }
        // A focused custom entry's line: what it does, above the bar.
        if let Some(Focus::Bar(k)) = self.bar.focus {
            if let Some(BarEntry::Custom { detail, .. }) = self.bar_entries().get(k) {
                if !detail.is_empty() {
                    let t = crate::lang::tr(detail);
                    let w = measure_plain(&t, 16.0).width;
                    draw_plain(&t, (screen_width() - w) / 2.0, self.bar_top() - 10.0, 16.0, skin::dim());
                }
            }
        }
        if let Some(list) = &self.bar.list {
            let (panel, rects) = self.list_rects(list);
            skin::panel(panel.x, panel.y, panel.w, panel.h);
            let head = rects.first().map_or(panel.y + 34.0, |r| r.y);
            draw_plain(&self.entry_text(&list.entry), panel.x + skin::inset().max(14.0), head - 8.0, FONT, skin::highlight());
            for (k, (row, r)) in list.rows.iter().zip(&rects).enumerate() {
                if k == list.sel {
                    skin::cursor(*r, Color::new(1.0, 0.86, 0.45, 1.0));
                }
                let color = if row.why.is_some() { skin::disabled() } else { skin::normal() };
                draw_plain(&row.label, r.x + 8.0, r.y + 17.0, FONT, color);
                let tail = row_tail(row);
                let tw = measure_plain(&tail, FONT - 2.0).width;
                draw_plain(&tail, r.right() - tw - 8.0, r.y + 17.0, FONT - 2.0, if row.why.is_some() { skin::disabled() } else { skin::dim() });
            }
            let x = panel.x + skin::inset().max(14.0);
            super::glyphs::draw_hint_in(LIST_HINT, x, panel.bottom() - skin::inset() - 4.0, 14.0, panel.right() - skin::inset().max(14.0) - x, skin::dim());
        }
    }

    // ------------------------------------------------------------------ status, autoplay

    /// `bar=` for status: `fight`, or `explore[use,cast,…]` with `:focus=ENTRY` and
    /// `:open=ENTRY[>0:Potion|1:Bread!why]`.
    pub(super) fn bar_status(&self) -> String {
        if self.fight.is_some() {
            return "fight".into();
        }
        let entries = self.bar_entries();
        let mut s = format!("explore[{}]", entries.iter().map(|e| e.id().to_string()).collect::<Vec<_>>().join(","));
        match self.bar.focus {
            Some(Focus::Bar(k)) => s += &format!(":focus={}", entries.get(k).map_or("-", |e| e.id())),
            None => {}
        }
        if let Some(l) = &self.bar.list {
            let rows: Vec<String> = l.rows.iter().enumerate().map(|(k, r)| format!("{}{k}:{}{}", if k == l.sel { ">" } else { "" }, r.label, r.why.map_or(String::new(), |w| format!("!{w}")))).collect();
            s += &format!(":open={}[{}]", l.entry.id(), rows.join("|"));
        }
        s
    }

    /// Autoplay `bar ENTRY`: activates the entry with that id (`use`, `cast`, `rest`, a custom
    /// entry's function). False without it.
    pub fn bar_pick_entry(&mut self, id: &str) -> bool {
        match self.bar_entries().iter().position(|e| e.id() == id) {
            Some(k) => {
                self.activate_entry(k);
                true
            }
            None => false,
        }
    }

    /// Autoplay `barpick K`: row K of the open list.
    pub fn bar_pick_row(&mut self, k: usize) -> bool {
        if self.bar.list.as_ref().is_none_or(|l| k >= l.rows.len()) {
            return false;
        }
        if let Some(l) = &mut self.bar.list {
            l.sel = k;
        }
        self.pick_bar_row(k);
        true
    }

}

/// A row's right-hand text: its detail, or why it can't be picked.
fn row_tail(r: &BarRow) -> String {
    match r.why {
        Some(w) => crate::lang::tr_ui(w).into_owned(),
        None => r.detail.clone(),
    }
}
