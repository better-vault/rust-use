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
//! - `ipc` serves the bridge over local sockets.
//! - `mcp` enables the agent bridge.

#![cfg_attr(
    feature = "macros",
    doc = r#"
## Embedding behaviors

Use a `#[behavior]` field to expose a behavior's description, actions and queries
through a primitive. The same field provides typed methods for application code.

```
use rust_use::prelude::*;

#[derive(Primitive)]
#[primitive(no_actions)]
struct Control<B: Behavior> {
    #[behavior]
    input: B,
}

let ui = Ui::new();
let switch = ui.mount(Control { input: Toggle::default() });
ui.invoke(switch.id(), "toggle", (), ChangeSource::Agent)?;
assert_eq!(switch.read(|c| c.input.is_on()), Some(true));

let _button = Control { input: Press::new() };
let _field = Control { input: Field::new(42) };
let _choices = Control { input: Selection::<String>::new(SelectionMode::Multiple) };
let _list = Control { input: List::new(vec!["pear", "plum"]) };
let _focus = Control { input: RovingFocus::new(Orientation::Vertical, 2) };
let _search = Control { input: Typeahead::default() };
let _menu = Control { input: Popup::default() };
let _board = Control { input: DragDrop::<u64>::default() };
# Ok::<(), rust_use::InvokeError>(())
```
"#
)]
#![cfg_attr(docsrs, feature(doc_cfg))]

pub use rust_use_core::{
    ActionError, ActionRequest, ActionSpec, Args, Behavior, ChangeEvent, ChangeKind, ChangeSet,
    ChangeSource, ComponentId, ComponentInfo, Decision, Describe, DragDrop, Field, Handle,
    InvokeError, Key, List, Orientation, Page, ParamSpec, Popup, Press, Primitive, Query, Ref,
    ResolveError, RovingFocus, Selection, SelectionMode, Subscription, Toggle, Typeahead, Ui,
    Value,
};

#[cfg(feature = "agent")]
pub use rust_use_agent as agent;
#[cfg(feature = "ipc")]
pub use rust_use_agent::ipc;
#[cfg(feature = "agent")]
pub use rust_use_agent::{
    ActError, ActOutcome, Agent, Diff, QueryOutcome, Script, Snapshot, SnapshotOptions, Step,
    UiAgentExt, step,
};
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
    #[cfg(feature = "ipc")]
    pub use crate::ipc::{
        Client, Endpoint, IpcError, RequestLog, Server, ServerGuard, list_endpoints,
    };
    pub use crate::{
        ActionError, ActionSpec, Args, Behavior, ChangeSource, ComponentId, Decision, Describe,
        DragDrop, Field, Handle, Key, List, Orientation, Page, ParamSpec, Popup, Press, Primitive,
        Query, Ref, RovingFocus, Selection, SelectionMode, Toggle, Typeahead, Ui, Value,
    };
    #[cfg(feature = "agent")]
    pub use crate::{UiAgentExt, step};
}
