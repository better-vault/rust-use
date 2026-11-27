use super::{action, dispatch};
use crate::{ActionError, ActionSpec, Args, Behavior, Describe, Value};

/// A popup with optional modal presentation.
///
/// ```
/// use rust_use_core::Popup;
/// let mut menu = Popup::default();
/// menu.open();
/// assert!(menu.open);
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct Popup {
    /// Whether the popup is visible.
    pub open: bool,
    /// Whether the popup restricts interaction to its contents.
    pub modal: bool,
}

impl Popup {
    /// Show the popup.
    pub fn open(&mut self) {
        self.open = true;
    }

    /// Hide the popup.
    pub fn close(&mut self) {
        self.open = false;
    }

    /// Switch visibility and return the new state.
    pub fn toggle(&mut self) -> bool {
        self.open = !self.open;
        self.open
    }
}

const ACTIONS: [ActionSpec; 2] = [
    action("open", "Show the popup.", &[]),
    action("close", "Hide the popup.", &[]),
];

impl Behavior for Popup {
    fn describe(&self, out: &mut Describe) {
        out.flag("open", self.open);
    }

    fn actions(&self) -> Vec<ActionSpec> {
        ACTIONS.to_vec()
    }

    fn owns(&self, action: &str) -> bool {
        ACTIONS.iter().any(|a| a.name == action)
    }

    fn is_enabled(&self, name: &str) -> bool {
        match name {
            "open" => !self.open,
            "close" => self.open,
            _ => false,
        }
    }

    fn invoke(&mut self, name: &str, args: &Args) -> Option<Result<Value, ActionError>> {
        dispatch(&ACTIONS, name, args, || {
            if !self.is_enabled(name) {
                return Err(ActionError::Disabled(name.into()));
            }
            if name == "open" {
                self.open();
            } else {
                self.close();
            }
            Ok(())
        })
    }
}
