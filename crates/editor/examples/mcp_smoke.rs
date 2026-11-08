//! Smoke test of the editor's AI agent server: an MCP client over Streamable HTTP that lists the
//! tools, reads and edits the example project and takes screenshots. Start the editor on a copy of
//! `example_project` with its server on, then run this:
//!
//! ```sh
//! RUSTPG2D_EDITOR_SCRIPT="mcp on 7743; wait 300; quit" target/debug/rustpg2d-editor <copy of example_project> &
//! cargo run -p rustpg2d-editor --example mcp_smoke -- 7743
//! ```
//!
//! Prints one PASS / FAIL line per check and exits with 1 when something failed.

use rmcp::model::*;
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::ServiceExt;
use serde_json::json;

fn text(r: &CallToolResult) -> String {
    r.content.iter().filter_map(|c| c.as_text()).map(|t| t.text.clone()).collect::<Vec<_>>().join("\n")
}

fn images(r: &CallToolResult) -> usize {
    r.content.iter().filter(|c| matches!(c, ContentBlock::Image(_))).count()
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let port = std::env::args().nth(1).unwrap_or_else(|| "7743".into());
    let transport = StreamableHttpClientTransport::from_uri(format!("http://127.0.0.1:{port}/mcp"));
    let client = ().serve(transport).await?;
    let mut failed = 0;
    let mut check = |name: &str, ok: bool, detail: String| {
        println!("{} {name}: {}", if ok { "PASS" } else { "FAIL" }, detail.lines().next().unwrap_or("").chars().take(140).collect::<String>());
        if !ok {
            failed += 1;
        }
    };
    let call = |name: &'static str, args: serde_json::Value| {
        let mut p = CallToolRequestParams::new(name);
        if let Some(o) = args.as_object() {
            p = p.with_arguments(o.clone());
        }
        client.call_tool(p)
    };

    let tools = client.list_all_tools().await?;
    check("tools/list", tools.len() >= 50, format!("{} tools", tools.len()));
    let r = call("project_info", json!({})).await?;
    check("project_info", text(&r).starts_with("Project ") && !r.is_error.unwrap_or(false), text(&r));
    let r = call("get_map", json!({"map": "village", "x": 0, "y": 0, "w": 20, "h": 10, "layers": [1]})).await?;
    check("get_map", text(&r).contains("layer 1 \"Detail\""), text(&r));
    let r = call("set_tiles", json!({"map": "village", "layer": 1, "tileset": "terrain", "cells": [[1, 1, 17]]})).await?;
    check("set_tiles", text(&r).starts_with("Set 1 of 1"), text(&r));
    let r = call("get_map", json!({"map": "village", "x": 0, "y": 0, "w": 4, "h": 3, "layers": [1], "collision": false, "objects": false})).await?;
    check("get_map after set_tiles", text(&r).contains("terrain#17"), text(&r));
    let r = call("undo", json!({})).await?;
    check("undo", text(&r).starts_with("Undid 1"), text(&r));
    let r = call("save", json!({})).await?;
    check("save", text(&r).starts_with("Saved: map"), text(&r));
    let r = call("view_image", json!({"kind": "tileset", "id": "terrain"})).await?;
    check("view_image", images(&r) == 1, text(&r));
    let r = call("screenshot_map", json!({"map": "village", "x": 10, "y": 8, "w": 20, "h": 12})).await?;
    check("screenshot_map", images(&r) == 1, text(&r));
    let r = call("get_map", json!({"map": "nowhere"})).await?;
    check("unknown map is a tool error", r.is_error.unwrap_or(false) && text(&r).contains("Known:"), text(&r));
    let res = client.read_resource(ReadResourceRequestParams::new("rustpg2d://docs/scripting")).await?;
    let doc = res.contents.iter().filter_map(|c| if let ResourceContents::TextResourceContents { text, .. } = c { Some(text.len()) } else { None }).sum::<usize>();
    check("resource docs/scripting", doc > 1000, format!("{doc} chars"));
    let mut gp = GetPromptRequestParams::new("design_quest");
    gp.arguments = json!({"theme": "a lost cat"}).as_object().cloned();
    let prompt = client.get_prompt(gp).await?;
    check("prompt design_quest", prompt.messages.len() == 1, format!("{} messages", prompt.messages.len()));
    let r = call("play_test", json!({"map": "village", "script": "wait 1; status; shot a", "timeout_secs": 30})).await?;
    check("play_test", text(&r).contains("[autoplay]") && images(&r) == 1, text(&r));
    client.cancel().await?;
    if failed > 0 {
        println!("{failed} check(s) failed");
        std::process::exit(1);
    }
    println!("all checks passed");
    Ok(())
}
