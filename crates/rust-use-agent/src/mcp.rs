//! Three compact text tools for MCP clients.
use rmcp::{
    ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerConfig},
    service::QuitReason,
    tool, tool_handler, tool_router,
};
use rust_use_core::ComponentId;
use schemars::JsonSchema;
use serde::Deserialize;
use std::fmt::Write;

/// Anything that answers the compact text protocol.
pub trait Endpoint: Send + Sync + 'static {
    /// Execute one request and return its text reply.
    fn handle(&self, request: &str) -> String;
}

impl Endpoint for crate::Agent {
    fn handle(&self, request: &str) -> String {
        self.handle(request)
    }
}

#[cfg(feature = "ipc")]
impl Endpoint for std::sync::Mutex<crate::ipc::Client> {
    fn handle(&self, request: &str) -> String {
        self.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .request(request)
            .unwrap_or_else(|e| format!("err {e}"))
    }
}

/// Encode query arguments for a text endpoint, preserving literal labels and filters.
pub fn query_request(target: &str, text: Option<&str>, from: Option<usize>) -> String {
    let mut request = String::from("query ");
    match rust_use_core::Ref::from(target) {
        rust_use_core::Ref::Id(id) => {
            let _ = write!(request, "{id}");
        }
        rust_use_core::Ref::Label(label) => {
            let _ = crate::format::string(&mut request, &label, true);
        }
    }
    if let Some(text) = text {
        request.push(' ');
        let _ = crate::format::string(&mut request, text, true);
    }
    if let Some(from) = from {
        let _ = write!(request, " from={from}");
    }
    request
}

/// Encode a snapshot request with optional paging arguments.
pub fn snapshot_request(verbose: bool, from: Option<ComponentId>, limit: Option<usize>) -> String {
    let mut request = String::from("snapshot");
    if verbose {
        request.push_str(" --verbose");
    }
    append_paging(&mut request, from, limit);
    request
}

/// Encode a component search, preserving literal kinds and filters.
pub fn search_request(
    kind: Option<&str>,
    text: Option<&str>,
    from: Option<ComponentId>,
    limit: Option<usize>,
) -> String {
    let mut request = String::from("query ");
    if let Some(kind) = kind {
        request.push_str("--kind ");
        let _ = crate::format::string(&mut request, kind, true);
        if let Some(text) = text {
            request.push(' ');
            let _ = crate::format::string(&mut request, text, true);
        }
    } else {
        request.push_str("--text ");
        let _ = crate::format::string(&mut request, text.unwrap_or_default(), true);
    }
    append_paging(&mut request, from, limit);
    request
}

fn append_paging(request: &mut String, from: Option<ComponentId>, limit: Option<usize>) {
    if let Some(from) = from {
        let _ = write!(request, " --from {from}");
    }
    if let Some(limit) = limit {
        let _ = write!(request, " --limit {limit}");
    }
}

#[derive(Deserialize, JsonSchema)]
struct Snapshot {
    verbose: Option<bool>,
    from: Option<String>,
    limit: Option<usize>,
}
#[derive(Deserialize, JsonSchema)]
struct Act {
    script: String,
}
#[derive(Deserialize, JsonSchema)]
struct Query {
    r#ref: String,
    text: Option<String>,
    from: Option<usize>,
}

/// An MCP server exposing snapshot, act, and query.
pub struct McpServer<E: Endpoint> {
    endpoint: E,
    tool_router: ToolRouter<Self>,
}

/// Failure to start or run the stdio service.
#[derive(Debug, thiserror::Error)]
pub enum McpError {
    /// Could not create the runtime or worker.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// Service initialization failed.
    #[error("MCP initialization failed: {0}")]
    Initialize(String),
    /// Service task failed.
    #[error(transparent)]
    Join(#[from] tokio::task::JoinError),
    /// The runtime thread stopped unexpectedly.
    #[error("MCP runtime thread panicked")]
    Thread,
}

#[tool_router]
impl<E: Endpoint> McpServer<E> {
    /// Wrap an endpoint in an MCP server.
    pub fn new(endpoint: E) -> Self {
        Self {
            endpoint,
            tool_router: Self::tool_router(),
        }
    }

    /// Serve over stdio until disconnect, using a dedicated thread if inside Tokio.
    ///
    /// # Errors
    /// Returns runtime, initialization, or service errors.
    pub fn serve_stdio(self) -> Result<(), McpError> {
        let run = move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?
                .block_on(async {
                    let server = self
                        .serve(rmcp::transport::stdio())
                        .await
                        .map_err(|e| McpError::Initialize(e.to_string()))?;
                    if let QuitReason::JoinError(e) = server.waiting().await? {
                        return Err(e.into());
                    }
                    Ok(())
                })
        };
        if tokio::runtime::Handle::try_current().is_ok() {
            std::thread::Builder::new()
                .spawn(run)?
                .join()
                .map_err(|_| McpError::Thread)?
        } else {
            run()
        }
    }

    fn reply(&self, request: &str) -> CallToolResult {
        let reply = self.endpoint.handle(request);
        let error = reply.starts_with("err");
        let content = vec![ContentBlock::text(reply)];
        if error {
            CallToolResult::error(content)
        } else {
            CallToolResult::success(content)
        }
    }

    #[tool(
        description = "Read up to limit screen components (default 100), starting at from (@id, inclusive). Set verbose for action docs. Follow the more line to page."
    )]
    fn snapshot(&self, Parameters(input): Parameters<Snapshot>) -> CallToolResult {
        let Ok(from) = input.from.as_deref().map(str::parse).transpose() else {
            return CallToolResult::error(vec![ContentBlock::text(
                "err expected from to be a component id such as @101",
            )]);
        };
        self.reply(&snapshot_request(
            input.verbose.unwrap_or(false),
            from,
            input.limit,
        ))
    }

    #[tool(
        description = "Run <ref> <action> [args] steps separated by ';'. Ref is @id, label or kind. Returns changes."
    )]
    fn act(&self, Parameters(input): Parameters<Act>) -> CallToolResult {
        self.reply(&format!("act {}", input.script))
    }

    #[tool(
        description = "List options for a ref with an optional text filter and starting offset."
    )]
    fn query(&self, Parameters(input): Parameters<Query>) -> CallToolResult {
        self.reply(&query_request(
            &input.r#ref,
            input.text.as_deref(),
            input.from,
        ))
    }
}

#[allow(
    clippy::unused_async_trait_impl,
    reason = "rmcp generates the async tool listing implementation"
)]
#[tool_handler(router = self.tool_router)]
impl<E: Endpoint> ServerHandler for McpServer<E> {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("rust-use", env!("CARGO_PKG_VERSION")))
            .with_instructions(
                "Use snapshot to discover components, following more for further pages. Use act to receive changes.",
            )
    }
}
