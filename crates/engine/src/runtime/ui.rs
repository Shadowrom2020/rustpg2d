//! In-game UI: dialog box, choices and toast messages (inventories live in `inventory_ui`).

use macroquad::prelude::*;

use super::input;
use crate::render::wrap_text;
use crate::state::WorldState;
use super::text::{draw_text, measure_text};

struct Dialog {
    /// Room for the speaker's portrait at the left (the game draws it into `portrait_rect`).
    portrait: bool,
    speaker: String,
    text: String,
    revealed: f32,
    choice: Option<(Vec<String>, String, usize)>,
}

#[derive(Default)]
pub struct Ui {
    dialog: Option<Dialog>,
    toasts: Vec<(String, f32)>,
    /// Choice option areas, recorded while drawing the previous frame.
    choice_rects: Vec<Rect>,
    /// Where the open dialog's portrait goes (set while drawing).
    pub portrait_rect: Option<Rect>,
    /// Dialog text speed from the player's options (set every frame; 0 = instant).
    pub text_speed: f32,
}

pub struct UiInput {
    /// A key press was used by the UI (don't also treat it as gameplay input).
    pub consumed: bool,
    /// A mouse click was used by the UI.
    pub mouse_consumed: bool,
}

const CHARS_PER_SEC: f32 = 60.0;
const FONT: u16 = 26;

fn confirm_pressed() -> bool {
    input::act_pressed(input::Act::Confirm)
}

impl Ui {
    pub fn toast(&mut self, msg: String) {
        self.toasts.push((msg, 2.5));
    }

    pub fn show_dialog(&mut self, speaker: &str, text: &str) {
        self.dialog = Some(Dialog { portrait: false, speaker: speaker.into(), text: text.into(), revealed: 0.0, choice: None });
    }

    /// The open dialog gets a portrait (call right after `show_dialog` / `show_choice`).
    pub fn with_portrait(&mut self) {
        if let Some(d) = &mut self.dialog {
            d.portrait = true;
        }
    }

    pub fn show_choice(&mut self, speaker: &str, text: &str, options: &[String], var: &str) {
        self.dialog = Some(Dialog {
            portrait: false,
            speaker: speaker.into(),
            text: text.into(),
            revealed: 0.0,
            choice: Some((options.to_vec(), var.to_string(), 0)),
        });
    }

    pub fn dialog_open(&self) -> bool {
        self.dialog.is_some()
    }

    pub fn update(&mut self, dt: f32, world: &mut WorldState) -> UiInput {
        for t in &mut self.toasts {
            t.1 -= dt;
        }
        self.toasts.retain(|t| t.1 > 0.0);
        let mut out = UiInput { consumed: false, mouse_consumed: false };
        let mouse = input::mouse_pos();
        let click = input::mouse_pressed(MouseButton::Left);

        if let Some(d) = &mut self.dialog {
            out.consumed = true;
            out.mouse_consumed = true;
            let len = d.text.chars().count() as f32;
            d.revealed = if self.text_speed <= 0.0 { len } else { (d.revealed + dt * CHARS_PER_SEC * self.text_speed).min(len) };
            if let Some((opts, _, sel)) = &mut d.choice {
                let n = opts.len().max(1);
                if input::act_pressed(input::Act::Up) {
                    *sel = (*sel + n - 1) % n;
                }
                if input::act_pressed(input::Act::Down) {
                    *sel = (*sel + 1) % n;
                }
            }
            let done = d.revealed >= len;
            let mut confirm = confirm_pressed();
            if let Some((_, _, sel)) = &mut d.choice {
                if let Some(k) = self.choice_rects.iter().position(|r| r.contains(mouse)) {
                    if done {
                        *sel = k; // hover selects
                    }
                    confirm |= click;
                }
            } else {
                confirm |= click;
            }
            if confirm {
                if d.revealed < len {
                    d.revealed = len;
                } else {
                    if let Some((opts, var, sel)) = &d.choice {
                        world.set_var(var, *sel as i64);
                        // The answer goes into the dialog log under the player's name.
                        let me = if world.player_name.trim().is_empty() { "You".to_string() } else { world.player_name.clone() };
                        if let Some(o) = opts.get(*sel) {
                            world.log_dialog(&me, o, true);
                        }
                    }
                    self.dialog = None;
                }
            }
            return out;
        }

        out
    }

    pub fn draw_toasts(&self) {
        let sw = screen_width();
        for (i, (msg, t)) in self.toasts.iter().enumerate() {
            let a = t.min(1.0);
            let w = measure_text(msg, None, FONT, 1.0).width + 24.0;
            let y = 16.0 + i as f32 * 40.0;
            draw_rectangle((sw - w) / 2.0, y, w, 34.0, Color::new(0.05, 0.05, 0.1, 0.8 * a));
            draw_text(msg, (sw - w) / 2.0 + 12.0, y + 24.0, FONT as f32, Color::new(1., 1., 1., a));
        }
    }

    pub fn draw_dialog(&mut self) {
        let (sw, sh) = (screen_width(), screen_height());
        self.choice_rects.clear();
        self.portrait_rect = None;

        if let Some(d) = &self.dialog {
            let margin = 16.0;
            let w = (sw - margin * 2.0).min(900.0);
            let x = (sw - w) / 2.0;
            // A portrait takes a square at the left; the text moves right of it.
            const PORTRAIT: f32 = 120.0;
            let inset = if d.portrait { PORTRAIT + 18.0 } else { 0.0 };
            let choice_rows = d.choice.as_ref().map_or(0, |c| c.0.len()) as f32;
            let lines = wrap_text(&d.text, FONT, w - 40.0 - inset);
            let mut h = 56.0 + lines.len().max(2) as f32 * 30.0 + choice_rows * 30.0;
            if d.portrait {
                h = h.max(PORTRAIT + 28.0);
            }
            let y = sh - h - margin;
            panel(x, y, w, h);
            if d.portrait {
                self.portrait_rect = Some(Rect::new(x + 14.0, y + 14.0, PORTRAIT, PORTRAIT));
            }
            let x = x + inset;
            let w = w - inset;
            let mut ty = y + 34.0;
            if !d.speaker.is_empty() {
                // (Dialog content is translated already: macroquad's draw_text, no UI lookup.)
                macroquad::text::draw_text(&d.speaker, x + 20.0, ty, 28.0, GOLD);
                ty += 32.0;
            }
            let mut budget = d.revealed as usize;
            for line in &lines {
                let shown: String = line.chars().take(budget).collect();
                budget = budget.saturating_sub(line.chars().count() + 1);
                macroquad::text::draw_text(&shown, x + 20.0, ty, FONT as f32, WHITE);
                ty += 30.0;
            }
            let done = d.revealed as usize >= d.text.chars().count();
            if let (Some((opts, _, sel)), true) = (&d.choice, done) {
                for (i, o) in opts.iter().enumerate() {
                    self.choice_rects.push(Rect::new(x + 20.0, ty - 22.0, w - 40.0, 30.0));
                    let c = if i == *sel { GOLD } else { LIGHTGRAY };
                    let marker = if i == *sel { ">" } else { " " };
                    macroquad::text::draw_text(format!("{marker} {o}"), x + 36.0, ty, FONT as f32, c);
                    ty += 30.0;
                }
            } else if done && (get_time() * 2.0) as i64 % 2 == 0 {
                let (cx, cy) = (x + w - 28.0, y + h - 20.0);
                draw_triangle(vec2(cx - 7.0, cy - 5.0), vec2(cx + 7.0, cy - 5.0), vec2(cx, cy + 4.0), WHITE);
            }
        }
    }
}

/// Seconds between two clicks that count as a double click (list rows, slots).
pub(super) const DOUBLE_CLICK: f64 = 0.35;
/// Pixels the pointer moves with the button down before a press becomes a drag.
pub(super) const DRAG_START: f32 = 6.0;
/// Inventory-like windows: slot size, the gap between slots, the panel padding, the title
/// strip's height and the details footer's height.
pub(super) const SLOT: f32 = 48.0;
pub(super) const GAP: f32 = 6.0;
pub(super) const PAD: f32 = 14.0;
pub(super) const TITLE_H: f32 = 40.0;
pub(super) const DETAIL_H: f32 = 84.0;

/// The size of a panel holding `n` slots in `cols` columns under a title.
pub(super) fn slot_grid_size(n: usize, cols: usize) -> Vec2 {
    let rows = n.div_ceil(cols).max(1);
    vec2(cols as f32 * (SLOT + GAP) - GAP + 2.0 * PAD, rows as f32 * (SLOT + GAP) - GAP + 2.0 * PAD + TITLE_H)
}

/// The slot rectangles of a grid of `n` slots in `cols` columns in a panel at `origin`, and the
/// panel's size.
pub(super) fn slot_grid(origin: Vec2, n: usize, cols: usize) -> (Vec<Rect>, Vec2) {
    let slots = (0..n).map(|i| Rect::new(origin.x + PAD + (i % cols) as f32 * (SLOT + GAP), origin.y + TITLE_H + PAD + (i / cols) as f32 * (SLOT + GAP), SLOT, SLOT)).collect();
    (slots, slot_grid_size(n, cols))
}

/// Pressing, dragging and clicking the slots of an inventory-like window. `K` tells the slot
/// groups apart (the inventory's sides, the companion window's panes).
#[derive(Clone, Debug)]
pub(super) struct DragState<K: Copy + PartialEq> {
    press: Option<(K, usize, Vec2)>,
    dragging: bool,
    last_click: Option<(K, usize, f64)>,
}

impl<K: Copy + PartialEq> Default for DragState<K> {
    fn default() -> Self {
        DragState { press: None, dragging: false, last_click: None }
    }
}

/// What happened to a pressed slot ([`DragState::update`]).
pub(super) enum SlotEvent<K> {
    None,
    /// Released after a drag: from a slot onto `to` (`None`: outside every slot).
    Dropped { from: (K, usize), to: Option<(K, usize)> },
    /// Released on the slot it was pressed on without dragging; `double` for the second click
    /// within [`DOUBLE_CLICK`].
    Clicked { at: (K, usize), double: bool },
}

impl<K: Copy + PartialEq> DragState<K> {
    /// A left press on slot `hit` at `m`: a click, or the start of a drag.
    pub fn press(&mut self, hit: (K, usize), m: Vec2) {
        self.press = Some((hit.0, hit.1, m));
        self.dragging = false;
    }

    /// Every frame after a press: the press turns into a drag once the pointer moved
    /// [`DRAG_START`]; on release (or when the button is found up) says what it came to.
    pub fn update(&mut self, m: Vec2, hit: Option<(K, usize)>) -> SlotEvent<K> {
        let Some((k, i, start)) = self.press else { return SlotEvent::None };
        if input::mouse_down(MouseButton::Left) && m.distance(start) > DRAG_START {
            self.dragging = true;
        }
        if input::mouse_released(MouseButton::Left) || !input::mouse_down(MouseButton::Left) {
            self.press = None;
            if std::mem::take(&mut self.dragging) {
                return SlotEvent::Dropped { from: (k, i), to: hit };
            }
            if hit == Some((k, i)) {
                let now = get_time();
                let double = self.last_click.is_some_and(|(s, j, t)| s == k && j == i && now - t < DOUBLE_CLICK);
                self.last_click = Some((k, i, now));
                return SlotEvent::Clicked { at: (k, i), double };
            }
        }
        SlotEvent::None
    }

    /// The slot being dragged, to draw it under the cursor.
    pub fn dragging(&self) -> Option<(K, usize)> {
        if self.dragging {
            self.press.map(|(k, i, _)| (k, i))
        } else {
            None
        }
    }
}

/// Keeps `sel` inside `len` rows and `scroll` such that the selection is among the `visible` rows.
pub(super) fn clamp_list(sel: &mut usize, scroll: &mut usize, len: usize, visible: usize) {
    *sel = (*sel).min(len.saturating_sub(1));
    *scroll = (*scroll).min(len.saturating_sub(visible));
    if *sel < *scroll {
        *scroll = *sel;
    } else if *sel >= *scroll + visible {
        *scroll = *sel + 1 - visible;
    }
}

/// Rows with a selection, a scroll offset and double-click detection: the trade window's two
/// lists and the crafting window. Row `k` is drawn at `row_rect(k - scroll)` while visible.
#[derive(Clone, Debug, Default)]
pub(super) struct ScrollList {
    pub sel: usize,
    pub scroll: usize,
    last_click: Option<(usize, f64)>,
}

/// What a click on a [`ScrollList`] did.
pub(super) enum ListClick {
    None,
    /// A row was selected (`sel`).
    Select,
    /// The row's button was clicked, or the row double-clicked.
    Activate(usize),
}

impl ScrollList {
    /// See [`clamp_list`].
    pub fn clamp(&mut self, len: usize, visible: usize) {
        clamp_list(&mut self.sel, &mut self.scroll, len, visible);
    }

    /// A left click at `m`: selects the row under it; a click on the row's `button` part or a
    /// second click within [`DOUBLE_CLICK`] activates it. `row_rect` gives visible row `vis`.
    pub fn click(&mut self, m: Vec2, len: usize, visible: usize, row_rect: impl Fn(usize) -> Rect, button: impl Fn(Rect) -> Rect) -> ListClick {
        for vis in 0..visible {
            let k = self.scroll + vis;
            let r = row_rect(vis);
            if k >= len || !r.contains(m) {
                continue;
            }
            self.sel = k;
            let now = get_time();
            let double = self.last_click.is_some_and(|(i, t)| i == k && now - t < DOUBLE_CLICK);
            self.last_click = Some((k, now));
            if button(r).contains(m) || double {
                self.last_click = None;
                return ListClick::Activate(k);
            }
            return ListClick::Select;
        }
        ListClick::None
    }
}

/// The details panel of an inventory-like window: the focused stack's name with its count, its
/// stats in gold, two lines of description, and the help line (smaller when it doesn't fit).
pub(super) fn draw_item_detail(db: &crate::defs::Database, d: Rect, pad: f32, stack: Option<&crate::inventory::ItemStack>, hint: &str) {
    panel(d.x, d.y, d.w, d.h);
    if let Some(s) = stack {
        let def = db.item(&s.item);
        let name = def.map_or(s.item.as_str(), |d| d.name.as_str());
        let title = if s.count > 1 { format!("{name}  x{}", s.count) } else { name.to_string() };
        draw_text(&title, d.x + pad, d.y + 24.0, 22.0, WHITE);
        if let Some(def) = def {
            let tw = measure_text(&title, None, 22, 1.0).width;
            if let Some(line) = item_stats_line(def) {
                draw_text(&line, d.x + pad + tw + 16.0, d.y + 23.0, 18.0, GOLD);
            }
            for (k, line) in wrap_text(&super::glyphs::expand_keys(&def.description), 18, d.w - 2.0 * pad).iter().take(2).enumerate() {
                draw_text(line, d.x + pad, d.y + 44.0 + k as f32 * 18.0, 18.0, LIGHTGRAY);
            }
        }
    }
    let size = super::glyphs::hint_size(hint, d.w - 2.0 * pad, 16);
    super::glyphs::draw_hint(hint, d.x + pad, d.y + d.h - 8.0, size, GRAY);
}

/// One line of an item's numbers: tool kind, weapon damage / cooldown / reach, armor slot /
/// defense / speed.
pub(super) fn item_stats_line(d: &crate::defs::ItemDef) -> Option<String> {
    if !d.tool.is_empty() {
        return Some(format!("Tool · {}", d.tool));
    }
    if let Some(w) = &d.weapon {
        return Some(match &w.ranged {
            Some(r) => format!("Damage {} · every {:.2}s · range {:.0}", w.damage, w.cooldown, r.range),
            None => format!("Damage {} · every {:.2}s · reach {:.1}", w.damage, w.cooldown, w.reach),
        });
    }
    d.armor.as_ref().map(|a| {
        let mut s = format!("{} · Defense +{}", a.slot.label(), a.defense);
        if a.speed != 0.0 {
            s.push_str(&format!(" · Speed {:+.0}%", a.speed * 100.0));
        }
        s
    })
}

pub(super) fn panel(x: f32, y: f32, w: f32, h: f32) {
    draw_rectangle(x, y, w, h, Color::new(0.06, 0.06, 0.12, 0.92));
    draw_rectangle_lines(x, y, w, h, 3.0, Color::new(0.85, 0.8, 0.6, 1.0));
}
