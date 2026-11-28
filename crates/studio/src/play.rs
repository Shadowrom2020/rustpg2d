//! The Play panel: the game running in this process, drawn by the engine into a picture of its
//! own (`gfx::Screen`) that egui shows, with the panel's keys and pointer as its input.

use std::path::Path;

use anyhow::{Context, Result};
use eframe::egui;
use gfx::input::{Input, KeyCode, MouseButton};
use rustpg2d_engine::project::Project;
use rustpg2d_engine::runtime::{Game, Pointed, StartOptions};

/// What the Play panel asks of the window.
pub enum PlayAsk {
    /// Start again: as it was started, from the title screen, or on the start map.
    Again,
    FromTitle,
    FromStart,
    /// The same start in the game's own window (the real runner).
    OwnWindow,
    /// Open what was clicked in the game.
    Open(Pointed),
    /// Keep the running game's state as the project's test setup; forget the setup.
    KeepSetup(Box<editor_core::testsetup::TestSetup>),
    ForgetSetup,
    /// *Test setup…*: the setup as a form.
    EditSetup,
}

pub struct PlayView {
    game: Game,
    screen: gfx::Screen,
    input: Input,
    /// The screen's picture as egui knows it, and the size it was registered at.
    texture: Option<(egui::TextureId, (u32, u32))>,
    pub paused: bool,
    /// How fast the game's clock runs against the real one (0.25 to 4).
    pub speed: f32,
    /// One step while paused, with the next frame.
    step_once: bool,
    /// An event to raise once the game has run for a moment (*Try it*): seconds left, the
    /// event, whose it is, and what to say of it. Then what came of it.
    raise: Option<(f32, rustpg2d_engine::events::TriggerOn, rustpg2d_engine::runtime::EventOwner, String)>,
    pub raised: Option<String>,
    /// Clicks tell what is under them instead of playing (Alt+click does it at any time).
    pub inspect: bool,
    /// The State panel is shown, and what its lists are searched for.
    pub show_state: bool,
    state_filter: String,
    /// What is typed into the State panel's "add" rows: a flag, a variable, an item.
    add: [String; 3],
    /// Keys the script tapped: down for this frame, up in the next.
    tapped: Vec<KeyCode>,
    /// Keys the script holds down, with the seconds each is still held.
    held: Vec<(KeyCode, f32)>,
}

impl PlayView {
    pub fn open(project_dir: &Path, start: StartOptions) -> Result<PlayView> {
        let project = Project::load(project_dir).with_context(|| format!("loading {}", project_dir.display()))?;
        let mut screen = gfx::Screen::new(1280, 720, 1.0);
        let mut input = Input::new();
        // The game is made with its screen current: it sizes things by it.
        let game = gfx::with_screen(&mut screen, &mut input, || Game::new(project, start))?;
        Ok(PlayView { game, screen, input, texture: None, paused: false, speed: 1.0, step_once: false, raise: None, raised: None, inspect: false, show_state: false, state_filter: String::new(), add: Default::default(), tapped: Vec::new(), held: Vec::new() })
    }

    pub fn status(&self) -> String {
        self.game.status()
    }

    /// A key press for the game from outside (the script).
    pub fn tap(&mut self, key: KeyCode) {
        self.input.key_down(key, false);
        self.tapped.push(key);
    }

    /// A key held down for the game for `secs` (the script: walking takes longer than a tap).
    pub fn hold(&mut self, key: KeyCode, secs: f32) {
        self.input.key_down(key, false);
        self.held.push((key, secs));
    }

    /// Raises an event for a thing of the map once the game has settled (*Try it*).
    pub fn raise_soon(&mut self, on: rustpg2d_engine::events::TriggerOn, owner: rustpg2d_engine::runtime::EventOwner, what: String) {
        self.raise = Some((0.4, on, owner, what));
    }

    /// One step with the next frame, while paused.
    pub fn step(&mut self) {
        self.step_once = true;
    }

    /// Sets or clears a flag of the running game (what the State panel's boxes do).
    pub fn set_flag(&mut self, flag: &str, on: bool) {
        if on {
            self.game.world.flags.insert(flag.to_string());
        } else {
            self.game.world.flags.remove(flag);
        }
    }

    /// The running game's state as a test setup (its level, bag, flags, quests, clock, party).
    pub fn as_setup(&self) -> editor_core::testsetup::TestSetup {
        editor_core::testsetup::TestSetup::of(&self.game.world)
    }

    /// Sets a variable of the running game.
    pub fn set_var(&mut self, name: &str, value: i64) {
        self.game.world.vars.insert(name.to_string(), value);
    }

    /// The panel: its bar, the State panel when it is on, the game in the rest.
    /// `setup`: the project's test setup as the State panel says it (empty: there is none).
    pub fn show(&mut self, ui: &mut egui::Ui, rs: &eframe::egui_wgpu::RenderState, setup: &str) -> Vec<PlayAsk> {
        let mut asks = Vec::new();
        egui::Panel::top("play bar").show(ui, |ui| {
            self.bar(ui, &mut asks);
            if let Some(said) = &self.raised {
                ui.weak(said.as_str());
            }
        });
        if self.show_state {
            egui::Panel::right("play state").default_size(300.0).resizable(true).show(ui, |ui| self.state_panel(ui, setup, &mut asks));
        }
        self.game_view(ui, rs, &mut asks);
        asks
    }

    fn bar(&mut self, ui: &mut egui::Ui, asks: &mut Vec<PlayAsk>) {
        ui.horizontal_wrapped(|ui| {
            ui.menu_button("Restart", |ui| {
                if ui.button("As it was started").clicked() {
                    asks.push(PlayAsk::Again);
                    ui.close();
                }
                if ui.button("From the title screen").on_hover_text("As players get it (F5)").clicked() {
                    asks.push(PlayAsk::FromTitle);
                    ui.close();
                }
                if ui.button("On the start map").on_hover_text("Without the title screen").clicked() {
                    asks.push(PlayAsk::FromStart);
                    ui.close();
                }
            });
            if ui.selectable_label(self.paused, "Pause").on_hover_text("Stops the game's clock; the panel stays as it is").clicked() {
                self.paused = !self.paused;
            }
            if ui.add_enabled(self.paused, egui::Button::new("Step")).on_hover_text("One sixtieth of a second further").clicked() {
                self.step_once = true;
            }
            ui.separator();
            ui.weak("Speed");
            for (speed, label) in [(0.25, "1/4"), (0.5, "1/2"), (1.0, "1x"), (2.0, "2x"), (4.0, "4x")] {
                if ui.selectable_label(self.speed == speed, label).clicked() {
                    self.speed = speed;
                }
            }
            ui.separator();
            ui.toggle_value(&mut self.inspect, "Inspect").on_hover_text("A click on a character, a prop or an item opens what it is instead of playing (Alt+click does it at any time)");
            ui.toggle_value(&mut self.show_state, "State").on_hover_text("Flags, variables, quests, the party and the bag of the running game: seen and changed");
            ui.separator();
            if ui.button("Test setup…").on_hover_text("What a test-play starts with: the hero's level, class and bag, flags, variables, quests, the hour, the party").clicked() {
                asks.push(PlayAsk::EditSetup);
            }
            if ui.button("Own window").on_hover_text("The same start in the game's own window, as players run it: for the last check").clicked() {
                asks.push(PlayAsk::OwnWindow);
            }
        });
    }

    /// What the running game holds: seen, searched and changed while it runs.
    fn state_panel(&mut self, ui: &mut egui::Ui, setup: &str, asks: &mut Vec<PlayAsk>) {
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            if ui.button("Keep as the test setup").on_hover_text("What the game holds now (the hero's level, the bag, flags, variables, quests, the hour, the party) is what Play from here with the test setup starts with").clicked() {
                asks.push(PlayAsk::KeepSetup(Box::new(self.as_setup())));
            }
            if !setup.is_empty() && ui.button("Forget it").on_hover_text(format!("The test setup now: {setup}")).clicked() {
                asks.push(PlayAsk::ForgetSetup);
            }
        });
        ui.add(egui::TextEdit::singleline(&mut self.state_filter).hint_text("Search").desired_width(f32::INFINITY)).widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, "Search the state"));
        let f = self.state_filter.trim().to_lowercase();
        let fits = |name: &str| f.is_empty() || name.to_lowercase().contains(&f);
        let world = &mut self.game.world;
        let add = &mut self.add;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("On {}", world.current_map));
                ui.separator();
                ui.label("Day");
                ui.add(egui::DragValue::new(&mut world.day)).widget_info(|| egui::WidgetInfo::labeled(egui::Role::SpinButton, true, "Day"));
                ui.label("Hour");
                ui.add(egui::DragValue::new(&mut world.hour).range(0.0..=23.99).speed(0.05).fixed_decimals(1)).widget_info(|| egui::WidgetInfo::labeled(egui::Role::SpinButton, true, "Hour"));
            });
            egui::CollapsingHeader::new(format!("Flags ({})", world.flags.len())).default_open(true).show(ui, |ui| {
                let mut off = None;
                for flag in world.flags.iter().filter(|n| fits(n)) {
                    let mut on = true;
                    if ui.checkbox(&mut on, flag.as_str()).on_hover_text("Untick to clear it").changed() {
                        off = Some(flag.clone());
                    }
                }
                if let Some(flag) = off {
                    world.flags.remove(&flag);
                }
                ui.horizontal(|ui| {
                    let r = ui.add(egui::TextEdit::singleline(&mut add[0]).hint_text("a flag to set").desired_width(150.0));
                    r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, "A flag to set"));
                    let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if (ui.button("Set").clicked() || enter) && !add[0].trim().is_empty() {
                        world.flags.insert(add[0].trim().to_string());
                        add[0].clear();
                    }
                });
            });
            egui::CollapsingHeader::new(format!("Variables ({})", world.vars.len())).default_open(true).show(ui, |ui| {
                for (name, value) in world.vars.iter_mut().filter(|(n, _)| fits(n)) {
                    ui.horizontal(|ui| {
                        let by = ui.label(name.as_str()).id;
                        ui.add(egui::DragValue::new(value)).labelled_by(by);
                    });
                }
                ui.horizontal(|ui| {
                    let r = ui.add(egui::TextEdit::singleline(&mut add[1]).hint_text("a variable").desired_width(150.0));
                    r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, "A variable to add"));
                    let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if (ui.button("Add").clicked() || enter) && !add[1].trim().is_empty() {
                        world.vars.entry(add[1].trim().to_string()).or_insert(0);
                        add[1].clear();
                    }
                });
            });
            egui::CollapsingHeader::new(format!("Quests ({})", world.quests.len())).default_open(true).show(ui, |ui| {
                use rustpg2d_engine::quest::QuestStatus;
                for (id, q) in world.quests.iter_mut().filter(|(n, _)| fits(n)) {
                    ui.horizontal(|ui| {
                        let by = ui.label(id.as_str()).id;
                        ui.add(egui::DragValue::new(&mut q.stage).prefix("stage ")).labelled_by(by);
                        for (status, label) in [(QuestStatus::Active, "active"), (QuestStatus::Completed, "done"), (QuestStatus::Failed, "failed")] {
                            if ui.selectable_label(q.status == status, label).clicked() {
                                q.status = status;
                            }
                        }
                    });
                }
                if world.quests.is_empty() {
                    ui.weak("None started.");
                }
            });
            egui::CollapsingHeader::new(format!("Party ({})", world.companions.len())).default_open(true).show(ui, |ui| {
                for c in &world.companions {
                    ui.label(format!("{}{}{}", if c.name.is_empty() { format!("#{}", c.uid) } else { c.name.clone() }, c.hp.map_or(String::new(), |hp| format!(" · {hp} hit points")), if c.down.is_some() { " · down" } else { "" }));
                }
                if world.companions.is_empty() {
                    ui.weak("The hero is alone.");
                }
            });
            let held = world.inventory.slots.iter().flatten().count();
            egui::CollapsingHeader::new(format!("Bag ({held})")).default_open(true).show(ui, |ui| {
                for stack in world.inventory.slots.iter_mut() {
                    let Some(st) = stack.as_mut().filter(|st| fits(&st.item)) else { continue };
                    let mut gone = false;
                    ui.horizontal(|ui| {
                        let by = ui.label(st.item.as_str()).id;
                        ui.add(egui::DragValue::new(&mut st.count).range(0..=9999)).labelled_by(by);
                        gone = st.count == 0;
                    });
                    if gone {
                        *stack = None;
                    }
                }
                ui.horizontal(|ui| {
                    let r = ui.add(egui::TextEdit::singleline(&mut add[2]).hint_text("an item's id").desired_width(150.0));
                    r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, "An item to give"));
                    let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if (ui.button("Give").clicked() || enter) && !add[2].trim().is_empty() {
                        world.inventory.add(add[2].trim(), 1);
                        add[2].clear();
                    }
                });
            });
        });
    }

    /// Runs a frame of the game at the panel's size and shows it.
    fn game_view(&mut self, ui: &mut egui::Ui, rs: &eframe::egui_wgpu::RenderState, asks: &mut Vec<PlayAsk>) {
        let rect = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(rect, egui::Sense::click());
        let ppp = ui.ctx().pixels_per_point();
        let size = ((rect.width() * ppp).round().max(1.0) as u32, (rect.height() * ppp).round().max(1.0) as u32);
        self.screen.resize(size.0, size.1, ppp);

        // The panel's input becomes the game's. While inspecting (or with Alt held) the
        // pointer's buttons stay here: a click asks what is under it.
        let over = response.hovered();
        let inspecting = self.inspect || ui.input(|i| i.modifiers.alt);
        let mut clicked = false;
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
                    egui::Event::PointerButton { button, pressed, .. } if inspecting && over => clicked |= *pressed && *button == egui::PointerButton::Primary,
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

        // The game's clock: stopped, a step at a time, slower or faster than the real one. (A
        // fast clock takes several steps a frame; keys pressed count in the first.)
        let real = gfx::time::get_frame_time();
        let (steps, dt) = match (self.paused, std::mem::take(&mut self.step_once)) {
            (true, true) => (1, 1.0 / 60.0),
            (true, false) => (0, 0.0),
            (false, _) if self.speed > 1.0 => (self.speed.round() as u32, real),
            (false, _) => (1, real * self.speed),
        };
        // An event to try: raised once the game has run for a moment.
        if let Some((left, on, owner, what)) = self.raise.take() {
            let left = left - if self.paused { 0.0 } else { real.min(0.1) };
            if left > 0.0 {
                self.raise = Some((left, on, owner, what));
            } else {
                let ran = self.game.raise(&on, owner);
                self.raised = Some(if ran { format!("Tried: {what}") } else { format!("Tried: {what}. Nothing ran: its conditions don't hold in this game") });
                println!("[try] {}", self.raised.as_deref().unwrap_or_default());
            }
        }
        let game = &mut self.game;
        let plain = !self.paused && self.speed == 1.0;
        for _ in 0..if plain { 0 } else { steps } {
            gfx::with_screen(&mut self.screen, &mut self.input, || game.step(dt));
            self.input.end_frame();
        }
        let pointed = gfx::with_screen(&mut self.screen, &mut self.input, || {
            // (At the real clock's speed the game runs its own frame, measuring itself.)
            if plain {
                game.frame();
            } else {
                game.draw();
            }
            (inspecting && over).then(|| game.thing_under_pointer()).flatten()
        });
        self.input.end_frame();
        for k in self.tapped.drain(..) {
            self.input.key_up(k);
        }
        let passed = ui.input(|i| i.stable_dt).min(0.1);
        for h in &mut self.held {
            h.1 -= passed;
            if h.1 <= 0.0 {
                self.input.key_up(h.0);
            }
        }
        self.held.retain(|h| h.1 > 0.0);

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
        if self.paused {
            ui.painter().text(rect.left_top() + egui::vec2(10.0, 8.0), egui::Align2::LEFT_TOP, "Paused", egui::FontId::proportional(14.0), egui::Color32::from_rgb(255, 220, 120));
        }
        // What a click would open, said at the pointer.
        if let Some(p) = pointed {
            let what = if p.name.is_empty() { p.def.clone() } else { format!("{} ({})", p.name, p.def) };
            response.clone().on_hover_text_at_pointer(format!("{what}: click to open what it is"));
            if clicked {
                asks.push(PlayAsk::Open(p));
            }
        }
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
