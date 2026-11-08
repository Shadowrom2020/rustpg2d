//! Input: keyboard, mouse and gamepads, with optional injected keys, buttons and mouse state
//! (used for automated play-throughs). Gameplay and menus ask for named actions ([`Act`]:
//! `act_pressed(Act::Confirm)`), which the keyboard and a gamepad both drive; text fields read
//! raw keys and typed characters. Gamepads come through gilrs, polled once per frame
//! ([`poll_pad`]); the left stick and the d-pad both count as the [`Pad`] directions.

use std::cell::RefCell;

use macroquad::prelude::{
    is_key_down, is_key_pressed, is_mouse_button_down, is_mouse_button_pressed, is_mouse_button_released, mouse_position, KeyCode,
    MouseButton, Vec2,
};

/// Injected mouse state for one frame.
#[derive(Clone, Debug, Default)]
pub struct MouseInject {
    pub pos: Vec2,
    pub pressed: Vec<MouseButton>,
    pub down: Vec<MouseButton>,
    pub released: Vec<MouseButton>,
    /// Mouse wheel (vertical) this frame.
    pub wheel: f32,
}

#[derive(Default)]
struct Injected {
    down: Vec<KeyCode>,
    pressed: Vec<KeyCode>,
    mouse: Option<MouseInject>,
    /// Typed characters waiting to be read.
    chars: std::collections::VecDeque<char>,
}

thread_local! {
    static INJECTED: RefCell<Injected> = RefCell::new(Injected::default());
}

/// Sets the keys held and pressed (this frame) in addition to the real keyboard.
pub fn inject(down: &[KeyCode], pressed: &[KeyCode]) {
    INJECTED.with(|i| {
        let mut i = i.borrow_mut();
        i.down = down.to_vec();
        i.pressed = pressed.to_vec();
    });
}

/// Injects the mouse state for this frame; `None` returns to the real mouse.
pub fn inject_mouse(m: Option<MouseInject>) {
    INJECTED.with(|i| i.borrow_mut().mouse = m);
}

/// Mouse position in screen pixels.
pub fn mouse_pos() -> Vec2 {
    INJECTED.with(|i| i.borrow().mouse.as_ref().map(|m| m.pos)).unwrap_or_else(|| Vec2::from(mouse_position()))
}

fn injected(b: MouseButton, f: fn(&MouseInject) -> &Vec<MouseButton>) -> Option<bool> {
    INJECTED.with(|i| i.borrow().mouse.as_ref().map(|m| f(m).contains(&b)))
}

pub fn mouse_pressed(b: MouseButton) -> bool {
    injected(b, |m| &m.pressed).unwrap_or_else(|| is_mouse_button_pressed(b))
}

pub fn mouse_down(b: MouseButton) -> bool {
    injected(b, |m| &m.down).unwrap_or_else(|| is_mouse_button_down(b))
}

pub fn mouse_released(b: MouseButton) -> bool {
    injected(b, |m| &m.released).unwrap_or_else(|| is_mouse_button_released(b))
}

/// Vertical mouse wheel movement this frame (positive = up).
pub fn wheel() -> f32 {
    INJECTED.with(|i| i.borrow().mouse.as_ref().map(|m| m.wheel)).unwrap_or_else(|| macroquad::input::mouse_wheel().1)
}

/// Queues typed text (for automated play-throughs).
pub fn inject_text(text: &str) {
    INJECTED.with(|i| i.borrow_mut().chars.extend(text.chars()));
}

/// Next typed character (injected text first, then the keyboard).
pub fn next_char() -> Option<char> {
    INJECTED.with(|i| i.borrow_mut().chars.pop_front()).or_else(macroquad::input::get_char_pressed)
}

pub fn down(k: KeyCode) -> bool {
    is_key_down(k) || INJECTED.with(|i| i.borrow().down.contains(&k))
}

pub fn pressed(k: KeyCode) -> bool {
    is_key_pressed(k) || INJECTED.with(|i| i.borrow().pressed.contains(&k))
}

pub fn any_pressed(keys: &[KeyCode]) -> bool {
    keys.iter().any(|k| pressed(*k))
}

/// Every key that can be bound, with its name (autoplay scripts, the settings file and the
/// Controls page show these).
const KEY_NAMES: &[(KeyCode, &str)] = &[
    (KeyCode::A, "a"),
    (KeyCode::B, "b"),
    (KeyCode::C, "c"),
    (KeyCode::D, "d"),
    (KeyCode::E, "e"),
    (KeyCode::F, "f"),
    (KeyCode::G, "g"),
    (KeyCode::H, "h"),
    (KeyCode::I, "i"),
    (KeyCode::J, "j"),
    (KeyCode::K, "k"),
    (KeyCode::L, "l"),
    (KeyCode::M, "m"),
    (KeyCode::N, "n"),
    (KeyCode::O, "o"),
    (KeyCode::P, "p"),
    (KeyCode::Q, "q"),
    (KeyCode::R, "r"),
    (KeyCode::S, "s"),
    (KeyCode::T, "t"),
    (KeyCode::U, "u"),
    (KeyCode::V, "v"),
    (KeyCode::W, "w"),
    (KeyCode::X, "x"),
    (KeyCode::Y, "y"),
    (KeyCode::Z, "z"),
    (KeyCode::Key0, "0"),
    (KeyCode::Key1, "1"),
    (KeyCode::Key2, "2"),
    (KeyCode::Key3, "3"),
    (KeyCode::Key4, "4"),
    (KeyCode::Key5, "5"),
    (KeyCode::Key6, "6"),
    (KeyCode::Key7, "7"),
    (KeyCode::Key8, "8"),
    (KeyCode::Key9, "9"),
    (KeyCode::Up, "up"),
    (KeyCode::Down, "down"),
    (KeyCode::Left, "left"),
    (KeyCode::Right, "right"),
    (KeyCode::Space, "space"),
    (KeyCode::Enter, "enter"),
    (KeyCode::Tab, "tab"),
    (KeyCode::Backspace, "backspace"),
    (KeyCode::Delete, "delete"),
    (KeyCode::Insert, "insert"),
    (KeyCode::Home, "home"),
    (KeyCode::End, "end"),
    (KeyCode::PageUp, "pageup"),
    (KeyCode::PageDown, "pagedown"),
    (KeyCode::LeftShift, "shift"),
    (KeyCode::RightShift, "rshift"),
    (KeyCode::LeftControl, "ctrl"),
    (KeyCode::RightControl, "rctrl"),
    (KeyCode::LeftAlt, "alt"),
    (KeyCode::RightAlt, "altgr"),
    (KeyCode::Minus, "minus"),
    (KeyCode::Equal, "equal"),
    (KeyCode::Comma, "comma"),
    (KeyCode::Period, "period"),
    (KeyCode::Slash, "slash"),
    (KeyCode::Backslash, "backslash"),
    (KeyCode::Semicolon, "semicolon"),
    (KeyCode::Apostrophe, "apostrophe"),
    (KeyCode::LeftBracket, "lbracket"),
    (KeyCode::RightBracket, "rbracket"),
    (KeyCode::GraveAccent, "grave"),
    (KeyCode::Kp0, "kp0"),
    (KeyCode::Kp1, "kp1"),
    (KeyCode::Kp2, "kp2"),
    (KeyCode::Kp3, "kp3"),
    (KeyCode::Kp4, "kp4"),
    (KeyCode::Kp5, "kp5"),
    (KeyCode::Kp6, "kp6"),
    (KeyCode::Kp7, "kp7"),
    (KeyCode::Kp8, "kp8"),
    (KeyCode::Kp9, "kp9"),
    (KeyCode::KpAdd, "kpplus"),
    (KeyCode::KpSubtract, "kpminus"),
    (KeyCode::KpMultiply, "kpmultiply"),
    (KeyCode::KpDivide, "kpdivide"),
    (KeyCode::KpEnter, "kpenter"),
    (KeyCode::KpDecimal, "kpdecimal"),
    (KeyCode::F1, "f1"),
    (KeyCode::F2, "f2"),
    (KeyCode::F3, "f3"),
    (KeyCode::F4, "f4"),
    (KeyCode::F5, "f5"),
    (KeyCode::F6, "f6"),
    (KeyCode::F7, "f7"),
    (KeyCode::F8, "f8"),
    (KeyCode::F9, "f9"),
    (KeyCode::F10, "f10"),
    (KeyCode::F11, "f11"),
    (KeyCode::F12, "f12"),
    (KeyCode::Escape, "escape"),
];

/// Parses key names (autoplay scripts, the settings file); a few short forms are accepted too.
pub fn key_from_name(name: &str) -> Option<KeyCode> {
    let n = name.trim().to_ascii_lowercase();
    let n = match n.as_str() {
        "esc" => "escape",
        "del" => "delete",
        "pgup" => "pageup",
        "pgdn" => "pagedown",
        "lshift" => "shift",
        "plus" => "equal",
        other => other,
    };
    KEY_NAMES.iter().find(|(_, s)| *s == n).map(|(k, _)| *k)
}

/// The name of a key (None for keys that can't be bound).
pub fn key_name(k: KeyCode) -> Option<&'static str> {
    KEY_NAMES.iter().find(|(c, _)| *c == k).map(|(_, s)| *s)
}

/// How a key is shown to players: "E", "Space", "Shift", "F5", "Num +".
pub fn key_label(k: KeyCode) -> String {
    let n = key_name(k).unwrap_or("?");
    match n {
        "kpplus" => "Num +".into(),
        "kpminus" => "Num -".into(),
        "kpmultiply" => "Num *".into(),
        "kpdivide" => "Num /".into(),
        "kpenter" => "Num Enter".into(),
        "kpdecimal" => "Num .".into(),
        "equal" => "=".into(),
        "minus" => "-".into(),
        "comma" => ",".into(),
        "period" => ".".into(),
        "slash" => "/".into(),
        "backslash" => "\\".into(),
        "semicolon" => ";".into(),
        "apostrophe" => "'".into(),
        "lbracket" => "[".into(),
        "rbracket" => "]".into(),
        "grave" => "`".into(),
        "pageup" => "Page Up".into(),
        "pagedown" => "Page Down".into(),
        "rshift" => "Right Shift".into(),
        "rctrl" => "Right Ctrl".into(),
        "altgr" => "AltGr".into(),
        n if n.starts_with("kp") => format!("Num {}", &n[2..]),
        n if n.len() == 1 || (n.starts_with('f') && n[1..].parse::<u8>().is_ok()) => n.to_uppercase(),
        n => {
            let mut c = n.chars();
            c.next().map_or(String::new(), |f| f.to_uppercase().collect::<String>() + c.as_str())
        }
    }
}

/// The key pressed this frame, injected ones first (the Controls page waits for one).
pub fn last_key_pressed() -> Option<KeyCode> {
    INJECTED.with(|i| i.borrow().pressed.first().copied()).or_else(macroquad::input::get_last_key_pressed)
}

// ------------------------------------------------------------------ gamepads

/// Gamepad buttons (Xbox names; the left stick counts as the d-pad).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Pad {
    /// A (Xbox) / Cross (PlayStation).
    South,
    /// B / Circle.
    East,
    /// X / Square.
    West,
    /// Y / Triangle.
    North,
    Lb,
    Rb,
    Lt,
    Rt,
    Select,
    Start,
    /// Stick clicks.
    LStick,
    RStick,
    Up,
    Down,
    Left,
    Right,
}

impl Pad {
    pub const ALL: [Pad; 16] = [
        Pad::South,
        Pad::East,
        Pad::West,
        Pad::North,
        Pad::Lb,
        Pad::Rb,
        Pad::Lt,
        Pad::Rt,
        Pad::Select,
        Pad::Start,
        Pad::LStick,
        Pad::RStick,
        Pad::Up,
        Pad::Down,
        Pad::Left,
        Pad::Right,
    ];

    /// The name used by autoplay scripts (`pad a`) and shown to players.
    pub fn name(self) -> &'static str {
        match self {
            Pad::South => "a",
            Pad::East => "b",
            Pad::West => "x",
            Pad::North => "y",
            Pad::Lb => "lb",
            Pad::Rb => "rb",
            Pad::Lt => "lt",
            Pad::Rt => "rt",
            Pad::Select => "select",
            Pad::Start => "start",
            Pad::LStick => "ls",
            Pad::RStick => "rs",
            Pad::Up => "up",
            Pad::Down => "down",
            Pad::Left => "left",
            Pad::Right => "right",
        }
    }

    pub fn from_name(name: &str) -> Option<Pad> {
        let n = name.trim().to_ascii_lowercase();
        Pad::ALL.into_iter().find(|p| p.name() == n)
    }

    fn bit(self) -> u32 {
        1 << Pad::ALL.iter().position(|p| *p == self).unwrap()
    }
}

/// Stick deflection that counts as a direction (and the lower one that keeps it held).
#[cfg(not(target_arch = "wasm32"))]
const STICK_ON: f32 = 0.5;
#[cfg(not(target_arch = "wasm32"))]
const STICK_OFF: f32 = 0.35;

#[derive(Default)]
struct PadState {
    /// (No gilrs in the web game: browsers' gamepads aren't read yet.)
    #[cfg(not(target_arch = "wasm32"))]
    gilrs: Option<gilrs::Gilrs>,
    #[cfg(not(target_arch = "wasm32"))]
    started: bool,
    /// Buttons held now / last frame (bits of [`Pad::bit`]).
    down: u32,
    prev: u32,
    /// The connected gamepad's name ("" = none).
    name: String,
    inj_down: Vec<Pad>,
    inj_pressed: Vec<Pad>,
}

thread_local! {
    static PAD: RefCell<PadState> = RefCell::new(PadState::default());
}

/// Reads the gamepads; call once per frame before the game looks at input. Without gamepad
/// support (no udev, no permission) it quietly does nothing.
pub fn poll_pad() {
    PAD.with(|p| {
        let mut p = p.borrow_mut();
        let prev = p.down;
        let (down, name) = read_gamepads(&mut p, prev);
        p.prev = prev;
        p.down = down;
        p.name = name;
    });
}

/// The buttons held on connected gamepads (with the left stick as a d-pad) and the first one's name.
#[cfg(target_arch = "wasm32")]
fn read_gamepads(_: &mut PadState, _: u32) -> (u32, String) {
    (0, String::new())
}

#[cfg(not(target_arch = "wasm32"))]
fn read_gamepads(p: &mut PadState, prev: u32) -> (u32, String) {
    use gilrs::{Axis, Button};
    {
        if !p.started {
            p.started = true;
            p.gilrs = match gilrs::Gilrs::new() {
                Ok(g) => Some(g),
                Err(e) => {
                    eprintln!("[input] gamepads unavailable: {e}");
                    None
                }
            };
        }
        let mut down = 0;
        let mut name = String::new();
        if let Some(g) = &mut p.gilrs {
            while g.next_event().is_some() {}
            for (_, gp) in g.gamepads().filter(|(_, gp)| gp.is_connected()) {
                if name.is_empty() {
                    name = gp.name().to_string();
                }
                let buttons = [
                    (Button::South, Pad::South),
                    (Button::East, Pad::East),
                    (Button::West, Pad::West),
                    (Button::North, Pad::North),
                    (Button::LeftTrigger, Pad::Lb),
                    (Button::RightTrigger, Pad::Rb),
                    (Button::LeftTrigger2, Pad::Lt),
                    (Button::RightTrigger2, Pad::Rt),
                    (Button::Select, Pad::Select),
                    (Button::Start, Pad::Start),
                    (Button::LeftThumb, Pad::LStick),
                    (Button::RightThumb, Pad::RStick),
                    (Button::DPadUp, Pad::Up),
                    (Button::DPadDown, Pad::Down),
                    (Button::DPadLeft, Pad::Left),
                    (Button::DPadRight, Pad::Right),
                ];
                for (b, pad) in buttons {
                    if gp.is_pressed(b) {
                        down |= pad.bit();
                    }
                }
                // The left stick as a d-pad, with a little hysteresis.
                let (x, y) = (gp.value(Axis::LeftStickX), gp.value(Axis::LeftStickY));
                for (v, pad) in [(-y, Pad::Down), (y, Pad::Up), (-x, Pad::Left), (x, Pad::Right)] {
                    let held = prev & pad.bit() != 0;
                    if v > STICK_ON || (held && v > STICK_OFF) {
                        down |= pad.bit();
                    }
                }
            }
        }
        (down, name)
    }
}

/// Sets gamepad buttons held and pressed (this frame) in addition to a real gamepad.
pub fn inject_pad(down: &[Pad], pressed: &[Pad]) {
    PAD.with(|p| {
        let mut p = p.borrow_mut();
        p.inj_down = down.to_vec();
        p.inj_pressed = pressed.to_vec();
    });
}

pub fn pad_down(b: Pad) -> bool {
    PAD.with(|p| {
        let p = p.borrow();
        p.down & b.bit() != 0 || p.inj_down.contains(&b)
    })
}

pub fn pad_pressed(b: Pad) -> bool {
    PAD.with(|p| {
        let p = p.borrow();
        (p.down & !p.prev) & b.bit() != 0 || p.inj_pressed.contains(&b)
    })
}

/// The connected gamepad's name, or "none".
pub fn pad_name() -> String {
    PAD.with(|p| {
        let n = p.borrow().name.clone();
        if n.is_empty() {
            "none".into()
        } else {
            n
        }
    })
}

// ------------------------------------------------------------------ actions

/// What the player wants to do; each action has keyboard keys and gamepad buttons.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Act {
    /// Movement and menu navigation.
    Up,
    Down,
    Left,
    Right,
    /// Hold to run (and to buy / sell ten, to scroll the map faster).
    Run,
    /// Talk, interact, advance dialogs, choose in menus, use the selected item.
    Confirm,
    /// Close a window, go back in a menu.
    Cancel,
    /// Open the pause menu.
    Pause,
    Attack,
    SwitchWeapon,
    /// The character window (character sheet and inventory).
    Character,
    Skills,
    Spells,
    /// The next / previous tab of a window.
    NextTab,
    PrevTab,
    Map,
    Journal,
    HideUi,
    /// Inventory: drop the selected stack; use the selected item; loot: take everything.
    Drop,
    Use,
    TakeAll,
    /// Map screen.
    ZoomIn,
    ZoomOut,
    MapCenter,
    Quick1,
    Quick2,
    Quick3,
    Quick4,
    Quick5,
    Quick6,
    Quick7,
    Quick8,
    QuickSave,
    QuickLoad,
    Debug,
    /// The companions' orders menu.
    Command,
}

impl Act {
    pub const QUICK: [Act; 8] = [Act::Quick1, Act::Quick2, Act::Quick3, Act::Quick4, Act::Quick5, Act::Quick6, Act::Quick7, Act::Quick8];

    pub const ALL: [Act; 36] = [
        Act::Up,
        Act::Down,
        Act::Left,
        Act::Right,
        Act::Run,
        Act::Confirm,
        Act::Attack,
        Act::SwitchWeapon,
        Act::Character,
        Act::Skills,
        Act::Spells,
        Act::NextTab,
        Act::PrevTab,
        Act::Map,
        Act::Journal,
        Act::HideUi,
        Act::Command,
        Act::Drop,
        Act::Use,
        Act::TakeAll,
        Act::ZoomIn,
        Act::ZoomOut,
        Act::MapCenter,
        Act::Quick1,
        Act::Quick2,
        Act::Quick3,
        Act::Quick4,
        Act::Quick5,
        Act::Quick6,
        Act::Quick7,
        Act::Quick8,
        Act::QuickSave,
        Act::QuickLoad,
        Act::Cancel,
        Act::Pause,
        Act::Debug,
    ];

    /// The name in the settings file (`keys: {"attack": ["x"]}`).
    pub fn id(self) -> &'static str {
        match self {
            Act::Up => "up",
            Act::Down => "down",
            Act::Left => "left",
            Act::Right => "right",
            Act::Run => "run",
            Act::Confirm => "confirm",
            Act::Cancel => "cancel",
            Act::Pause => "pause",
            Act::Attack => "attack",
            Act::SwitchWeapon => "switch_weapon",
            Act::Character => "character",
            Act::Skills => "skills",
            Act::Spells => "spells",
            Act::NextTab => "next_tab",
            Act::PrevTab => "prev_tab",
            Act::Map => "map",
            Act::Journal => "journal",
            Act::HideUi => "hide_ui",
            Act::Drop => "drop",
            Act::Use => "use",
            Act::TakeAll => "take_all",
            Act::ZoomIn => "zoom_in",
            Act::ZoomOut => "zoom_out",
            Act::MapCenter => "map_center",
            Act::Quick1 => "quick1",
            Act::Quick2 => "quick2",
            Act::Quick3 => "quick3",
            Act::Quick4 => "quick4",
            Act::Quick5 => "quick5",
            Act::Quick6 => "quick6",
            Act::Quick7 => "quick7",
            Act::Quick8 => "quick8",
            Act::QuickSave => "quick_save",
            Act::QuickLoad => "quick_load",
            Act::Debug => "debug",
            Act::Command => "command",
        }
    }

    pub fn from_id(id: &str) -> Option<Act> {
        Act::ALL.into_iter().find(|a| a.id() == id)
    }

    /// How the Controls page names it.
    pub fn label(self) -> &'static str {
        match self {
            Act::Up => "Move up",
            Act::Down => "Move down",
            Act::Left => "Move left",
            Act::Right => "Move right",
            Act::Run => "Run (hold)",
            Act::Confirm => "Interact / confirm",
            Act::Cancel => "Back / close",
            Act::Pause => "Pause menu",
            Act::Attack => "Attack",
            Act::SwitchWeapon => "Switch weapon",
            Act::Character => "Character & inventory",
            Act::Skills => "Skills",
            Act::Spells => "Spells",
            Act::NextTab => "Next tab",
            Act::PrevTab => "Previous tab",
            Act::Map => "Map",
            Act::Journal => "Journal",
            Act::HideUi => "Hide interface",
            Act::Drop => "Drop item",
            Act::Use => "Use item",
            Act::TakeAll => "Take all (loot)",
            Act::ZoomIn => "Map: zoom in",
            Act::ZoomOut => "Map: zoom out",
            Act::MapCenter => "Map: center",
            Act::Quick1 => "Quick slot 1",
            Act::Quick2 => "Quick slot 2",
            Act::Quick3 => "Quick slot 3",
            Act::Quick4 => "Quick slot 4",
            Act::Quick5 => "Quick slot 5",
            Act::Quick6 => "Quick slot 6",
            Act::Quick7 => "Quick slot 7",
            Act::Quick8 => "Quick slot 8",
            Act::QuickSave => "Quick save",
            Act::QuickLoad => "Quick load",
            Act::Debug => "Debug overlay",
            Act::Command => "Companion orders",
        }
    }

    /// Players can change its keys (Escape stays the way back; F3 is for creators).
    pub fn rebindable(self) -> bool {
        !matches!(self, Act::Cancel | Act::Pause | Act::Debug)
    }

    /// Where it is used (bits): 1 = playing, 2 = inventory / loot / shop windows, 4 = map
    /// screen. Two actions only conflict when they share a key and a place.
    fn places(self) -> u8 {
        match self {
            Act::Up | Act::Down | Act::Left | Act::Right | Act::Run => 1 | 2 | 4,
            Act::Confirm | Act::Character | Act::NextTab => 1 | 2,
            // LB only switches tabs in an open window (it gives companion orders in play).
            Act::PrevTab => 2,
            Act::Quick1 | Act::Quick2 | Act::Quick3 | Act::Quick4 | Act::Quick5 | Act::Quick6 | Act::Quick7 | Act::Quick8 => 1 | 2,
            Act::Drop | Act::Use | Act::TakeAll => 2,
            Act::Map => 1 | 4,
            Act::ZoomIn | Act::ZoomOut | Act::MapCenter => 4,
            Act::Cancel => 1 | 2 | 4,
            _ => 1,
        }
    }

    /// The keyboard keys it has unless the player changed them.
    pub fn default_keys(self) -> &'static [KeyCode] {
        use KeyCode as K;
        match self {
            Act::Up => &[K::Up, K::W],
            Act::Down => &[K::Down, K::S],
            Act::Left => &[K::Left, K::A],
            Act::Right => &[K::Right, K::D],
            Act::Run => &[K::LeftShift, K::RightShift],
            Act::Confirm => &[K::E, K::Space, K::Enter],
            Act::Cancel | Act::Pause => &[K::Escape],
            Act::Attack => &[K::X],
            Act::SwitchWeapon => &[K::Q],
            Act::Character => &[K::C, K::I],
            Act::Skills => &[K::K],
            Act::Spells => &[K::L],
            Act::NextTab => &[K::Tab],
            Act::PrevTab => &[],
            Act::Map => &[K::M],
            Act::Journal => &[K::J],
            Act::HideUi => &[K::H],
            Act::Drop => &[K::G],
            Act::Use => &[K::U],
            Act::TakeAll => &[K::T],
            Act::ZoomIn => &[K::Equal, K::KpAdd],
            Act::ZoomOut => &[K::Minus, K::KpSubtract],
            Act::MapCenter => &[K::Space, K::Home, K::C],
            Act::Quick1 => &[K::Key1],
            Act::Quick2 => &[K::Key2],
            Act::Quick3 => &[K::Key3],
            Act::Quick4 => &[K::Key4],
            Act::Quick5 => &[K::Key5],
            Act::Quick6 => &[K::Key6],
            Act::Quick7 => &[K::Key7],
            Act::Quick8 => &[K::Key8],
            Act::QuickSave => &[K::F5],
            Act::QuickLoad => &[K::F9],
            Act::Debug => &[K::F3],
            Act::Command => &[K::G],
        }
    }

    /// The gamepad buttons that trigger it.
    pub fn pad(self) -> &'static [Pad] {
        match self {
            Act::Up => &[Pad::Up],
            Act::Down => &[Pad::Down],
            Act::Left => &[Pad::Left],
            Act::Right => &[Pad::Right],
            Act::Run => &[Pad::Lt],
            Act::Confirm => &[Pad::South],
            Act::Cancel => &[Pad::East, Pad::Start],
            Act::Pause => &[Pad::Start],
            Act::Attack => &[Pad::West],
            Act::SwitchWeapon => &[Pad::Rt],
            Act::Character => &[Pad::North],
            Act::NextTab => &[Pad::Rb],
            Act::PrevTab => &[Pad::Lb],
            Act::Map => &[Pad::Select],
            Act::Journal => &[Pad::RStick],
            Act::HideUi => &[Pad::LStick],
            Act::TakeAll => &[Pad::West],
            Act::ZoomIn => &[Pad::Rb],
            Act::ZoomOut => &[Pad::Lb],
            Act::MapCenter => &[Pad::South],
            Act::Command => &[Pad::Lb],
            _ => &[],
        }
    }
}

thread_local! {
    /// Keys the player bound instead of the defaults (Options › Controls).
    static BINDINGS: RefCell<std::collections::HashMap<Act, Vec<KeyCode>>> = RefCell::new(Default::default());
}

/// The keys that trigger `a` now.
pub fn keys_of(a: Act) -> Vec<KeyCode> {
    BINDINGS.with(|b| b.borrow().get(&a).cloned()).unwrap_or_else(|| a.default_keys().to_vec())
}

/// Applies the player's bindings (`settings.keys`: action id -> key names). Unknown actions,
/// unknown keys and actions that can't be rebound are ignored.
pub fn set_bindings(keys: &std::collections::BTreeMap<String, Vec<String>>) {
    let map = keys
        .iter()
        .filter_map(|(id, names)| {
            let a = Act::from_id(id).filter(|a| a.rebindable())?;
            Some((a, names.iter().filter_map(|n| key_from_name(n)).collect::<Vec<_>>()))
        })
        .collect();
    BINDINGS.with(|b| *b.borrow_mut() = map);
}

/// Key names for `settings.keys` from a binding table (only actions that differ from the defaults).
pub fn binding_names(table: &std::collections::HashMap<Act, Vec<KeyCode>>) -> std::collections::BTreeMap<String, Vec<String>> {
    table
        .iter()
        .filter(|(a, keys)| a.rebindable() && keys.as_slice() != a.default_keys())
        .map(|(a, keys)| (a.id().to_string(), keys.iter().filter_map(|k| key_name(*k)).map(String::from).collect()))
        .collect()
}

/// Other (rebindable) actions that `key` would also trigger where `a` is used. Escape, shared
/// by *Back* and *Pause* on purpose, can't be bound (it cancels the key prompt).
pub fn conflicts(a: Act, key: KeyCode) -> Vec<Act> {
    Act::ALL.into_iter().filter(|b| *b != a && b.rebindable() && b.places() & a.places() != 0 && keys_of(*b).contains(&key)).collect()
}

/// The action is held (keyboard or gamepad).
pub fn act_down(a: Act) -> bool {
    keys_of(a).iter().any(|k| down(*k)) || a.pad().iter().any(|b| pad_down(*b))
}

/// The action was triggered this frame.
pub fn act_pressed(a: Act) -> bool {
    keys_of(a).iter().any(|k| pressed(*k)) || a.pad().iter().any(|b| pad_pressed(*b))
}

pub fn any_act_pressed(acts: &[Act]) -> bool {
    acts.iter().any(|a| act_pressed(*a))
}

/// Menu navigation in text-entry screens: arrow keys or the gamepad (not WASD, which types).
pub fn nav_pressed(a: Act) -> bool {
    // The arrow keys always navigate here, whatever movement is bound to.
    let key = match a {
        Act::Up => KeyCode::Up,
        Act::Down => KeyCode::Down,
        Act::Left => KeyCode::Left,
        Act::Right => KeyCode::Right,
        _ => return act_pressed(a),
    };
    pressed(key) || a.pad().iter().any(|b| pad_pressed(*b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pad_names_round_trip() {
        for p in Pad::ALL {
            assert_eq!(Pad::from_name(p.name()), Some(p));
        }
        assert_eq!(Pad::from_name(" A "), Some(Pad::South));
        assert_eq!(Pad::from_name("z"), None);
        // Every button has its own bit.
        let bits: u32 = Pad::ALL.iter().map(|p| p.bit()).fold(0, |a, b| {
            assert_eq!(a & b, 0);
            a | b
        });
        assert_eq!(bits.count_ones() as usize, Pad::ALL.len());
    }

    #[test]
    fn actions_keep_the_old_keys() {
        assert_eq!(Act::Confirm.default_keys(), &[KeyCode::E, KeyCode::Space, KeyCode::Enter]);
        assert_eq!(Act::Attack.default_keys(), &[KeyCode::X]);
        assert_eq!(Act::Character.default_keys(), &[KeyCode::C, KeyCode::I]);
        assert!(Act::QUICK.iter().enumerate().all(|(k, a)| a.default_keys().len() == 1 && key_from_name(&(k + 1).to_string()) == Some(a.default_keys()[0])));
        // The gamepad reaches everything a controller player needs.
        for a in [Act::Up, Act::Down, Act::Left, Act::Right, Act::Confirm, Act::Cancel, Act::Pause, Act::Attack, Act::Character, Act::NextTab, Act::PrevTab, Act::Map, Act::Journal] {
            assert!(!a.pad().is_empty(), "{a:?} has no gamepad button");
        }
    }

    #[test]
    fn names_and_bindings() {
        for (k, n) in KEY_NAMES {
            assert_eq!(key_from_name(n), Some(*k));
            assert_eq!(key_name(*k), Some(*n));
        }
        for a in Act::ALL {
            assert_eq!(Act::from_id(a.id()), Some(a));
            for k in a.default_keys() {
                assert!(key_name(*k).is_some(), "{a:?}: {k:?} has no name");
            }
        }
        assert_eq!(Act::ALL.len(), Act::ALL.iter().collect::<std::collections::HashSet<_>>().len());
        assert_eq!((key_label(KeyCode::Space), key_label(KeyCode::F5), key_label(KeyCode::KpAdd), key_label(KeyCode::X)), ("Space".into(), "F5".into(), "Num +".into(), "X".into()));
        // The defaults have no conflicts where it matters.
        for a in Act::ALL.into_iter().filter(|a| a.rebindable()) {
            for k in a.default_keys() {
                assert!(conflicts(a, *k).is_empty(), "{a:?} {k:?}: {:?}", conflicts(a, *k));
            }
        }
        // Bindings: only changed actions are stored; unknown names are ignored.
        let mut keys = std::collections::BTreeMap::new();
        keys.insert("attack".to_string(), vec!["z".to_string(), "nonsense".to_string()]);
        keys.insert("pause".to_string(), vec!["p".to_string()]);
        set_bindings(&keys);
        assert_eq!(keys_of(Act::Attack), vec![KeyCode::Z]);
        assert_eq!(keys_of(Act::Pause), vec![KeyCode::Escape], "pause can't be rebound");
        let mut table = std::collections::HashMap::new();
        table.insert(Act::Attack, vec![KeyCode::Z]);
        table.insert(Act::Map, vec![KeyCode::M]);
        assert_eq!(binding_names(&table).into_iter().collect::<Vec<_>>(), vec![("attack".to_string(), vec!["z".to_string()])]);
        // A bound key conflicts with the action that still has it.
        keys.insert("attack".to_string(), vec!["j".to_string()]);
        set_bindings(&keys);
        assert_eq!(conflicts(Act::Attack, KeyCode::J), vec![Act::Journal]);
        set_bindings(&Default::default());
        assert_eq!(keys_of(Act::Attack), vec![KeyCode::X]);
    }
}
