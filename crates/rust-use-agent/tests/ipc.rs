//! Local socket transport integration tests.
#![cfg(feature = "ipc")]

use rust_use_agent::{
    UiAgentExt,
    ipc::{Client, Endpoint, IpcError, Server},
};
use rust_use_core::{ActionError, ActionSpec, Args, Behavior, Describe, Primitive, Ui, Value};
use std::{
    io::{Read, Write},
    time::{Duration, Instant},
};

struct Counter(u32);
impl Primitive for Counter {
    const NAME: &'static str = "counter";
}
impl Behavior for Counter {
    fn describe(&self, d: &mut Describe) {
        d.value(self.0);
    }
    fn actions(&self) -> Vec<ActionSpec> {
        vec![ActionSpec {
            name: "inc",
            doc: "Increment.",
            params: &[],
        }]
    }
    fn invoke(&mut self, action: &str, _: &Args) -> Option<Result<Value, ActionError>> {
        if action != "inc" {
            return None;
        }
        self.0 += 1;
        Some(Ok(self.0.into()))
    }
}

struct Title;
impl Primitive for Title {
    const NAME: &'static str = "title";
}
impl Behavior for Title {
    fn describe(&self, d: &mut Describe) {
        d.value("Démo");
    }
}

fn screen() -> Ui {
    let ui = Ui::new();
    ui.mount(Counter(0));
    ui.mount(Title);
    ui
}

fn endpoint(test: &str) -> Result<Endpoint, IpcError> {
    Endpoint::new(&format!("ipc-{}-{test}", std::process::id()))
}

#[test]
fn round_trip_and_independent_sessions() -> Result<(), IpcError> {
    let guard = Server::new(&screen())
        .endpoint(endpoint("sessions")?)
        .spawn()?;
    let mut first = Client::connect(guard.endpoint())?;
    let mut second = Client::connect(guard.endpoint())?;
    let reference = screen();
    let a = reference.agent();
    let b = reference.agent();
    for command in ["snapshot", "snapshot --verbose", "query title", "unknown"] {
        assert_eq!(first.request(command)?, a.handle(command));
    }
    assert_eq!(second.request("snapshot")?, b.handle("snapshot"));
    for command in ["act counter inc", "diff"] {
        assert_eq!(first.request(command)?, a.handle(command));
    }
    assert_eq!(second.request("diff")?, b.handle("diff"));
    Ok(())
}

#[test]
fn policy_commands_observation() -> Result<(), Box<dyn std::error::Error>> {
    let ui = screen();
    let (tx, rx) = std::sync::mpsc::channel();
    let guard = Server::new(&ui)
        .endpoint(endpoint("custom")?)
        .command("echo", str::to_owned)
        .allow(|_| false)
        .observe(move |r| {
            let _ = tx.send((r.request.to_owned(), r.reply.to_owned(), r.connection));
        })
        .spawn()?;
    let mut client = Client::connect(guard.endpoint())?;
    assert_eq!(client.request("echo two\nlines")?, "two\nlines");
    let denial = client.request("act counter inc")?;
    assert!(denial.contains("denied"), "{denial}");
    let first = rx.recv_timeout(Duration::from_secs(1))?;
    let second = rx.recv_timeout(Duration::from_secs(1))?;
    assert_eq!(
        first,
        ("echo two\nlines".into(), "two\nlines".into(), second.2)
    );
    assert_eq!(second.1, denial);
    drop(guard);
    assert!(client.request("act counter inc").is_err());
    assert!(ui.agent().handle("act counter inc").starts_with("ok"));
    Ok(())
}

#[cfg(unix)]
fn path(endpoint: &Endpoint) -> std::path::PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .filter(|p| !p.is_empty())
        .map_or_else(
            || {
                std::env::temp_dir()
                    .join(format!("rust-use-{}", rustix::process::geteuid().as_raw()))
            },
            |p| std::path::PathBuf::from(p).join("rust-use"),
        )
        .join(format!("{}.sock", endpoint.name()))
}

#[test]
fn oversized_frame() -> Result<(), IpcError> {
    use interprocess::local_socket::{Stream, prelude::*};
    let guard = Server::new(&screen())
        .endpoint(endpoint("oversize")?)
        .spawn()?;
    #[cfg(unix)]
    let address =
        path(guard.endpoint()).to_fs_name::<interprocess::local_socket::GenericFilePath>()?;
    #[cfg(windows)]
    let address = format!("rust-use-{}", guard.endpoint().name())
        .to_ns_name::<interprocess::local_socket::GenericNamespaced>()?;
    let mut raw = Stream::connect(address)?;
    raw.write_all(&((16_u32 << 20) + 1).to_le_bytes())?;
    let mut header = [0; 4];
    raw.read_exact(&mut header)?;
    let len = u32::from_le_bytes(header) as usize;
    assert!(len < 100);
    let mut reply = vec![0; len];
    raw.read_exact(&mut reply)?;
    assert!(reply.starts_with(b"err frame too large"));
    assert_eq!(raw.read(&mut header)?, 0);
    assert!(
        Client::connect(guard.endpoint())?
            .request("snapshot")?
            .starts_with("rev")
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn cleanup_and_stale_recovery() -> Result<(), IpcError> {
    let ui = screen();
    let name = endpoint("cleanup")?;
    let start = || Server::new(&ui).endpoint(name.clone()).spawn();
    let guard = start()?;
    assert!(start().is_err());
    assert!(rust_use_agent::ipc::list_endpoints().contains(&name));
    drop(guard);
    assert!(!path(&name).exists());
    drop(std::os::unix::net::UnixListener::bind(path(&name))?);
    let guard = start()?;
    assert_eq!(
        Client::connect(&name)?.request("snapshot")?,
        ui.agent().handle("snapshot")
    );
    drop(guard);
    assert!(!path(&name).exists());
    Ok(())
}

#[test]
fn latency_smoke() -> Result<(), IpcError> {
    let ui = Ui::new();
    for n in 0..20 {
        ui.mount(Counter(n));
    }
    let guard = Server::new(&ui).endpoint(endpoint("latency")?).spawn()?;
    let mut client = Client::connect(guard.endpoint())?;
    let expected = ui.agent().handle("snapshot");
    let start = Instant::now();
    for _ in 0..1000 {
        assert_eq!(client.request("snapshot")?, expected);
    }
    let elapsed = start.elapsed();
    eprintln!("Mean snapshot round trip: {:?}", elapsed / 1000);
    assert!(elapsed < Duration::from_secs(2), "{elapsed:?}");
    Ok(())
}

#[test]
fn fresh_connection_latency() -> Result<(), IpcError> {
    const ITERATIONS: u32 = 200;
    let ui = screen();
    let guard = Server::new(&ui).endpoint(endpoint("fresh")?).spawn()?;
    let expected = ui.agent().handle("snapshot");

    let start = Instant::now();
    for _ in 0..ITERATIONS {
        let mut client = Client::connect(guard.endpoint())?;
        assert_eq!(client.request("snapshot")?, expected);
    }
    let mean = start.elapsed() / ITERATIONS;
    eprintln!("Mean connect + snapshot + disconnect: {mean:?}");
    assert!(mean < Duration::from_millis(2), "{mean:?}");
    Ok(())
}
