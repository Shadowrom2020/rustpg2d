//! AI agent server: the Model Context Protocol (MCP) served from inside the running editor, so
//! agents (Claude Code, Cursor, Claude Desktop through `mcp-remote`, …) read and build the open
//! project live — their edits show in the editor at once and are undoable (Ctrl+Z).
//!
//! Transport: Streamable HTTP on `127.0.0.1:<port>/mcp` (`rmcp` on an axum listener) on its own
//! tokio thread (`server.rs`). Every tool call travels to the main thread as an [`AgentRequest`],
//! is executed against the `Editor` between two frames (`exec*.rs`, [`Editor::mcp_drain`]) and
//! answered through a oneshot; only long operations (running the game for a play-test) finish on
//! the tokio side (`play.rs`), and screenshots are taken after the next frame was drawn
//! ([`Editor::mcp_capture_shots`]). `tools.rs` is the catalogue the agent sees, `watcher.rs`
//! reloads project files other programs (an agent's own file tools) change on disk, `ui.rs` the
//! *Assets › AI agent server…* window.

pub mod exec;
pub mod exec_defs;
pub mod exec_maps;
pub mod grid;
pub mod patch;
pub mod play;
pub mod server;
pub mod tools;
pub mod ui;
pub mod watcher;

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::mpsc;
use std::time::Instant;

use crate::app::Editor;

/// The port the server listens on unless the settings say otherwise.
pub const DEFAULT_PORT: u16 = 7743;
/// Requests executed per frame at most, and the time budget for them.
const DRAIN_MAX: usize = 40;
const DRAIN_MS: f64 = 8.0;
const LOG_MAX: usize = 100;

/// A tool call waiting for the main thread.
pub struct AgentRequest {
    pub call: exec::Call,
    /// The parameters, abbreviated, for the call log.
    pub summary: String,
    pub reply: tokio::sync::oneshot::Sender<Result<exec::Reply, String>>,
    pub started: Instant,
}

/// One line of the window's call log.
pub struct LogEntry {
    pub at: f64,
    pub tool: &'static str,
    pub summary: String,
    pub ok: bool,
    pub ms: u32,
}

/// The listening server.
pub struct Running {
    pub addr: SocketAddr,
    rx: mpsc::Receiver<AgentRequest>,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

/// A screenshot an agent asked for: taken once the next frame is drawn.
pub struct ShotRequest {
    pub frames_left: u8,
    pub spec: exec::ShotSpec,
    /// Camera and view options to restore afterwards.
    pub restore: Option<(macroquad::math::Vec2, f32, crate::app::ViewOpts)>,
    pub reply: tokio::sync::oneshot::Sender<Result<exec::Reply, String>>,
    pub started: Instant,
    pub summary: String,
}

/// The editor's side of the agent server (kept across project switches).
pub struct McpState {
    pub running: Option<Running>,
    pub port: u16,
    /// Start the server when the editor starts (saved in the editor settings).
    pub autostart: bool,
    pub window_open: bool,
    pub log: VecDeque<LogEntry>,
    pub calls: usize,
    pub last_error: Option<String>,
    pub shots: Vec<ShotRequest>,
}

impl Default for McpState {
    fn default() -> Self {
        McpState { running: None, port: DEFAULT_PORT, autostart: false, window_open: false, log: VecDeque::new(), calls: 0, last_error: None, shots: Vec::new() }
    }
}

impl McpState {
    /// Starts listening on `127.0.0.1:port` (0 = any free port); the server runs on its own thread.
    pub fn start(&mut self, port: u16) -> Result<SocketAddr, String> {
        if let Some(r) = &self.running {
            return Err(format!("the agent server is already listening on http://{}/mcp", r.addr));
        }
        let listener = std::net::TcpListener::bind(("127.0.0.1", port)).map_err(|e| format!("cannot listen on 127.0.0.1:{port}: {e}"))?;
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let addr = listener.local_addr().map_err(|e| e.to_string())?;
        let (tx, rx) = mpsc::channel();
        let (stop_tx, stop_rx) = tokio::sync::oneshot::channel();
        let thread = std::thread::Builder::new().name("rustpg2d-mcp".into()).spawn(move || server::run(listener, stop_rx, tx)).map_err(|e| e.to_string())?;
        self.running = Some(Running { addr, rx, stop: Some(stop_tx), thread: Some(thread) });
        self.port = addr.port();
        self.last_error = None;
        println!("[mcp] listening on http://{addr}/mcp");
        Ok(addr)
    }

    /// Stops the server (open connections are dropped; the port is free again when this returns).
    pub fn stop(&mut self) {
        let Some(mut r) = self.running.take() else { return };
        if let Some(s) = r.stop.take() {
            let _ = s.send(());
        }
        if let Some(t) = r.thread.take() {
            let _ = t.join();
        }
        // Agents waiting for a reply get "the editor stopped the server" (their oneshots drop).
        self.shots.clear();
        println!("[mcp] stopped");
    }

    pub fn url(&self) -> Option<String> {
        self.running.as_ref().map(|r| format!("http://{}/mcp", r.addr))
    }

    fn log_call(&mut self, tool: &'static str, summary: String, ok: bool, started: Instant) {
        self.calls += 1;
        if self.log.len() >= LOG_MAX {
            self.log.pop_front();
        }
        self.log.push_back(LogEntry { at: macroquad::time::get_time(), tool, summary, ok, ms: started.elapsed().as_millis().min(u32::MAX as u128) as u32 });
    }

    /// `mcp=` for the editor script's `status` line.
    pub fn status(&self) -> String {
        match &self.running {
            Some(r) => format!("{} calls={}", r.addr, self.calls),
            None => format!("off calls={}", self.calls),
        }
    }
}

impl Editor {
    /// Runs the agents' pending tool calls (before the frame is drawn, like script commands).
    pub fn mcp_drain(&mut self) {
        let start = Instant::now();
        let mut n = 0;
        loop {
            if n >= DRAIN_MAX || start.elapsed().as_secs_f64() * 1000.0 > DRAIN_MS {
                break;
            }
            let Some(running) = &self.mcp.running else { break };
            let req = match running.rx.try_recv() {
                Ok(r) => r,
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.mcp.last_error = Some("the agent server thread ended".into());
                    self.mcp.running = None;
                    break;
                }
            };
            n += 1;
            let tool = req.call.name();
            // A tool that panics (a bug, or data it didn't expect) answers with an error; the
            // editor keeps running with its project as it was before the call's undo step.
            let result = match rustpg2d_engine::desktop::catch_panic(|| self.mcp_exec(req.call)) {
                Ok(r) => r,
                Err(msg) => {
                    eprintln!("[mcp] {tool} panicked: {msg}");
                    self.mcp.last_error = Some(format!("{tool} panicked: {msg}"));
                    if let Some(d) = &mut self.doc {
                        d.end_stroke();
                    }
                    Err(format!("internal error in {tool}: {msg} (the editor kept running; read the map or definition again before going on)"))
                }
            };
            match result {
                Ok(exec::Reply::Shot(spec)) => {
                    let restore = self.mcp_prepare_shot(&spec);
                    self.mcp.shots.push(ShotRequest { frames_left: 2, spec, restore, reply: req.reply, started: req.started, summary: req.summary });
                }
                result => {
                    let ok = result.is_ok();
                    let _ = req.reply.send(result);
                    self.mcp.log_call(tool, req.summary, ok, req.started);
                }
            }
        }
    }

    /// Applies the editor settings' choice: starts the server if asked to.
    pub fn mcp_autostart(&mut self) {
        if self.mcp.autostart && self.mcp.running.is_none() {
            if let Err(e) = self.mcp.start(self.mcp.port) {
                self.mcp.last_error = Some(e.clone());
                self.set_status(format!("AI agent server: {e}"));
            }
        }
    }
}
