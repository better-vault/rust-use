//! Core state, primitives, and behaviors for building user interfaces.
//! Most users should depend on the `rust-use` facade.

#![deny(missing_docs)]

mod action;
mod behavior;
mod behaviors;
mod describe;
mod error;
mod ui;
mod value;

pub use action::{ActionRequest, ActionSpec, ChangeSource, Decision, ParamSpec};
pub use behavior::{Behavior, Page, Primitive, Query};
pub use behaviors::{
    DragDrop, Field, Key, List, Orientation, Popup, Press, RovingFocus, Selection, SelectionMode,
    Toggle, Typeahead,
};
pub use describe::Describe;
pub use error::{ActionError, InvokeError, ResolveError};
pub use ui::{
    ChangeEvent, ChangeKind, ChangeSet, ComponentId, ComponentInfo, Handle, Ref, Subscription, Ui,
};
pub use value::{Args, Value};

#[doc(hidden)]
pub mod __private {
    use crate::{ActionError, ActionSpec, Args, Value};
    pub use serde_json;

    #[doc(hidden)]
    #[diagnostic::on_unimplemented(
        message = "add an `#[actions] impl` block or `#[primitive(no_actions)]`"
    )]
    pub trait OwnActions {
        const ACTIONS: &'static [ActionSpec];
        fn own_is_enabled(&self, action: &str) -> Option<bool>;
        fn own_invoke(&mut self, action: &str, args: &Args) -> Option<Result<Value, ActionError>>;
    }
}
