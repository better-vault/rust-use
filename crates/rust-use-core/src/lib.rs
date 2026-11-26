//! Core state, primitives, and behaviors for building user interfaces.
//! Most users should depend on the `rust-use` facade.

#![deny(missing_docs)]

mod action;
mod behavior;
mod describe;
mod error;
mod ui;
mod value;

pub use action::{ActionRequest, ActionSpec, ChangeSource, Decision, ParamSpec};
pub use behavior::{Behavior, Page, Primitive, Query};
pub use describe::Describe;
pub use error::{ActionError, InvokeError, ResolveError};
pub use ui::{
    ChangeEvent, ChangeKind, ChangeSet, ComponentId, ComponentInfo, Handle, Ref, Subscription, Ui,
};
pub use value::{Args, Value};
