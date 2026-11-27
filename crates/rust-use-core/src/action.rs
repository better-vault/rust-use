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
#[non_exhaustive]
pub struct ParamSpec {
    /// Parameter name used in diagnostics.
    pub name: &'static str,
    /// Short type description such as `int`.
    pub ty: &'static str,
    /// Whether the argument may be omitted.
    pub optional: bool,
}

impl ParamSpec {
    /// Describe a required positional parameter.
    pub const fn new(name: &'static str, ty: &'static str) -> Self {
        Self {
            name,
            ty,
            optional: false,
        }
    }
    /// Allow the caller to omit this parameter.
    #[must_use]
    pub const fn optional(mut self) -> Self {
        self.optional = true;
        self
    }
}

/// Agent-visible description of an action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct ActionSpec {
    /// Prompt for human approval of an agent request.
    pub confirm: Option<&'static str>,
    /// Dispatch name.
    pub name: &'static str,
    /// Human-readable description.
    pub doc: &'static str,
    /// Parameters in call order.
    pub params: &'static [ParamSpec],
}

impl ActionSpec {
    /// Describe an action and its positional parameters.
    pub const fn new(name: &'static str, doc: &'static str, params: &'static [ParamSpec]) -> Self {
        Self {
            name,
            doc,
            params,
            confirm: None,
        }
    }
    /// Require a person to approve agent dispatch using this prompt.
    #[must_use]
    pub const fn confirm(mut self, text: &'static str) -> Self {
        self.confirm = Some(text);
        self
    }
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
    /// Human approval prompt, when required for agent dispatch.
    pub confirm: Option<&'a str>,
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
