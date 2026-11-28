//! The AI agents page: the agent server started and stopped, the set-up an agent's program
//! needs to find it (copied with a click), what to ask first, and what the agents did.
//!
//! The server itself is `editor_core::mcp`.

use eframe::egui::{self, Color32, RichText};
use editor_core::mcp::{Client, FIRST_ASK};
use editor_core::Workspace;

#[derive(Default)]
pub struct AgentsPage {
    /// What was copied last: a client's name, or `ask` (shown beside its button; for scripts).
    pub copied: Option<&'static str>,
}

/// What the page asks the window for.
pub enum Want {
    /// The manual's chapter on agents.
    Help,
}

impl AgentsPage {
    /// The script's line: `off|ADDRESS calls=N autostart=on|off copied=…`.
    pub fn status(&self, ws: &Workspace) -> String {
        format!("{} autostart={} copied={}", ws.mcp.status(), if ws.mcp.autostart { "on" } else { "off" }, self.copied.unwrap_or("-"))
    }

    /// The script's words: `start [PORT]`, `stop`, `autostart on|off`, `copy claude|cursor|desktop|ask`.
    pub fn word(&mut self, ctx: &egui::Context, ws: &mut Workspace, args: &[String]) -> Result<(), String> {
        let arg = |i: usize| args.get(i).map(String::as_str).unwrap_or("");
        match arg(0) {
            "" => {}
            "start" => {
                let port = arg(1).parse().unwrap_or(ws.mcp.port);
                ws.mcp.start(port)?;
            }
            "stop" => ws.mcp.stop(),
            "autostart" => {
                ws.mcp.autostart = arg(1) == "on";
                remember(ws);
            }
            "copy" => match (arg(1), Client::named(arg(1))) {
                ("ask", _) => self.copy(ctx, "ask", FIRST_ASK.to_string()),
                (_, Some(client)) => self.copy(ctx, client.name(), client.setup(&ws.mcp.address())),
                _ => return Err("agents copy: claude, cursor, desktop, ask".into()),
            },
            other => return Err(format!("agents {other}: start, stop, autostart, copy")),
        }
        Ok(())
    }

    fn copy(&mut self, ctx: &egui::Context, what: &'static str, text: String) {
        ctx.copy_text(text);
        self.copied = Some(what);
    }

    pub fn show(&mut self, ui: &mut egui::Ui, ws: &mut Workspace) -> Option<Want> {
        let mut want = None;
        ui.heading("AI agents");
        ui.horizontal_wrapped(|ui| {
            ui.label("An AI assistant such as Claude Code, Cursor or Claude Desktop can work in the open project: read and draw maps, place characters, write events, quests and scripts, and play-test. What it does shows here at once, and every change is a step that can be undone.");
            if ui.link("Read about it").on_hover_text("The manual's chapter on agents: what to ask, and every tool they have").clicked() {
                want = Some(Want::Help);
            }
        });
        ui.add_space(8.0);

        // The server.
        ui.strong("1. Start the agent server");
        let (mut start, mut stop, mut changed) = (false, false, false);
        match ws.mcp.url() {
            Some(url) => {
                ui.horizontal(|ui| {
                    ui.colored_label(Color32::from_rgb(140, 210, 140), format!("Listening on {url}"));
                    stop = ui.button("Stop").on_hover_text("Closes the port: agents can't reach the editor any more").clicked();
                });
            }
            None => {
                ui.horizontal(|ui| {
                    ui.label("It is off.");
                    start = ui.button("Start").on_hover_text("Listens for agents on this computer").clicked();
                    let by = ui.label("Port").id;
                    changed |= ui.add(egui::DragValue::new(&mut ws.mcp.port).range(1024..=65535)).labelled_by(by).on_hover_text("Another number when something else uses this one").changed();
                });
            }
        }
        if let Some(e) = &ws.mcp.last_error {
            ui.colored_label(ui.visuals().error_fg_color, e);
        }
        changed |= ui.checkbox(&mut ws.mcp.autostart, "Start it with the editor").on_hover_text("Remembered in the editor's settings").changed();
        ui.weak("It listens on this computer only (127.0.0.1) and asks for no password: any program here that reaches the port can change the open project. Stop it when no agent is at work.");
        if start {
            if let Err(e) = ws.mcp.start(ws.mcp.port) {
                ws.mcp.last_error = Some(e);
            }
        }
        if stop {
            ws.mcp.stop();
        }
        if changed {
            remember(ws);
        }
        ui.add_space(10.0);

        // The set-up for each kind of program.
        ui.strong("2. Tell the agent where the editor is");
        ui.weak("Once per program. Copy the set-up for the one you use:");
        let address = ws.mcp.address();
        for client in Client::ALL {
            let setup = client.setup(&address);
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(client.title()).strong());
                if ui.button("Copy the set-up").on_hover_text(client.place()).widget_info_named(format!("Copy the set-up for {}", client.title())).clicked() {
                    self.copy(ui.ctx(), client.name(), setup.clone());
                }
                if self.copied == Some(client.name()) {
                    ui.weak("copied");
                }
            });
            ui.add(egui::Label::new(RichText::new(&setup).monospace()).wrap().selectable(true));
            ui.weak(client.place());
        }
        ui.add_space(10.0);

        // What to say first.
        ui.strong("3. Ask it");
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(FIRST_ASK).italics());
            if ui.button("Copy").on_hover_text("To paste into the agent's chat as the first thing to ask").widget_info_named("Copy what to ask first").clicked() {
                self.copy(ui.ctx(), "ask", FIRST_ASK.to_string());
            }
            if self.copied == Some("ask") {
                ui.weak("copied");
            }
        });
        ui.weak("Then ask in your own words: \"Add a fisherman to the village who sells trout in the morning\", \"Why can't I finish the quest?\"");
        ui.add_space(10.0);

        // What they did.
        ui.horizontal(|ui| {
            ui.strong(format!("What the agents did ({} calls)", ws.mcp.calls));
            if !ws.mcp.log.is_empty() && ui.small_button("Clear the list").clicked() {
                ws.mcp.log.clear();
            }
        });
        if ws.mcp.log.is_empty() {
            ui.weak(if ws.mcp.running.is_some() { "Nothing yet. Once an agent is connected, every tool it uses is listed here with how it went." } else { "Nothing yet: start the server, connect an agent and ask it something." });
        } else {
            let now = editor_core::now();
            egui::ScrollArea::vertical().id_salt("agent calls").max_height(260.0).stick_to_bottom(true).show(ui, |ui| {
                for e in &ws.mcp.log {
                    ui.horizontal(|ui| {
                        ui.monospace(format!("{:>5.0} s ago", (now - e.at).max(0.0)));
                        ui.colored_label(if e.ok { Color32::from_rgb(140, 210, 140) } else { ui.visuals().error_fg_color }, if e.ok { "ok" } else { "failed" });
                        ui.monospace(format!("{:<20} {:>5} ms", e.tool, e.ms));
                        ui.weak(&e.summary);
                    });
                }
            });
        }
        want
    }
}

/// The server's port and whether it starts with the editor, kept in the editor's settings.
fn remember(ws: &Workspace) {
    editor_core::files::save_settings(&editor_core::files::EditorSettings { mcp_autostart: ws.mcp.autostart, mcp_port: ws.mcp.port, ..editor_core::files::load_settings() });
}

/// A control named for screen readers and tests when several share their text.
trait Named {
    fn widget_info_named(self, name: impl Into<String>) -> Self;
}

impl Named for egui::Response {
    fn widget_info_named(self, name: impl Into<String>) -> Self {
        let name = name.into();
        self.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, name.as_str()));
        self
    }
}

#[cfg(test)]
mod ui_tests {
    use super::*;
    use egui_kittest::kittest::Queryable;
    use egui_kittest::Harness;

    struct State {
        ws: Workspace,
        page: AgentsPage,
        asked: usize,
    }

    #[test]
    fn the_server_is_started_and_its_set_up_copied_for_each_program() {
        // (Nothing here changes the port or *Start it with the editor*: the settings aren't written.)
        let mut ws = Workspace::empty();
        // (Any free port: another editor may hold the usual one.)
        ws.mcp.port = 0;
        let mut h = Harness::builder().with_size(egui::vec2(1000.0, 800.0)).build_ui_state(
            |ui, s: &mut State| {
                if let Some(Want::Help) = s.page.show(ui, &mut s.ws) {
                    s.asked += 1;
                }
            },
            State { ws, page: AgentsPage::default(), asked: 0 },
        );
        h.run();
        // Off: the set-up names the port it would listen on.
        h.get_by_label("It is off.");
        assert!(h.query_all_by_label_contains("claude mcp add --transport http rustpg2d http://127.0.0.1:").next().is_some());
        // Started, it says where it listens, and the set-ups name that address.
        h.state_mut().ws.mcp.start(0).unwrap();
        h.run();
        let url = h.state().ws.mcp.url().unwrap();
        h.get_by_label(format!("Listening on {url}").as_str());
        assert!(h.query_all_by_label_contains(&format!("\"url\": \"{url}\"")).next().is_some(), "Cursor's set-up names the address");
        // Each program's set-up is a click away.
        h.get_by_label("Copy the set-up for Claude Code").click();
        h.run();
        assert_eq!(h.state().page.copied, Some("claude"));
        h.get_by_label("Copy the set-up for Claude Desktop and others").click();
        h.run();
        assert_eq!(h.state().page.copied, Some("desktop"));
        h.get_by_label("Copy what to ask first").click();
        h.run();
        assert_eq!(h.state().page.copied, Some("ask"));
        assert!(h.state().page.status(&h.state().ws).ends_with("autostart=off copied=ask"), "{}", h.state().page.status(&h.state().ws));
        // The manual is a click away; Stop closes the port.
        h.get_by_label("Read about it").click();
        h.run();
        assert_eq!(h.state().asked, 1);
        h.get_by_label("Stop").click();
        h.run();
        assert!(h.state().ws.mcp.running.is_none());
        h.get_by_label("It is off.");
    }

    #[test]
    fn every_program_gets_a_set_up_that_names_the_address() {
        let url = "http://127.0.0.1:7743/mcp";
        assert_eq!(Client::ClaudeCode.setup(url), "claude mcp add --transport http rustpg2d http://127.0.0.1:7743/mcp");
        for client in [Client::Cursor, Client::Desktop] {
            let v: serde_json::Value = serde_json::from_str(&client.setup(url)).expect("the set-up is JSON");
            assert!(v["mcpServers"]["rustpg2d"].is_object());
            assert!(client.setup(url).contains(url));
        }
        assert!(Client::ALL.iter().all(|c| Client::named(c.name()) == Some(*c)));
        // The manual shows the same lines (chapter 13).
        let manual = editor_core::manual::chapters().iter().find(|c| c.file == "13-ai-agents.md").unwrap();
        for client in Client::ALL {
            assert!(manual.text.contains(&client.setup(url)), "chapter 13 lacks the set-up for {}", client.title());
        }
    }
}
