use crate::ComponentId;

/// Failure while checking arguments or applying an action.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ActionError {
    /// A required positional argument was absent.
    #[error("missing argument `{0}`")]
    MissingArg(&'static str),
    /// The supplied value could not be decoded.
    #[error("invalid argument `{name}`: {reason}")]
    InvalidArg {
        /// Declared parameter name.
        name: &'static str,
        /// Deserialization error.
        reason: String,
    },
    /// The action is temporarily unavailable.
    #[error("action `{0}` is not available right now")]
    Disabled(String),
    /// The behavior rejected the operation.
    #[error("{0}")]
    Rejected(String),
}

/// Failure to identify a unique mounted primitive.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ResolveError {
    /// No mounted component matched.
    #[error("no component matches `{0}`")]
    NotFound(String),
    /// Several components matched at the same priority.
    #[error("`{query}` is ambiguous: {candidates:?}")]
    Ambiguous {
        /// Original label or kind.
        query: String,
        /// Matching ids in ascending order.
        candidates: Vec<ComponentId>,
    },
}

/// Failure to dispatch an action.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum InvokeError {
    /// The target could not be resolved.
    #[error(transparent)]
    Resolve(#[from] ResolveError),
    /// The target does not implement this action.
    #[error("`{component}` has no action `{action}` (available: {available})")]
    UnknownAction {
        /// Primitive kind.
        component: String,
        /// Requested action.
        action: String,
        /// Comma-separated enabled actions.
        available: String,
    },
    /// An interceptor vetoed the request.
    #[error("denied: {0}")]
    Denied(String),
    /// The behavior could not perform the action.
    #[error(transparent)]
    Action(#[from] ActionError),
}

impl From<String> for ActionError {
    fn from(reason: String) -> Self {
        Self::Rejected(reason)
    }
}
