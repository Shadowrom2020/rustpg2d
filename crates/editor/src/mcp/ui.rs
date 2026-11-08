//! *Assets › AI agent server…*: start / stop the server, its port and URL, how to connect the
//! usual clients, and the log of the agents' calls.

use egui_macroquad::egui::{self, Color32, RichText};

use crate::app::Editor;
use crate::ui::{self, ph};

impl Editor {
    pub fn mcp_window(&mut self, ctx: &egui::Context) {
        if !self.mcp.window_open {
            return;
        }
        let mut open = true;
        let mut start = false;
        let mut stop = false;
        let mut save_settings = false;
        egui::Window::new(ui::t(ph::ROBOT, "AI agent server")).open(&mut open).default_width(560.0).show(ctx, |ui| {
            ui.label("Lets AI agents (Claude Code, Cursor, Claude Desktop, …) work on the open project through the Model Context Protocol: they read maps and definitions, build maps, write dialogs, quests and scripts, and play-test. Their edits show here at once and are undoable (Ctrl+Z).");
            ui.weak("The server listens on this computer only (127.0.0.1). Anything that can reach that port can edit the open project.");
            ui.separator();
            match self.mcp.url() {
                Some(url) => {
                    ui.horizontal(|ui| {
                        ui.colored_label(Color32::LIGHT_GREEN, format!("{} Listening on {url}", ph::CHECK_CIRCLE));
                        if ui.button("Stop").clicked() {
                            stop = true;
                        }
                    });
                }
                None => {
                    ui.horizontal(|ui| {
                        ui.label("Stopped.");
                        ui.label("Port");
                        if ui.add(egui::DragValue::new(&mut self.mcp.port).range(1024..=65535)).changed() {
                            save_settings = true;
                        }
                        if ui.button("Start").clicked() {
                            start = true;
                        }
                    });
                }
            }
            if let Some(e) = &self.mcp.last_error {
                ui.colored_label(Color32::LIGHT_RED, e);
            }
            if ui.checkbox(&mut self.mcp.autostart, "Start with the editor").on_hover_text("Remembered in the editor's settings").changed() {
                save_settings = true;
            }
            let url = format!("http://127.0.0.1:{}/mcp", self.mcp.port);
            ui.collapsing("Connect a client", |ui| {
                let snippet = |ui: &mut egui::Ui, title: &str, text: &str| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(title).strong());
                        if ui.small_button(ui::t(ph::COPY, "Copy")).clicked() {
                            ui.ctx().copy_text(text.to_string());
                        }
                    });
                    ui.add(egui::TextEdit::multiline(&mut text.to_string()).code_editor().desired_rows(1).desired_width(f32::INFINITY));
                };
                snippet(ui, "Claude Code (terminal)", &format!("claude mcp add --transport http rustpg2d {url}"));
                snippet(ui, "Cursor / VS Code (.cursor/mcp.json, .vscode/mcp.json)", &format!("{{\"mcpServers\": {{\"rustpg2d\": {{\"url\": \"{url}\"}}}}}}"));
                snippet(ui, "Claude Desktop and other stdio-only clients (needs Node.js)", &format!("{{\"mcpServers\": {{\"rustpg2d\": {{\"command\": \"npx\", \"args\": [\"-y\", \"mcp-remote\", \"{url}\"]}}}}}}"));
                ui.weak("Then ask the agent e.g. \"Look at the project and show me the village\" or use the prompts design_quest / build_map.");
            });
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("Calls: {}", self.mcp.calls)).strong());
                if ui.small_button("Clear log").clicked() {
                    self.mcp.log.clear();
                }
            });
            egui::ScrollArea::vertical().max_height(220.0).stick_to_bottom(true).show(ui, |ui| {
                let now = macroquad::time::get_time();
                for e in &self.mcp.log {
                    ui.horizontal(|ui| {
                        ui.monospace(format!("{:>5.0}s", now - e.at));
                        ui.colored_label(if e.ok { Color32::LIGHT_GREEN } else { Color32::LIGHT_RED }, if e.ok { ph::CHECK } else { ph::X });
                        ui.monospace(format!("{:<20} {:>5} ms", e.tool, e.ms));
                        ui.weak(&e.summary);
                    });
                }
                if self.mcp.log.is_empty() {
                    ui.weak("No calls yet.");
                }
            });
        });
        if start {
            match self.mcp.start(self.mcp.port) {
                Ok(_) => self.set_status("AI agent server started"),
                Err(e) => {
                    self.mcp.last_error = Some(e.clone());
                    self.set_status(format!("AI agent server: {e}"));
                }
            }
        }
        if stop {
            self.mcp.stop();
            self.set_status("AI agent server stopped");
        }
        if save_settings {
            crate::files::save_settings(&crate::files::EditorSettings { mcp_autostart: self.mcp.autostart, mcp_port: self.mcp.port });
        }
        self.mcp.window_open = open;
    }
}
