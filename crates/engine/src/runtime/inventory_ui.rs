//! Inventory (the character window's Character tab: sheet + grid) and the container loot
//! window (grid of slots, mouse + keyboard), the quick bar and the HUD buttons.
//!
//! Inventory: click selects, double-click / right-click / Enter uses or equips, drag rearranges.
//! Loot: click a container stack to take it, right-click takes one, click your own stack to
//! store it, drag between the grids, "Take all" (T). Esc / I closes.

use macroquad::prelude::*;

use super::input::{self, Act};
use super::ui::{panel, slot_grid, slot_grid_size, DragState, SlotEvent, DETAIL_H, GAP, PAD, SLOT};
use super::Game;
use crate::defs::EquipSlot;
use crate::inventory::{Inventory, ItemStack};
use crate::render::wrap_text;
use super::text::{draw_text, measure_text};


#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Side {
    Player,
    Container,
    /// Character sheet equipment slots (0 = head, 1 = body, 2 = weapon).
    Equip,
    /// Quick action bar (shortcuts to item types, not the items themselves).
    Quick,
}

/// Number of quick bar slots (keys 1-8).
pub const QUICK_SLOTS: usize = 8;
const QUICK_SLOT: f32 = 44.0;

/// Screen rects of the quick bar slots (bottom center).
fn quick_rects() -> Vec<Rect> {
    let (sw, sh) = (screen_width(), screen_height());
    let w = QUICK_SLOTS as f32 * (QUICK_SLOT + GAP) - GAP;
    let x0 = ((sw - w) / 2.0).round();
    (0..QUICK_SLOTS).map(|k| Rect::new(x0 + k as f32 * (QUICK_SLOT + GAP), sh - QUICK_SLOT - 12.0, QUICK_SLOT, QUICK_SLOT)).collect()
}

const EQUIP_SLOTS: [EquipSlot; 6] = [EquipSlot::Clothing, EquipSlot::Head, EquipSlot::Body, EquipSlot::Weapon, EquipSlot::Offhand, EquipSlot::Tool];
const SHEET_W: f32 = 270.0;

#[derive(Default)]
pub(super) struct InvUi {
    /// Player inventory window open.
    open: bool,
    /// Entity uid of the container being looted.
    loot: Option<u32>,
    /// Container to open once the current event sequence has finished.
    pub pending_loot: Option<u32>,
    sel: Option<(Side, usize)>,
    drag: DragState<Side>,
    /// Quick bar slot that was just used (for a short highlight).
    quick_flash: Option<(usize, f64)>,
}

impl InvUi {
    pub fn is_open(&self) -> bool {
        self.open || self.loot.is_some()
    }
    pub fn close(&mut self) {
        self.open = false;
        self.loot = None;
        self.drag = DragState::default();
    }
}

struct Grid {
    side: Side,
    title: String,
    panel: Rect,
    slots: Vec<Rect>,
    cols: usize,
}

struct Layout {
    /// The character window around the Character tab (None while looting).
    window: Option<Rect>,
    grids: Vec<Grid>,
    take_all: Option<Rect>,
    close: Rect,
    detail: Rect,
}

impl Layout {
    fn hit(&self, p: Vec2) -> Option<(Side, usize)> {
        self.grids.iter().find_map(|g| g.slots.iter().position(|r| r.contains(p)).map(|i| (g.side, i)))
    }
    fn grid(&self, side: Side) -> Option<&Grid> {
        self.grids.iter().find(|g| g.side == side)
    }
    fn contains(&self, p: Vec2) -> bool {
        self.window.is_some_and(|w| w.contains(p)) || self.grids.iter().any(|g| g.panel.contains(p)) || self.detail.contains(p)
    }
}

/// Buttons in the bottom-right corner: the character window (character, inventory, skills,
/// spells) and the map.
#[derive(Clone, Copy, PartialEq, Eq)]
enum HudButton {
    Window,
    Map,
}

/// Help line of the inventory (Character tab).
const INVENTORY_HINT: &str = "Double-click / {key:confirm}: use or equip · Drag to slots or quick bar (1-8) · {key:drop} / drag out: drop ({key:run}+{key:drop}: one) · {key:next_tab}: next tab · {key:cancel}: close";

impl Game {
    /// The HUD buttons with their screen rects, right to left.
    fn hud_buttons(&self) -> Vec<(HudButton, Rect)> {
        let (sw, sh) = (screen_width(), screen_height());
        [HudButton::Window, HudButton::Map].into_iter().enumerate().map(|(k, b)| (b, Rect::new(sw - 64.0 - k as f32 * 60.0, sh - 64.0, 52.0, 52.0))).collect()
    }

    /// Screen position of a HUD button ("window" - also "character", "bag", "skills" - or
    /// "map") for automated play-throughs.
    pub fn hud_button_pos(&self, name: &str) -> Option<Vec2> {
        let want = if name == "map" { HudButton::Map } else { HudButton::Window };
        self.hud_buttons().into_iter().find(|(b, _)| *b == want).map(|(_, r)| r.center())
    }

    fn draw_hud_buttons(&self) {
        let m = input::mouse_pos();
        for (b, r) in self.hud_buttons() {
            let hover = r.contains(m);
            draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.06, 0.06, 0.12, if hover { 0.95 } else { 0.7 }));
            draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, Color::new(0.85, 0.8, 0.6, 1.0));
            let key = match b {
                HudButton::Window => {
                    // Head and shoulders with a bag at the side: character, inventory, skills.
                    let c = Color::new(0.85, 0.8, 0.6, 1.0);
                    draw_circle(r.x + 22.0, r.y + 18.0, 8.0, c);
                    // Shoulders: the upper half of a disc, ending at the button's bottom edge.
                    let (cx, cy, rad) = (r.x + 22.0, r.y + 46.0, 15.0);
                    let n = 16;
                    for k in 0..n {
                        let a0 = std::f32::consts::PI * (1.0 + k as f32 / n as f32);
                        let a1 = std::f32::consts::PI * (1.0 + (k + 1) as f32 / n as f32);
                        draw_triangle(vec2(cx, cy), vec2(cx + a0.cos() * rad, cy + a0.sin() * rad), vec2(cx + a1.cos() * rad, cy + a1.sin() * rad), c);
                    }
                    let bag = Color::new(0.65, 0.45, 0.25, 1.0);
                    draw_circle(r.x + 38.0, r.y + 30.0, 7.0, bag);
                    draw_rectangle(r.x + 34.5, r.y + 20.0, 7.0, 5.0, bag);
                    "C"
                }
                HudButton::Map => {
                    // A folded parchment map with a red cross.
                    let paper = Color::new(0.88, 0.8, 0.6, 1.0);
                    let fold = Color::new(0.7, 0.6, 0.42, 1.0);
                    let (x, y) = (r.x + 9.0, r.y + 12.0);
                    for k in 0..3 {
                        let fx = x + k as f32 * 11.0;
                        let (top, bottom) = if k % 2 == 0 { (0.0, 3.0) } else { (3.0, 0.0) };
                        draw_triangle(vec2(fx, y + top), vec2(fx + 11.0, y + bottom), vec2(fx, y + 26.0 + top), if k == 1 { fold } else { paper });
                        draw_triangle(vec2(fx + 11.0, y + bottom), vec2(fx + 11.0, y + 26.0 + bottom), vec2(fx, y + 26.0 + top), if k == 1 { fold } else { paper });
                    }
                    let brown = Color::new(0.45, 0.3, 0.15, 1.0);
                    draw_line(x + 4.0, y + 20.0, x + 14.0, y + 12.0, 1.5, brown);
                    draw_line(x + 14.0, y + 12.0, x + 22.0, y + 16.0, 1.5, brown);
                    let red = Color::new(0.85, 0.15, 0.1, 1.0);
                    draw_line(x + 22.0, y + 6.0, x + 28.0, y + 12.0, 2.0, red);
                    draw_line(x + 28.0, y + 6.0, x + 22.0, y + 12.0, 2.0, red);
                    "M"
                }
            };
            draw_text(key, r.x + 40.0, r.y + 48.0, 16.0, LIGHTGRAY);
        }
    }
}

fn grid_size(n: usize, cols: usize) -> Vec2 {
    slot_grid_size(n, cols)
}

/// Vertical distance of the equipment slots.
const EQUIP_STEP: f32 = 50.0;

/// Height of the character sheet panel.
const SHEET_H: f32 = 530.0;

/// Container panels are at least this wide (title + "Take all" button).
const MIN_CONTAINER_W: f32 = 300.0;

fn make_grid(side: Side, title: String, origin: Vec2, n: usize, cols: usize) -> Grid {
    let (slots, mut size) = slot_grid(origin, n, cols);
    if side == Side::Container {
        size.x = size.x.max(MIN_CONTAINER_W);
    }
    Grid { side, title, panel: Rect::new(origin.x, origin.y, size.x, size.y), slots, cols }
}

impl Game {
    /// A container's loot window is open.
    pub(super) fn looting(&self) -> bool {
        self.inv.loot.is_some()
    }

    /// Opens the inventory (Character tab) with slot 0 of `side` selected.
    pub(super) fn open_inventory(&mut self, side: Side) {
        if !self.inv.open {
            self.inv.open = true;
            self.inv.sel = Some((side, 0));
        }
    }

    pub(super) fn inv_select(&mut self, side: Side, i: usize) {
        self.inv.sel = Some((side, i));
    }

    /// Size of the Character tab's contents: character sheet + inventory grid, and the details
    /// line below.
    pub(super) fn character_content_size(&self) -> Vec2 {
        let g = grid_size(self.world.inventory.capacity(), 6);
        vec2(SHEET_W + 24.0 + g.x, g.y.max(SHEET_H) + 6.0 + DETAIL_H)
    }

    pub(super) fn inventory_open(&self) -> bool {
        self.inv.is_open() || self.shop.is_open() || self.crafting.is_open() || self.journal.is_open() || self.skills_open() || self.companion_win_open()
    }

    fn container_name(&self, uid: u32) -> String {
        let def = self.loaded.map.entity(uid).map(|e| e.def.clone()).unwrap_or_default();
        self.project.db.prop(&def).map_or("Container".into(), |d| d.name.clone())
    }

    /// The contents of a container entity on the current map (created from its spec on first use).
    pub(super) fn container_inv(&mut self, uid: u32) -> Option<&mut Inventory> {
        let map = self.loaded.map.name.clone();
        let limits = self.world.inventory.limits();
        if self.world.containers.get(&map).is_none_or(|m| !m.contains_key(&uid)) {
            let e = self.loaded.map.entity(uid)?;
            let spec = self.project.db.container_of(e)?;
            let inv = Inventory::with_stacks(spec.size as usize, &spec.items, limits.clone());
            self.world.containers.entry(map.clone()).or_default().insert(uid, inv);
        }
        let inv = self.world.containers.get_mut(&map)?.get_mut(&uid)?;
        inv.set_limits(limits);
        Some(inv)
    }

    /// Opens a container if its conditions hold (called after its Interact triggers were queued).
    pub(super) fn try_open_container(&mut self, i: usize) {
        let uid = self.loaded.entities[i].uid;
        let Some(spec) = self.loaded.map.entity(uid).and_then(|e| self.project.db.container_of(e)).cloned() else {
            return;
        };
        let ctx = crate::events::runner::ActionCtx { owner: Some(uid) };
        if !crate::events::runner::conditions_met(self, &spec.conditions, &ctx) {
            let msg = if spec.locked_text.is_empty() { "It's locked.".to_string() } else { crate::lang::tr(&spec.locked_text) };
            self.ui.toast(msg);
            return;
        }
        if !spec.open_state.is_empty() {
            let open = self.project.db.prop(&self.loaded.entities[i].def).and_then(|d| d.state_index(&spec.open_state));
            if open.is_some_and(|s| s != self.loaded.entities[i].state) {
                self.set_prop_state(i, &spec.open_state);
            }
        } else if spec.open_frame > 0 && self.loaded.entities[i].frame != spec.open_frame {
            self.loaded.entities[i].frame = spec.open_frame;
            self.persist(i);
        }
        self.container_inv(uid);
        self.inv.pending_loot = Some(uid);
    }

    fn layout(&self) -> Layout {
        let (sw, sh) = (screen_width(), screen_height());
        let cap = self.world.inventory.capacity();
        let player_title = format!("Inventory  {}/{}", self.world.inventory.used_slots(), cap);
        let cols = 6;
        let mut grids = Vec::new();
        let mut take_all = None;
        match self.inv.loot {
            None => {
                // Character sheet on the left, inventory grid on the right (in the character
                // window's content area).
                let c = self.window_content();
                let size = grid_size(cap, cols);
                let h = size.y.max(SHEET_H);
                let total = SHEET_W + 24.0 + size.x;
                let top = c.y + super::charwin::MARGIN;
                let x0 = (c.x + (c.w - total) / 2.0).round();
                let slots = (0..EQUIP_SLOTS.len()).map(|k| Rect::new(x0 + 134.0, top + 50.0 + k as f32 * EQUIP_STEP, SLOT, SLOT)).collect();
                grids.push(Grid { side: Side::Equip, title: "Character".into(), panel: Rect::new(x0, top, SHEET_W, h), slots, cols: 1 });
                let mut g = make_grid(Side::Player, player_title, vec2(x0 + SHEET_W + 24.0, top), cap, cols);
                g.panel.h = h;
                grids.push(g);
            }
            Some(uid) => {
                let n = self.world.containers.get(&self.loaded.map.name).and_then(|m| m.get(&uid)).map_or(1, |c| c.capacity());
                let ccols = n.clamp(1, 6);
                let (mut a, b) = (grid_size(n, ccols), grid_size(cap, cols));
                a.x = a.x.max(MIN_CONTAINER_W);
                let total = a.x + 24.0 + b.x;
                let top = ((sh - a.y.max(b.y) - DETAIL_H) / 2.0).max(10.0).round();
                let x0 = ((sw - total) / 2.0).round();
                let g = make_grid(Side::Container, self.container_name(uid), vec2(x0, top), n, ccols);
                take_all = Some(Rect::new(g.panel.x + g.panel.w - 104.0, g.panel.y + 8.0, 90.0, 26.0));
                grids.push(g);
                grids.push(make_grid(Side::Player, player_title, vec2(x0 + a.x + 24.0, top), cap, cols));
            }
        }
        let left = grids.iter().map(|g| g.panel.x).fold(f32::MAX, f32::min);
        let right = grids.iter().map(|g| g.panel.x + g.panel.w).fold(0.0, f32::max);
        let bottom = grids.iter().map(|g| g.panel.y + g.panel.h).fold(0.0, f32::max);
        let last = grids.last().unwrap().panel;
        let slots = quick_rects();
        let bar = Rect::new(slots[0].x - 6.0, slots[0].y - 6.0, slots[QUICK_SLOTS - 1].right() - slots[0].x + 12.0, QUICK_SLOT + 12.0);
        grids.push(Grid { side: Side::Quick, title: String::new(), panel: bar, slots, cols: QUICK_SLOTS });
        let close = if self.inv.loot.is_some() { Rect::new(last.x + last.w - 30.0, last.y + 8.0, 22.0, 22.0) } else { self.window_close() };
        Layout {
            window: self.inv.loot.is_none().then(|| self.window_rect()),
            close,
            detail: match self.inv.loot {
                // The character window: across its content.
                None => {
                    let c = self.window_content();
                    Rect::new(c.x + super::charwin::MARGIN, bottom + 6.0, c.w - 2.0 * super::charwin::MARGIN, DETAIL_H)
                }
                // Loot: at least wide enough for the help line.
                Some(_) => {
                    let w = (right - left).max(super::glyphs::measure_hint(self.hint(), 16.0) + 2.0 * PAD);
                    Rect::new(((left + right) / 2.0 - w / 2.0).round(), bottom + 6.0, w, DETAIL_H)
                }
            },
            grids,
            take_all,
        }
    }

    /// Screen center of a slot (for automated play-throughs): side "player", "chest" or "equip".
    pub fn slot_screen_pos(&self, side: &str, i: usize) -> Option<Vec2> {
        if self.companion_win_open() {
            return self.comp_slot_pos(side, i);
        }
        let side = match side {
            "chest" => Side::Container,
            "equip" => Side::Equip,
            "quick" => Side::Quick,
            _ => Side::Player,
        };
        let l = self.layout();
        l.grid(side)?.slots.get(i).map(|r| r.center())
    }

    fn inv_of(&mut self, side: Side) -> Option<&mut Inventory> {
        match side {
            Side::Player => Some(&mut self.world.inventory),
            Side::Container => {
                let uid = self.inv.loot?;
                self.container_inv(uid)
            }
            Side::Equip | Side::Quick => None,
        }
    }

    fn slot_item(&mut self, side: Side, i: usize) -> Option<String> {
        self.slot_stack(side, i).map(|s| s.item)
    }

    /// The stack shown in a slot of any panel.
    fn slot_stack(&self, side: Side, i: usize) -> Option<ItemStack> {
        match side {
            Side::Player => self.world.inventory.slots.get(i).cloned().flatten(),
            Side::Container => self.inv.loot.and_then(|u| self.world.containers.get(&self.loaded.map.name)?.get(&u)?.slots.get(i).cloned().flatten()),
            Side::Equip => EQUIP_SLOTS.get(i).and_then(|s| self.world.equipment.get(*s).cloned()),
            Side::Quick => self.quick_item(i).filter(|id| Self::quick_spell(id).is_none()).map(|id| ItemStack::new(&id, self.world.item_count(&id))),
        }
    }

    fn quick_item(&self, k: usize) -> Option<String> {
        self.world.quickbar.get(k).filter(|id| !id.is_empty()).cloned()
    }

    /// Puts an item type (or `spell:<id>`) on quick bar slot `k` (moving it if it was on
    /// another slot).
    pub(super) fn assign_quick(&mut self, k: usize, item: &str) {
        let bar = &mut self.world.quickbar;
        bar.resize(QUICK_SLOTS, String::new());
        for s in bar.iter_mut() {
            if s == item {
                s.clear();
            }
        }
        bar[k] = item.to_string();
    }

    fn clear_quick(&mut self, k: usize) {
        if let Some(s) = self.world.quickbar.get_mut(k) {
            s.clear();
        }
    }

    /// Uses the item on quick bar slot `k` (same as using it from the inventory).
    pub(super) fn use_quick(&mut self, k: usize) {
        let Some(id) = self.quick_item(k) else { return };
        if let Some(spell) = Self::quick_spell(&id).map(str::to_string) {
            self.inv.quick_flash = Some((k, get_time()));
            self.cast_spell(&spell);
            return;
        }
        let name = self.project.db.item(&id).map_or(id.clone(), |d| d.name.clone());
        self.inv.quick_flash = Some((k, get_time()));
        if self.world.inventory.count(&id) == 0 {
            if self.world.equipment.count(&id) > 0 {
                self.ui.toast(format!("{name} is already equipped"));
            } else {
                self.ui.toast(format!("No {name} left"));
            }
            return;
        }
        self.use_item(&id);
    }

    /// Moves `count` (None = all) from a slot to the other side. Returns how many moved.
    fn move_between(&mut self, from: Side, i: usize, count: Option<u32>) -> u32 {
        let Some(uid) = self.inv.loot else { return 0 };
        self.container_inv(uid);
        let map = self.loaded.map.name.clone();
        let w = &mut self.world;
        let Some(chest) = w.containers.get_mut(&map).and_then(|m| m.get_mut(&uid)) else { return 0 };
        let (src, dst) = match from {
            Side::Container => (chest, &mut w.inventory),
            Side::Player => (&mut w.inventory, chest),
            Side::Equip | Side::Quick => return 0,
        };
        let had = src.slots.get(i).cloned().flatten();
        let moved = Inventory::transfer(src, i, dst, count);
        if let Some(stack) = had {
            if moved == 0 {
                self.ui.toast(if from == Side::Container { "Inventory full".into() } else { "No room in here".into() });
            } else if from == Side::Container {
                let name = self.project.db.item(&stack.item).map_or(stack.item.clone(), |d| d.name.clone());
                self.ui.toast(if moved > 1 { format!("Took {name} x{moved}") } else { format!("Took {name}") });
            }
        }
        moved
    }

    fn take_all(&mut self) {
        let Some(n) = self.inv.loot.and_then(|u| self.container_inv(u)).map(|c| c.capacity()) else { return };
        let mut moved = 0;
        let mut blocked = false;
        for i in 0..n {
            let Some(uid) = self.inv.loot else { return };
            let map = self.loaded.map.name.clone();
            let w = &mut self.world;
            let Some(chest) = w.containers.get_mut(&map).and_then(|m| m.get_mut(&uid)) else { return };
            if chest.slots[i].is_none() {
                continue;
            }
            let m = Inventory::transfer(chest, i, &mut w.inventory, None);
            blocked |= chest.slots[i].is_some();
            moved += m;
        }
        if moved > 0 {
            self.audio.play(&self.project.config.sfx.pickup);
        }
        if blocked {
            self.ui.toast("Inventory full".into());
        } else if moved > 0 {
            self.ui.toast(format!("Took {moved} item(s)"));
        }
    }

    /// Drag & drop between any two slots.
    fn drop_on(&mut self, from: (Side, usize), to: (Side, usize)) {
        if from == to {
            return;
        }
        match (from.0, to.0) {
            // Shortcuts: items (from the inventory or the character sheet) onto the quick bar.
            (Side::Player | Side::Equip, Side::Quick) => {
                if let Some(id) = self.slot_item(from.0, from.1) {
                    self.assign_quick(to.1, &id);
                }
                return;
            }
            (Side::Quick, Side::Quick) => {
                self.world.quickbar.resize(QUICK_SLOTS, String::new());
                self.world.quickbar.swap(from.1, to.1);
                return;
            }
            // Dragging a shortcut anywhere off the bar removes it.
            (Side::Quick, _) => {
                self.clear_quick(from.1);
                return;
            }
            (Side::Container, Side::Quick) => return,
            (Side::Player, Side::Equip) => {
                let fits = self.slot_item(Side::Player, from.1).and_then(|id| self.project.db.item(&id)?.equip_slot()) == EQUIP_SLOTS.get(to.1).copied();
                if fits {
                    self.equip_from_slot(from.1);
                } else {
                    self.ui.toast(format!("That doesn't go in the {} slot", EQUIP_SLOTS[to.1].label().to_lowercase()));
                }
                return;
            }
            (Side::Equip, Side::Player) => {
                self.unequip(EQUIP_SLOTS[from.1], Some(to.1));
                return;
            }
            (Side::Equip, _) | (_, Side::Equip) => return,
            _ => {}
        }
        if from.0 == to.0 {
            if let Some(inv) = self.inv_of(from.0) {
                inv.move_slot(from.1, to.1);
            }
            return;
        }
        let Some(uid) = self.inv.loot else { return };
        self.container_inv(uid);
        let map = self.loaded.map.name.clone();
        let w = &mut self.world;
        let Some(chest) = w.containers.get_mut(&map).and_then(|m| m.get_mut(&uid)) else { return };
        match from.0 {
            Side::Container => Inventory::transfer_to_slot(chest, from.1, &mut w.inventory, to.1),
            Side::Player => Inventory::transfer_to_slot(&mut w.inventory, from.1, chest, to.1),
            Side::Equip | Side::Quick => false,
        };
    }

    fn use_slot(&mut self, i: usize) {
        let Some(item) = self.slot_item(Side::Player, i) else { return };
        // Equipment goes into its slot (using the clicked stack).
        if self.project.db.item(&item).is_some_and(|d| d.equip_slot().is_some()) {
            self.equip_from_slot(i);
            return;
        }
        self.use_item(&item);
        // Close so dialogs / effects triggered by the item are visible.
        if self.runner.busy() {
            self.inv.close();
        }
    }

    /// Primary action on a slot (click / Enter).
    fn activate(&mut self, side: Side, i: usize, double: bool) {
        if side == Side::Quick {
            if double {
                self.use_quick(i);
            }
            return;
        }
        if side == Side::Equip {
            if double {
                self.unequip(EQUIP_SLOTS[i], None);
            }
            return;
        }
        match (self.inv.loot.is_some(), side) {
            (true, _) => {
                if self.slot_item(side, i).is_some() {
                    self.move_between(side, i, None);
                }
            }
            (false, _) => {
                if double {
                    self.use_slot(i);
                }
            }
        }
    }

    /// Handles the inventory / loot windows. Returns (key consumed, mouse consumed).
    pub(super) fn update_inventory_ui(&mut self, allow: bool, key_consumed: bool) -> (bool, bool) {
        if !allow {
            return (false, false);
        }
        if let Some(uid) = self.inv.pending_loot.take() {
            self.inv.open = false;
            self.inv.loot = Some(uid);
            self.inv.sel = Some((Side::Container, 0));
            self.audio.play(&self.project.config.sfx.open_container);
        }
        let m = input::mouse_pos();
        let (click, rclick) = (input::mouse_pressed(MouseButton::Left), input::mouse_pressed(MouseButton::Right));
        if !self.inv.is_open() {
            // A hidden HUD: its buttons and the quick bar can't be clicked (keys still work).
            if click && !self.ui_hidden {
                if let Some((b, _)) = self.hud_buttons().into_iter().find(|(_, r)| r.contains(m)) {
                    match b {
                        HudButton::Window => {
                            self.open_tab(super::charwin::WinTab::Character);
                            self.inv.sel = Some((Side::Equip, 0));
                        }
                        HudButton::Map => self.open_world_map(),
                    }
                    return (false, true);
                }
            }
            // Quick bar: keys 1-8 or a click use the item.
            if !key_consumed {
                if let Some(k) = Act::QUICK.iter().position(|a| input::act_pressed(*a)) {
                    self.use_quick(k);
                    return (true, false);
                }
            }
            if (click || rclick) && !self.ui_hidden {
                if let Some(k) = quick_rects().iter().position(|r| r.contains(m)) {
                    self.use_quick(k);
                    return (false, true);
                }
            }
            return (false, false);
        }

        // ---- keyboard
        let layout = self.layout();
        if input::act_pressed(Act::Cancel) || (self.inv.loot.is_some() && input::act_pressed(Act::Character)) {
            self.inv.close();
            return (true, true);
        }
        if self.inv.loot.is_some() && input::act_pressed(Act::TakeAll) {
            self.take_all();
        }
        // 1-8 puts the selected item on the quick bar.
        if let Some(k) = Act::QUICK.iter().position(|a| input::act_pressed(*a)) {
            if let Some((side @ (Side::Player | Side::Equip), i)) = self.inv.sel {
                if let Some(id) = self.slot_item(side, i) {
                    self.assign_quick(k, &id);
                }
            }
        }
        if self.inv.loot.is_some() && input::any_act_pressed(&[Act::NextTab, Act::PrevTab]) {
            if let Some((s, _)) = self.inv.sel {
                self.inv.sel = Some((if s == Side::Player { Side::Container } else { Side::Player }, 0));
            }
        }
        if let Some((side, i)) = self.inv.sel.filter(|s| s.0 != Side::Quick) {
            if let Some(g) = layout.grid(side) {
                let n = g.slots.len() as i32;
                let c = g.cols as i32;
                let mut j = i as i32;
                let left = input::act_pressed(Act::Left);
                let right = input::act_pressed(Act::Right);
                // Hop between the character sheet and the inventory grid.
                if side == Side::Player && left && i % g.cols == 0 && layout.grid(Side::Equip).is_some() {
                    self.inv.sel = Some((Side::Equip, (i / g.cols).min(EQUIP_SLOTS.len() - 1)));
                    return (true, true);
                }
                if side == Side::Equip && right {
                    self.inv.sel = Some((Side::Player, i * cols_of(&layout, Side::Player)));
                    return (true, true);
                }
                if left {
                    j -= 1;
                }
                if right {
                    j += 1;
                }
                if input::act_pressed(Act::Up) {
                    j -= c;
                }
                if input::act_pressed(Act::Down) {
                    j += c;
                }
                self.inv.sel = Some((side, j.clamp(0, n - 1) as usize));
                let i = self.inv.sel.unwrap().1;
                if input::act_pressed(Act::Confirm) {
                    if side == Side::Equip {
                        self.unequip(EQUIP_SLOTS[i], None);
                    } else if self.inv.loot.is_some() {
                        self.activate(side, i, false);
                    } else {
                        self.use_slot(i);
                    }
                } else if input::act_pressed(Act::Use) && side == Side::Player {
                    self.use_slot(i);
                } else if input::act_pressed(Act::Drop) && side == Side::Player {
                    // G drops the stack, Shift+G a single item.
                    let one = input::act_down(Act::Run);
                    self.drop_slot(i, one.then_some(1));
                }
            }
        }

        // ---- mouse
        let hit = layout.hit(m);
        if click {
            if layout.close.contains(m) {
                self.inv.close();
                return (true, true);
            }
            if layout.take_all.is_some_and(|r| r.contains(m)) {
                self.take_all();
                return (true, true);
            }
            match hit {
                Some(at) => self.inv.drag.press(at, m),
                None if !layout.contains(m) => {
                    self.inv.close();
                    return (true, true);
                }
                None => {}
            }
        }
        match self.inv.drag.update(m, hit) {
            SlotEvent::Dropped { from, to: Some(to) } => {
                self.drop_on(from, to);
                if to.0 != Side::Quick {
                    self.inv.sel = Some(to);
                }
            }
            // Dragging a shortcut off the bar removes it.
            SlotEvent::Dropped { from: (Side::Quick, i), to: None } => self.clear_quick(i),
            // Dragging a stack out of the window drops it on the ground.
            SlotEvent::Dropped { from: (Side::Player, i), to: None } if !layout.contains(m) => {
                self.drop_slot(i, None);
            }
            SlotEvent::Dropped { .. } => {}
            SlotEvent::Clicked { at: (side, i), double } => {
                self.inv.sel = Some((side, i));
                self.activate(side, i, double);
            }
            SlotEvent::None => {}
        }
        if rclick {
            if let Some((side, i)) = hit {
                self.inv.sel = Some((side, i));
                match (self.inv.loot.is_some(), side) {
                    (_, Side::Quick) => self.clear_quick(i),
                    (_, Side::Equip) => {
                        self.unequip(EQUIP_SLOTS[i], None);
                    }
                    (true, _) => {
                        self.move_between(side, i, Some(1));
                    }
                    (false, _) => self.use_slot(i),
                }
            }
        }
        (true, true)
    }

    // ------------------------------------------------------------ drawing

    pub(super) fn draw_inventory_ui(&mut self) {
        let m = input::mouse_pos();
        if (!self.ui.dialog_open() && !self.ui_hidden) || self.inv.is_open() {
            self.draw_quickbar();
        }
        if !self.inv.is_open() {
            if !self.ui.dialog_open() && !self.skills_open() && !self.ui_hidden {
                self.draw_hud_buttons();
            }
            return;
        }
        let layout = self.layout();
        let hover = layout.hit(m);
        let dragging = self.inv.drag.dragging();
        for g in &layout.grids {
            let p = g.panel;
            if g.side != Side::Quick {
                panel(p.x, p.y, p.w, p.h);
                draw_text(&g.title, p.x + PAD, p.y + 28.0, 26.0, GOLD);
            }
            if g.side == Side::Equip {
                self.draw_sheet(g, hover, dragging);
                continue;
            }
            if g.side == Side::Quick {
                continue;
            }
            let inv = match g.side {
                Side::Player => Some(&self.world.inventory),
                Side::Container => self.inv.loot.and_then(|u| self.world.containers.get(&self.loaded.map.name)?.get(&u)),
                Side::Equip | Side::Quick => None,
            };
            let Some(inv) = inv else { continue };
            for (i, r) in g.slots.iter().enumerate() {
                let key = (g.side, i);
                let selected = self.inv.sel == Some(key);
                let bg = if hover == Some(key) { Color::new(1.0, 1.0, 1.0, 0.16) } else { Color::new(0.0, 0.0, 0.0, 0.35) };
                draw_rectangle(r.x, r.y, r.w, r.h, bg);
                let border = if selected { GOLD } else { Color::new(0.85, 0.8, 0.6, 0.35) };
                draw_rectangle_lines(r.x, r.y, r.w, r.h, if selected { 3.0 } else { 1.0 }, border);
                let Some(stack) = inv.slots.get(i).cloned().flatten() else { continue };
                let tint = if dragging == Some(key) { Color::new(1.0, 1.0, 1.0, 0.3) } else { WHITE };
                if let Some(d) = self.project.db.item(&stack.item) {
                    self.assets.draw_frame_in(&d.icon.sheet, d.icon.frame, Rect::new(r.x + 8.0, r.y + 8.0, 32.0, 32.0), tint);
                } else {
                    draw_text("?", r.x + 18.0, r.y + 32.0, 24.0, RED);
                }
                if stack.count > 1 {
                    let t = stack.count.to_string();
                    let w = measure_text(&t, None, 18, 1.0).width;
                    draw_text(&t, r.x + r.w - w - 3.0, r.y + r.h - 4.0, 18.0, WHITE);
                }
            }
        }
        if let Some(r) = layout.take_all {
            let h = r.contains(m);
            draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.3, 0.25, 0.1, if h { 1.0 } else { 0.8 }));
            draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.0, GOLD);
            draw_text("Take all (T)", r.x + 6.0, r.y + 18.0, 16.0, WHITE);
        }
        if self.inv.loot.is_some() {
            let c = layout.close;
            draw_text("x", c.x + 6.0, c.y + 17.0, 24.0, if c.contains(m) { WHITE } else { GRAY });
        }

        // Details of the hovered or selected stack, and the help line.
        let focus = hover.or(self.inv.sel);
        let stack = focus.and_then(|(side, i)| self.slot_stack(side, i));
        super::ui::draw_item_detail(&self.project.db, layout.detail, PAD, stack.as_ref(), self.hint());

        // The stack being dragged follows the cursor.
        if let Some((side, i)) = dragging {
            let item = self.slot_stack(side, i);
            if let Some(def) = item.and_then(|s| self.project.db.item(&s.item)) {
                self.assets.draw_frame_in(&def.icon.sheet, def.icon.frame, Rect::new(m.x - 16.0, m.y - 16.0, 32.0, 32.0), WHITE);
            }
        }
    }
}

fn cols_of(layout: &Layout, side: Side) -> usize {
    layout.grid(side).map_or(1, |g| g.cols)
}

/// Short stat summary of an equippable item, e.g. "Damage 2 · every 0.35s · reach 1.1".
impl Game {
    /// Character sheet: portrait, equipment slots and stats.
    fn draw_sheet(&self, g: &Grid, hover: Option<(Side, usize)>, dragging: Option<(Side, usize)>) {
        let p = g.panel;
        self.draw_portrait(Rect::new(p.x + PAD, p.y + 50.0, 104.0, 168.0));
        for (k, r) in g.slots.iter().enumerate() {
            let key = (Side::Equip, k);
            let selected = self.inv.sel == Some(key);
            let bg = if hover == Some(key) { Color::new(1.0, 1.0, 1.0, 0.16) } else { Color::new(0.0, 0.0, 0.0, 0.35) };
            draw_rectangle(r.x, r.y, r.w, r.h, bg);
            draw_rectangle_lines(r.x, r.y, r.w, r.h, if selected { 3.0 } else { 1.5 }, if selected { GOLD } else { Color::new(0.85, 0.8, 0.6, 0.6) });
            let slot = EQUIP_SLOTS[k];
            draw_text(slot.label(), r.x + r.w + 8.0, r.y + 20.0, 18.0, LIGHTGRAY);
            match self.world.equipment.get(slot).and_then(|s| self.project.db.item(&s.item)) {
                Some(d) => {
                    let tint = if dragging == Some(key) { Color::new(1.0, 1.0, 1.0, 0.3) } else { WHITE };
                    self.assets.draw_frame_in(&d.icon.sheet, d.icon.frame, Rect::new(r.x + 8.0, r.y + 8.0, 32.0, 32.0), tint);
                    for (n, line) in wrap_text(&d.name, 15, 80.0).iter().take(2).enumerate() {
                        draw_text(line, r.x + r.w + 8.0, r.y + 36.0 + n as f32 * 13.0, 15.0, WHITE);
                    }
                }
                None => {
                    let hint = match slot {
                        EquipSlot::Weapon => "bare hands",
                        EquipSlot::Offhand if self.two_handed_equipped() => "(two-handed)",
                        EquipSlot::Offhand => "shield / light",
                        EquipSlot::Tool => "fishing rod, ...",
                        _ => "empty",
                    };
                    draw_text(hint, r.x + r.w + 8.0, r.y + 38.0, 15.0, GRAY);
                }
            }
        }
        let st = self.stats();
        let hp = self.hp();
        let reach = if st.ranged { format!("Range       {:.0} tiles", st.reach) } else { format!("Reach       {:.1} tiles", st.reach) };
        let mut lines = vec![
            format!("Health      {} / {}", hp, self.player_max_hp()),
            format!("Attack      {}", st.attack),
            format!("Defense     {}", st.defense),
            format!("Speed       {:.0}", st.speed),
            format!("Attack time {:.2}s", st.cooldown),
            reach,
        ];
        if self.progression_on() {
            let (have, need) = self.xp_progress();
            lines.insert(0, format!("Level       {}  (XP {have} / {need})", self.world.player_level()));
        }
        let y0 = p.y + 50.0 + EQUIP_SLOTS.len() as f32 * EQUIP_STEP + 20.0;
        draw_line(p.x + PAD, y0 - 12.0, p.x + p.w - PAD, y0 - 12.0, 1.0, Color::new(0.85, 0.8, 0.6, 0.4));
        for (n, l) in lines.iter().enumerate() {
            draw_text(l, p.x + PAD, y0 + 8.0 + n as f32 * 22.0, 19.0, WHITE);
        }
    }
}

impl Game {
    /// The quick action bar (bottom center): icon, owned count, key number.
    fn draw_quickbar(&self) {
        let m = input::mouse_pos();
        let rects = quick_rects();
        let first = rects[0];
        let last = rects[QUICK_SLOTS - 1];
        draw_rectangle(first.x - 6.0, first.y - 6.0, last.right() - first.x + 12.0, QUICK_SLOT + 12.0, Color::new(0.04, 0.04, 0.08, 0.55));
        let open = self.inv.is_open();
        let dragging = self.inv.drag.dragging();
        let flash = self.inv.quick_flash.filter(|(_, t)| get_time() - t < 0.25).map(|(k, _)| k);
        for (k, r) in rects.iter().enumerate() {
            let hover = r.contains(m) && (open || !self.ui.dialog_open());
            let bg = if flash == Some(k) {
                Color::new(1.0, 0.9, 0.5, 0.45)
            } else if hover {
                Color::new(1.0, 1.0, 1.0, 0.18)
            } else {
                Color::new(0.0, 0.0, 0.0, 0.45)
            };
            draw_rectangle(r.x, r.y, r.w, r.h, bg);
            // While dragging something droppable, highlight the bar.
            let drop_target = dragging.is_some_and(|(s, _)| matches!(s, Side::Player | Side::Equip | Side::Quick));
            let border = if drop_target { GOLD } else { Color::new(0.85, 0.8, 0.6, 0.5) };
            draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.5, border);
            draw_text((k + 1).to_string(), r.x + 3.0, r.y + 13.0, 15.0, Color::new(1.0, 1.0, 1.0, 0.7));
            let Some(id) = self.quick_item(k) else { continue };
            if let Some(spell) = Self::quick_spell(&id).and_then(|s| self.project.db.spell(s)) {
                let ready = self.world.spells.contains(&spell.id) && self.world.mana >= spell.mana as f32;
                let alpha = if ready { 1.0 } else { 0.4 };
                if !spell.icon.sheet.is_empty() {
                    self.assets.draw_frame_in(&spell.icon.sheet, spell.icon.frame, Rect::new(r.x + 8.0, r.y + 8.0, 28.0, 28.0), Color::new(1.0, 1.0, 1.0, alpha));
                }
                let t = spell.mana.to_string();
                let w = measure_text(&t, None, 14, 1.0).width;
                draw_text(&t, r.x + r.w - w - 3.0, r.y + r.h - 4.0, 14.0, Color::new(0.6, 0.7, 1.0, 1.0));
                continue;
            }
            let Some(def) = self.project.db.item(&id) else { continue };
            let owned = self.world.inventory.count(&id);
            let equipped = self.world.equipment.count(&id) > 0;
            let alpha = if dragging == Some((Side::Quick, k)) { 0.3 } else if owned == 0 && !equipped { 0.35 } else { 1.0 };
            self.assets.draw_frame_in(&def.icon.sheet, def.icon.frame, Rect::new(r.x + 8.0, r.y + 8.0, 28.0, 28.0), Color::new(1.0, 1.0, 1.0, alpha));
            if equipped {
                draw_text("E", r.x + r.w - 11.0, r.y + 13.0, 15.0, GOLD);
            }
            if !(equipped && owned == 0) && (owned != 1 || def.max_stack > 1) {
                let t = owned.to_string();
                let w = measure_text(&t, None, 16, 1.0).width;
                let c = if owned == 0 { Color::new(1.0, 0.45, 0.45, 1.0) } else { WHITE };
                draw_text(&t, r.x + r.w - w - 3.0, r.y + r.h - 4.0, 16.0, c);
            }
        }
    }
}

impl Game {
    /// Help line at the bottom of the inventory / loot window.
    /// Where dropped items land: the tile in front of the player if it is free, else at
    /// the player's feet.
    fn drop_position(&self) -> Vec2 {
        let ts = self.ts();
        let (dx, dy) = self.player.facing.dir();
        let ahead = self.player.pos + vec2(dx, dy) * ts * 0.9;
        let (tx, ty) = self.tile_of(ahead);
        let blocked = self.loaded.entities.iter().any(|e| e.active && e.solid && e.body.rect().contains(vec2((tx as f32 + 0.5) * ts, (ty as f32 + 0.8) * ts)));
        if self.loaded.map.in_bounds(tx, ty) && !self.tile_solid(tx, ty) && !blocked {
            self.tile_spot((tx, ty))
        } else {
            self.player.pos
        }
    }

    /// Puts items on the ground at `pos` on the current map (joining a pile of the same item
    /// close by). They are saved with the game until picked up.
    pub(super) fn place_drop(&mut self, item: &str, n: u32, pos: Vec2) {
        let map = self.loaded.map.name.clone();
        let (uid, new) = self.world.add_dropped(&map, item, n, pos.x, pos.y, self.ts() * 0.5);
        if new {
            let mut rt = self.new_rt_entity(crate::entity::EntityKind::Item, item, uid, "", pos, crate::entity::Facing::Down, true, 0);
            rt.count = n;
            self.loaded.entities.push(rt);
        } else if let Some(e) = self.loaded.entities.iter_mut().find(|e| e.uid == uid) {
            e.count += n;
            e.active = true;
        }
    }

    /// Drops `count` items (None = the whole stack) of inventory slot `i` on the ground.
    /// They stay on this map (also in save games) until picked up again.
    pub(super) fn drop_slot(&mut self, i: usize, count: Option<u32>) -> bool {
        let Some(stack) = self.world.inventory.slots.get(i).cloned().flatten() else { return false };
        let n = count.unwrap_or(stack.count).min(stack.count);
        if n == 0 {
            return false;
        }
        if let Some(Some(s)) = self.world.inventory.slots.get_mut(i) {
            s.count -= n;
            if s.count == 0 {
                self.world.inventory.slots[i] = None;
            }
        }
        let pos = self.drop_position();
        self.place_drop(&stack.item, n, pos);
        let name = self.item_name(&stack.item);
        self.ui.toast(if n > 1 { format!("Dropped {n} {name}") } else { format!("Dropped {name}") });
        self.audio.play(&self.project.config.sfx.pickup);
        true
    }

    fn hint(&self) -> &'static str {
        if self.inv.loot.is_some() {
            "Click: take/store stack · Right-click: one · Drag: move · {key:take_all}: take all · {key:cancel}: close"
        } else {
            INVENTORY_HINT
        }
    }
}

/// The quick bar key (1-8) pressed this frame, if any.
pub(super) fn quick_key_pressed() -> Option<usize> {
    Act::QUICK.iter().position(|a| input::act_pressed(*a))
}
