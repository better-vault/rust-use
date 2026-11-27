use crate::format;
use rust_use_core::{Args, ComponentId, ComponentInfo, InvokeError, Page, Ref, Value};
use serde_json::json;
use std::fmt::{self, Display, Formatter};

/// Controls component paging and optional action documentation.
#[derive(Debug, Clone, Copy)]
pub struct SnapshotOptions {
    /// Include documentation for enabled actions.
    pub docs: bool,
    /// Maximum number of component entries. Defaults to 100.
    pub limit: usize,
    /// Start at this component id or the next available id.
    pub from: Option<ComponentId>,
}

impl Default for SnapshotOptions {
    fn default() -> Self {
        Self {
            docs: false,
            limit: 100,
            from: None,
        }
    }
}

/// Continuation for a page of components.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapshotMore {
    /// Number of matching components after this page.
    pub remaining: usize,
    /// First matching id after this page, to use as the next inclusive start.
    pub from: ComponentId,
}

/// A page of components from the registry.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    /// Observed registry revision.
    pub revision: u64,
    /// Mounted components in id order.
    pub components: Vec<ComponentInfo>,
    /// Whether to include action documentation.
    pub docs: bool,
    /// Continuation if more components match.
    pub more: Option<SnapshotMore>,
}

impl Snapshot {
    /// Return structured content for transport clients.
    pub fn to_json(&self) -> Value {
        let components: Vec<_> = self
            .components
            .iter()
            .map(|c| format::json_component(c, self.docs))
            .collect();
        let mut result = json!({"revision": self.revision, "components": components});
        if let Some(more) = self.more {
            result["more"] = json!({"remaining": more.remaining, "from": more.from.get()});
        }
        result
    }
}

impl Display for Snapshot {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "rev {}", self.revision)?;
        for c in &self.components {
            format::component(f, c, self.docs)?;
        }
        if let Some(more) = self.more {
            write!(f, "\nmore {} from={}", more.remaining, more.from)?;
        }
        Ok(())
    }
}

/// Registry changes since a revision.
#[derive(Debug, Clone, PartialEq)]
pub struct Diff {
    /// Observed registry revision.
    pub revision: u64,
    /// Changed components still mounted.
    pub changed: Vec<ComponentInfo>,
    /// Components unmounted during the interval.
    pub removed: Vec<ComponentId>,
}

impl Diff {
    /// Return structured content for transport clients.
    pub fn to_json(&self) -> Value {
        let changed: Vec<_> = self
            .changed
            .iter()
            .map(|c| format::json_component(c, false))
            .collect();
        let removed: Vec<_> = self.removed.iter().map(|id| id.get()).collect();
        json!({"revision": self.revision, "changed": changed, "removed": removed})
    }
}

impl Display for Diff {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "rev {}", self.revision)?;
        for c in &self.changed {
            format::component(f, c, false)?;
        }
        for id in &self.removed {
            write!(f, "\n-{id}")?;
        }
        Ok(())
    }
}

/// One action in a batch.
#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    /// Component address.
    pub target: Ref,
    /// Action name.
    pub action: String,
    /// Positional arguments.
    pub args: Args,
}

/// Build a typed action step.
pub fn step(target: impl Into<Ref>, action: impl Into<String>, args: impl Into<Args>) -> Step {
    Step {
        target: target.into(),
        action: action.into(),
        args: args.into(),
    }
}

impl Display for Step {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match &self.target {
            Ref::Id(id) => write!(f, "{id}")?,
            Ref::Label(s) => format::string(f, s, false)?,
        }
        write!(f, " {}", self.action)?;
        for arg in self.args.as_slice() {
            write!(f, " ")?;
            format::value(f, arg)?;
        }
        Ok(())
    }
}

/// A batch of typed steps or text parsed at dispatch time.
pub struct Script(pub(crate) Batch);

pub(crate) enum Batch {
    Text(String),
    Steps(Vec<Step>),
}

impl From<String> for Script {
    fn from(s: String) -> Self {
        Self(Batch::Text(s))
    }
}

impl From<&str> for Script {
    fn from(s: &str) -> Self {
        s.to_owned().into()
    }
}

impl From<Vec<Step>> for Script {
    fn from(v: Vec<Step>) -> Self {
        Self(Batch::Steps(v))
    }
}

impl<const N: usize> From<[Step; N]> for Script {
    fn from(v: [Step; N]) -> Self {
        Vec::from(v).into()
    }
}

impl From<Step> for Script {
    fn from(s: Step) -> Self {
        vec![s].into()
    }
}

/// Results of a successful batch.
#[derive(Debug, Clone, PartialEq)]
pub struct ActOutcome {
    /// Non-null results paired with one-based step indices.
    pub values: Vec<(usize, Value)>,
    /// Changes during dispatch.
    pub diff: Diff,
}

impl Display for ActOutcome {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "ok")?;
        for (index, v) in &self.values {
            write!(f, "\n=> {index} ")?;
            format::value(f, v)?;
        }
        write!(f, "\n{}", self.diff)
    }
}

/// Syntax or dispatch failure.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ActError {
    /// Invalid text script.
    #[error("parse error at {line}:{column}: {message}")]
    Parse {
        /// One-based source line.
        line: usize,
        /// One-based character column.
        column: usize,
        /// Syntax diagnostic.
        message: String,
    },
    /// A step failed after earlier steps completed.
    #[error("step {index}: {source}")]
    Step {
        /// One-based failing index.
        index: usize,
        /// Canonical text of the failing step.
        step: String,
        /// Core dispatch diagnostic.
        #[source]
        source: InvokeError,
        /// Partial batch changes.
        diff: Diff,
    },
}

impl ActError {
    /// Render the wire protocol failure reply.
    pub fn to_reply(&self) -> String {
        match self {
            Self::Parse { .. } => format!("err {self}"),
            Self::Step {
                index,
                step,
                source,
                diff,
            } => format!("err {index} {step}: {source}\n{diff}"),
        }
    }
}

/// A page returned by a query.
#[derive(Debug, Clone, PartialEq)]
pub struct QueryOutcome {
    /// Absent when the component has no query behavior.
    pub page: Option<Page>,
}

impl Display for QueryOutcome {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let Some(page) = &self.page else {
            return write!(f, "not queryable");
        };
        for item in &page.items {
            format::value(f, item)?;
            writeln!(f)?;
        }
        if let Some(next) = page.next {
            write!(f, "more from={next}")
        } else {
            write!(f, "total {}", page.total)
        }
    }
}
