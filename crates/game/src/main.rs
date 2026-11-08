//! R(ust)PG2D game runner.
// No console window on Windows (output goes to the terminal it was started from, if any).
#![cfg_attr(windows, windows_subsystem = "windows")]
//!
//! Usage: rustpg2d-game [PROJECT_DIR] [--map NAME] [--spawn NAME] [--pos X,Y] [--no-title] [--dev] [--lang CODE]
//!
//! Without PROJECT_DIR a released game runs the project in `data/` next to the executable
//! (saves in `~/.local/share/<game>/saves`); otherwise `example_project` is used.

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use macroquad::prelude::*;
use rustpg2d_engine::project::{Project, ProjectConfig};
use rustpg2d_engine::release;
use rustpg2d_engine::runtime::{Game, StartOptions};

/// Output of automated runs and errors: the terminal, or the browser console in the web game.
macro_rules! say {
    ($($t:tt)*) => {{
        #[cfg(not(target_arch = "wasm32"))]
        println!($($t)*);
        #[cfg(target_arch = "wasm32")]
        macroquad::logging::info!($($t)*);
    }};
}

/// Project folder and whether it is the bundle next to the executable (the web game: the
/// files of `data.pack`, mounted as `data/`).
fn default_root() -> (PathBuf, bool) {
    if cfg!(target_arch = "wasm32") {
        return (PathBuf::from("data"), true);
    }
    match release::bundled_root() {
        Some(r) => (r, true),
        None => (PathBuf::from("example_project"), false),
    }
}

/// The project folder given on the command line, if any.
fn root_arg() -> Option<PathBuf> {
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--map" | "--spawn" | "--pos" | "--cutscene" | "--lang" => {
                args.next();
            }
            s if s.starts_with('-') => {}
            s => return Some(PathBuf::from(s)),
        }
    }
    None
}

fn parse_args() -> Result<(PathBuf, StartOptions)> {
    let mut root = default_root().0;
    let mut start = StartOptions::default();
    let mut no_title = false;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut value = || args.next().with_context(|| format!("{a} needs a value"));
        match a.as_str() {
            "--map" => start.map = Some(value()?),
            "--no-title" => no_title = true,
            "--dev" => start.dev = true,
            "--lang" => start.lang = Some(value()?),
            "--engine-build" => {}
            "--spawn" => start.spawn = Some(value()?),
            "--cutscene" => start.cutscene = Some(value()?),
            "--pos" => {
                let v = value()?;
                let (x, y) = v.split_once(',').context("--pos expects X,Y")?;
                start.pos = Some(vec2(x.trim().parse()?, y.trim().parse()?));
            }
            "-h" | "--help" => {
                println!("usage: rustpg2d-game [PROJECT_DIR] [--map NAME] [--spawn NAME] [--pos X,Y] [--no-title] [--cutscene FUNCTION] [--dev] [--lang CODE]");
                std::process::exit(0);
            }
            s if s.starts_with('-') => bail!("unknown option {s}"),
            s => root = PathBuf::from(s),
        }
    }
    // The title screen is skipped when starting somewhere specific (editor play-tests).
    start.title = !no_title && start.map.is_none() && start.spawn.is_none() && start.pos.is_none();
    Ok((root, start))
}

fn window_conf() -> Conf {
    rustpg2d_engine::desktop::startup("The game");
    if std::env::args().any(|a| a == "--engine-build") {
        println!("{}", rustpg2d_engine::ENGINE_BUILD);
        std::process::exit(0);
    }
    // The window is titled with the game's name.
    let root = root_arg().unwrap_or_else(|| default_root().0);
    let title = std::fs::read_to_string(root.join("project.ron"))
        .ok()
        .and_then(|s| ron::from_str::<ProjectConfig>(&s).ok())
        .map(|c| c.name)
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| "R(ust)PG2D".to_owned());
    // The window icon: the player character, else R(ust)PG2D's emblem.
    let icon = Project::load(&root).ok().and_then(|p| rustpg2d_engine::icon::hero_icon(&p)).unwrap_or_else(rustpg2d_engine::icon::emblem);
    Conf {
        window_title: title,
        icon: Some(rustpg2d_engine::icon::window_icon(&icon)),
        window_width: 1280,
        window_height: 720,
        high_dpi: true,
        ..Default::default()
    }
}

/// The web game: fetches `data.pack` (every file of the game, written by the web export) and
/// mounts it as `data/`, showing a loading screen meanwhile.
#[cfg(target_arch = "wasm32")]
async fn web_mount() -> Result<()> {
    loading_screen("Loading...", None);
    next_frame().await;
    let pack = load_file("data.pack").await.map_err(|e| anyhow::anyhow!("data.pack: {e}"))?;
    let files: Vec<(String, Vec<u8>)> = rustpg2d_engine::release::read_web_pack(&pack)?;
    rustpg2d_engine::sys::web::mount(files.into_iter().map(|(k, v)| (format!("data/{k}"), v)).collect());
    Ok(())
}

/// A plain loading screen (the web game, before the game itself runs).
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
fn loading_screen(text: &str, progress: Option<f32>) {
    clear_background(Color::from_rgba(12, 12, 18, 255));
    let w = measure_text(text, None, 28, 1.0).width;
    draw_text(text, (screen_width() - w) / 2.0, screen_height() / 2.0 - 10.0, 28.0, LIGHTGRAY);
    if let Some(p) = progress {
        let (bw, x, y) = (300.0, (screen_width() - 300.0) / 2.0, screen_height() / 2.0 + 10.0);
        draw_rectangle_lines(x, y, bw, 10.0, 2.0, GRAY);
        draw_rectangle(x + 2.0, y + 2.0, (bw - 4.0) * p.clamp(0.0, 1.0), 6.0, LIGHTGRAY);
    }
}

/// The web game can't start: the reason on the canvas (and in the browser console).
#[cfg(target_arch = "wasm32")]
async fn web_failed(e: anyhow::Error) {
    macroquad::logging::error!("{e:#}");
    loop {
        loading_screen(&format!("The game could not start: {e}"), None);
        next_frame().await;
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    #[cfg(target_arch = "wasm32")]
    {
        std::panic::set_hook(Box::new(|info| {
            macroquad::logging::error!("{info}");
        }));
        if let Err(e) = web_mount().await {
            return web_failed(e).await;
        }
    }
    let project = parse_args().and_then(|(root, start)| Ok((Project::load(&root).with_context(|| format!("loading project {}", root.display()))?, start)));
    // The web game: sounds decode asynchronously in the browser; load them before the game.
    #[cfg(target_arch = "wasm32")]
    if let Ok((p, _)) = &project {
        let mut shown = 0.0;
        rustpg2d_engine::audio::preload(p, |done, total| {
            shown = done as f32 / total.max(1) as f32;
            loading_screen("Loading sounds...", Some(shown));
        })
        .await;
    }
    let result = (|| -> Result<Game> {
        let (project, mut start) = project?;
        // A released game (the bundle next to the executable) never gets the developer console.
        if start.dev && root_arg().is_none() && default_root().1 {
            eprintln!("[game] --dev is only for play-tests from the editor");
            start.dev = false;
        }
        // A released game may live in a read-only folder: keep saves in the user's data folder.
        if root_arg().is_none() && default_root().1 {
            if let Some(dir) = release::user_saves_dir(&project.config.name) {
                rustpg2d_engine::project::set_saves_dir(dir);
            }
        }
        Game::new(project, start)
    })();
    let mut game = match result {
        Ok(g) => g,
        Err(e) => {
            #[cfg(target_arch = "wasm32")]
            return web_failed(e).await;
            #[cfg(not(target_arch = "wasm32"))]
            {
                eprintln!("error: {e:#}");
                std::process::exit(1);
            }
        }
    };
    // Debug aids:
    //   RUSTPG2D_SHOT=out.png        screenshot after ~1s, then exit
    //   RUSTPG2D_AUTOPLAY="hold right 2; press e; wait 1; shot a.png; status; quit"
    //   the web game: index.html?autoplay=wait 2; status (output in the browser console)
    #[cfg(target_arch = "wasm32")]
    let web_script = rustpg2d_engine::sys::web::query().strip_prefix("autoplay=").map(str::to_string).ok_or(std::env::VarError::NotPresent);
    #[cfg(not(target_arch = "wasm32"))]
    let web_script: Result<String, std::env::VarError> = Err(std::env::VarError::NotPresent);
    let mut auto = match (std::env::var("RUSTPG2D_AUTOPLAY").or(web_script), std::env::var("RUSTPG2D_SHOT")) {
        (Ok(script), _) => Some(Autoplay::parse(&script)),
        (_, Ok(path)) => Some(Autoplay::parse(&format!("wait 1; shot {path}; quit"))),
        _ => None,
    };
    // The window's close button asks whether to save first (see Game::request_close).
    prevent_quit();
    loop {
        if let Some(a) = &mut auto {
            if !a.step(&game) {
                break;
            }
            if std::mem::take(&mut a.close) {
                game.request_close();
            }
            if let Some(h) = a.hour.take() {
                game.set_hour(h);
            }
            if let Some(h) = a.advance.take() {
                game.advance_time(h);
            }
            for line in std::mem::take(&mut a.console) {
                game.console_command(&line);
            }
            for (cmd, id) in std::mem::take(&mut a.skill_cmds) {
                match cmd.as_str() {
                    "skillpoint" => game.spend_skill_point(&id),
                    "call" => game.run_script(&id),
                    "cast" => {
                        game.cast_spell(&id);
                    }
                    _ => {
                        game.learn_spell(&id);
                    }
                }
            }
        }
        if is_quit_requested() {
            game.request_close();
        }
        game.frame();
        if game.wants_quit() {
            break;
        }
        if let Some(path) = auto.as_mut().and_then(|a| a.shot.take()) {
            save_shot(&path);
        }
        next_frame().await;
    }
}

/// Scripted input for automated play-throughs.
struct Autoplay {
    cmds: std::collections::VecDeque<Vec<String>>,
    timer: f32,
    held: Vec<KeyCode>,
    /// Gamepad buttons held by `padhold`.
    held_pad: Vec<rustpg2d_engine::runtime::input::Pad>,
    /// Screenshot to take after the next frame is drawn.
    shot: Option<String>,
    mouse: Option<Vec2>,
    mouse_frames: std::collections::VecDeque<rustpg2d_engine::runtime::input::MouseInject>,
    /// Simulates the window's close button.
    close: bool,
    /// `settime H`: applied by the main loop.
    hour: Option<f32>,
    /// `settime +H`: hours to move the clock on.
    advance: Option<f32>,
    skill_cmds: Vec<(String, String)>,
    /// `console LINE`: applied by the main loop.
    console: Vec<String>,
    /// `minigame hit|miss|bite`: waiting for that moment (probe name, seconds left), then E.
    until: Option<(&'static str, f32)>,
}

impl Autoplay {
    fn parse(script: &str) -> Self {
        let cmds = script
            .split(';')
            .map(|c| c.split_whitespace().map(str::to_string).collect::<Vec<_>>())
            .filter(|c| !c.is_empty())
            .collect();
        Autoplay { cmds, timer: 0.0, held: Vec::new(), held_pad: Vec::new(), shot: None, mouse: None, mouse_frames: Default::default(), close: false, hour: None, advance: None, skill_cmds: Vec::new(), console: Vec::new(), until: None }
    }

    /// Advances the script; returns false to quit.
    fn step(&mut self, game: &Game) -> bool {
        use rustpg2d_engine::runtime::input::{inject, inject_mouse, inject_pad, key_from_name, MouseInject, Pad};
        // Queued mouse frames (clicks, drags) play one per frame; afterwards the cursor rests.
        if let Some(f) = self.mouse_frames.pop_front() {
            self.mouse = Some(f.pos);
            inject_mouse(Some(f));
            return true;
        }
        if let Some(p) = self.mouse {
            inject_mouse(Some(MouseInject { pos: p, ..Default::default() }));
        }
        let click = |frames: &mut std::collections::VecDeque<MouseInject>, p: Vec2, b: MouseButton| {
            frames.push_back(MouseInject { pos: p, pressed: vec![b], down: vec![b], ..Default::default() });
            frames.push_back(MouseInject { pos: p, released: vec![b], ..Default::default() });
            frames.push_back(MouseInject { pos: p, ..Default::default() });
        };
        if let Some((probe, left)) = &mut self.until {
            if game.minigame_probe(probe) {
                self.until = None;
                inject(&[KeyCode::E], &[KeyCode::E]);
                self.timer = 0.1;
                return true;
            }
            *left -= get_frame_time();
            if *left <= 0.0 {
                say!("[autoplay] minigame: '{probe}' never came");
                self.until = None;
            }
            inject(&[], &[]);
            return true;
        }
        if self.timer > 0.0 {
            self.timer -= get_frame_time();
            inject(&self.held, &[]);
            inject_pad(&self.held_pad, &[]);
            return true;
        }
        self.held.clear();
        self.held_pad.clear();
        inject(&[], &[]);
        inject_pad(&[], &[]);
        while let Some(cmd) = self.cmds.pop_front() {
            let arg = |i: usize| cmd.get(i).map(String::as_str).unwrap_or("");
            let secs = |i: usize| arg(i).parse::<f32>().unwrap_or(0.5);
            match arg(0) {
                "hold" => {
                    self.held = arg(1).split('+').filter_map(key_from_name).collect();
                    self.timer = secs(2);
                    inject(&self.held, &[]);
                    return true;
                }
                "press" => {
                    let keys: Vec<_> = arg(1).split('+').filter_map(key_from_name).collect();
                    inject(&keys, &keys); // held too, so "shift+g" works
                    // Let one frame consume the press, then idle a little.
                    self.timer = 0.1;
                    return true;
                }
                // pad BUTTON[+BUTTON]: press gamepad buttons (a b x y lb rb lt rt select start ls rs
                // up down left right) · padhold BUTTON SECS: hold them
                "pad" | "padhold" => {
                    let buttons: Vec<Pad> = arg(1).split('+').filter_map(Pad::from_name).collect();
                    if buttons.is_empty() {
                        say!("[autoplay] pad: unknown button '{}'", arg(1));
                        continue;
                    }
                    if arg(0) == "pad" {
                        inject_pad(&buttons, &buttons);
                        self.timer = 0.1;
                    } else {
                        inject_pad(&buttons, &[]);
                        self.timer = secs(2);
                        self.held_pad = buttons;
                    }
                    return true;
                }
                // click X Y [right]: mouse click on a tile (tile coordinates)
                "click" | "rclick" | "clickenemy" => {
                    let p = if arg(0) == "clickenemy" {
                        let Some((x, y)) = game.nearest_enemy_tile() else { continue };
                        game.tile_to_screen(x, y)
                    } else {
                        game.tile_to_screen(secs(1), secs(2))
                    };
                    let b = if arg(0) == "rclick" { MouseButton::Right } else { MouseButton::Left };
                    click(&mut self.mouse_frames, p, b);
                    return true;
                }
                // slot player|chest|equip IDX [right|double]: click a slot of the open inventory / loot window
                // hud bag|character|skills|map: click a button in the bottom-right corner
                "hud" => {
                    let Some(p) = game.hud_button_pos(arg(1)) else { continue };
                    click(&mut self.mouse_frames, p, MouseButton::Left);
                    return true;
                }
                // wintab character|skills|spells: click a tab of the open character window
                "wintab" => {
                    let Some(p) = game.window_tab_pos(arg(1)) else {
                        say!("[autoplay] wintab: no such tab (or the window is closed)");
                        continue;
                    };
                    click(&mut self.mouse_frames, p, MouseButton::Left);
                    return true;
                }
                // opt ROW prev|next|row: click the "<" / ">" arrows (or the label) of an Options row
                "opt" => {
                    let Some(p) = game.option_click_pos(secs(1) as usize, arg(2)) else {
                        say!("[autoplay] opt: Options page not open");
                        continue;
                    };
                    click(&mut self.mouse_frames, p, MouseButton::Left);
                    return true;
                }
                "slot" => {
                    let Some(p) = game.slot_screen_pos(arg(1), secs(2) as usize) else { continue };
                    let b = if arg(3) == "right" { MouseButton::Right } else { MouseButton::Left };
                    click(&mut self.mouse_frames, p, b);
                    if arg(3) == "double" {
                        click(&mut self.mouse_frames, p, b);
                    }
                    return true;
                }
                // shop buy|sell ROW [shift]: click the trade button of a row in the open shop
                "shop" => {
                    let Some(p) = game.shop_button_pos(arg(1), secs(2) as usize) else {
                        say!("[autoplay] shop: no {} row {}", arg(1), arg(2));
                        continue;
                    };
                    click(&mut self.mouse_frames, p, MouseButton::Left);
                    return true;
                }
                // minigame hit|miss|bite: press E when the lockpick marker is in the sweet spot
                // (hit) / well outside it (miss), or when a fish bites (bite); waits up to 8 s
                "minigame" => {
                    let probe = match arg(1) {
                        "hit" => "in_spot",
                        "miss" => "out_spot",
                        _ => "bite",
                    };
                    self.until = Some((probe, 8.0));
                    return true;
                }
                // craft K: click the Make button of recipe row K in the crafting window
                "craft" => {
                    let Some(p) = game.crafting_button_pos(secs(1) as usize) else {
                        say!("[autoplay] craft: no recipe row {}", arg(1));
                        continue;
                    };
                    click(&mut self.mouse_frames, p, MouseButton::Left);
                    return true;
                }
                // mapbtn close|zoomin|zoomout|center: click a button of the open map screen
                "mapbtn" => {
                    let Some(p) = game.world_map_button_pos(arg(1)) else {
                        say!("[autoplay] mapbtn: map screen not open");
                        continue;
                    };
                    click(&mut self.mouse_frames, p, MouseButton::Left);
                    return true;
                }
                // drag X1 Y1 X2 Y2: left-button drag in screen pixels
                "drag" => {
                    let (a, b) = (vec2(secs(1), secs(2)), vec2(secs(3), secs(4)));
                    let l = MouseButton::Left;
                    self.mouse_frames.push_back(MouseInject { pos: a, pressed: vec![l], down: vec![l], ..Default::default() });
                    for k in 1..=8 {
                        self.mouse_frames.push_back(MouseInject { pos: a.lerp(b, k as f32 / 8.0), down: vec![l], ..Default::default() });
                    }
                    self.mouse_frames.push_back(MouseInject { pos: b, released: vec![l], ..Default::default() });
                    self.mouse_frames.push_back(MouseInject { pos: b, ..Default::default() });
                    return true;
                }
                // wheel DY: one mouse wheel step (positive = up) at the current mouse position
                "wheel" => {
                    let pos = self.mouse.unwrap_or(vec2(screen_width() / 2.0, screen_height() / 2.0));
                    self.mouse_frames.push_back(MouseInject { pos, wheel: secs(1), ..Default::default() });
                    self.mouse_frames.push_back(MouseInject { pos, ..Default::default() });
                    return true;
                }
                // slotdrag player|chest IDX player|chest IDX
                "slotdrag" => {
                    // "slotdrag player 3 out" drags the stack out of the window (drops it).
                    let to = if arg(3) == "out" { Some(vec2(8.0, screen_height() / 2.0)) } else { game.slot_screen_pos(arg(3), secs(4) as usize) };
                    let (Some(a), Some(b)) = (game.slot_screen_pos(arg(1), secs(2) as usize), to) else { continue };
                    let l = MouseButton::Left;
                    self.mouse_frames.push_back(MouseInject { pos: a, pressed: vec![l], down: vec![l], ..Default::default() });
                    for k in 1..=8 {
                        self.mouse_frames.push_back(MouseInject { pos: a.lerp(b, k as f32 / 8.0), down: vec![l], ..Default::default() });
                    }
                    self.mouse_frames.push_back(MouseInject { pos: b, released: vec![l], ..Default::default() });
                    self.mouse_frames.push_back(MouseInject { pos: b, ..Default::default() });
                    return true;
                }
                // type TEXT: typed characters (rest of the command, spaces kept)
                "type" => {
                    rustpg2d_engine::runtime::input::inject_text(&cmd[1..].join(" "));
                }
                "wait" => {
                    self.timer = secs(1);
                    return true;
                }
                "shot" => {
                    self.shot = Some(arg(1).to_string());
                    return true;
                }
                "status" => say!("[autoplay] {} order={}", game.status(), game.order_debug()),
                // console LINE: runs a developer console line (needs --dev), like typing it after F4
                "console" => {
                    self.console.push(cmd[1..].join(" "));
                    // Runs before the next command (e.g. a status right after it).
                    return true;
                }
                "spawners" => say!("[autoplay] spawners: {}", game.spawner_status()),
                // settime H: jump to a time of day (hours)
                // settime H: the hour · settime +H: H hours later (days pass, props regrow)
                "settime" => match arg(1).strip_prefix('+') {
                    Some(h) => self.advance = h.parse().ok(),
                    None => self.hour = arg(1).parse().ok(),
                },
                // skillpoint ID: spend a skill point · cast ID: cast a spell · learn ID: learn a spell
                // call FN: runs a script function (its queued actions too)
                "skillpoint" | "cast" | "learn" | "call" => self.skill_cmds.push((arg(0).to_string(), arg(1).to_string())),
                "quit" => return false,
                // close: as if the window's close button was clicked
                "close" => {
                    self.close = true;
                    return true;
                }
                other => say!("[autoplay] unknown command '{other}'"),
            }
        }
        true
    }
}

/// `shot PATH`: the screen as a PNG file; in the web game logged to the browser console as
/// `[shot] PATH data:image/png;base64,…` (automated runs read it from there).
fn save_shot(path: &str) {
    #[cfg(not(target_arch = "wasm32"))]
    get_screen_data().export_png(path);
    #[cfg(target_arch = "wasm32")]
    {
        let shot = get_screen_data();
        let (w, h) = (shot.width as u32, shot.height as u32);
        let mut img = rustpg2d_engine::imageio::Rgba::new(w, h);
        // (Screen data comes bottom row first.)
        for y in 0..h {
            let src = ((h - 1 - y) * w * 4) as usize;
            img.px[(y * w * 4) as usize..((y + 1) * w * 4) as usize].copy_from_slice(&shot.bytes[src..src + (w * 4) as usize]);
        }
        match img.encode() {
            Ok(png) => say!("[shot] {path} data:image/png;base64,{}", rustpg2d_engine::sys::base64(&png)),
            Err(e) => say!("[shot] {path} failed: {e}"),
        }
    }
}
