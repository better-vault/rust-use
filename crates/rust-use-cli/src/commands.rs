use crate::{
    cli::{Cli, Command},
    state::Cursor,
};
use rust_use_agent::{
    ipc::{Client, Endpoint, IpcError, list_endpoints},
    mcp::{McpServer, query_request},
};
use std::{
    error::Error,
    io::{self, Write},
    process::ExitCode,
    sync::Mutex,
};

fn connect(app: Option<&str>) -> Result<Client, String> {
    let result = match app {
        Some(name) => Endpoint::new(name).and_then(|e| Client::connect(&e)),
        None => Client::connect_default(),
    };
    result.map_err(|e| match e {
        IpcError::Ambiguous(names) => format!(
            "multiple apps running ({}); select one with --app",
            names.join(", ")
        ),
        IpcError::InvalidName(_) => format!("{e}; use a name matching [a-z0-9-] of length 1 to 40"),
        _ => format!("{e}; start the app with the ipc feature enabled and check --app"),
    })
}

pub(crate) fn run(cli: Cli) -> Result<ExitCode, Box<dyn Error>> {
    if matches!(cli.command, Command::List) {
        for endpoint in list_endpoints() {
            println!("{}", endpoint.name());
        }
        return Ok(ExitCode::SUCCESS);
    }
    let mut client = connect(cli.app.as_deref())?;
    let cursor = || Cursor::open(client.endpoint()).map(Some);
    let (request, cursor) = match cli.command {
        Command::Mcp => {
            McpServer::new(Mutex::new(client)).serve_stdio()?;
            return Ok(ExitCode::SUCCESS);
        }
        Command::Snapshot { verbose } => (
            if verbose {
                "snapshot --verbose"
            } else {
                "snapshot"
            }
            .into(),
            cursor()?,
        ),
        Command::Act { script } => (format!("act {}", script.join(" ")), cursor()?),
        Command::Diff => {
            let mut state = Cursor::open(client.endpoint())?;
            (format!("diff {}", state.revision()?), Some(state))
        }
        Command::Query { target, text, from } => {
            (query_request(&target, text.as_deref(), from), None)
        }
        Command::Raw { payload } => (payload.join(" "), None),
        Command::List => return Ok(ExitCode::SUCCESS),
    };
    let reply = client
        .request(&request)
        .map_err(|e| format!("{e}; check the app is running with ipc enabled"))?;
    io::stdout().lock().write_all(reply.as_bytes())?;
    if let Some(mut cursor) = cursor {
        cursor.observe(&reply)?;
    }
    Ok(if reply.starts_with("err") {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}
