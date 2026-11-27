use crate::{
    cli::{Cli, Command},
    state::{Cursor, parse_hello},
};
use rust_use_agent::{
    ipc::{Client, Endpoint, IpcError, list_endpoints},
    mcp::{McpServer, query_request, search_request, snapshot_request},
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
    let is_diff = matches!(cli.command, Command::Diff);
    let cursor = || Cursor::open(client.endpoint()).map(Some);
    let (mut request, cursor) = match cli.command {
        Command::Mcp => {
            McpServer::new(Mutex::new(client)).serve_stdio()?;
            return Ok(ExitCode::SUCCESS);
        }
        Command::Snapshot {
            verbose,
            from,
            limit,
        } => (snapshot_request(verbose, from, limit), cursor()?),
        Command::Act { script } => (format!("act {}", script.join(" ")), cursor()?),
        Command::Diff => ("diff".into(), cursor()?),
        Command::Query {
            target,
            text,
            kind,
            search_text,
            from,
            limit,
        } => {
            let request = if kind.is_some() || search_text.is_some() {
                search_request(
                    kind.as_deref(),
                    search_text.or(target).as_deref(),
                    from.as_deref().map(str::parse).transpose()?,
                    limit,
                )
            } else {
                query_request(
                    target.as_deref().unwrap_or_default(),
                    text.as_deref(),
                    from.as_deref().map(str::parse).transpose()?,
                )
            };
            (request, None)
        }
        Command::Raw { payload } => (payload.join(" "), None),
        Command::List => return Ok(ExitCode::SUCCESS),
    };
    let mut send = |payload: &str| {
        client
            .request(payload)
            .map_err(|e| format!("{e}; check the app is running with ipc enabled"))
    };
    let mut restarted = false;
    let mut state = None;
    if let Some(cursor) = cursor {
        let (session, revision) = parse_hello(&send("hello")?)?;
        if is_diff {
            let since = match cursor.read()? {
                Some(saved) if saved.session == Some(session) && saved.revision <= revision => {
                    saved.revision
                }
                Some(_) => {
                    restarted = true;
                    0
                }
                None => 0,
            };
            request = format!("diff {since}");
        }
        state = Some((cursor, session));
    }
    let reply = send(&request)?;
    let mut stdout = io::stdout().lock();
    if restarted {
        writeln!(stdout, "# app restarted; showing everything")?;
    }
    stdout.write_all(reply.as_bytes())?;
    if let Some((cursor, session)) = state {
        cursor.observe(session, &reply)?;
    }
    Ok(if reply.starts_with("err") {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}
