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

#[doc(hidden)]
pub mod __private {
    pub use rust_use_core as core;
    pub use serde_json;
}

/// Define an agent-visible primitive and its actions.
///
/// ```
/// use rust_use::prelude::*;
/// #[derive(Primitive, Default)]
/// struct Button { #[agent(value)] pressed: bool }
/// #[actions]
/// impl Button {
///     /// Press the button.
///     fn press(&mut self) { self.pressed = true; }
/// }
/// let ui = Ui::new();
/// let button = ui.mount(Button::default());
/// ui.invoke(button.id(), "press", (), ChangeSource::Agent)?;
/// assert_eq!(button.read(|b| b.pressed), Some(true));
/// # Ok::<(), rust_use::InvokeError>(())
/// ```
#[cfg(feature = "macros")]
pub use rust_use_macros::{Primitive, actions};

pub mod prelude {
    //! Common imports for applications using `rust-use`.
    #[cfg(feature = "macros")]
    pub use crate::actions;
    pub use crate::{
        ActionError, ActionSpec, Args, Behavior, ChangeSource, ComponentId, Decision, Describe,
        Handle, Page, ParamSpec, Primitive, Query, Ref, Ui, Value,
    };
}
