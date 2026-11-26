//! The new editor, at its very beginning: an eframe window that shares its wgpu device with the
//! engine and shows the running game in a panel (ROADMAP.md, *0.36.0 — The Studio*).
//!
//! `rustpg2d-studio [PROJECT_DIR] [--map NAME --spawn NAME]`; without a project the repository's
//! example. `rustpg2d-studio --headless PROJECT [--mcp PORT] [--seconds N]` opens no window: the
//! agent server works on the project (`editor-core`'s workspace). `RUSTPG2D_STUDIO_SCRIPT="wait 3; status; shot /path/a.png; quit"` drives it for tests
//! (commands: `wait SECS`, `status`, `shot PATH` = the whole window, `key NAME` = a key for the
//! game, `pause` / `resume`, `quit`).

mod play;

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
    project_dir: PathBuf,
    start: rustpg2d_engine::runtime::StartOptions,
    play: Option<play::PlayView>,
    error: Option<String>,
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
        Studio { project_dir, start, play: None, error: None, script: Script::from_env(), frames: 0, fps_time: 0.0, fps: 0.0 }
    }

    fn run_script(&mut self, ctx: &egui::Context, dt: f32) {
        self.script.wait -= dt;
        while self.script.wait <= 0.0 {
            let Some(cmd) = self.script.cmds.pop_front() else { return };
            let arg = |i: usize| cmd.get(i).map(String::as_str).unwrap_or("");
            match arg(0) {
                "wait" => self.script.wait = arg(1).parse().unwrap_or(1.0),
                "status" => {
                    let game = self.play.as_ref().map(|p| p.status()).unwrap_or_else(|| "play=none".into());
                    println!("[status] fps={:.0} {}", self.fps, &game[..game.len().min(600)]);
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
                "key" => {
                    if let (Some(p), Some(k)) = (self.play.as_mut(), play::key_named(arg(1))) {
                        p.tap(k);
                    }
                }
                "pause" => {
                    if let Some(p) = self.play.as_mut() {
                        p.paused = true;
                    }
                }
                "resume" => {
                    if let Some(p) = self.play.as_mut() {
                        p.paused = false;
                    }
                }
                "quit" => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                other => println!("[script] unknown command '{other}'"),
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
        if self.play.is_none() && self.error.is_none() {
            match play::PlayView::open(&self.project_dir, self.start.clone()) {
                Ok(p) => self.play = Some(p),
                Err(e) => self.error = Some(format!("{e:#}")),
            }
        }
        self.run_script(&ctx, dt);

        egui::Panel::left("project").default_size(240.0).show(ui, |ui| {
            ui.heading("R(ust)PG2D Studio");
            ui.label(format!("{:.0} frames a second", self.fps));
            ui.separator();
            ui.label(self.project_dir.display().to_string());
            if let Some(p) = self.play.as_mut() {
                let label = if p.paused { "Resume" } else { "Pause" };
                if ui.button(label).clicked() {
                    p.paused = !p.paused;
                }
                let stats = gfx::window::frame_stats();
                ui.label(format!("{} draw calls, {} passes", stats.draw_calls, stats.passes));
                ui.separator();
                ui.label("Click the game to play it: the keys go to it while the pointer is over it.");
            }
            if let Some(e) = &self.error {
                ui.colored_label(egui::Color32::LIGHT_RED, e);
            }
        });
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(ui, |ui| {
            if let (Some(p), Some(rs)) = (self.play.as_mut(), frame.wgpu_render_state()) {
                p.show(ui, rs);
            }
        });

        gfx::end_frame();
        // The game moves on by itself: a frame after every frame.
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
