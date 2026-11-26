//! The Play panel: the game running in this process, drawn by the engine into a picture of its
//! own (`gfx::Screen`) that egui shows, with the panel's keys and pointer as its input.

use std::path::Path;

use anyhow::{Context, Result};
use eframe::egui;
use gfx::input::{Input, KeyCode, MouseButton};
use rustpg2d_engine::project::Project;
use rustpg2d_engine::runtime::{Game, StartOptions};

pub struct PlayView {
    game: Game,
    screen: gfx::Screen,
    input: Input,
    /// The screen's picture as egui knows it, and the size it was registered at.
    texture: Option<(egui::TextureId, (u32, u32))>,
    pub paused: bool,
    /// Keys the script tapped: down for this frame, up in the next.
    tapped: Vec<KeyCode>,
}

impl PlayView {
    pub fn open(project_dir: &Path, start: StartOptions) -> Result<PlayView> {
        let project = Project::load(project_dir).with_context(|| format!("loading {}", project_dir.display()))?;
        let mut screen = gfx::Screen::new(1280, 720, 1.0);
        let mut input = Input::new();
        // The game is made with its screen current: it sizes things by it.
        let game = gfx::with_screen(&mut screen, &mut input, || Game::new(project, start))?;
        Ok(PlayView { game, screen, input, texture: None, paused: false, tapped: Vec::new() })
    }

    pub fn status(&self) -> String {
        self.game.status()
    }

    /// A key press for the game from outside (the script).
    pub fn tap(&mut self, key: KeyCode) {
        self.input.key_down(key, false);
        self.tapped.push(key);
    }

    /// Runs a frame of the game at the panel's size and shows it.
    pub fn show(&mut self, ui: &mut egui::Ui, rs: &eframe::egui_wgpu::RenderState) {
        let rect = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(rect, egui::Sense::click());
        let ppp = ui.ctx().pixels_per_point();
        let size = ((rect.width() * ppp).round().max(1.0) as u32, (rect.height() * ppp).round().max(1.0) as u32);
        self.screen.resize(size.0, size.1, ppp);

        // The panel's input becomes the game's.
        let over = response.hovered();
        ui.input(|i| {
            if let Some(pos) = i.pointer.latest_pos() {
                let p = (pos - rect.min) * ppp;
                self.input.mouse_move(p.x, p.y);
            }
            for e in &i.events {
                match e {
                    egui::Event::Key { key, pressed, repeat, .. } if over || !*pressed => {
                        if let Some(k) = key_of(*key) {
                            if *pressed {
                                self.input.key_down(k, *repeat);
                            } else {
                                self.input.key_up(k);
                            }
                        }
                    }
                    egui::Event::Text(t) if over => t.chars().for_each(|c| self.input.char(c)),
                    egui::Event::PointerButton { button, pressed, .. } if over || !*pressed => {
                        let b = match button {
                            egui::PointerButton::Primary => MouseButton::Left,
                            egui::PointerButton::Secondary => MouseButton::Right,
                            egui::PointerButton::Middle => MouseButton::Middle,
                            _ => MouseButton::Unknown,
                        };
                        self.input.mouse_button(b, *pressed);
                    }
                    egui::Event::MouseWheel { delta, unit, .. } if over => {
                        let notch = if *unit == egui::MouseWheelUnit::Point { 40.0 } else { 1.0 };
                        self.input.wheel(delta.x / notch, delta.y / notch);
                    }
                    _ => {}
                }
            }
        });

        if !self.paused {
            let game = &mut self.game;
            gfx::with_screen(&mut self.screen, &mut self.input, || game.frame());
        }
        self.input.end_frame();
        for k in self.tapped.drain(..) {
            self.input.key_up(k);
        }

        // egui shows the screen's texture; a new size is a new texture.
        let view = self.screen.texture().view();
        let id = match self.texture {
            Some((id, at)) if at == size => id,
            Some((id, _)) => {
                rs.renderer.write().update_egui_texture_from_wgpu_texture(&rs.device, view, eframe::wgpu::FilterMode::Nearest, id);
                self.texture = Some((id, size));
                id
            }
            None => {
                let id = rs.renderer.write().register_native_texture(&rs.device, view, eframe::wgpu::FilterMode::Nearest);
                self.texture = Some((id, size));
                id
            }
        };
        ui.painter().image(id, rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
    }
}

/// egui's key as the game's.
fn key_of(key: egui::Key) -> Option<KeyCode> {
    use egui::Key as E;
    use KeyCode as K;
    Some(match key {
        E::ArrowDown => K::Down,
        E::ArrowLeft => K::Left,
        E::ArrowRight => K::Right,
        E::ArrowUp => K::Up,
        E::Escape => K::Escape,
        E::Tab => K::Tab,
        E::Backspace => K::Backspace,
        E::Enter => K::Enter,
        E::Space => K::Space,
        E::Insert => K::Insert,
        E::Delete => K::Delete,
        E::Home => K::Home,
        E::End => K::End,
        E::PageUp => K::PageUp,
        E::PageDown => K::PageDown,
        E::Minus => K::Minus,
        E::Equals => K::Equal,
        E::Comma => K::Comma,
        E::Period => K::Period,
        E::Slash => K::Slash,
        E::Semicolon => K::Semicolon,
        E::OpenBracket => K::LeftBracket,
        E::CloseBracket => K::RightBracket,
        E::Backslash => K::Backslash,
        E::Backtick => K::GraveAccent,
        E::Num0 => K::Key0,
        E::Num1 => K::Key1,
        E::Num2 => K::Key2,
        E::Num3 => K::Key3,
        E::Num4 => K::Key4,
        E::Num5 => K::Key5,
        E::Num6 => K::Key6,
        E::Num7 => K::Key7,
        E::Num8 => K::Key8,
        E::Num9 => K::Key9,
        E::A => K::A,
        E::B => K::B,
        E::C => K::C,
        E::D => K::D,
        E::E => K::E,
        E::F => K::F,
        E::G => K::G,
        E::H => K::H,
        E::I => K::I,
        E::J => K::J,
        E::K => K::K,
        E::L => K::L,
        E::M => K::M,
        E::N => K::N,
        E::O => K::O,
        E::P => K::P,
        E::Q => K::Q,
        E::R => K::R,
        E::S => K::S,
        E::T => K::T,
        E::U => K::U,
        E::V => K::V,
        E::W => K::W,
        E::X => K::X,
        E::Y => K::Y,
        E::Z => K::Z,
        E::F1 => K::F1,
        E::F2 => K::F2,
        E::F3 => K::F3,
        E::F4 => K::F4,
        E::F5 => K::F5,
        E::F6 => K::F6,
        E::F7 => K::F7,
        E::F8 => K::F8,
        E::F9 => K::F9,
        E::F10 => K::F10,
        E::F11 => K::F11,
        E::F12 => K::F12,
        _ => return None,
    })
}

/// A key by the name scripts use (`e`, `escape`, `down`…).
pub fn key_named(name: &str) -> Option<KeyCode> {
    egui::Key::from_name(name).or_else(|| egui::Key::from_name(&name.to_uppercase())).and_then(key_of).or(match name {
        "up" => Some(KeyCode::Up),
        "down" => Some(KeyCode::Down),
        "left" => Some(KeyCode::Left),
        "right" => Some(KeyCode::Right),
        "escape" => Some(KeyCode::Escape),
        "enter" => Some(KeyCode::Enter),
        "space" => Some(KeyCode::Space),
        "tab" => Some(KeyCode::Tab),
        _ => None,
    })
}
