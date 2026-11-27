//! A library of behaviors for agents to build and drive the next generation of user interfaces.
//!
//! You write ordinary Rust: a struct for a component's state, methods for what it can do. Mount it
//! in a [`Ui`] and an agent can see it and operate it by name, through one compact line of text
//! per component, without screenshots, accessibility-tree crawling or pixel clicks. People get
//! the same component through your UI (GPUI first), and both go through the same actions, change
//! events and `allow` checks.
//!
//! **rust-use is in beta.** APIs and the compact format may change between minor versions until
//! 1.0.
//!
//! ```toml
//! [dependencies]
//! rust-use = "0.1"                                     # or: features = ["gpui", "ipc"]
//! ```
//!
//! | Feature | Default | Adds |
//! | --- | --- | --- |
//! | `macros` | ✓ | [`Primitive`](macro@Primitive) and [`actions`] |
//! | `agent` | ✓ | the agent bridge: [`agent`] (`snapshot`, `diff`, `act`, `query`, `allow`) |
//! | `gpui` | | the GPUI adapter, as `rust_use::gpui` |
//! | `ipc` | | a local socket server and client so out-of-process agents reach a running app |
//! | `mcp` | | the same three tools served over MCP |

#![cfg_attr(
    all(feature = "macros", feature = "agent"),
    doc = r#"
## Quick start

```
use rust_use::prelude::*;

/// A button that mutes audio.
#[derive(Primitive, Default)]
struct MuteButton {
    #[agent(value)]
    muted: bool,
}

#[actions]
impl MuteButton {
    /// Mute or unmute.
    fn toggle(&mut self) {
        self.muted = !self.muted;
    }
}

let ui = Ui::new();
let mute = ui.mount(MuteButton::default()).label("Mute");

// What an agent reads: one line per component, with the actions available right now.
let agent = ui.agent();
assert_eq!(
    agent.snapshot().to_string(),
    "rev 2\n@1 mute-button \"Mute\" =false !toggle"
);

// What an agent sends. The reply carries only what changed.
let reply = agent.act("Mute toggle")?;
assert!(reply.to_string().ends_with("@1 mute-button \"Mute\" =true !toggle"));
assert_eq!(mute.read(|m| m.muted), Some(true));
# Ok::<(), rust_use::agent::ActError>(())
```

## Behaviors

Reusable interaction logic embeds into a component with `#[behavior]`. Its state and actions merge
into the component's own, and its typed methods stay available to your code.

```
use rust_use::prelude::*;

/// Tabs along the top of a settings window.
#[derive(Primitive)]
#[primitive(no_actions)]
struct Settings {
    #[behavior]
    tabs: Tabs,
}

let ui = Ui::new();
let settings = ui.mount(Settings { tabs: Tabs::new(["general", "privacy"]) }).label("Settings");
ui.agent().act("Settings select privacy")?;
assert_eq!(settings.read(|s| s.tabs.is_visible("privacy")), Some(true));
# Ok::<(), rust_use::agent::ActError>(())
```
"#
)]
//!
//! ## Where things are
//!
//! - The core, at the crate root: [`Ui`], [`Handle`], [`Primitive`], [`Behavior`], [`Describe`],
//!   [`ChangeSource`] and the behaviors ([`Toggle`], [`Selection`], [`List`], [`RovingFocus`], …).
//! - [`prelude`]: the imports nearly every program needs.
#![cfg_attr(docsrs, feature(doc_cfg))]

pub use rust_use_core::{
    ActionError, ActionRequest, ActionSpec, Args, Behavior, ChangeEvent, ChangeKind, ChangeSet,
    ChangeSource, Combobox, ComponentId, ComponentInfo, Decision, Describe, DragDrop, Field,
    Handle, InvokeError, Key, List, Orientation, Page, ParamSpec, Popup, Press, Primitive, Query,
    Range, Ref, ResolveError, RovingFocus, Selection, SelectionMode, Subscription, Tabs, Toggle,
    Transfer, Tree, TreeRow, Typeahead, Ui, Value,
};

#[cfg(feature = "agent")]
pub use rust_use_agent as agent;
#[cfg(feature = "ipc")]
pub use rust_use_agent::ipc;
#[cfg(feature = "mcp")]
pub use rust_use_agent::mcp;
#[cfg(feature = "agent")]
pub use rust_use_agent::{
    ActError, ActOutcome, Agent, Diff, QueryOutcome, Script, Snapshot, SnapshotMore,
    SnapshotOptions, Step, UiAgentExt, step,
};
#[cfg(feature = "gpui")]
pub use rust_use_gpui as gpui;
#[cfg(feature = "gpui")]
pub use rust_use_gpui::{HandleExt, UiBinding, UiExt};

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
    #[cfg(feature = "mcp")]
    pub use crate::mcp::{Endpoint as McpEndpoint, McpError, McpServer};
    pub use crate::{
        ActionError, ActionSpec, Args, Behavior, ChangeSource, Combobox, ComponentId, Decision,
        Describe, DragDrop, Field, Handle, Key, List, Orientation, Page, ParamSpec, Popup, Press,
        Primitive, Query, Range, Ref, RovingFocus, Selection, SelectionMode, Tabs, Toggle,
        Transfer, Tree, TreeRow, Typeahead, Ui, Value,
    };
    #[cfg(feature = "gpui")]
    pub use crate::{HandleExt, UiBinding, UiExt};
    #[cfg(feature = "agent")]
    pub use crate::{SnapshotOptions, UiAgentExt, step};
}
