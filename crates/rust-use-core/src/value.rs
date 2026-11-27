use crate::ActionError;
use serde::{Serialize, de::DeserializeOwned};
pub use serde_json::Value;

/// Positional action arguments.
///
/// ```
/// use rust_use_core::Args;
/// let args = Args::from(("e2e4",));
/// assert_eq!(args.get::<String>(0, "move")?, "e2e4");
/// # Ok::<(), rust_use_core::ActionError>(())
/// ```
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Args(Vec<Value>);

impl Args {
    /// Construct an empty argument list.
    pub fn new() -> Self {
        Self::default()
    }

    /// Wrap a vector of argument values.
    pub fn from_vec(v: Vec<Value>) -> Self {
        Self(v)
    }

    /// Number of supplied arguments.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether no arguments were supplied.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Borrow the positional values.
    pub fn as_slice(&self) -> &[Value] {
        &self.0
    }

    /// Decode a required argument.
    ///
    /// # Errors
    /// Returns a named error for missing or invalid values.
    pub fn get<T: DeserializeOwned>(
        &self,
        index: usize,
        name: &'static str,
    ) -> Result<T, ActionError> {
        self.get_opt(index, name)?
            .ok_or(ActionError::MissingArg(name))
    }

    /// Decode an optional argument, returning `None` when absent.
    ///
    /// # Errors
    /// Returns a named error if the supplied value cannot be decoded.
    pub fn get_opt<T: DeserializeOwned>(
        &self,
        index: usize,
        name: &'static str,
    ) -> Result<Option<T>, ActionError> {
        self.0
            .get(index)
            .map(|value| {
                T::deserialize(value).map_err(|err| ActionError::InvalidArg {
                    name,
                    reason: err.to_string(),
                })
            })
            .transpose()
    }
}

impl From<()> for Args {
    fn from((): ()) -> Self {
        Self::new()
    }
}

impl From<Vec<Value>> for Args {
    fn from(v: Vec<Value>) -> Self {
        Self(v)
    }
}

macro_rules! tuple_args {
    ($($ty:ident $index:tt),+) => {
        impl<$($ty: Serialize),+> From<($($ty,)+)> for Args {
            fn from(values: ($($ty,)+)) -> Self {
                Self(vec![$(serde_json::to_value(values.$index).unwrap_or(Value::Null)),+])
            }
        }
    }
}

tuple_args!(A 0);

tuple_args!(A 0,B 1);

tuple_args!(A 0,B 1,C 2);

tuple_args!(A 0,B 1,C 2,D 3);
