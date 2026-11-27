use clap::{Parser, Subcommand};
use rust_use_core::ComponentId;

/// Drive a running rust-use app.
#[derive(Parser)]
#[command(
    name = "rust-use",
    version,
    after_help = "Examples:
  rust-use snapshot --verbose
  rust-use snapshot --from @101 --limit 50
  rust-use diff
  rust-use act 'board move e2e4; clock press'
  rust-use query Country sw --from 50
  rust-use query --kind select Country
  rust-use raw echo hello
  rust-use list
  rust-use --app chess mcp"
)]
pub(crate) struct Cli {
    /// Select an app by name (defaults to the only running app).
    #[arg(long, env = "RUST_USE_APP", value_name = "NAME")]
    pub(crate) app: Option<String>,
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Subcommand)]
pub(crate) enum Command {
    /// Read the compact screen snapshot.
    Snapshot {
        /// Include action documentation.
        #[arg(long)]
        verbose: bool,
        /// Start at this component id, inclusive.
        #[arg(long, value_name = "@ID")]
        from: Option<ComponentId>,
        /// Maximum number of components (default 100).
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Read changes since the previous snapshot, act, or diff.
    Diff,
    /// Run one or more action steps.
    Act {
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        script: Vec<String>,
    },
    /// Search components or list a component's options.
    #[command(group(clap::ArgGroup::new("component_search").args(["kind", "search_text"])))]
    Query {
        /// Component reference, or a text filter when using --kind.
        #[arg(value_name = "REF", required_unless_present_any = ["kind", "search_text"])]
        target: Option<String>,
        #[arg(conflicts_with = "kind")]
        text: Option<String>,
        /// Filter components by exact kind.
        #[arg(long)]
        kind: Option<String>,
        /// Search component labels, kinds, and values.
        #[arg(long = "text", conflicts_with = "target")]
        search_text: Option<String>,
        /// Start at this component id for search or offset for option queries.
        #[arg(long, value_name = "@ID|OFFSET")]
        from: Option<String>,
        /// Maximum number of matching components (default 100).
        #[arg(long, requires = "component_search")]
        limit: Option<usize>,
    },
    /// Send an arbitrary text protocol request.
    Raw {
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        payload: Vec<String>,
    },
    /// List live app endpoints.
    List,
    /// Serve the app's three MCP tools over stdio.
    Mcp,
}
