//! The new editor (ROADMAP.md, *0.36.0 — The Studio*): an eframe window on the engine's own wgpu
//! device, over `editor-core`'s workspace. `shell.rs` is the window, `play.rs` the game in a tab.
//!
//! `rustpg2d-studio [PROJECT_DIR] [--map NAME --spawn NAME]`; without a project the repository's
//! example. `rustpg2d-studio --headless PROJECT [--mcp PORT] [--seconds N]` opens no window: the
//! agent server works on the project (`editor-core`'s workspace). `RUSTPG2D_STUDIO_SCRIPT="wait 3; status; shot /path/a.png; quit"` drives it for tests
//! (`wait SECS`, `status`, `shot PATH` = the whole window, `tab NAME`, `palette TEXT` / `goto
//! TEXT` / `pick N`, `key NAME` = a key for the game, `pause` / `resume`, `quit`; any other line
//! is a command of the editor, e.g. `map.open village` or `edit.undo`).

mod play;
mod shell;

use std::collections::VecDeque;
use std::path::PathBuf;

use eframe::egui;

struct Script {
    cmds: VecDeque<Vec<String>>,
    wait: f32,
}

impl Script {
    fn from_env() -> Script {
        let text = std::env::var("RUSTPG2D_STUDIO_SCRIPT").unwrap_or_default();
        Script { cmds: text.split(';').map(|c| c.split_whitespace().map(str::to_string).collect::<Vec<_>>()).filter(|c| !c.is_empty()).collect(), wait: 0.0 }
    }
}

struct Studio {
    shell: shell::Shell,
    script: Script,
    frames: u32,
    fps_time: f64,
    fps: f32,
}

impl Studio {
    fn new(cc: &eframe::CreationContext<'_>, project_dir: PathBuf, start: rustpg2d_engine::runtime::StartOptions) -> Studio {
        // The engine draws with eframe's device: what it draws is a texture egui can show.
        let rs = cc.wgpu_render_state.as_ref().expect("eframe was started with the wgpu renderer");
        gfx::init(rs.device.clone(), rs.queue.clone(), 16, 16, &gfx::conf::Conf::default());
        let mut shell = shell::Shell::open(project_dir, start);
        shell.ws.looking_at = "the overview".into();
        Studio { shell, script: Script::from_env(), frames: 0, fps_time: 0.0, fps: 0.0 }
    }

    fn run_script(&mut self, ctx: &egui::Context, dt: f32) {
        self.script.wait -= dt;
        while self.script.wait <= 0.0 {
            let Some(cmd) = self.script.cmds.pop_front() else { return };
            let arg = |i: usize| cmd.get(i).map(String::as_str).unwrap_or("");
            match arg(0) {
                "wait" => self.script.wait = arg(1).parse().unwrap_or(1.0),
                "status" => {
                    let game = self.shell.play.as_ref().map(|p| p.status()).unwrap_or_else(|| "play=none".into());
                    println!("[status] fps={:.0} {} | {}", self.fps, self.shell.status(), &game[..game.len().min(300)]);
                }
                "shot" => {
                    let path = arg(1).to_string();
                    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::ScreenshotCallback::new(move |img: std::sync::Arc<egui::ColorImage>| {
                        let px: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
                        let png = rustpg2d_engine::imageio::Rgba { w: img.size[0] as u32, h: img.size[1] as u32, px }.encode();
                        match png.map_err(|e| e.to_string()).and_then(|b| std::fs::write(&path, b).map_err(|e| e.to_string())) {
                            Ok(()) => println!("[shot] {path}"),
                            Err(e) => println!("[shot] {path} failed: {e}"),
                        }
                    })));
                    // (The picture is taken with the next frame.)
                    self.script.wait = 0.2;
                }
                // A tab by its name (`overview`, `play`, `problems`, `history`, `def:item:potion`, `map:village`).
                "tab" => match self.shell.tab_named(arg(1)) {
                    Some(t) => self.shell.show_tab(t),
                    None => println!("[script] no tab '{}'", arg(1)),
                },
                // The palettes: `palette TEXT…` (commands) and `goto TEXT…`; `pick N` = Enter on entry N.
                "palette" | "goto" => {
                    let kind = if arg(0) == "palette" { shell::PaletteKind::Commands } else { shell::PaletteKind::Search };
                    self.shell.open_palette(kind, &cmd[1..].join(" "));
                    for (k, (label, detail, _)) in self.shell.palette_entries().iter().enumerate().take(8) {
                        println!("[palette] {k} {label} | {detail}");
                    }
                }
                "pick" => {
                    if self.shell.palette_pick(arg(1).parse().unwrap_or(0)) {
                        self.shell.palette.open = None;
                    }
                    if let Some(a) = &self.shell.palette.answer {
                        println!("[palette] answer: {a:?}");
                    }
                }
                "key" => {
                    if let (Some(p), Some(k)) = (self.shell.play.as_mut(), play::key_named(arg(1))) {
                        p.tap(k);
                    }
                }
                "pause" | "resume" => {
                    if let Some(p) = self.shell.play.as_mut() {
                        p.paused = arg(0) == "pause";
                    }
                }
                "quit" => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                // Anything else is a command of the editor (`map.open village`, `edit.undo`…).
                _ => {
                    let line = cmd.join(" ");
                    match self.shell.run(&line) {
                        Ok(m) => println!("[cmd] {line}: {}", m.replace('\n', " | ")),
                        Err(e) => println!("[cmd] {line}: failed: {e}"),
                    }
                }
            }
        }
    }
}

impl eframe::App for Studio {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        gfx::begin_frame();
        let dt = gfx::time::get_frame_time();
        self.frames += 1;
        self.fps_time += dt as f64;
        if self.fps_time >= 0.5 {
            self.fps = self.frames as f32 / self.fps_time as f32;
            self.frames = 0;
            self.fps_time = 0.0;
        }
        self.run_script(&ctx, dt);
        // Agents work between frames, like script commands.
        self.shell.ws.mcp_drain();
        self.shell.ui(ui, frame.wgpu_render_state(), self.fps);
        self.shell.ws.mcp_answer_shots(None);
        gfx::end_frame();
        // The game and the agents move on by themselves: a frame after every frame.
        ctx.request_repaint();
    }
}

/// `--headless PROJECT [--mcp PORT]`: the editor without a window. The project is opened and the
/// agent server answers until the process is stopped (or for `--seconds N`): agents and scripts
/// work on a project where there is no display. No textures are loaded and nothing is drawn, so
/// screenshots and thumbnails aren't available; play-tests start the game as usual.
fn headless(project: PathBuf, port: u16, seconds: Option<f64>) -> std::process::ExitCode {
    use editor_core::{Gpu, Workspace};
    let mut ws = match Workspace::open_path(project.clone(), Gpu::No) {
        Ok(ws) => ws,
        Err(e) => {
            eprintln!("[studio] {}: {e:#}", project.display());
            return std::process::ExitCode::FAILURE;
        }
    };
    ws.looking_at = "nothing (no window)".into();
    ws.refresh_refs();
    if let Err(e) = ws.mcp.start(port) {
        eprintln!("[studio] the agent server: {e}");
        return std::process::ExitCode::FAILURE;
    }
    let started = std::time::Instant::now();
    let mut last_tick = 0.0;
    while seconds.is_none_or(|s| started.elapsed().as_secs_f64() < s) {
        ws.mcp_drain();
        ws.mcp_answer_shots(None);
        let now = started.elapsed().as_secs_f64();
        if now - last_tick > 1.0 {
            last_tick = now;
            ws.watch_project();
            ws.project_file_tick();
        }
        std::thread::sleep(std::time::Duration::from_millis(4));
    }
    ws.mcp.stop();
    ws.finish_project_file();
    std::process::ExitCode::SUCCESS
}

fn main() -> std::process::ExitCode {
    // Started in a checkout of R(ust)PG2D: the asset library is the starter library's folder.
    editor_core::repo_mode::init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let opt = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    if let Some(project) = opt("--headless") {
        let port = opt("--mcp").and_then(|p| p.parse().ok()).unwrap_or(editor_core::DEFAULT_PORT);
        return headless(PathBuf::from(project), port, opt("--seconds").and_then(|s| s.parse().ok()));
    }
    match window(&args) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[studio] {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn window(args: &[String]) -> eframe::Result {
    let opt = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let project_dir = args.first().filter(|a| !a.starts_with("--")).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("example_project"));
    let start = rustpg2d_engine::runtime::StartOptions { map: opt("--map"), spawn: opt("--spawn"), title: opt("--map").is_none(), dev: true, ..Default::default() };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_title("R(ust)PG2D Studio").with_inner_size([1500.0, 800.0]),
        renderer: eframe::Renderer::Wgpu,
        // An X11 window where there is an X server, as the game's (`gfx::window`): a Wayland
        // compositor hands no frames to a covered window, and scripted runs would stand still.
        event_loop_builder: Some(Box::new(|builder| {
            #[cfg(all(unix, not(target_os = "macos")))]
            {
                use winit::platform::x11::EventLoopBuilderExtX11;
                let x11 = std::env::var_os("DISPLAY").is_some_and(|d| !d.is_empty());
                if x11 && !std::env::var_os("RUSTPG2D_WAYLAND").is_some_and(|v| v == "1") {
                    builder.with_x11();
                }
            }
            #[cfg(not(all(unix, not(target_os = "macos"))))]
            let _ = builder;
        })),
        ..Default::default()
    };
    eframe::run_native("rustpg2d-studio", options, Box::new(move |cc| Ok(Box::new(Studio::new(cc, project_dir, start)))))
}
