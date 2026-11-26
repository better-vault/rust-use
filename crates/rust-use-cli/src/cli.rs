use clap::{Parser, Subcommand};

/// Drive a running rust-use app.
#[derive(Parser)]
#[command(
    name = "rust-use",
    version,
    after_help = "Examples:
  rust-use snapshot --verbose
  rust-use diff
  rust-use act 'board move e2e4; clock press'
  rust-use query Country sw --from 50
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
    },
    /// Read changes since the previous snapshot, act, or diff.
    Diff,
    /// Run one or more action steps.
    Act {
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        script: Vec<String>,
    },
    /// Search a component's options.
    Query {
        #[arg(value_name = "REF")]
        target: String,
        text: Option<String>,
        #[arg(long, value_name = "N")]
        from: Option<usize>,
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
