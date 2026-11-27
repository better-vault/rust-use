use super::{action, dispatch};
use crate::{ActionError, ActionSpec, Args, Behavior, Describe, Value};
use std::fmt;

/// Counts activations and optionally calls an application handler.
///
/// ```
/// use rust_use_core::Press;
/// let mut button = Press::new().on_press(|| {});
/// assert_eq!(button.press(), 1);
/// ```
#[derive(Default)]
pub struct Press {
    /// Total activations, saturating at the maximum count.
    pub presses: u64,
    pressed: bool,
    handler: Option<Box<dyn FnMut() + Send>>,
}

impl fmt::Debug for Press {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Press")
            .field("presses", &self.presses)
            .field("pressed", &self.pressed)
            .finish_non_exhaustive()
    }
}

impl Press {
    /// Create an idle button.
    pub fn new() -> Self {
        Self::default()
    }

    /// Install the activation handler.
    #[must_use]
    pub fn on_press(mut self, f: impl FnMut() + Send + 'static) -> Self {
        self.handler = Some(Box::new(f));
        self
    }

    /// Activate and return the cumulative count.
    pub fn press(&mut self) -> u64 {
        self.presses = self.presses.saturating_add(1);
        if let Some(f) = &mut self.handler {
            f();
        }
        self.presses
    }

    /// Record whether a human is holding the button down.
    pub fn set_pressed(&mut self, pressed: bool) {
        self.pressed = pressed;
    }

    /// Whether the button is currently held down.
    pub fn is_pressed(&self) -> bool {
        self.pressed
    }
}

const ACTIONS: [ActionSpec; 1] = [action("press", "Activate the control.", &[])];

impl Behavior for Press {
    fn describe(&self, out: &mut Describe) {
        out.flag("pressed", self.pressed);
    }

    fn actions(&self) -> Vec<ActionSpec> {
        ACTIONS.to_vec()
    }

    fn owns(&self, action: &str) -> bool {
        ACTIONS.iter().any(|a| a.name == action)
    }

    fn invoke(&mut self, name: &str, args: &Args) -> Option<Result<Value, ActionError>> {
        dispatch(&ACTIONS, name, args, || {
            self.press();
            Ok(())
        })
    }
}
