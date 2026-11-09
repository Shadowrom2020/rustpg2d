//! The companion window: a companion's gear (paperdoll and equipment slots), its bag and the
//! player's inventory side by side. Opened by clicking a companion's HUD row, *Gear* in the
//! orders menu or `open_companion(name)`; it pauses the game like the inventory.
//!
//! Click a stack to hand it over / take it back (right-click: one), drag an item onto a gear slot
//! (or Enter on a bag item) to equip it, click a worn item to take it off; drag between all
//! three. Tab switches the panel, Esc closes.

use macroquad::prelude::*;

use super::input::{self, Act};
use super::ui::{panel, slot_grid, DragState, SlotEvent, DETAIL_H, PAD, SLOT};
use super::Game;
use crate::defs::EquipSlot;
use crate::inventory::{Inventory, ItemStack};
use crate::render::wrap_text;
use super::text::{draw_text, measure_text};

/// Different kinds of items a companion carries.
pub(super) const BAG_SLOTS: usize = 12;
const BAG_COLS: usize = 4;
const PLAYER_COLS: usize = 6;
const GEAR: [EquipSlot; 6] = [EquipSlot::Clothing, EquipSlot::Head, EquipSlot::Body, EquipSlot::Weapon, EquipSlot::Offhand, EquipSlot::Tool];
const SHEET_W: f32 = 250.0;
const HINT: &str = "Click: give / take (right-click: one) · Drag onto a gear slot (or Enter on a bag item): equip · Tab: next panel · Esc: close";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Pane {
    Gear,
    Bag,
    Player,
}

/// The open companion window.
pub(super) struct CompanionWin {
    /// The companion (runtime entity uid).
    uid: u32,
    /// Its carried items as slots (written back to the entity after every change).
    bag: Inventory,
    sel: (Pane, usize),
    drag: DragState<Pane>,
}

struct Layout {
    window: Rect,
    gear: Vec<Rect>,
    bag: Vec<Rect>,
    player: Vec<Rect>,
    panels: [Rect; 3],
    close: Rect,
    detail: Rect,
}

impl Layout {
    fn hit(&self, p: Vec2) -> Option<(Pane, usize)> {
        [(Pane::Gear, &self.gear), (Pane::Bag, &self.bag), (Pane::Player, &self.player)].into_iter().find_map(|(pane, rs)| rs.iter().position(|r| r.contains(p)).map(|i| (pane, i)))
    }
    fn slots(&self, pane: Pane) -> &[Rect] {
        match pane {
            Pane::Gear => &self.gear,
            Pane::Bag => &self.bag,
            Pane::Player => &self.player,
        }
    }
}


impl Game {
    pub(super) fn companion_win_open(&self) -> bool {
        self.comp_win.is_some()
    }

    /// The companion window's entity index, if open (and the companion still here).
    fn comp_index(&self) -> Option<usize> {
        let uid = self.comp_win.as_ref()?.uid;
        self.loaded.entities.iter().position(|e| e.uid == uid && e.active && e.companion.is_some())
    }

    /// Opens the window for companion `i` (`Action::OpenCompanion`, HUD row, orders menu).
    pub(super) fn open_companion_window(&mut self, i: usize) {
        let e = &self.loaded.entities[i];
        if e.companion.is_none() {
            return;
        }
        let mut bag = Inventory::new(BAG_SLOTS.max(e.inventory.len()));
        bag.set_limits(self.world.inventory.limits());
        for s in &e.inventory {
            bag.add(&s.item, s.count);
        }
        self.inv.close();
        self.comp_win = Some(CompanionWin { uid: e.uid, bag, sel: (Pane::Bag, 0), drag: DragState::default() });
        self.audio.play(&self.project.config.sfx.open_container);
    }


    /// Whether companion `i` may wear gear (`CharacterDef::gear_locked`).
    fn wears_gear(&self, i: usize) -> bool {
        !self.project.db.character(&self.loaded.entities[i].def).is_some_and(|d| d.gear_locked)
    }

    /// Writes the window's bag back to the companion (saved) and updates its fighting values.
    fn store_bag(&mut self, i: usize) {
        let Some(w) = &self.comp_win else { return };
        let mut list: Vec<ItemStack> = Vec::new();
        for s in w.bag.slots.iter().flatten() {
            match list.iter_mut().find(|x| x.item == s.item) {
                Some(x) => x.count += s.count,
                None => list.push(s.clone()),
            }
        }
        self.loaded.entities[i].inventory = list;
        super::npc_items::apply_gear(&self.project.db, &mut self.loaded.entities[i]);
        self.persist_npc_items(i);
    }

    fn pane_stack(&self, pane: Pane, k: usize) -> Option<ItemStack> {
        let i = self.comp_index()?;
        match pane {
            Pane::Gear => self.loaded.entities[i].equipment.get(*GEAR.get(k)?).cloned(),
            Pane::Bag => self.comp_win.as_ref()?.bag.slots.get(k).cloned().flatten(),
            Pane::Player => self.world.inventory.slots.get(k).cloned().flatten(),
        }
    }

    /// Moves `count` (None = all) of a stack between the bag and the player's inventory.
    fn comp_move(&mut self, from: Pane, k: usize, count: Option<u32>) {
        let Some(i) = self.comp_index() else { return };
        let Some(w) = self.comp_win.as_mut() else { return };
        let (src, dst) = match from {
            Pane::Bag => (&mut w.bag, &mut self.world.inventory),
            Pane::Player => (&mut self.world.inventory, &mut w.bag),
            Pane::Gear => {
                self.comp_unequip(k, Pane::Bag, None);
                return;
            }
        };
        let had = src.slots.get(k).is_some_and(|s| s.is_some());
        let moved = Inventory::transfer(src, k, dst, count);
        if had && moved == 0 {
            self.ui.toast(if from == Pane::Bag { "Inventory full".into() } else { "Their bag is full".into() });
        } else if had {
            self.audio.play(&self.project.config.sfx.pickup);
        }
        self.store_bag(i);
    }

    /// Puts the item of `pane` slot `k` on the companion (into the slot it belongs in); what it
    /// wore there goes into its bag (or back to the player when the bag is full).
    fn comp_equip(&mut self, pane: Pane, k: usize) {
        let Some(i) = self.comp_index() else { return };
        let Some(stack) = self.pane_stack(pane, k) else { return };
        let Some(slot) = self.project.db.item(&stack.item).and_then(|d| d.equip_slot()) else {
            self.ui.toast("That can't be worn or held.".into());
            return;
        };
        if !self.wears_gear(i) {
            let name = self.entity_display_name(self.loaded.entities[i].uid).unwrap_or_default();
            self.ui.toast(format!("{name} can't wear gear."));
            return;
        }
        let Some(w) = self.comp_win.as_mut() else { return };
        let taken = match pane {
            Pane::Bag => w.bag.take_from_slot(k, Some(1)),
            Pane::Player => self.world.inventory.take_from_slot(k, Some(1)),
            Pane::Gear => None,
        };
        let Some(one) = taken else { return };
        let old = self.loaded.entities[i].equipment.slot_mut(slot).replace(one);
        if let Some(old) = old {
            let w = self.comp_win.as_mut().expect("open");
            let left = w.bag.add(&old.item, old.count);
            if left > 0 {
                let back = self.world.inventory.add(&old.item, left);
                if back > 0 {
                    // Nowhere to put it: on the ground at the player's feet.
                    let p = self.player.pos;
                    self.place_drop(&old.item, back, p);
                }
            }
        }
        self.audio.play(&self.project.config.sfx.pickup);
        self.store_bag(i);
    }

    /// Takes off what the companion wears in gear slot `k` into `to` (slot `at`, or anywhere).
    fn comp_unequip(&mut self, k: usize, to: Pane, at: Option<usize>) {
        let Some(i) = self.comp_index() else { return };
        let Some(slot) = GEAR.get(k).copied() else { return };
        let Some(stack) = self.loaded.entities[i].equipment.get(slot).cloned() else { return };
        if !self.wears_gear(i) {
            let name = self.entity_display_name(self.loaded.entities[i].uid).unwrap_or_default();
            self.ui.toast(format!("{name} keeps what it wears."));
            return;
        }
        let w = self.comp_win.as_mut().expect("open");
        let dst = if to == Pane::Player { &mut self.world.inventory } else { &mut w.bag };
        let fits = match at {
            Some(j) if dst.slots.get(j).is_some_and(|s| s.is_none()) => {
                dst.slots[j] = Some(stack.clone());
                true
            }
            _ => dst.add(&stack.item, stack.count) == 0,
        };
        if !fits {
            self.ui.toast(if to == Pane::Player { "Inventory full".into() } else { "Their bag is full".into() });
            return;
        }
        self.loaded.entities[i].equipment.slot_mut(slot).take();
        self.store_bag(i);
    }

    /// Click / Enter on a slot: a worn item goes into the bag, a stack to the other side.
    fn comp_activate(&mut self, pane: Pane, k: usize) {
        if self.pane_stack(pane, k).is_none() {
            return;
        }
        match pane {
            Pane::Gear => self.comp_unequip(k, Pane::Bag, None),
            Pane::Bag | Pane::Player => self.comp_move(pane, k, None),
        }
    }

    /// Drag and drop between two slots.
    fn comp_drop(&mut self, from: (Pane, usize), to: (Pane, usize)) {
        if from == to {
            return;
        }
        let Some(i) = self.comp_index() else { return };
        match (from.0, to.0) {
            (Pane::Bag | Pane::Player, Pane::Gear) => {
                let fits = self.pane_stack(from.0, from.1).and_then(|s| self.project.db.item(&s.item)?.equip_slot()) == GEAR.get(to.1).copied();
                if fits {
                    self.comp_equip(from.0, from.1);
                } else {
                    self.ui.toast(format!("That doesn't go in the {} slot", GEAR[to.1].label().to_lowercase()));
                }
            }
            (Pane::Gear, Pane::Bag | Pane::Player) => self.comp_unequip(from.1, to.0, Some(to.1)),
            (Pane::Gear, Pane::Gear) => {}
            (a, b) if a == b => {
                let w = self.comp_win.as_mut().expect("open");
                let inv = if a == Pane::Bag { &mut w.bag } else { &mut self.world.inventory };
                inv.move_slot(from.1, to.1);
                self.store_bag(i);
            }
            (a, _) => {
                let w = self.comp_win.as_mut().expect("open");
                let (src, dst) = if a == Pane::Bag { (&mut w.bag, &mut self.world.inventory) } else { (&mut self.world.inventory, &mut w.bag) };
                Inventory::transfer_to_slot(src, from.1, dst, to.1);
                self.store_bag(i);
            }
        }
    }

    fn comp_layout(&self) -> Layout {
        let (sw, sh) = (screen_width(), screen_height());
        let cap = self.world.inventory.capacity();
        let bag_n = self.comp_win.as_ref().map_or(BAG_SLOTS, |w| w.bag.capacity());
        let (_, bag_size) = slot_grid(Vec2::ZERO, bag_n, BAG_COLS);
        let (_, player_size) = slot_grid(Vec2::ZERO, cap, PLAYER_COLS);
        let h = (40.0 + GEAR.len() as f32 * 50.0 + 120.0).max(bag_size.y).max(player_size.y);
        let total = SHEET_W + 16.0 + bag_size.x + 16.0 + player_size.x;
        let x0 = ((sw - total) / 2.0).round();
        let top = ((sh - h - DETAIL_H - 6.0) / 2.0).max(10.0).round();
        let gear = (0..GEAR.len()).map(|k| Rect::new(x0 + 124.0, top + 50.0 + k as f32 * 50.0, SLOT, SLOT)).collect();
        let bx = x0 + SHEET_W + 16.0;
        let (bag, _) = slot_grid(vec2(bx, top), bag_n, BAG_COLS);
        let px = bx + bag_size.x + 16.0;
        let (player, _) = slot_grid(vec2(px, top), cap, PLAYER_COLS);
        let panels = [Rect::new(x0, top, SHEET_W, h), Rect::new(bx, top, bag_size.x, h), Rect::new(px, top, player_size.x, h)];
        let window = Rect::new(x0, top, total, h + 6.0 + DETAIL_H);
        Layout { window, gear, bag, player, panels, close: Rect::new(px + player_size.x - 30.0, top + 8.0, 22.0, 22.0), detail: Rect::new(x0, top + h + 6.0, total, DETAIL_H) }
    }

    /// Screen center of a slot for automated play-throughs: "gear", "bag" or "player".
    pub(super) fn comp_slot_pos(&self, side: &str, k: usize) -> Option<Vec2> {
        self.comp_win.as_ref()?;
        let l = self.comp_layout();
        let pane = match side {
            "gear" | "equip" => Pane::Gear,
            "bag" | "companion" | "chest" => Pane::Bag,
            _ => Pane::Player,
        };
        l.slots(pane).get(k).map(|r| r.center())
    }

    /// Input of the open window. Returns true while open (it takes all input).
    pub(super) fn update_companion_window(&mut self) -> bool {
        // Opened by an event (`open_companion`) once it is over.
        if let Some(uid) = self.pending_companion.take() {
            if let Some(i) = self.loaded.entities.iter().position(|e| e.uid == uid && e.active) {
                self.open_companion_window(i);
            }
        }
        if self.comp_win.is_none() {
            return false;
        }
        if self.comp_index().is_none() {
            self.comp_win = None;
            return false;
        }
        let l = self.comp_layout();
        let m = input::mouse_pos();
        if input::act_pressed(Act::Cancel) || input::act_pressed(Act::Character) {
            self.comp_win = None;
            return true;
        }
        // ---- keyboard
        let (mut pane, mut k) = self.comp_win.as_ref().map(|w| w.sel).expect("open");
        if input::any_act_pressed(&[Act::NextTab]) {
            pane = match pane {
                Pane::Gear => Pane::Bag,
                Pane::Bag => Pane::Player,
                Pane::Player => Pane::Gear,
            };
            k = 0;
        }
        let cols = match pane {
            Pane::Gear => 1,
            Pane::Bag => BAG_COLS,
            Pane::Player => PLAYER_COLS,
        } as i32;
        let n = l.slots(pane).len() as i32;
        let mut j = k as i32;
        if input::act_pressed(Act::Left) {
            j -= 1;
        }
        if input::act_pressed(Act::Right) {
            j += 1;
        }
        if input::act_pressed(Act::Up) {
            j -= cols;
        }
        if input::act_pressed(Act::Down) {
            j += cols;
        }
        k = j.clamp(0, (n - 1).max(0)) as usize;
        if let Some(w) = self.comp_win.as_mut() {
            w.sel = (pane, k);
        }
        if input::act_pressed(Act::Confirm) {
            // Enter on a bag item that can be worn equips it; else it moves over.
            let wearable = pane == Pane::Bag && self.pane_stack(pane, k).and_then(|s| self.project.db.item(&s.item)?.equip_slot()).is_some();
            if wearable {
                self.comp_equip(pane, k);
            } else {
                self.comp_activate(pane, k);
            }
        }
        // ---- mouse
        let (click, rclick) = (input::mouse_pressed(MouseButton::Left), input::mouse_pressed(MouseButton::Right));
        let hit = l.hit(m);
        if click {
            if l.close.contains(m) {
                self.comp_win = None;
                return true;
            }
            match hit {
                Some(at) => {
                    if let Some(w) = self.comp_win.as_mut() {
                        w.drag.press(at, m);
                    }
                }
                None if !l.window.contains(m) => {
                    self.comp_win = None;
                    return true;
                }
                None => {}
            }
        }
        let event = self.comp_win.as_mut().map_or(SlotEvent::None, |w| w.drag.update(m, hit));
        match event {
            SlotEvent::Dropped { from, to: Some(to) } => {
                self.comp_drop(from, to);
                if let Some(w) = self.comp_win.as_mut() {
                    w.sel = to;
                }
            }
            SlotEvent::Dropped { .. } => {}
            SlotEvent::Clicked { at: (p, s), .. } => {
                if let Some(w) = self.comp_win.as_mut() {
                    w.sel = (p, s);
                }
                self.comp_activate(p, s);
            }
            SlotEvent::None => {}
        }
        if rclick {
            if let Some((p, s)) = hit {
                if let Some(w) = self.comp_win.as_mut() {
                    w.sel = (p, s);
                }
                if p == Pane::Gear {
                    self.comp_unequip(s, Pane::Bag, None);
                } else {
                    self.comp_move(p, s, Some(1));
                }
            }
        }
        true
    }

    pub(super) fn draw_companion_window(&self) {
        let (Some(w), Some(i)) = (&self.comp_win, self.comp_index()) else { return };
        let l = self.comp_layout();
        let m = input::mouse_pos();
        let hover = l.hit(m);
        let dragging = w.drag.dragging();
        draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(0.0, 0.0, 0.0, 0.35));
        let e = &self.loaded.entities[i];
        let name = self.entity_display_name(e.uid).unwrap_or_else(|| e.name.clone());
        let titles = [name.clone(), format!("{name}'s bag"), format!("Inventory  {}/{}", self.world.inventory.used_slots(), self.world.inventory.capacity())];
        for (p, t) in l.panels.iter().zip(&titles) {
            panel(p.x, p.y, p.w, p.h);
            draw_text(t, p.x + PAD, p.y + 28.0, 24.0, GOLD);
        }
        // The companion as dressed now, its HP, defense and weapon.
        let pr = Rect::new(l.panels[0].x + PAD, l.panels[0].y + 50.0, 96.0, 150.0);
        draw_rectangle(pr.x, pr.y, pr.w, pr.h, Color::new(0.0, 0.0, 0.0, 0.3));
        if !e.appearance.body.is_empty() {
            let (fw, fh) = self.character_frame_size(&e.appearance);
            let s = ((pr.w - 8.0) / fw).min((pr.h - 8.0) / fh);
            let (dw, dh) = (fw * s, fh * s);
            self.draw_character_in(&e.appearance, &e.equipment, Rect::new(pr.x + (pr.w - dw) / 2.0, pr.bottom() - 4.0 - dh, dw, dh));
        }
        let locked = !self.wears_gear(i);
        for (k, r) in l.gear.iter().enumerate() {
            let key = (Pane::Gear, k);
            self.comp_slot_frame(r, hover == Some(key), w.sel == key);
            let slot = GEAR[k];
            draw_text(slot.label(), r.x + r.w + 6.0, r.y + 18.0, 16.0, LIGHTGRAY);
            match e.equipment.get(slot).and_then(|s| self.project.db.item(&s.item)) {
                Some(d) => {
                    let tint = if dragging == Some(key) { Color::new(1.0, 1.0, 1.0, 0.3) } else { WHITE };
                    self.assets.draw_frame_in(&d.icon.sheet, d.icon.frame, Rect::new(r.x + 8.0, r.y + 8.0, 32.0, 32.0), tint);
                    for (n, line) in wrap_text(&d.name, 14, 70.0).iter().take(2).enumerate() {
                        draw_text(line, r.x + r.w + 6.0, r.y + 33.0 + n as f32 * 12.0, 14.0, WHITE);
                    }
                }
                None => {
                    draw_text(if locked { "-" } else { "empty" }, r.x + r.w + 6.0, r.y + 35.0, 14.0, GRAY);
                }
            }
        }
        let wpn = e.weapon.as_ref().map_or("none".to_string(), |w| format!("{} dmg", w.damage));
        let stats = [format!("Health  {} / {}", e.hp, e.max_hp), format!("Defense {}", e.defense), format!("Weapon  {wpn}")];
        let sy = l.gear.last().map_or(pr.bottom(), |r| r.bottom()) + 24.0;
        for (n, t) in stats.iter().enumerate() {
            draw_text(t, l.panels[0].x + PAD, sy + n as f32 * 20.0, 18.0, WHITE);
        }
        if locked {
            draw_text("Can't wear gear", l.panels[0].x + PAD, sy + 64.0, 16.0, GRAY);
        }
        for (pane, rects, inv) in [(Pane::Bag, &l.bag, &w.bag), (Pane::Player, &l.player, &self.world.inventory)] {
            for (k, r) in rects.iter().enumerate() {
                let key = (pane, k);
                self.comp_slot_frame(r, hover == Some(key), w.sel == key);
                let Some(s) = inv.slots.get(k).cloned().flatten() else { continue };
                let tint = if dragging == Some(key) { Color::new(1.0, 1.0, 1.0, 0.3) } else { WHITE };
                if let Some(d) = self.project.db.item(&s.item) {
                    self.assets.draw_frame_in(&d.icon.sheet, d.icon.frame, Rect::new(r.x + 8.0, r.y + 8.0, 32.0, 32.0), tint);
                }
                if s.count > 1 {
                    let t = s.count.to_string();
                    let tw = measure_text(&t, None, 18, 1.0).width;
                    draw_text(&t, r.right() - tw - 3.0, r.bottom() - 4.0, 18.0, WHITE);
                }
            }
        }
        let c = l.close;
        draw_text("x", c.x + 6.0, c.y + 17.0, 24.0, if c.contains(m) { WHITE } else { GRAY });
        // Details of the hovered / selected stack (count and stats like the inventory's) and the help line.
        let focus = hover.unwrap_or(w.sel);
        let stack = self.pane_stack(focus.0, focus.1);
        super::ui::draw_item_detail(&self.project.db, l.detail, PAD, stack.as_ref(), HINT);
        if let Some((p, s)) = dragging {
            if let Some(def) = self.pane_stack(p, s).and_then(|s| self.project.db.item(&s.item)) {
                self.assets.draw_frame_in(&def.icon.sheet, def.icon.frame, Rect::new(m.x - 16.0, m.y - 16.0, 32.0, 32.0), WHITE);
            }
        }
    }

    fn comp_slot_frame(&self, r: &Rect, hover: bool, selected: bool) {
        draw_rectangle(r.x, r.y, r.w, r.h, if hover { Color::new(1.0, 1.0, 1.0, 0.16) } else { Color::new(0.0, 0.0, 0.0, 0.35) });
        draw_rectangle_lines(r.x, r.y, r.w, r.h, if selected { 3.0 } else { 1.0 }, if selected { GOLD } else { Color::new(0.85, 0.8, 0.6, 0.35) });
    }

    /// `status()`: the companion whose window is open, its bag in window order.
    pub(super) fn companion_win_status(&self) -> String {
        match (&self.comp_win, self.comp_index()) {
            (Some(w), Some(i)) => {
                let bag: Vec<String> = w.bag.slots.iter().flatten().map(|s| format!("{}x{}", s.item, s.count)).collect();
                format!("{}[{}]", self.loaded.entities[i].name, bag.join(","))
            }
            _ => "-".into(),
        }
    }
}
