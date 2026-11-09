//! Developer play-tests (`rustpg2d-game --dev`, what the editor starts): script hot reload and
//! the in-game console.
//!
//! - **Hot reload**: the mtimes and sizes of `scripts/*.rhai` are checked twice a second; on a
//!   change the scripts are compiled again (`ScriptHost::reload_dir`). If a file doesn't
//!   compile, the previous scripts keep running and the error is shown (toast + console).
//! - **Console** (F4): pauses the game while open. Commands: `call FN`, `settime H`,
//!   `give ITEM [N]`, `take ITEM [N]`, `flag NAME on|off`, `var NAME V`, `teleport MAP SPAWN`,
//!   `hp N`, `xp N`, `effects`, `status`, `reload`, `clear`, `help`; anything else is run as
//!   Rhai with the project's script functions (`double(21)`, `set_flag("x", true)`).
//!   Actions that take time (dialogs, teleports, moves) run once the console is closed.
//!
//! Released games ignore `--dev` (see the game binary), so players never get the console.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use macroquad::prelude::*;

use super::input;
use super::Game;
use crate::events::runner::{ActionCtx, ActionRunner};
use crate::events::Action;
use crate::render::wrap_text;

/// Lines the console keeps.
const LOG_MAX: usize = 500;
const FONT: u16 = 20;
const LINE_H: f32 = 22.0;

/// One script file as last seen: path, modification time, size.
type Stamp = (PathBuf, Option<SystemTime>, u64);

#[derive(Default)]
pub(super) struct Dev {
    /// `--dev` was given.
    pub on: bool,
    watch: Vec<Stamp>,
    next_check: f32,
    /// Successful reloads (status).
    pub reloads: u32,
    /// The last reload failed (status / console).
    pub reload_error: bool,
    pub open: bool,
    input: String,
    log: Vec<String>,
    history: Vec<String>,
    /// Position while stepping through `history` with Up / Down.
    hist: Option<usize>,
    /// Time the console is open (cursor blink).
    time: f32,
}

fn snapshot(dir: &Path) -> Vec<Stamp> {
    let mut v: Vec<Stamp> = crate::project::script_files_in(dir)
        .into_iter()
        .map(|p| {
            let m = std::fs::metadata(&p).ok();
            let stamp = (m.as_ref().and_then(|m| m.modified().ok()), m.map_or(0, |m| m.len()));
            (p, stamp.0, stamp.1)
        })
        .collect();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v
}

impl Dev {
    pub fn new(on: bool, scripts: &Path) -> Self {
        let mut d = Dev { on, next_check: 0.5, ..Default::default() };
        if on {
            d.watch = snapshot(scripts);
            d.print("R(ust)PG2D developer console - type help, F4 closes");
        }
        d
    }

    fn print(&mut self, line: impl Into<String>) {
        for l in line.into().lines() {
            self.log.push(l.to_string());
        }
        if self.log.len() > LOG_MAX {
            let n = self.log.len() - LOG_MAX;
            self.log.drain(..n);
        }
    }
}

impl Game {
    /// Hot reload and the console. Returns true while the console is open (the game is
    /// paused then).
    pub(super) fn update_dev(&mut self, dt: f32) -> bool {
        if !self.dev.on {
            return false;
        }
        self.dev.next_check -= dt;
        if self.dev.next_check <= 0.0 {
            self.dev.next_check = 0.5;
            let snap = snapshot(&self.project.scripts_dir());
            if snap != self.dev.watch {
                self.dev.watch = snap;
                self.reload_scripts();
            }
        }
        if input::pressed(KeyCode::F4) {
            self.dev.open = !self.dev.open;
            self.dev.time = 0.0;
            // Characters typed in the same frame don't belong to the console.
            while input::next_char().is_some() {}
            return true;
        }
        if !self.dev.open {
            return false;
        }
        self.dev.time += dt;
        while let Some(ch) = input::next_char() {
            if !ch.is_control() {
                self.dev.input.push(ch);
            }
        }
        if input::pressed(KeyCode::Backspace) {
            self.dev.input.pop();
        }
        // Esc clears the line first, then closes (F4 closes right away).
        if input::pressed(KeyCode::Escape) {
            if self.dev.input.is_empty() {
                self.dev.open = false;
            } else {
                self.dev.input.clear();
                self.dev.hist = None;
            }
            return true;
        }
        let up = input::pressed(KeyCode::Up);
        if up || input::pressed(KeyCode::Down) {
            let n = self.dev.history.len();
            if n > 0 {
                let i = match (self.dev.hist, up) {
                    (None, true) => Some(n - 1),
                    (None, false) => None,
                    (Some(i), true) => Some(i.saturating_sub(1)),
                    (Some(i), false) => (i + 1 < n).then_some(i + 1),
                };
                self.dev.hist = i;
                self.dev.input = i.map(|i| self.dev.history[i].clone()).unwrap_or_default();
            }
        }
        if input::pressed(KeyCode::Enter) || input::pressed(KeyCode::KpEnter) {
            let line = std::mem::take(&mut self.dev.input);
            self.dev.hist = None;
            if !line.trim().is_empty() && self.dev.history.last() != Some(&line) {
                self.dev.history.push(line.clone());
            }
            self.console_command(&line);
        }
        true
    }

    /// Compiles the scripts again (hot reload, console `reload`).
    pub(super) fn reload_scripts(&mut self) {
        match self.scripts.reload_dir(&self.project.scripts_dir()) {
            Ok(n) => {
                self.dev.reloads += 1;
                self.dev.reload_error = false;
                self.dev.print(format!("scripts reloaded ({n} functions)"));
                self.ui.toast("Scripts reloaded".into());
            }
            Err(errors) => {
                self.dev.reload_error = true;
                for e in &errors {
                    self.dev.print(format!("script error: {e}"));
                }
                self.dev.print("the previous scripts keep running until the error is fixed");
                let first = errors.first().map_or("", |e| e.as_str());
                let short: String = if first.chars().count() > 64 { first.chars().take(61).chain("...".chars()).collect() } else { first.to_string() };
                self.ui.toast(format!("Script error: {short} (F4: console)"));
            }
        }
    }

    /// Runs one console line (also the autoplay command `console LINE`).
    pub fn console_command(&mut self, line: &str) {
        let line = line.trim();
        if !self.dev.on || line.is_empty() {
            return;
        }
        self.dev.print(format!("> {line}"));
        let w: Vec<&str> = line.split_whitespace().collect();
        let arg = |i: usize| w.get(i).copied().unwrap_or("");
        let num = |i: usize| arg(i).parse::<i64>().ok();
        let errors_before = self.scripts.errors.len();
        // Actions run by the action runner: right away when the game continues.
        let mut queued: Vec<Action> = Vec::new();
        match arg(0) {
            "help" => {
                for l in [
                    "call FN            run a script function",
                    "settime H          set the time of day (hours)",
                    "give ITEM [N]      give items / take ITEM [N]",
                    "flag NAME on|off   set a flag      var NAME V   set a variable",
                    "teleport MAP SPAWN go to a spawn point of a map",
                    "hp N  xp N         set HP / gain experience",
                    "effects  status    show status effects / the game state",
                    "reload  clear      reload the scripts / clear this log",
                    "anything else runs as Rhai, e.g. double(21) or set_var(\"gold\", 5)",
                    "Up / Down: earlier lines   Esc: clear the line / close   F4: close",
                ] {
                    self.dev.print(l);
                }
            }
            "clear" => self.dev.log.clear(),
            "status" => {
                let s = self.status();
                self.dev.print(s);
            }
            "reload" => self.reload_scripts(),
            "effects" => {
                let s = self.effects_status();
                self.dev.print(s);
            }
            "call" if !arg(1).is_empty() => {
                if self.scripts.has_function(arg(1)) {
                    self.sync_script_player();
                    self.sync_script_player();
                    queued = self.scripts.call(&mut self.world, arg(1), None);
                } else {
                    self.dev.print(format!("no script function {}", arg(1)));
                }
            }
            "settime" => match arg(1).parse::<f32>() {
                Ok(h) => {
                    self.set_hour(h);
                    self.dev.print(format!("time {:.1}", self.world.hour));
                }
                Err(_) => self.dev.print("settime H, e.g. settime 21.5"),
            },
            "give" | "take" if !arg(1).is_empty() => {
                let count = num(2).unwrap_or(1).max(1) as u32;
                if self.project.db.item(arg(1)).is_none() {
                    self.dev.print(format!("unknown item {}", arg(1)));
                } else if arg(0) == "give" {
                    queued.push(Action::GiveItem { item: arg(1).into(), count });
                } else {
                    queued.push(Action::TakeItem { item: arg(1).into(), count });
                }
            }
            "flag" if !arg(1).is_empty() => {
                let on = !matches!(arg(2), "off" | "false" | "0");
                self.world.set_flag(arg(1), on);
                self.dev.print(format!("flag {} = {on}", arg(1)));
            }
            "var" if num(2).is_some() => {
                let v = num(2).unwrap_or(0);
                self.world.set_var(arg(1), v);
                self.dev.print(format!("var {} = {v}", arg(1)));
            }
            "teleport" if !arg(1).is_empty() => {
                if self.project.maps_dir().join(format!("{}.map", arg(1))).exists() {
                    queued.push(Action::Teleport { map: arg(1).into(), spawn: arg(2).into() });
                } else {
                    self.dev.print(format!("no map {}", arg(1)));
                }
            }
            "hp" if num(1).is_some() => {
                let max = self.player_max_hp();
                let hp = (num(1).unwrap_or(1) as i32).clamp(1, max.max(1));
                self.world.player_hp = Some(hp);
                self.dev.print(format!("hp {hp} / {max}"));
            }
            "xp" if num(1).is_some() => {
                self.give_xp(num(1).unwrap_or(0));
                self.dev.print(format!("xp {}", self.world.xp));
            }
            _ => {
                self.sync_script_player();
                self.sync_script_player();
                let (r, actions) = self.scripts.eval(&mut self.world, line);
                match r {
                    Ok(v) if !v.is_empty() => self.dev.print(v),
                    Ok(_) => {}
                    Err(e) => self.dev.print(format!("error: {e}")),
                }
                queued = actions;
            }
        }
        for p in self.scripts.take_printed() {
            self.dev.print(p);
        }
        for e in self.scripts.errors.iter().skip(errors_before).cloned().collect::<Vec<_>>() {
            self.dev.print(format!("error: {e}"));
        }
        if !queued.is_empty() {
            // Instant actions (items, flags, ...) take effect now; the runner continues with
            // whatever has to wait (dialogs, teleports, walking) once the game runs again.
            let ctx = ActionCtx::default();
            let n = queued.iter().take_while(|a| Self::console_instant(a)).count();
            let rest = queued.split_off(n);
            // A runner of its own, so items / flags / variables behave as in triggers.
            let mut now = ActionRunner::default();
            now.push(&queued, &ctx);
            now.update(self, 0.0);
            if !rest.is_empty() {
                self.dev.print(format!("{} action(s) run when the console is closed", rest.len()));
                self.runner.push(&rest, &ctx);
            }
        }
    }

    /// Actions the console carries out at once (they never wait for anything).
    fn console_instant(a: &Action) -> bool {
        matches!(
            a,
            Action::GiveItem { .. }
                | Action::TakeItem { .. }
                | Action::SetFlag { .. }
                | Action::SetVar { .. }
                | Action::Heal { .. }
                | Action::HealCharacter { .. }
                | Action::CompanionOrder { .. }
                | Action::GiveXp { .. }
                | Action::SetTime { .. }
                | Action::Quest { .. }
                | Action::ApplyEffect { .. }
                | Action::CureEffect { .. }
                | Action::GiveSkillPoints { .. }
                | Action::AddMana { .. }
                | Action::SetActive { .. }
                | Action::SetState { .. }
                | Action::SetLocked { .. }
                | Action::PlaySound { .. }
                | Action::PlayMusic { .. }
        )
    }

    /// `status()` part: `dev=off` or `dev=on console=open|closed reloads=N reload_error=B`.
    pub(super) fn dev_status(&self) -> String {
        if !self.dev.on {
            return "dev=off".into();
        }
        format!("dev=on console={} reloads={} reload_error={}", if self.dev.open { "open" } else { "closed" }, self.dev.reloads, self.dev.reload_error)
    }

    /// The console panel over the top of the screen.
    pub(super) fn draw_console(&self) {
        if !self.dev.open {
            return;
        }
        let sw = screen_width();
        let h = (screen_height() * 0.5).round().max(160.0);
        draw_rectangle(0.0, 0.0, sw, h, Color::new(0.02, 0.03, 0.06, 0.95));
        draw_line(0.0, h, sw, h, 2.0, Color::new(0.4, 0.75, 1.0, 1.0));
        let pad = 12.0;
        draw_text("Developer console (F4: close, help: commands)", pad, 22.0, FONT as f32, Color::new(0.5, 0.8, 1.0, 1.0));
        // The input line at the bottom, the log above it (newest last).
        let input_y = h - 12.0;
        let cursor = if (self.dev.time * 2.0) as i32 % 2 == 0 { "_" } else { " " };
        draw_text(format!("> {}{cursor}", self.dev.input), pad, input_y, FONT as f32, WHITE);
        let mut lines: Vec<(String, Color)> = Vec::new();
        for l in &self.dev.log {
            let col = if l.starts_with("> ") {
                Color::new(0.65, 0.65, 0.7, 1.0)
            } else if l.starts_with("error") || l.starts_with("script error") {
                Color::new(1.0, 0.5, 0.45, 1.0)
            } else {
                Color::new(0.9, 0.92, 0.85, 1.0)
            };
            // Lines that fit keep their spacing (the help's columns); longer ones wrap.
            if measure_text(l, None, FONT, 1.0).width <= sw - 2.0 * pad {
                lines.push((l.clone(), col));
            } else {
                for w in wrap_text(l, FONT, sw - 2.0 * pad) {
                    lines.push((w, col));
                }
            }
        }
        let room = ((input_y - LINE_H - 34.0) / LINE_H).floor().max(0.0) as usize;
        let start = lines.len().saturating_sub(room);
        for (i, (l, c)) in lines[start..].iter().enumerate() {
            draw_text(l, pad, 34.0 + LINE_H * (i as f32 + 1.0), FONT as f32, *c);
        }
    }
}
