//! Synchronous local IPC for the agent bridge. Each connection is an independent
//! session. Requests and replies are UTF-8 with a four-byte little-endian byte
//! count and a maximum payload of 16 MiB.
//!
//! Unix sockets live at `$XDG_RUNTIME_DIR/rust-use/<name>.sock` or, when unset,
//! in `rust-use-<uid>` below the temporary directory. Windows uses named pipes
//! named `rust-use-<name>`.

mod client;

mod endpoint;

mod frame;

mod server;

pub use client::Client;
pub use endpoint::{Endpoint, list_endpoints};
pub use server::{RequestLog, Server, ServerGuard};

/// Local transport, discovery, or framing failure.
#[derive(Debug, thiserror::Error)]
pub enum IpcError {
    /// Socket or filesystem operation failed.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// Endpoint name did not match `[a-z0-9-]{1,40}`.
    #[error("invalid endpoint name: {0:?}")]
    InvalidName(String),
    /// Discovery found no live endpoints.
    #[error("no running server found")]
    NoServer,
    /// Discovery found multiple live endpoints.
    #[error("multiple servers found: {0:?}")]
    Ambiguous(Vec<String>),
    /// Payload exceeded the wire limit, in bytes.
    #[error("frame too large: {0} bytes (maximum 16 MiB)")]
    FrameTooLarge(usize),
}
