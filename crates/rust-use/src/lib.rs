//! A library of UI behaviors that agents can use to build and drive user interfaces.
//! Most users should depend on this facade crate for access to the core and optional adapters.
//!
//! ```toml
//! [dependencies]
//! rust-use = "0.1"
//! ```
//!
//! - `macros` enables procedural macros and is enabled by default.
//! - `agent` enables the agent bridge and is enabled by default.
//! - `gpui` enables the GPUI adapter and the agent bridge.
//! - `ipc` and `mcp` currently enable the agent bridge and reserve future integrations.

#![cfg_attr(docsrs, feature(doc_cfg))]

pub use rust_use_core::{
    ActionError, ActionRequest, ActionSpec, Args, Behavior, ChangeEvent, ChangeKind, ChangeSet,
    ChangeSource, ComponentId, ComponentInfo, Decision, Describe, Handle, InvokeError, Page,
    ParamSpec, Primitive, Query, Ref, ResolveError, Subscription, Ui, Value,
};

#[cfg(feature = "agent")]
pub use rust_use_agent as agent;
#[cfg(feature = "gpui")]
pub use rust_use_gpui as gpui;

pub mod prelude {
    //! Common imports for applications using `rust-use`.
    pub use crate::{
        ActionSpec, Args, Behavior, ChangeSource, ComponentId, Decision, Describe, Handle, Page,
        ParamSpec, Primitive, Query, Ref, Ui, Value,
    };
}
