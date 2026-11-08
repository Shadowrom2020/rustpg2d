//! The MCP server proper: `rmcp`'s Streamable HTTP service on an axum listener, run on the
//! editor's agent thread. The [`Handler`] owns the tool catalogue (`tools.rs`), the resources
//! (documentation) and the prompts; it forwards every tool call to the main thread and turns the
//! reply into MCP content.

use std::future::IntoFuture;
use std::sync::mpsc;
use std::time::Instant;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::model::*;
use rmcp::service::RequestContext;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::{StreamableHttpServerConfig, StreamableHttpService};
use rmcp::{tool_handler, ErrorData, RoleServer, ServerHandler};

use super::exec::{Call, Reply};
use super::AgentRequest;

/// What agents read first (`initialize` → `instructions`).
const INSTRUCTIONS: &str = "You are connected to the R(ust)PG2D editor: a 2D top-down RPG engine. The project open in the editor is yours to read and build. \
Start with `project_info`. Tiles are 32 px and every coordinate you give or get is in tiles (x right, y down, origin top-left; an entity's position is the tile its feet stand on). \
Write tools save the project at once and each one is a single undo step in the editor (the person can press Ctrl+Z), so make deliberate, complete edits and read back with `get_map` afterwards. \
`paint_grid` takes the same text grids `get_map` prints. Ids are strict (unknown ids are errors with suggestions); `events_reference` lists the JSON shapes of triggers, conditions and actions; \
`def_template` gives a complete definition to fill in. `play_test` runs the game with an autoplay script and returns its status lines and screenshots; `screenshot_map` shows a map region. \
Read the resources `rustpg2d://docs/*` for the scripting API, the event system and the autoplay commands.";

/// The per-session handler (the service makes one for each client).
pub struct Handler {
    tx: mpsc::Sender<AgentRequest>,
    tool_router: ToolRouter<Handler>,
}

impl Handler {
    pub fn new(tx: mpsc::Sender<AgentRequest>) -> Self {
        Handler { tx, tool_router: Self::tool_router() }
    }

    /// Sends a call to the editor's main thread and waits for its answer.
    pub(crate) async fn run(&self, call: Call, summary: String) -> Result<CallToolResult, ErrorData> {
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        self.tx
            .send(AgentRequest { call, summary, reply: reply_tx, started: Instant::now() })
            .map_err(|_| ErrorData::internal_error("the editor stopped the agent server", None))?;
        match reply_rx.await {
            Err(_) => Err(ErrorData::internal_error("the editor dropped the request (server stopped or project closed)", None)),
            Ok(Err(msg)) => Ok(CallToolResult::error(vec![ContentBlock::text(msg)])),
            Ok(Ok(Reply::PlayPlan(plan))) => Ok(to_result(super::play::run(*plan).await)),
            Ok(Ok(reply)) => Ok(to_result(reply)),
        }
    }
}

/// A reply as MCP content: text first, then any PNG screenshots.
pub fn to_result(reply: Reply) -> CallToolResult {
    match reply {
        Reply::Text(t) => CallToolResult::success(vec![ContentBlock::text(t)]),
        Reply::TextImages(t, images) => {
            let mut v = vec![ContentBlock::text(t)];
            v.extend(images.iter().map(|png| ContentBlock::image(rustpg2d_engine::hash::base64(png), "image/png")));
            CallToolResult::success(v)
        }
        Reply::PlayPlan(_) | Reply::Shot(_) => CallToolResult::error(vec![ContentBlock::text("internal: a deferred reply reached the client unresolved")]),
    }
}

/// The documentation served as MCP resources.
pub const DOCS: &[(&str, &str, &str, &str)] = &[
    ("rustpg2d://docs/mcp", "mcp", "How to work with this server: tools, conventions, the file watcher", include_str!("../../../../Documentation/mcp.md")),
    ("rustpg2d://docs/scripting", "scripting", "The Rhai scripting API of games (functions, events, examples)", include_str!("../../../../Documentation/scripting.md")),
    ("rustpg2d://docs/events", "events", "Triggers, conditions and actions: owners, events and the JSON shapes tools take", include_str!("../../../../Documentation/events.md")),
    ("rustpg2d://docs/autoplay", "autoplay", "The autoplay commands play_test accepts and the status fields it prints", include_str!("../../../../Documentation/autoplay.md")),
];
const PROJECT_URI: &str = "rustpg2d://project";

#[tool_handler(router = self.tool_router)]
impl ServerHandler for Handler {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().enable_resources().enable_prompts().build())
            .with_server_info(Implementation::new("rustpg2d-editor", env!("CARGO_PKG_VERSION")))
            .with_instructions(INSTRUCTIONS)
    }

    async fn list_resources(&self, _request: Option<PaginatedRequestParams>, _ctx: RequestContext<RoleServer>) -> Result<ListResourcesResult, ErrorData> {
        let mut items: Vec<Resource> = DOCS.iter().map(|(uri, name, desc, _)| Resource::new(*uri, *name).with_description(*desc).with_mime_type("text/markdown")).collect();
        items.push(Resource::new(PROJECT_URI, "project").with_description("The open project: maps, definitions, scripts, settings (same as the project_info tool)").with_mime_type("text/plain"));
        Ok(ListResourcesResult::with_all_items(items))
    }

    async fn read_resource(&self, request: ReadResourceRequestParams, _ctx: RequestContext<RoleServer>) -> Result<ReadResourceResponse, ErrorData> {
        if let Some((uri, _, _, text)) = DOCS.iter().find(|(uri, ..)| *uri == request.uri) {
            return Ok(ReadResourceResult::new(vec![ResourceContents::text(*text, *uri).with_mime_type("text/markdown")]).into());
        }
        if request.uri == PROJECT_URI {
            let text = match self.run(Call::ProjectInfo, String::new()).await? {
                r if r.is_error.unwrap_or(false) => return Err(ErrorData::internal_error(first_text(&r), None)),
                r => first_text(&r),
            };
            return Ok(ReadResourceResult::new(vec![ResourceContents::text(text, PROJECT_URI)]).into());
        }
        Err(ErrorData::resource_not_found(format!("no resource {}", request.uri), None))
    }

    async fn list_prompts(&self, _request: Option<PaginatedRequestParams>, _ctx: RequestContext<RoleServer>) -> Result<ListPromptsResult, ErrorData> {
        Ok(ListPromptsResult::with_all_items(super::tools::prompts()))
    }

    async fn get_prompt(&self, request: GetPromptRequestParams, _ctx: RequestContext<RoleServer>) -> Result<GetPromptResponse, ErrorData> {
        let args = request.arguments.unwrap_or_default();
        let arg = |k: &str| args.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
        let text = super::tools::prompt_text(&request.name, &arg).ok_or_else(|| ErrorData::invalid_params(format!("no prompt {}", request.name), None))?;
        Ok(GetPromptResult::new(vec![PromptMessage::new_text(Role::User, text)]).into())
    }
}

fn first_text(r: &CallToolResult) -> String {
    r.content.iter().find_map(|c| c.as_text().map(|t| t.text.clone())).unwrap_or_default()
}

/// The agent thread: serves until `stop` fires, then drops every connection.
pub fn run(listener: std::net::TcpListener, stop: tokio::sync::oneshot::Receiver<()>, tx: mpsc::Sender<AgentRequest>) {
    let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("[mcp] cannot start the server runtime: {e}");
            return;
        }
    };
    rt.block_on(async move {
        let listener = match tokio::net::TcpListener::from_std(listener) {
            Ok(l) => l,
            Err(e) => {
                eprintln!("[mcp] listener: {e}");
                return;
            }
        };
        let config = StreamableHttpServerConfig::default();
        let ct = config.cancellation_token.clone();
        let service = StreamableHttpService::new(move || Ok(Handler::new(tx.clone())), LocalSessionManager::default().into(), config);
        let router = axum::Router::new().nest_service("/mcp", service);
        let serve = axum::serve(listener, router).into_future();
        tokio::select! {
            r = serve => {
                if let Err(e) = r {
                    eprintln!("[mcp] server stopped: {e}");
                }
            }
            _ = stop => {}
        }
        ct.cancel();
    });
    // Connections still open (SSE streams) end with the runtime.
    rt.shutdown_timeout(std::time::Duration::from_secs(1));
}
