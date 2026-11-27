//! The agent bridge provides compact snapshots, batched actions, queries, and permissions.
//! Most users should depend on the `rust-use` facade with the `agent` feature.

mod agent;
mod format;
mod parse;
mod types;

pub use agent::{Agent, UiAgentExt};
pub use types::{
    ActError, ActOutcome, Diff, QueryOutcome, Script, Snapshot, SnapshotMore, SnapshotOptions,
    Step, step,
};

#[cfg(feature = "ipc")]
pub mod ipc;
#[cfg(feature = "mcp")]
pub mod mcp;
