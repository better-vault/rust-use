//! Stdio MCP server for generic accessibility automation.
use clap::Parser;
use rmcp::{
    ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerConfig},
    service::QuitReason,
    tool, tool_handler, tool_router,
};
use rust_use_agent::ipc::{Client, Endpoint, IpcError};
use schemars::JsonSchema;
use serde::Deserialize;
use std::sync::Mutex;

#[derive(Parser)]
#[command(about = "Serve UI automation tools over stdio.")]
struct Args {
    /// Name of the running app.
    #[arg(long, default_value = "chess")]
    app: String,
}

trait UiEndpoint: Send + Sync + 'static {
    fn request(&self, command: &str) -> Result<String, AppError>;
}

struct AppEndpoint {
    app: String,
    client: Mutex<Option<Client>>,
}

#[derive(Debug, thiserror::Error)]
enum AppError {
    #[error("Could not connect to app {app:?}: {source}")]
    Connect { app: String, source: IpcError },
    #[error("IPC request to app {app:?} failed: {source}")]
    Request { app: String, source: IpcError },
}

impl UiEndpoint for AppEndpoint {
    fn request(&self, command: &str) -> Result<String, AppError> {
        let mut connection = self
            .client
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut client = match connection.take() {
            Some(client) => client,
            None => Endpoint::new(&self.app)
                .and_then(|endpoint| Client::connect(&endpoint))
                .map_err(|source| AppError::Connect {
                    app: self.app.clone(),
                    source,
                })?,
        };
        let reply = client
            .request(command)
            .map_err(|source| AppError::Request {
                app: self.app.clone(),
                source,
            })?;
        *connection = Some(client);
        Ok(reply)
    }
}

#[derive(Deserialize, JsonSchema)]
struct Tree {}

#[derive(Deserialize, JsonSchema)]
struct Click {
    element_id: String,
}

#[derive(Deserialize, JsonSchema)]
struct TypeText {
    text: String,
}

struct AutomationServer<E: UiEndpoint> {
    endpoint: E,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl<E: UiEndpoint> AutomationServer<E> {
    fn new(endpoint: E) -> Self {
        Self {
            endpoint,
            tool_router: Self::tool_router(),
        }
    }

    fn reply(&self, command: &str) -> CallToolResult {
        match self.endpoint.request(command) {
            Ok(reply) => CallToolResult::success(vec![ContentBlock::text(reply)]),
            Err(error) => CallToolResult::error(vec![ContentBlock::text(error.to_string())]),
        }
    }

    #[tool(description = "Read the current UI accessibility tree.")]
    fn get_ui_tree(&self, Parameters(_): Parameters<Tree>) -> CallToolResult {
        self.reply("a11y tree")
    }

    #[tool(description = "Click an element by its ID.")]
    fn click(&self, Parameters(input): Parameters<Click>) -> CallToolResult {
        self.reply(&format!("a11y click {}", input.element_id))
    }

    #[tool(description = "Type text into the focused element.")]
    fn type_text(&self, Parameters(input): Parameters<TypeText>) -> CallToolResult {
        self.reply(&format!("a11y type {}", input.text))
    }
}

#[allow(
    clippy::unused_async_trait_impl,
    reason = "rmcp generates the async tool listing implementation"
)]
#[tool_handler(router = self.tool_router)]
impl<E: UiEndpoint> ServerHandler for AutomationServer<E> {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                "ui-automation",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(
                "Inspect the UI with get_ui_tree, then interact with click and type_text.",
            )
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let endpoint = AppEndpoint {
        app: args.app,
        client: Mutex::default(),
    };
    let server = AutomationServer::new(endpoint)
        .serve(rmcp::transport::stdio())
        .await?;
    if let QuitReason::JoinError(error) = server.waiting().await? {
        return Err(error.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::model::CallToolRequestParams;
    use serde_json::json;

    impl<F: Fn(&str) -> String + Send + Sync + 'static> UiEndpoint for F {
        fn request(&self, command: &str) -> Result<String, AppError> {
            Ok(self(command))
        }
    }

    #[tokio::test]
    async fn exposes_accessibility_tools_over_mcp() -> Result<(), Box<dyn std::error::Error>> {
        let stub = |command: &str| {
            match command {
                "a11y tree" => r#"{"role":"window","children":[{"id":"n2","role":"button"}]}"#,
                "a11y click n2" => r#"{"ok":true}"#,
                "a11y type  héllo\nworld " => r#"{"ok":false,"error":"no focused text field"}"#,
                _ => "unexpected command",
            }
            .to_owned()
        };
        let (a, b) = tokio::io::duplex(8192);
        let (server, client) = tokio::join!(AutomationServer::new(stub).serve(a), ().serve(b));
        let server = server?;
        let client = client?;

        let info = client.peer_info().ok_or("missing server info")?;
        assert_eq!(
            info.server_info.as_ref().map(|info| info.name.as_str()),
            Some("ui-automation")
        );
        assert_eq!(
            info.instructions.as_deref(),
            Some("Inspect the UI with get_ui_tree, then interact with click and type_text.")
        );

        let mut tools = client.list_all_tools().await?;
        tools.sort_unstable_by(|a, b| a.name.cmp(&b.name));
        let names: Vec<_> = tools.iter().map(|tool| tool.name.as_ref()).collect();
        assert_eq!(names, ["click", "get_ui_tree", "type_text"]);
        assert!(
            tools
                .iter()
                .map(|tool| tool.description.as_deref().unwrap_or_default().len())
                .sum::<usize>()
                <= 600
        );

        for (tool, args, command) in [
            ("get_ui_tree", json!({}), "a11y tree"),
            ("click", json!({"element_id":"n2"}), "a11y click n2"),
            (
                "type_text",
                json!({"text":" héllo\nworld "}),
                "a11y type  héllo\nworld ",
            ),
        ] {
            let reply = client
                .call_tool(
                    CallToolRequestParams::new(tool).with_arguments(serde_json::from_value(args)?),
                )
                .await?;
            assert!(!reply.is_error.unwrap_or(false));
            assert_eq!(reply.content.len(), 1);
            let expected = stub(command);
            assert_eq!(
                reply.content[0].as_text().map(|text| text.text.as_str()),
                Some(expected.as_str())
            );
        }
        client.cancel().await?;
        server.waiting().await?;
        Ok(())
    }

    #[tokio::test]
    async fn reports_connection_errors_as_tool_results() -> Result<(), Box<dyn std::error::Error>> {
        let endpoint = AppEndpoint {
            app: "invalid app".to_owned(),
            client: Mutex::default(),
        };
        let (a, b) = tokio::io::duplex(8192);
        let (server, client) = tokio::join!(AutomationServer::new(endpoint).serve(a), ().serve(b));
        let server = server?;
        let client = client?;

        let reply = client
            .call_tool(CallToolRequestParams::new("get_ui_tree"))
            .await?;
        assert_eq!(reply.is_error, Some(true));
        assert_eq!(reply.content.len(), 1);
        assert_eq!(
            reply.content[0].as_text().map(|text| text.text.as_str()),
            Some(
                "Could not connect to app \"invalid app\": invalid endpoint name: \"invalid app\""
            )
        );
        client.cancel().await?;
        server.waiting().await?;
        Ok(())
    }
}
