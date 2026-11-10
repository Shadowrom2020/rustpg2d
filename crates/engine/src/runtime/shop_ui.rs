//! Merchant trade window: the merchant's stock on the left, the player's items on the right.
//!
//! Click a row to select it, click its price button (or double-click the row, or press
//! Enter / E) to buy or sell one; hold Shift for up to ten. Up/Down select, Left/Right or
//! Tab switch sides, the mouse wheel moves the selection up / down, Esc closes.

use macroquad::prelude::*;

use super::input::{self, Act};
use super::ui::{panel, ListClick, ScrollList, PAD};
use super::Game;
use crate::defs::{ShopEntry, ShopSpec};
use crate::render::wrap_text;
use super::text::{draw_text, measure_text};

const ROW: f32 = 44.0;
const ROWS: usize = 8;
const COL_W: f32 = 380.0;
const TITLE_H: f32 = 44.0;
const FOOT_H: f32 = 96.0;
const BTN_W: f32 = 92.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ShopSide {
    Merchant,
    Player,
}

impl ShopSide {
    fn idx(self) -> usize {
        self as usize
    }
    fn other(self) -> ShopSide {
        match self {
            ShopSide::Merchant => ShopSide::Player,
            ShopSide::Player => ShopSide::Merchant,
        }
    }
}

#[derive(Default)]
pub(super) struct ShopUi {
    /// Entity uid of the merchant being traded with.
    open: Option<u32>,
    /// Merchant to open once the current event sequence has finished.
    pub pending: Option<u32>,
    side: Option<ShopSide>,
    /// The merchant's and the player's list.
    lists: [ScrollList; 2],
    /// Result of the last trade (shown in the footer) and when it happened.
    msg: Option<(String, bool, f64)>,
}

impl ShopUi {
    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }
    pub fn close(&mut self) {
        self.open = None;
        self.pending = None;
    }
}

/// One line in either list.
struct Row {
    item: String,
    /// Stock / owned count (None = unlimited).
    count: Option<u32>,
    /// Price per piece (None = the merchant won't buy it).
    price: Option<u32>,
}

struct Layout {
    panel: Rect,
    lists: [Rect; 2],
    close: Rect,
    foot: Rect,
}

fn layout() -> Layout {
    let (sw, sh) = (screen_width(), screen_height());
    let w = 2.0 * COL_W + 3.0 * PAD;
    let h = TITLE_H + ROWS as f32 * ROW + FOOT_H + 3.0 * PAD;
    let x = ((sw - w) / 2.0).max(4.0).round();
    let y = ((sh - h) / 2.0).max(4.0).round();
    let list = |k: f32| Rect::new(x + PAD + k * (COL_W + PAD), y + TITLE_H + PAD, COL_W, ROWS as f32 * ROW);
    Layout {
        panel: Rect::new(x, y, w, h),
        lists: [list(0.0), list(1.0)],
        close: Rect::new(x + w - 30.0, y + 8.0, 22.0, 22.0),
        foot: Rect::new(x + PAD, y + TITLE_H + 2.0 * PAD + ROWS as f32 * ROW, w - 2.0 * PAD, FOOT_H),
    }
}

fn row_rect(list: Rect, k: usize) -> Rect {
    Rect::new(list.x, list.y + k as f32 * ROW, list.w, ROW - 4.0)
}

fn button_rect(row: Rect) -> Rect {
    Rect::new(row.x + row.w - BTN_W - 6.0, row.y + 6.0, BTN_W, row.h - 12.0)
}

impl Game {
    /// The trade or the crafting window is open (both block the other windows).
    pub(super) fn shop_open(&self) -> bool {
        self.shop.is_open() || self.crafting.is_open()
    }

    /// The *Shop stock* action: adds `count` of `item` to merchant `i`'s stock (a new line at
    /// `price`), or sets it (0 removes the line). Unlimited lines stay unlimited.
    pub(super) fn change_stock(&mut self, i: usize, item: &str, op: crate::events::VarOp, count: u32, price: u32) {
        let uid = self.loaded.entities[i].uid;
        match self.shop_stock(uid) {
            Some(stock) => apply_stock(stock, item, op, count, price),
            None => eprintln!("[game] Shop stock: {} is no merchant", self.loaded.entities[i].def),
        }
    }

    /// What merchants on the current map have, for `stock_of()`: (uid, name, [(item, count)]),
    /// unlimited lines as 999.
    pub(super) fn merchant_stock(&self) -> Vec<crate::script::MerchantStock> {
        self.loaded
            .entities
            .iter()
            .filter(|e| e.active)
            .filter_map(|e| {
                let saved = self.saved_stock(e.uid).cloned();
                let stock = saved.or_else(|| self.shop_spec(e.uid).map(|s| s.items))?;
                Some((e.uid, e.name.clone(), stock.into_iter().map(|l| (l.item, if l.unlimited { 999 } else { l.count })).collect()))
            })
            .collect()
    }

    /// Where the merchant with runtime `uid` is placed: its home map, its uid there and its
    /// placed instance. A character visiting from another map (its daily schedule) is placed
    /// on its home map, and so are its shop and saved stock, the same wherever it stands.
    fn merchant_home(&self, uid: u32) -> Option<(&str, u32, &crate::entity::EntityInstance)> {
        let visitor = self.loaded.entities.iter().find(|e| e.uid == uid).and_then(|e| e.visitor.as_deref());
        match visitor {
            Some(v) => {
                let def = self.visitor_defs.as_deref()?.iter().find(|d| d.home == v.map && d.inst.uid == v.uid)?;
                Some((def.home.as_str(), v.uid, &def.inst))
            }
            None => Some((self.loaded.map.name.as_str(), uid, self.loaded.map.entity(uid)?)),
        }
    }

    /// The merchant spec of a character on the current map (a visitor's from its home map).
    fn shop_spec(&self, uid: u32) -> Option<ShopSpec> {
        let (_, _, e) = self.merchant_home(uid)?;
        self.project.db.shop_of(e).cloned()
    }

    /// A merchant's saved stock, if it has traded yet.
    fn saved_stock(&self, uid: u32) -> Option<&Vec<ShopEntry>> {
        let (map, home_uid, _) = self.merchant_home(uid)?;
        self.world.shops.get(map)?.get(&home_uid)
    }

    /// Current stock of a merchant (created from its spec the first time).
    pub(super) fn shop_stock(&mut self, uid: u32) -> Option<&mut Vec<ShopEntry>> {
        let (map, home_uid) = self.merchant_home(uid).map(|(m, u, _)| (m.to_string(), u))?;
        if self.world.shops.get(&map).is_none_or(|m| !m.contains_key(&home_uid)) {
            let spec = self.shop_spec(uid)?;
            let stock = spec.items.into_iter().filter(|e| !e.item.is_empty()).collect();
            self.world.shops.entry(map.clone()).or_default().insert(home_uid, stock);
        }
        self.world.shops.get_mut(&map)?.get_mut(&home_uid)
    }

    /// Queues the trade window of a merchant (opens once the current events have finished).
    /// `talk`: opened by talking to the character (respects `open_on_talk`).
    pub(super) fn try_open_shop(&mut self, i: usize, talk: bool) -> bool {
        let uid = self.loaded.entities[i].uid;
        match self.shop_spec(uid) {
            Some(spec) if !talk || spec.open_on_talk => {
                self.shop_stock(uid);
                self.shop.pending = Some(uid);
                true
            }
            _ => false,
        }
    }

    fn merchant_name(&self, uid: u32) -> String {
        let def = self.merchant_home(uid).map(|(_, _, e)| e.def.clone()).unwrap_or_default();
        self.project.db.character(&def).map_or("Merchant".into(), |d| d.name.clone())
    }

    /// Window title: the shop's title or the merchant's name.
    fn shop_title(&self, uid: u32) -> String {
        match self.shop_spec(uid) {
            Some(s) if !s.title.is_empty() => s.title,
            _ => self.merchant_name(uid),
        }
    }

    fn shop_msg(&mut self, text: String, ok: bool) {
        self.shop.msg = Some((text, ok, get_time()));
    }

    fn shop_rows(&self, side: ShopSide) -> Vec<Row> {
        let Some(uid) = self.shop.open else { return Vec::new() };
        let Some(spec) = self.shop_spec(uid) else { return Vec::new() };
        let empty = Vec::new();
        let stock = self.saved_stock(uid).unwrap_or(&empty);
        let db = &self.project.db;
        // Haggling: buying cheaper, selling better (never below 1).
        let h = self.haggle_percent() / 100.0;
        let cheaper = |p: u32| if p == 0 { 0 } else { ((p as f32 * (1.0 - h)).round() as u32).max(1) };
        let better = |p: u32| ((p as f32 * (1.0 + h)).round() as u32).max(1);
        match side {
            ShopSide::Merchant => stock
                .iter()
                .filter(|e| e.available())
                .map(|e| Row {
                    item: e.item.clone(),
                    count: (!e.unlimited).then_some(e.count),
                    price: Some(cheaper(ShopSpec::buy_price(stock, &e.item, db))),
                })
                .collect(),
            ShopSide::Player => {
                let mut rows: Vec<Row> = Vec::new();
                for s in self.world.inventory.slots.iter().flatten() {
                    if s.item == spec.currency {
                        continue;
                    }
                    match rows.iter_mut().find(|r| r.item == s.item) {
                        Some(r) => r.count = Some(r.count.unwrap_or(0) + s.count),
                        None => rows.push(Row { item: s.item.clone(), count: Some(s.count), price: spec.sell_price(stock, &s.item, db).map(better) }),
                    }
                }
                rows
            }
        }
    }

    fn money(&self) -> (String, u32) {
        let cur = self.shop.open.and_then(|u| self.shop_spec(u)).map_or_else(|| "coin".to_string(), |s| s.currency);
        let n = self.world.item_count(&cur);
        (cur, n)
    }

    /// Buys up to `n` of merchant row `k`. Returns how many were bought.
    fn buy(&mut self, k: usize, n: u32) -> u32 {
        let Some(uid) = self.shop.open else { return 0 };
        let Some(row) = self.shop_rows(ShopSide::Merchant).into_iter().nth(k) else { return 0 };
        let (cur, mut money) = self.money();
        let price = row.price.unwrap_or(0);
        let mut bought = 0;
        while bought < n && row.count.is_none_or(|c| bought < c) {
            if money < price {
                if bought == 0 {
                    self.shop_msg(format!("Not enough {}.", self.item_name(&cur).to_lowercase()), false);
                }
                break;
            }
            if self.world.give_item(&row.item, 1) > 0 {
                if bought == 0 {
                    self.shop_msg("Your inventory is full.".into(), false);
                }
                break;
            }
            self.world.take_item(&cur, price);
            money -= price;
            bought += 1;
        }
        if bought > 0 {
            if let Some(e) = self.shop_stock(uid).and_then(|s| s.iter_mut().find(|e| e.item == row.item)) {
                if !e.unlimited {
                    e.count -= bought.min(e.count);
                }
            }
            self.audio.play(&self.project.config.sfx.pickup);
            let name = self.item_name(&row.item);
            self.shop_msg(if bought > 1 { format!("Bought {bought} {name}") } else { format!("Bought {name}") }, true);
        }
        bought
    }

    /// Sells up to `n` of player row `k`. Returns how many were sold.
    fn sell(&mut self, k: usize, n: u32) -> u32 {
        let Some(uid) = self.shop.open else { return 0 };
        let Some(row) = self.shop_rows(ShopSide::Player).into_iter().nth(k) else { return 0 };
        let Some(price) = row.price else {
            self.shop_msg(format!("{} isn't interested in that.", self.merchant_name(uid)), false);
            return 0;
        };
        let (cur, _) = self.money();
        let base = match self.shop_stock(uid) {
            Some(s) => s.clone(),
            None => return 0,
        };
        let base = ShopSpec::buy_price(&base, &row.item, &self.project.db);
        let mut sold = 0;
        while sold < n && sold < row.count.unwrap_or(0) {
            if self.world.take_item(&row.item, 1) == 0 {
                break;
            }
            // Payment must fit (normally the freed slot makes room).
            let left = self.world.give_item(&cur, price);
            if left > 0 {
                self.world.take_item(&cur, price - left);
                self.world.give_item(&row.item, 1);
                if sold == 0 {
                    self.shop_msg("No room for the payment.".into(), false);
                }
                break;
            }
            sold += 1;
        }
        if sold > 0 {
            // The merchant now sells it (at their price, or the item's value).
            if let Some(stock) = self.shop_stock(uid) {
                match stock.iter_mut().find(|e| e.item == row.item) {
                    Some(e) if e.unlimited => {}
                    Some(e) => e.count += sold,
                    None => stock.push(ShopEntry::new(&row.item, sold, if base > 0 { base } else { 0 })),
                }
            }
            self.audio.play(&self.project.config.sfx.pickup);
            let name = self.item_name(&row.item);
            let total = sold * price;
            self.shop_msg(if sold > 1 { format!("Sold {sold} {name} for {total}") } else { format!("Sold {name} for {total}") }, true);
        }
        sold
    }

    fn trade(&mut self, side: ShopSide, k: usize) {
        let n = if input::act_down(Act::Run) { 10 } else { 1 };
        match side {
            ShopSide::Merchant => self.buy(k, n),
            ShopSide::Player => self.sell(k, n),
        };
        self.clamp_selection();
    }

    fn clamp_selection(&mut self) {
        for side in [ShopSide::Merchant, ShopSide::Player] {
            let n = self.shop_rows(side).len();
            self.shop.lists[side.idx()].clamp(n, ROWS);
        }
    }

    /// Open merchant and its stock (for automated play-throughs).
    pub(super) fn shop_status(&self) -> Option<String> {
        let uid = self.shop.open?;
        let stock = self.saved_stock(uid)?;
        let list: Vec<String> = stock.iter().map(|e| if e.unlimited { format!("{}:inf", e.item) } else { format!("{}:{}", e.item, e.count) }).collect();
        let msg = self.shop.msg.as_ref().map_or("", |m| m.0.as_str());
        Some(format!("{} [{}] msg={msg:?} side={:?} sel={:?} scroll={:?}", self.shop_title(uid), list.join(" "), self.shop.side, self.shop.lists.each_ref().map(|l| l.sel), self.shop.lists.each_ref().map(|l| l.scroll)))
    }

    /// Screen position of a row's trade button (for automated play-throughs):
    /// side "buy" or "sell", `k` = row index.
    pub fn shop_button_pos(&self, side: &str, k: usize) -> Option<Vec2> {
        let side = if side == "sell" { ShopSide::Player } else { ShopSide::Merchant };
        let l = layout();
        let vis = k.checked_sub(self.shop.lists[side.idx()].scroll).filter(|v| *v < ROWS)?;
        (k < self.shop_rows(side).len()).then(|| button_rect(row_rect(l.lists[side.idx()], vis)).center())
    }

    /// Handles the trade window. Returns (key consumed, mouse consumed).
    pub(super) fn update_shop_ui(&mut self, allow: bool) -> (bool, bool) {
        if !allow {
            return (false, false);
        }
        if let Some(uid) = self.shop.pending.take() {
            self.inv.close();
            self.shop.open = Some(uid);
            self.shop.side = Some(ShopSide::Merchant);
            self.shop.lists = Default::default();
            self.shop.msg = None;
            self.audio.play(&self.project.config.sfx.open_container);
        }
        if !self.shop.is_open() {
            return (false, false);
        }
        let l = layout();
        let m = input::mouse_pos();
        if input::any_act_pressed(&[Act::Cancel, Act::Character]) || (input::mouse_pressed(MouseButton::Left) && l.close.contains(m)) {
            self.shop.close();
            return (true, true);
        }
        let side = self.shop.side.unwrap_or(ShopSide::Merchant);
        if input::any_act_pressed(&[Act::NextTab, Act::PrevTab, Act::Left, Act::Right]) {
            self.shop.side = Some(side.other());
        }
        let side = self.shop.side.unwrap_or(ShopSide::Merchant);
        let s = side.idx();
        if input::act_pressed(Act::Down) {
            self.shop.lists[s].sel += 1;
        }
        if input::act_pressed(Act::Up) {
            self.shop.lists[s].sel = self.shop.lists[s].sel.saturating_sub(1);
        }
        self.clamp_selection();
        if input::act_pressed(Act::Confirm) && !self.shop_rows(side).is_empty() {
            self.trade(side, self.shop.lists[s].sel);
        }

        // ---- mouse
        // The wheel moves the selection (up / down) of the list under the pointer, else of the
        // active side; the list scrolls along to keep it in view.
        let wheel = input::wheel();
        if wheel != 0.0 {
            let side = [ShopSide::Merchant, ShopSide::Player].into_iter().find(|sd| l.lists[sd.idx()].contains(m)).unwrap_or(side);
            self.shop.side = Some(side);
            let sel = &mut self.shop.lists[side.idx()].sel;
            *sel = if wheel > 0.0 { sel.saturating_sub(1) } else { *sel + 1 };
            self.clamp_selection();
        }
        if input::mouse_pressed(MouseButton::Left) {
            for side in [ShopSide::Merchant, ShopSide::Player] {
                let list = l.lists[side.idx()];
                let n = self.shop_rows(side).len();
                match self.shop.lists[side.idx()].click(m, n, ROWS, |vis| row_rect(list, vis), button_rect) {
                    ListClick::None => {}
                    ListClick::Select => self.shop.side = Some(side),
                    ListClick::Activate(k) => {
                        self.shop.side = Some(side);
                        self.trade(side, k);
                    }
                }
            }
        }
        (true, l.panel.contains(m) || input::mouse_pressed(MouseButton::Left))
    }

    pub(super) fn draw_shop_ui(&self) {
        let Some(uid) = self.shop.open else { return };
        let l = layout();
        let m = input::mouse_pos();
        let p = l.panel;
        draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(0.0, 0.0, 0.0, 0.35));
        panel(p.x, p.y, p.w, p.h);
        let titles = [self.shop_title(uid), "Your items".to_string()];
        let (cur, money) = self.money();
        let side = self.shop.side.unwrap_or(ShopSide::Merchant);
        for (s, list) in l.lists.iter().enumerate() {
            let this = if s == 0 { ShopSide::Merchant } else { ShopSide::Player };
            let title_col = if this == side { GOLD } else { Color::new(0.85, 0.8, 0.6, 1.0) };
            draw_text(&titles[s], list.x, p.y + 32.0, 28.0, title_col);
            draw_rectangle(list.x, list.y, list.w, list.h, Color::new(0.0, 0.0, 0.0, 0.25));
            let rows = self.shop_rows(this);
            if rows.is_empty() {
                let msg = if this == ShopSide::Merchant { "Sold out." } else { "Nothing to sell." };
                draw_text(msg, list.x + 12.0, list.y + 28.0, 22.0, GRAY);
            }
            let scroll = self.shop.lists[s].scroll;
            for (vis, row) in rows.iter().skip(scroll).take(ROWS).enumerate() {
                let k = scroll + vis;
                let r = row_rect(*list, vis);
                let selected = this == side && self.shop.lists[s].sel == k;
                let hover = r.contains(m);
                let bg = if selected {
                    Color::new(0.85, 0.7, 0.3, 0.22)
                } else if hover {
                    Color::new(1.0, 1.0, 1.0, 0.1)
                } else {
                    Color::new(0.0, 0.0, 0.0, 0.25)
                };
                draw_rectangle(r.x, r.y, r.w, r.h, bg);
                if selected {
                    draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, GOLD);
                }
                if let Some(d) = self.project.db.item(&row.item) {
                    self.assets.draw_frame_in(&d.icon.sheet, d.icon.frame, Rect::new(r.x + 4.0, r.y + 4.0, 32.0, 32.0), WHITE);
                }
                let count = match row.count {
                    None => "in stock".to_string(),
                    Some(c) => format!("x{c}"),
                };
                draw_text(self.item_name(&row.item), r.x + 44.0, r.y + 18.0, 21.0, WHITE);
                draw_text(&count, r.x + 44.0, r.y + 36.0, 17.0, LIGHTGRAY);
                // Price button.
                let b = button_rect(r);
                let (label, ok) = match (this, row.price) {
                    (ShopSide::Merchant, Some(pr)) => (format!("Buy {pr}"), money >= pr),
                    (ShopSide::Player, Some(pr)) => (format!("Sell {pr}"), true),
                    (_, None) => ("-".to_string(), false),
                };
                let fill = if !ok {
                    Color::new(0.25, 0.1, 0.1, 0.8)
                } else if b.contains(m) {
                    Color::new(0.45, 0.35, 0.12, 1.0)
                } else {
                    Color::new(0.3, 0.24, 0.1, 0.9)
                };
                draw_rectangle(b.x, b.y, b.w, b.h, fill);
                draw_rectangle_lines(b.x, b.y, b.w, b.h, 1.0, Color::new(0.85, 0.8, 0.6, 0.8));
                let tw = measure_text(&label, None, 18, 1.0).width;
                draw_text(&label, b.x + (b.w - tw) / 2.0, b.y + b.h / 2.0 + 6.0, 18.0, if ok { WHITE } else { Color::new(0.9, 0.5, 0.5, 1.0) });
            }
            // Scroll hints.
            if scroll > 0 {
                draw_text("more above", list.x + list.w - 80.0, list.y - 2.0, 14.0, LIGHTGRAY);
            }
            if rows.len() > scroll + ROWS {
                draw_text("more below", list.x + list.w - 80.0, list.y + list.h + 12.0, 14.0, LIGHTGRAY);
            }
        }
        // Close button.
        let c = l.close;
        draw_rectangle_lines(c.x, c.y, c.w, c.h, 1.5, Color::new(0.85, 0.8, 0.6, 1.0));
        draw_line(c.x + 6.0, c.y + 6.0, c.x + c.w - 6.0, c.y + c.h - 6.0, 2.0, WHITE);
        draw_line(c.x + c.w - 6.0, c.y + 6.0, c.x + 6.0, c.y + c.h - 6.0, 2.0, WHITE);

        // Footer: money, details of the selected item, help.
        let f = l.foot;
        draw_rectangle(f.x, f.y, f.w, f.h, Color::new(0.0, 0.0, 0.0, 0.25));
        if let Some(d) = self.project.db.item(&cur) {
            self.assets.draw_frame_in(&d.icon.sheet, d.icon.frame, Rect::new(f.x + f.w - 150.0, f.y + 8.0, 28.0, 28.0), WHITE);
        }
        draw_text(format!("{money}"), f.x + f.w - 116.0, f.y + 30.0, 26.0, GOLD);
        let sel = self.shop_rows(side).into_iter().nth(self.shop.lists[side.idx()].sel);
        if let Some(d) = sel.as_ref().and_then(|r| self.project.db.item(&r.item)) {
            draw_text(&d.name, f.x + 10.0, f.y + 24.0, 22.0, GOLD);
            for (k, line) in wrap_text(&super::glyphs::expand_keys(&d.description), 18, f.w - 190.0).iter().take(2).enumerate() {
                draw_text(line, f.x + 10.0, f.y + 46.0 + k as f32 * 18.0, 18.0, LIGHTGRAY);
            }
        }
        if let Some((text, ok, t)) = &self.shop.msg {
            let age = (get_time() - t) as f32;
            if age < 4.0 {
                let a = (4.0 - age).min(1.0);
                let c = if *ok { Color::new(0.6, 0.95, 0.6, a) } else { Color::new(1.0, 0.55, 0.5, a) };
                let tw = measure_text(text, None, 20, 1.0).width;
                draw_text(text, f.x + f.w - tw - 10.0, f.y + 62.0, 20.0, c);
            }
        }
        let hint = "Click price / {key:confirm}: trade one   {key:run}: ten   Wheel / {key:up} / {key:down}: select   {key:next_tab}: switch side   {key:cancel}: close";
        super::glyphs::draw_hint(hint, f.x + 10.0, f.y + f.h - 8.0, 16.0, GRAY);
    }
}

/// Adds `count` of `item` to a stock (a new line at `price`) or sets it (0 removes the line);
/// unlimited lines stay unlimited.
fn apply_stock(stock: &mut Vec<ShopEntry>, item: &str, op: crate::events::VarOp, count: u32, price: u32) {
    use crate::events::VarOp;
    match (stock.iter().position(|e| e.item == item), op) {
        (Some(k), _) if stock[k].unlimited => {}
        (Some(k), VarOp::Set) if count == 0 => {
            stock.remove(k);
        }
        (Some(k), VarOp::Set) => stock[k].count = count,
        (Some(k), VarOp::Add) => stock[k].count = stock[k].count.saturating_add(count),
        (None, _) if count > 0 => stock.push(ShopEntry { item: item.to_string(), count, unlimited: false, price }),
        (None, _) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stock_changes() {
        use crate::events::VarOp;
        let mut stock = vec![ShopEntry { item: "apple".into(), count: 0, unlimited: true, price: 0 }];
        apply_stock(&mut stock, "trout", VarOp::Add, 3, 0);
        apply_stock(&mut stock, "trout", VarOp::Add, 2, 9);
        assert_eq!((stock[1].item.as_str(), stock[1].count, stock[1].price), ("trout", 5, 0), "added up, first price kept");
        apply_stock(&mut stock, "trout", VarOp::Set, 4, 0);
        assert_eq!(stock[1].count, 4);
        apply_stock(&mut stock, "apple", VarOp::Set, 1, 0);
        assert!(stock[0].unlimited, "unlimited lines stay");
        apply_stock(&mut stock, "trout", VarOp::Set, 0, 0);
        assert_eq!(stock.len(), 1, "set to 0 removes the line");
        apply_stock(&mut stock, "bread", VarOp::Set, 0, 0);
        assert_eq!(stock.len(), 1, "nothing to remove, nothing added");
    }
}
