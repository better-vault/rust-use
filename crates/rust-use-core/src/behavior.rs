use crate::{ActionError, ActionSpec, Args, Describe, Value};
use std::any::Any;

/// Reusable interaction logic exposing state and actions to agents.
///
/// ```
/// use rust_use_core::{Behavior, Describe, Primitive};
/// struct Counter(u32);
/// impl Behavior for Counter {
///     fn describe(&self, out: &mut Describe) { out.value(self.0); }
/// }
/// impl Primitive for Counter { const NAME: &'static str = "counter"; }
/// ```
pub trait Behavior: Send + 'static {
    /// Write agent-visible state.
    fn describe(&self, out: &mut Describe);

    /// List all declared actions, including disabled ones.
    fn actions(&self) -> Vec<ActionSpec> {
        Vec::new()
    }

    /// Whether this behavior declares `action`, including when the action is disabled.
    /// The default scans [`Behavior::actions`]. Override this when the names can be checked
    /// without allocating.
    ///
    /// ```
    /// use rust_use_core::{Behavior, List};
    /// let list = List::<u64>::default();
    /// assert!(list.owns("move"));
    /// assert!(!list.owns("press"));
    /// ```
    fn owns(&self, action: &str) -> bool {
        self.actions().iter().any(|a| a.name == action)
    }

    /// Check whether a declared action is currently available.
    fn is_enabled(&self, _action: &str) -> bool {
        true
    }

    /// Run an owned action or return `None` to allow composite dispatch to continue.
    fn invoke(&mut self, _action: &str, _args: &Args) -> Option<Result<Value, ActionError>> {
        None
    }

    /// Whether this behavior supports [`Behavior::query`]. Defaults to `false`.
    /// Query implementations that return a page should advertise support here without doing
    /// any search or pagination, since this is checked on every snapshot.
    ///
    /// ```
    /// use rust_use_core::{Behavior, List, Press};
    /// assert!(List::<u64>::default().queryable());
    /// assert!(!Press::default().queryable());
    /// ```
    fn queryable(&self) -> bool {
        false
    }

    /// Search an option list, returning `None` for non-queryable behaviors.
    fn query(&self, _query: &Query) -> Option<Page> {
        None
    }
}

/// A behavior mountable as an agent-visible component.
pub trait Primitive: Behavior {
    /// Whether agents initially have access to this primitive (hidden when true).
    const HIDDEN: bool = false;
    /// Kebab-case component kind.
    const NAME: &'static str;

    /// Capture the primitive's state for an atomic action batch.
    ///
    /// The default returns `None`, which excludes this primitive from atomic batches.
    /// Return an owned snapshot of all mutable state an action can change, including
    /// fields omitted from agent descriptions. Implement [`Primitive::restore`] as well.
    /// With the derive macro, `#[primitive(checkpoint)]` uses the primitive's `Clone`.
    fn checkpoint(&self) -> Option<Box<dyn Any + Send>> {
        None
    }

    /// Restore a snapshot returned by [`Primitive::checkpoint`] on this primitive.
    ///
    /// The default does nothing. A manual implementation must accept its own snapshot
    /// and restore state without failing. External effects require their own rollback.
    fn restore(&mut self, _saved: Box<dyn Any + Send>) {}
}

/// Text filter and pagination for an option list.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Query {
    /// Optional search text interpreted by the behavior.
    pub text: Option<String>,
    /// Offset into filtered results.
    pub from: usize,
    /// Requested page size, defaulting to and capped at 50.
    pub limit: usize,
}

impl Query {
    /// Return the bounded, nonzero page size.
    pub fn effective_limit(&self) -> usize {
        if self.limit == 0 {
            50
        } else {
            self.limit.min(50)
        }
    }
}

/// Page of matching options.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Page {
    /// Values on the requested page.
    pub items: Vec<Value>,
    /// Total matching values before pagination.
    pub total: usize,
    /// Offset of the next page when more values exist.
    pub next: Option<usize>,
}

impl Page {
    /// Paginate already-filtered values in a single pass.
    pub fn from_iter<I: IntoIterator<Item = Value>>(items: I, query: &Query) -> Self {
        let mut iter = items.into_iter().fuse();
        let skipped = iter.by_ref().take(query.from).count();
        let items: Vec<_> = iter.by_ref().take(query.effective_limit()).collect();
        let end = skipped + items.len();
        let total = end + iter.count();
        Self {
            items,
            total,
            next: (end < total).then_some(end),
        }
    }
}
