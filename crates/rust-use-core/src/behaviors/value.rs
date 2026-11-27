use super::{action, dispatch, param};
use crate::{ActionError, ActionSpec, Args, Behavior, Describe, Value};
use serde::{Serialize, de::DeserializeOwned};

type Validator<T> = fn(&T) -> Result<(), String>;

/// A typed field with optional validation on every assignment.
///
/// ```
/// use rust_use_core::Field;
/// let mut age = Field::new(20);
/// age.set(21)?;
/// assert_eq!(*age.get(), 21);
/// # Ok::<(), rust_use_core::ActionError>(())
/// ```
#[derive(Debug, Clone, Default)]
pub struct Field<T> {
    value: T,
    validator: Option<Validator<T>>,
}

impl<T> Field<T> {
    /// Store an initial value.
    pub fn new(value: T) -> Self {
        Self {
            value,
            validator: None,
        }
    }

    /// Check subsequent assignments with this function.
    #[must_use]
    pub fn with_validator(mut self, validator: Validator<T>) -> Self {
        self.validator = Some(validator);
        self
    }

    /// Borrow the stored value.
    pub fn get(&self) -> &T {
        &self.value
    }

    /// Assign a value after validation.
    /// # Errors
    /// Returns the validator's rejection and preserves the previous value on failure.
    pub fn set(&mut self, value: T) -> Result<(), ActionError> {
        if let Some(validate) = self.validator {
            validate(&value)?;
        }
        self.value = value;
        Ok(())
    }
}

const ACTIONS: [ActionSpec; 1] = [action("set", "Assign a value.", &[param("value", "any")])];

impl<T: Serialize + DeserializeOwned + Clone + Send + 'static> Behavior for Field<T> {
    fn describe(&self, out: &mut Describe) {
        out.value(&self.value);
    }

    fn actions(&self) -> Vec<ActionSpec> {
        ACTIONS.to_vec()
    }

    fn owns(&self, action: &str) -> bool {
        ACTIONS.iter().any(|a| a.name == action)
    }

    fn invoke(&mut self, name: &str, args: &Args) -> Option<Result<Value, ActionError>> {
        dispatch(&ACTIONS, name, args, || self.set(args.get(0, "value")?))
    }
}
