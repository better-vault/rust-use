use super::{action, dispatch, param};
use crate::{ActionError, ActionSpec, Args, Behavior, Describe, Value};

/// An on/off control.
///
/// ```
/// use rust_use_core::Toggle;
/// let mut switch = Toggle::default();
/// assert!(switch.toggle());
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct Toggle {
    on: bool,
}

impl Toggle {
    /// Start in the given state.
    pub fn new(on: bool) -> Self {
        Self { on }
    }

    /// Flip the state and return its new value.
    pub fn toggle(&mut self) -> bool {
        self.set(!self.on)
    }

    /// Set and return the new state.
    pub fn set(&mut self, on: bool) -> bool {
        self.on = on;
        on
    }

    /// Read the current state.
    pub fn is_on(&self) -> bool {
        self.on
    }
}

const ACTIONS: [ActionSpec; 2] = [
    action("toggle", "Flip the switch.", &[]),
    action("set", "Set the switch.", &[param("on", "bool")]),
];

impl Behavior for Toggle {
    fn describe(&self, out: &mut Describe) {
        out.value(self.on);
    }

    fn actions(&self) -> Vec<ActionSpec> {
        ACTIONS.to_vec()
    }

    fn owns(&self, action: &str) -> bool {
        ACTIONS.iter().any(|a| a.name == action)
    }

    fn invoke(&mut self, name: &str, args: &Args) -> Option<Result<Value, ActionError>> {
        dispatch(&ACTIONS, name, args, || {
            if name == "toggle" {
                self.toggle();
            } else {
                self.set(args.get(0, "on")?);
            }
            Ok(())
        })
    }
}
