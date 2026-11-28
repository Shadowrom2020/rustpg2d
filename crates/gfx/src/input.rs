//! Keys, the mouse and touches: what is down, and what went down or up this frame.
//!
//! The window feeds an [`Input`]; an application that embeds the engine feeds its own and makes it
//! the current one with [`crate::with_screen`]. The functions here answer from the current one.

use std::collections::{HashMap, HashSet, VecDeque};

use glam::Vec2;

use crate::get_context;

/// A key, named by what it types in the keyboard's layout (as miniquad's key codes were).
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[rustfmt::skip]
pub enum KeyCode {
    Space, Apostrophe, Comma, Minus, Period, Slash,
    Key0, Key1, Key2, Key3, Key4, Key5, Key6, Key7, Key8, Key9,
    Semicolon, Equal,
    A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
    LeftBracket, Backslash, RightBracket, GraveAccent, World1, World2,
    Escape, Enter, Tab, Backspace, Insert, Delete, Right, Left, Down, Up, PageUp, PageDown, Home, End,
    CapsLock, ScrollLock, NumLock, PrintScreen, Pause,
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12, F13, F14, F15, F16, F17, F18, F19, F20, F21, F22, F23, F24, F25,
    Kp0, Kp1, Kp2, Kp3, Kp4, Kp5, Kp6, Kp7, Kp8, Kp9, KpDecimal, KpDivide, KpMultiply, KpSubtract, KpAdd, KpEnter, KpEqual,
    LeftShift, LeftControl, LeftAlt, LeftSuper, RightShift, RightControl, RightAlt, RightSuper, Menu,
    /// Android's back button.
    Back,
    Unknown,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
    Unknown,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum TouchPhase {
    Started,
    Stationary,
    Moved,
    Ended,
    Cancelled,
}

/// A finger on the screen; `position` in points.
#[derive(Clone, Debug)]
pub struct Touch {
    pub id: u64,
    pub phase: TouchPhase,
    pub position: Vec2,
}

/// The state of keys, mouse and touches, fed with events and read through this module's functions.
/// Positions are fed in pixels of the screen and read in points.
#[derive(Default)]
pub struct Input {
    keys_down: HashSet<KeyCode>,
    keys_pressed: HashSet<KeyCode>,
    keys_released: HashSet<KeyCode>,
    /// The order keys went down this frame.
    pressed_order: Vec<KeyCode>,
    chars: VecDeque<char>,
    mouse_down: HashSet<MouseButton>,
    mouse_pressed: HashSet<MouseButton>,
    mouse_released: HashSet<MouseButton>,
    mouse_position: Vec2,
    last_mouse_position: Option<Vec2>,
    mouse_wheel: Vec2,
    touches: HashMap<u64, Touch>,
    /// Touches also act as the left mouse button (on by default, see [`Input::new`]).
    simulate_mouse_with_touch: bool,
}

impl Input {
    pub fn new() -> Input {
        Input { simulate_mouse_with_touch: true, ..Default::default() }
    }

    pub fn key_down(&mut self, key: KeyCode, repeat: bool) {
        self.keys_down.insert(key);
        if !repeat {
            self.keys_pressed.insert(key);
            self.pressed_order.push(key);
        }
    }

    pub fn key_up(&mut self, key: KeyCode) {
        self.keys_down.remove(&key);
        self.keys_released.insert(key);
    }

    /// A typed character.
    pub fn char(&mut self, c: char) {
        self.chars.push_back(c);
    }

    /// The pointer moved to a pixel of the screen.
    pub fn mouse_move(&mut self, x: f32, y: f32) {
        self.mouse_position = Vec2::new(x, y);
    }

    pub fn mouse_button(&mut self, button: MouseButton, down: bool) {
        if down {
            self.mouse_down.insert(button);
            self.mouse_pressed.insert(button);
        } else {
            self.mouse_down.remove(&button);
            self.mouse_released.insert(button);
        }
    }

    /// The wheel turned by `x`, `y` notches since the last frame.
    pub fn wheel(&mut self, x: f32, y: f32) {
        self.mouse_wheel += Vec2::new(x, y);
    }

    /// A finger's news; `x`, `y` in pixels of the screen.
    pub fn touch(&mut self, id: u64, phase: TouchPhase, x: f32, y: f32) {
        self.touches.insert(id, Touch { id, phase, position: Vec2::new(x, y) });
        if self.simulate_mouse_with_touch {
            match phase {
                TouchPhase::Started => {
                    self.mouse_move(x, y);
                    self.mouse_button(MouseButton::Left, true);
                }
                TouchPhase::Moved => self.mouse_move(x, y),
                TouchPhase::Ended | TouchPhase::Cancelled => self.mouse_button(MouseButton::Left, false),
                TouchPhase::Stationary => {}
            }
        }
    }

    /// Every key and button goes up (the window lost the keyboard).
    pub fn release_all(&mut self) {
        for k in std::mem::take(&mut self.keys_down) {
            self.keys_released.insert(k);
        }
        for b in std::mem::take(&mut self.mouse_down) {
            self.mouse_released.insert(b);
        }
    }

    /// A frame is over: nothing is "just pressed" any more.
    pub fn end_frame(&mut self) {
        self.keys_pressed.clear();
        self.keys_released.clear();
        self.pressed_order.clear();
        self.mouse_pressed.clear();
        self.mouse_released.clear();
        self.mouse_wheel = Vec2::ZERO;
        self.last_mouse_position = Some(self.mouse_position);
        self.touches.retain(|_, t| !matches!(t.phase, TouchPhase::Ended | TouchPhase::Cancelled));
        for t in self.touches.values_mut() {
            t.phase = TouchPhase::Stationary;
        }
    }
}

fn input() -> &'static mut Input {
    &mut get_context().input
}

fn dpi() -> f32 {
    get_context().screen.dpi_scale()
}

/// The pointer's position in points of the screen.
pub fn mouse_position() -> (f32, f32) {
    let p = input().mouse_position / dpi();
    (p.x, p.y)
}

fn to_local(p: Vec2) -> Vec2 {
    let (w, h) = get_context().screen.logical_size();
    Vec2::new(p.x / w, p.y / h) * 2.0 - Vec2::ONE
}

/// The pointer's position in -1..1 of the screen.
pub fn mouse_position_local() -> Vec2 {
    let (x, y) = mouse_position();
    to_local(Vec2::new(x, y))
}

/// Last frame's pointer position minus this frame's, in -1..1 of the screen (as macroquad's).
pub fn mouse_delta_position() -> Vec2 {
    let now = mouse_position_local();
    let last = input().last_mouse_position.map_or(now, |p| to_local(p / dpi()));
    last - now
}

/// The wheel's turn this frame: (x, y) in notches.
pub fn mouse_wheel() -> (f32, f32) {
    let w = input().mouse_wheel;
    (w.x, w.y)
}

/// The fingers on the screen, positions in points.
pub fn touches() -> Vec<Touch> {
    let d = dpi();
    input().touches.values().map(|t| Touch { id: t.id, phase: t.phase, position: t.position / d }).collect()
}

pub fn simulate_mouse_with_touch(option: bool) {
    input().simulate_mouse_with_touch = option;
}

pub fn is_simulating_mouse_with_touch() -> bool {
    input().simulate_mouse_with_touch
}

/// Went down this frame.
pub fn is_key_pressed(key_code: KeyCode) -> bool {
    input().keys_pressed.contains(&key_code)
}

pub fn is_key_down(key_code: KeyCode) -> bool {
    input().keys_down.contains(&key_code)
}

/// Went up this frame.
pub fn is_key_released(key_code: KeyCode) -> bool {
    input().keys_released.contains(&key_code)
}

pub fn is_any_key_down() -> bool {
    !input().keys_down.is_empty()
}

/// The next typed character, if one waits.
pub fn get_char_pressed() -> Option<char> {
    input().chars.pop_front()
}

/// The key that went down last this frame.
pub fn get_last_key_pressed() -> Option<KeyCode> {
    input().pressed_order.last().copied()
}

pub fn get_keys_pressed() -> HashSet<KeyCode> {
    input().keys_pressed.clone()
}

pub fn get_keys_down() -> HashSet<KeyCode> {
    input().keys_down.clone()
}

pub fn get_keys_released() -> HashSet<KeyCode> {
    input().keys_released.clone()
}

/// Forgets typed characters and what was pressed this frame.
pub fn clear_input_queue() {
    let i = input();
    i.chars.clear();
    i.keys_pressed.clear();
    i.keys_released.clear();
    i.pressed_order.clear();
    i.mouse_pressed.clear();
    i.mouse_released.clear();
}

pub fn is_mouse_button_down(btn: MouseButton) -> bool {
    input().mouse_down.contains(&btn)
}

pub fn is_mouse_button_pressed(btn: MouseButton) -> bool {
    input().mouse_pressed.contains(&btn)
}

pub fn is_mouse_button_released(btn: MouseButton) -> bool {
    input().mouse_released.contains(&btn)
}

/// Shows or hides the pointer over the window.
pub fn show_mouse(shown: bool) {
    get_context().wishes.show_mouse = Some(shown);
}

/// Keeps the pointer inside the window.
pub fn set_cursor_grab(grab: bool) {
    get_context().wishes.grab = Some(grab);
}

/// The window's close button no longer closes it: [`is_quit_requested`] tells the program.
pub fn prevent_quit() {
    get_context().wishes.prevent_quit = true;
}

/// Was the window's close button pressed (after [`prevent_quit`])?
pub fn is_quit_requested() -> bool {
    get_context().wishes.quit_requested
}
