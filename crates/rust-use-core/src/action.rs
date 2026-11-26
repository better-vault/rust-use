use crate::{Args, ComponentId};

/// Origin of a change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChangeSource {
    /// Human interaction.
    User,
    /// Agent interaction.
    Agent,
    /// Application logic.
    Program,
}

/// One positional action parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParamSpec {
    /// Parameter name used in diagnostics.
    pub name: &'static str,
    /// Short type description such as `int`.
    pub ty: &'static str,
    /// Whether the argument may be omitted.
    pub optional: bool,
}

/// Agent-visible description of an action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionSpec {
    /// Dispatch name.
    pub name: &'static str,
    /// Human-readable description.
    pub doc: &'static str,
    /// Parameters in call order.
    pub params: &'static [ParamSpec],
}

/// Request presented to interceptors before dispatch.
#[derive(Debug)]
pub struct ActionRequest<'a> {
    /// Stable target id.
    pub id: ComponentId,
    /// Primitive kind.
    pub component: &'static str,
    /// Current human label.
    pub label: Option<&'a str>,
    /// Requested action name.
    pub action: &'a str,
    /// Positional arguments.
    pub args: &'a Args,
    /// Request origin.
    pub source: ChangeSource,
}

/// Result of an interceptor check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Continue dispatch.
    Allow,
    /// Stop dispatch with this reason.
    Deny(String),
}
impl From<bool> for Decision {
    fn from(allow: bool) -> Self {
        if allow {
            Self::Allow
        } else {
            Self::Deny("denied".into())
        }
    }
}
