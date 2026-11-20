//! In-game UI: dialog box, choices and toast messages (inventories live in `inventory_ui`).

use macroquad::prelude::*;

use super::input;
use crate::render::wrap_text;
use crate::state::WorldState;
use super::text::{draw_plain, draw_text, measure_plain, measure_text, Role};
use crate::dialog_codes::{self, BoxAt, Code, Piece};

/// The first private-use character that stands for a code inside a dialog's text (the codes'
/// places survive wrapping, and each counts as one character of the typing).
const MARK: u32 = 0xF0000;

struct Dialog {
    /// Room for the speaker's portrait at the left (the game draws it into `portrait_rect`).
    portrait: bool,
    speaker: String,
    /// The text with each code (colour, icon, pause, speed) replaced by `MARK + its index`.
    text: Vec<char>,
    codes: Vec<Code>,
    /// The text without codes (`status`).
    plain: String,
    at: BoxAt,
    /// Characters typed out so far, and the time not yet spent on the next one.
    revealed: usize,
    clock: f32,
    /// A `{w:…}` pause still running, and the `{s:…}` speed.
    pause: f32,
    speed: f32,
    choice: Option<(Vec<String>, String, usize)>,
}

impl Dialog {
    fn new(speaker: &str, text: &str, choice: Option<(Vec<String>, String, usize)>) -> Self {
        let mut codes = Vec::new();
        let mut marked = Vec::new();
        let mut at = BoxAt::Bottom;
        let mut speaker = speaker.to_string();
        for p in dialog_codes::split(text) {
            match p {
                Piece::Text(t) => marked.extend(t.chars()),
                Piece::Code(Code::At(a), _) => at = a,
                Piece::Code(Code::Name(n), _) => speaker = n,
                Piece::Code(c, _) => {
                    marked.push(char::from_u32(MARK + codes.len() as u32).unwrap_or(' '));
                    codes.push(c);
                }
            }
        }
        let choice = choice.map(|(o, v, s)| (o.iter().map(|o| dialog_codes::plain(o)).collect(), v, s));
        Dialog { portrait: false, speaker: dialog_codes::plain(&speaker), text: marked, codes, plain: dialog_codes::plain(text), at, revealed: 0, clock: 0.0, pause: 0.0, speed: 1.0, choice }
    }

    /// The code a character of the text stands for.
    fn code(&self, c: char) -> Option<&Code> {
        (c as u32).checked_sub(MARK).and_then(|k| self.codes.get(k as usize))
    }

    /// Types the text out for `dt` seconds at `rate` characters per second (pauses and speed
    /// codes take effect when the typing reaches them).
    fn type_out(&mut self, dt: f32, rate: f32) {
        if rate <= 0.0 {
            self.revealed = self.text.len();
            return;
        }
        self.clock += dt;
        while self.revealed < self.text.len() {
            if self.pause > 0.0 {
                let t = self.pause.min(self.clock);
                self.pause -= t;
                self.clock -= t;
                if self.pause > 0.0 {
                    break;
                }
            }
            match self.code(self.text[self.revealed]).cloned() {
                Some(Code::Wait(s)) => self.pause = s,
                Some(Code::Speed(v)) => self.speed = v,
                // (An icon takes a character's time.)
                Some(Code::Icon(_)) | None => {
                    let cost = 1.0 / (rate * self.speed);
                    if self.clock < cost {
                        break;
                    }
                    self.clock -= cost;
                }
                Some(_) => {}
            }
            self.revealed += 1;
        }
        if self.revealed >= self.text.len() {
            self.clock = 0.0;
        }
    }

    fn done(&self) -> bool {
        self.revealed >= self.text.len()
    }

    /// The width of a piece of the marked text at `font` (codes take none, icons a square).
    fn width(&self, s: &str, font: f32) -> f32 {
        let mut w = 0.0;
        let mut run = String::new();
        for c in s.chars() {
            match self.code(c) {
                Some(code) => {
                    w += measure_plain(&std::mem::take(&mut run), font).width;
                    if matches!(code, Code::Icon(_)) {
                        w += icon_size(font) + 4.0;
                    }
                }
                None => run.push(c),
            }
        }
        w + measure_plain(&run, font).width
    }
}

/// The side of an inline item icon in a dialog line.
fn icon_size(font: f32) -> f32 {
    (font * 1.1).round()
}

#[derive(Default)]
pub struct Ui {
    dialog: Option<Dialog>,
    toasts: Vec<(String, f32)>,
    /// Choice option areas, recorded while drawing the previous frame.
    choice_rects: Vec<Rect>,
    /// Where the open dialog's portrait goes (set while drawing).
    pub portrait_rect: Option<Rect>,
    /// The `{i:…}` item icons of the open dialog and where they go (set while drawing).
    pub icon_spots: Vec<(String, Rect)>,
    /// Dialog text speed from the player's options (set every frame; 0 = instant).
    pub text_speed: f32,
    /// The size of what players read (Options › Accessibility › Text size; set every frame).
    pub text_scale: f32,
}

thread_local! {
    /// Options › Accessibility › High contrast (set every frame): panels, toasts, captions.
    static CONTRAST: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// Options › Accessibility › Text size (set every frame): the details footer.
    static TEXT_SCALE: std::cell::Cell<f32> = const { std::cell::Cell::new(1.0) };
}

/// Switches the high-contrast look of panels, toasts and captions.
pub(super) fn set_contrast(on: bool) {
    CONTRAST.with(|c| c.set(on));
}

/// Sets the scale of what players read (for code without the `Ui` at hand).
pub(super) fn set_text_scale(s: f32) {
    TEXT_SCALE.with(|c| c.set(s.max(1.0)));
}

/// The scale of what players read (at least 1).
pub(super) fn text_scale() -> f32 {
    TEXT_SCALE.with(|c| c.get())
}

/// Whether the high-contrast look is on.
pub(super) fn contrast() -> bool {
    CONTRAST.with(|c| c.get())
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

    /// Opens a dialog box; the text may hold message codes (`dialog_codes`).
    pub fn show_dialog(&mut self, speaker: &str, text: &str) {
        self.dialog = Some(Dialog::new(speaker, text, None));
    }

    /// The open dialog gets a portrait (call right after `show_dialog` / `show_choice`).
    pub fn with_portrait(&mut self) {
        if let Some(d) = &mut self.dialog {
            d.portrait = true;
        }
    }

    pub fn show_choice(&mut self, speaker: &str, text: &str, options: &[String], var: &str) {
        self.dialog = Some(Dialog::new(speaker, text, Some((options.to_vec(), var.to_string(), 0))));
    }

    pub fn dialog_open(&self) -> bool {
        self.dialog.is_some()
    }

    /// The open dialog's text without its codes (`status`).
    pub fn dialog_text(&self) -> Option<&str> {
        self.dialog.as_ref().map(|d| d.plain.as_str())
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
            d.type_out(dt, CHARS_PER_SEC * self.text_speed.max(0.0));
            if let Some((opts, _, sel)) = &mut d.choice {
                let n = opts.len().max(1);
                if input::act_pressed(input::Act::Up) {
                    *sel = (*sel + n - 1) % n;
                }
                if input::act_pressed(input::Act::Down) {
                    *sel = (*sel + 1) % n;
                }
            }
            let done = d.done();
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
                if !d.done() {
                    d.revealed = d.text.len();
                    d.pause = 0.0;
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

    /// The scale of what players read (at least 1).
    fn scale(&self) -> f32 {
        self.text_scale.max(1.0)
    }

    pub fn draw_toasts(&self) {
        let _font = super::text::role(Role::Hud);
        let sw = screen_width();
        let s = self.scale();
        let font = (FONT as f32 * s).round();
        for (i, (msg, t)) in self.toasts.iter().enumerate() {
            let a = t.min(1.0);
            let w = measure_text(msg, None, font as u16, 1.0).width + 24.0;
            let (y, h) = (16.0 + i as f32 * 40.0 * s, 34.0 * s);
            super::skin::toast_box(Rect::new((sw - w) / 2.0, y, w, h), a);
            draw_text(msg, (sw - w) / 2.0 + 12.0, y + h * 0.7, font, super::skin::toast_text(a));
        }
    }

    pub fn draw_dialog(&mut self) {
        let _font = super::text::role(Role::Dialog);
        let (sw, sh) = (screen_width(), screen_height());
        self.choice_rects.clear();
        self.portrait_rect = None;
        self.icon_spots.clear();

        let s = self.scale();
        let (font, line) = ((FONT as f32 * s).round(), 30.0 * s);
        if let Some(d) = &self.dialog {
            let margin = 16.0;
            // (Larger text: a wider box, up to the screen.)
            let w = (sw - margin * 2.0).min(900.0 * s);
            let x = (sw - w) / 2.0;
            // A portrait takes a square at the left; the text moves right of it.
            const PORTRAIT: f32 = 120.0;
            let inset = if d.portrait { PORTRAIT + 18.0 } else { 0.0 };
            let choice_rows = d.choice.as_ref().map_or(0, |c| c.0.len()) as f32;
            let marked: String = d.text.iter().collect();
            let lines = super::text::wrap_with(&marked, w - 40.0 - inset, |s| d.width(s, font));
            // A skin's name box sits on the box's top edge: the name needs no line inside.
            let name_box = super::skin::has_name_box() && !d.speaker.is_empty();
            let name_size = (28.0 * s).round();
            let pad = super::skin::name_box_pad();
            let name_h = (name_size + 2.0 * pad).round();
            // The text starts below the name box's lower half (else below the name's line).
            let below_box = ((name_h / 2.0) - 14.0 * s).max(0.0).round();
            let name_line = if name_box { below_box } else { 30.0 * s };
            let mut h = 26.0 + name_line + lines.len().max(2) as f32 * line + choice_rows * line;
            if d.portrait {
                h = h.max(PORTRAIT + 28.0);
            }
            let y = match d.at {
                BoxAt::Bottom => sh - h - margin,
                BoxAt::Middle => ((sh - h) / 2.0).round(),
                BoxAt::Top => margin + if name_box { (name_h / 2.0).round() } else { 0.0 },
            };
            super::skin::dialog_box(x, y, w, h);
            if name_box {
                let nw = (measure_plain(&d.speaker, name_size).width + 2.0 * pad).round();
                let nx = x + 16.0 + if d.portrait { PORTRAIT + 18.0 } else { 0.0 };
                let ny = y - (name_h / 2.0).round();
                super::skin::name_box(Rect::new(nx, ny, nw, name_h));
                draw_plain(&d.speaker, nx + pad, ny + pad + name_size * 0.8, name_size, super::skin::title());
            }
            if d.portrait {
                self.portrait_rect = Some(Rect::new(x + 14.0, y + 14.0, PORTRAIT, PORTRAIT));
            }
            let x = x + inset;
            let w = w - inset;
            let mut ty = y + 8.0 + 26.0 * s + if name_box { below_box } else { 0.0 };
            if !d.speaker.is_empty() && !name_box {
                // (Dialog content is translated already: no UI lookup.)
                draw_plain(&d.speaker, x + 20.0, ty, name_size, super::skin::title());
                ty += 32.0 * s;
            }
            let mut budget = d.revealed;
            let text_color = super::skin::dialog_text();
            let mut color = text_color;
            // (Each line's own count of the text's characters: Japanese lines end on no space.)
            for (l, used) in &lines {
                let mut lx = x + 20.0;
                let mut run = String::new();
                for c in l.chars().take(budget) {
                    let Some(code) = d.code(c) else {
                        run.push(c);
                        continue;
                    };
                    lx += draw_plain(&std::mem::take(&mut run), lx, ty, font, color).width;
                    match code {
                        Code::Color(Some([r, g, b])) => color = Color::from_rgba(*r, *g, *b, 255),
                        Code::Color(None) => color = text_color,
                        Code::Icon(id) => {
                            let side = icon_size(font);
                            self.icon_spots.push((id.clone(), Rect::new(lx + 2.0, (ty - font * 0.85).round(), side, side)));
                            lx += side + 4.0;
                        }
                        _ => {}
                    }
                }
                draw_plain(&run, lx, ty, font, color);
                budget = budget.saturating_sub(*used);
                ty += line;
            }
            let done = d.done();
            if let (Some((opts, _, sel)), true) = (&d.choice, done) {
                let dim = if contrast() { WHITE } else { super::skin::dim() };
                for (i, o) in opts.iter().enumerate() {
                    let r = Rect::new(x + 20.0, ty - 22.0 * s, w - 40.0, line);
                    self.choice_rects.push(r);
                    if i == *sel && super::skin::has_cursor() {
                        super::skin::cursor(r, Color::new(0.0, 0.0, 0.0, 0.0));
                    }
                    let c = if i == *sel { super::skin::highlight() } else { dim };
                    let marker = if i == *sel { ">" } else { " " };
                    draw_plain(&format!("{marker} {o}"), x + 36.0, ty, font, c);
                    ty += line;
                }
            } else if done && (get_time() * 2.0) as i64 % 2 == 0 {
                let (cx, cy) = (x + w - 28.0, y + h - 20.0);
                draw_triangle(vec2(cx - 7.0, cy - 5.0), vec2(cx + 7.0, cy - 5.0), vec2(cx, cy + 4.0), text_color);
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
const DETAIL_H: f32 = 84.0;

/// The details footer's height, grown with the text size (Options › Accessibility).
pub(super) fn detail_h() -> f32 {
    DETAIL_H + (text_scale() - 1.0) * 80.0
}

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
pub(super) fn draw_item_detail(db: &crate::defs::Database, d: Rect, pad: f32, stack: Option<&crate::inventory::ItemStack>, hint: &str, attuned: &[String]) {
    panel(d.x, d.y, d.w, d.h);
    if let Some(s) = stack {
        let spent = s.spent;
        let def = db.item(&s.item);
        let name = def.map_or(s.item.as_str(), |d| d.name.as_str());
        let title = if s.count > 1 { format!("{name}  x{}", s.count) } else { name.to_string() };
        let s = text_scale();
        let (big, small) = ((22.0 * s).round(), (18.0 * s).round());
        draw_text(&title, d.x + pad, d.y + 24.0 * s, big, super::skin::normal());
        if let Some(def) = def {
            let tw = measure_text(&title, None, big as u16, 1.0).width;
            let magic = magic_line(def, spent, attuned);
            let line = match (item_stats_line(def), magic) {
                (Some(a), Some(b)) => Some(format!("{a} · {b}")),
                (a, b) => a.or(b),
            };
            if let Some(line) = line {
                draw_text(&line, d.x + pad + tw + 16.0, d.y + 23.0 * s, small, super::skin::highlight());
            }
            let desc = if contrast() { WHITE } else { super::skin::dim() };
            for (k, line) in wrap_text(&super::glyphs::expand_keys(&def.description), small as u16, d.w - 2.0 * pad).iter().take(2).enumerate() {
                draw_text(line, d.x + pad, d.y + 44.0 * s + k as f32 * small, small, desc);
            }
        }
    }
    // (The largest size up to 16 that fits; too long even at 70 %: two lines.)
    super::glyphs::draw_hint_in(hint, d.x + pad, d.y + d.h - 8.0, 16.0, d.w - 2.0 * pad, super::skin::disabled());
}

/// A magic item's part of the numbers: its +N, rarity, attunement and charges left
/// ("+1 · Rare · Attuned · 5/7 charges"); a plain item's uses and silvering.
pub(super) fn magic_line(d: &crate::defs::ItemDef, spent: u32, attuned: &[String]) -> Option<String> {
    // A plain item's uses (a healer's kit: "7/10 uses"); silvered.
    let Some(m) = d.magic.as_ref() else {
        let mut v = Vec::new();
        if d.silvered {
            v.push(crate::lang::tr_ui("Silvered").into_owned());
        }
        if d.uses > 0 {
            v.push(format!("{}/{} {}", d.uses.saturating_sub(spent), d.uses, crate::lang::tr_ui("uses")));
        }
        return (!v.is_empty()).then(|| v.join(" · "));
    };
    let mut v = Vec::new();
    if m.bonus != 0 {
        v.push(format!("{:+}", m.bonus));
    }
    v.push(crate::lang::tr_ui(m.rarity.label()).into_owned());
    if m.attunement {
        v.push(crate::lang::tr_ui(if attuned.contains(&d.id) { "Attuned" } else { "Requires attunement" }).into_owned());
    }
    if m.charges > 0 {
        v.push(format!("{}/{} {}", m.charges.saturating_sub(spent), m.charges, crate::lang::tr_ui("charges")));
    }
    Some(v.join(" · "))
}

/// One line of an item's numbers: tool kind, weapon damage / cooldown / reach, armor slot /
/// defense / speed.
pub(super) fn item_stats_line(d: &crate::defs::ItemDef) -> Option<String> {
    if !d.tool.is_empty() {
        return Some(format!("Tool · {}", d.tool));
    }
    if d.bag_slots > 0 {
        return Some(format!("Backpack · +{} inventory slots", d.bag_slots));
    }
    if let Some(w) = &d.weapon {
        return Some(match &w.ranged {
            Some(r) => format!("{} · {} ft", w.damage_dice(), (r.range * 5.0).round()),
            None => {
                let mut s = w.damage_dice();
                if !w.versatile.is_empty() {
                    s.push_str(&format!(" ({})", w.versatile));
                }
                if let Some((n, l)) = w.thrown_ft {
                    s.push_str(&format!(" · {n}/{l} ft"));
                }
                s
            }
        } + &w.properties.iter().map(|p| format!(" · {}", crate::lang::tr_ui(if *p == crate::defs::WeaponProperty::Light { "Light (weapon)" } else { p.label() }))).collect::<String>()[..]);
    }
    d.armor.as_ref().map(|a| {
        use crate::defs::ArmorCategory as C;
        let mut s = match a.category {
            Some(C::Light | C::Medium | C::Heavy) => {
                let dex = match a.dex_max {
                    Some(0) => String::new(),
                    Some(m) => format!(" + DEX (max {m})"),
                    None => " + DEX".to_string(),
                };
                format!("{} · AC {}{dex}", crate::lang::tr_ui(a.category.map_or("", |c| c.label())), a.base_ac)
            }
            Some(C::Shield) => format!("{} · AC +{}", crate::lang::tr_ui("Shield"), a.base_ac),
            _ => crate::lang::tr_ui(a.slot.label()).to_string(),
        };
        if a.str_min > 0 {
            s.push_str(&format!(" · STR {}", a.str_min));
        }
        if a.speed != 0.0 {
            s.push_str(&format!(" · Speed {:+.0}%", a.speed * 100.0));
        }
        s
    })
}

/// A window or menu panel in the skin's look ([`super::skin::panel`]).
pub(super) fn panel(x: f32, y: f32, w: f32, h: f32) {
    super::skin::panel(x, y, w, h);
}
