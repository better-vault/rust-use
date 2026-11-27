use std::{
    any::Any,
    collections::{BTreeMap, VecDeque, hash_map::RandomState},
    fmt,
    hash::BuildHasher,
    marker::PhantomData,
    num::ParseIntError,
    str::FromStr,
    sync::{
        Arc, Mutex, MutexGuard, PoisonError,
        atomic::{AtomicU64, Ordering},
    },
    time::SystemTime,
};

use crate::approvals::Registration;
use crate::{
    ActionError, ActionRequest, ActionSpec, Approvals, Args, Behavior, ChangeSource, Decision,
    Describe, InvokeError, Page, Primitive, Query, ResolveError, Value,
};

/// Stable numeric component identifier, displayed as `@1`, `@2`, etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ComponentId(u32);

impl ComponentId {
    /// Return the numeric identifier.
    pub fn get(self) -> u32 {
        self.0
    }
}

impl fmt::Display for ComponentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "@{}", self.0)
    }
}

impl FromStr for ComponentId {
    type Err = ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.strip_prefix('@').unwrap_or(s).parse().map(Self)
    }
}

/// Component address resolved within a single registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ref {
    /// Address a stable id.
    Id(ComponentId),
    /// Match exact label, case-insensitive label, then primitive kind.
    Label(String),
}

impl From<ComponentId> for Ref {
    fn from(id: ComponentId) -> Self {
        Self::Id(id)
    }
}

impl From<String> for Ref {
    fn from(s: String) -> Self {
        if s.starts_with('@')
            && let Ok(id) = s.parse()
        {
            return Self::Id(id);
        }
        Self::Label(s)
    }
}

impl From<&str> for Ref {
    fn from(s: &str) -> Self {
        s.to_owned().into()
    }
}

/// Agent-visible snapshot of one component.
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentInfo {
    /// Whether agents are denied access.
    pub hidden: bool,
    /// Stable id.
    pub id: ComponentId,
    /// Primitive kind.
    pub name: &'static str,
    /// Human label.
    pub label: Option<String>,
    /// Global revision at the latest change to this component.
    pub revision: u64,
    /// Visible state.
    pub describe: Describe,
    /// Currently enabled actions, in declaration order.
    pub actions: Vec<ActionSpec>,
    /// Whether the component supports queries.
    pub queryable: bool,
}

/// Notification of a committed change.
#[derive(Debug, Clone, PartialEq)]
pub struct ChangeEvent {
    /// Affected component.
    pub id: ComponentId,
    /// Global revision assigned to this change.
    pub revision: u64,
    /// Change origin.
    pub source: ChangeSource,
    /// Operation performed.
    pub kind: ChangeKind,
}

/// Operation that advanced the registry revision.
#[derive(Debug, Clone, PartialEq)]
pub enum ChangeKind {
    /// A new primitive was mounted.
    Mounted,
    /// A primitive was removed.
    Unmounted,
    /// A named action succeeded.
    Action(String),
    /// An atomic batch restored the primitive's checkpoint.
    Restored,
    /// App code directly updated the primitive.
    Updated,
    /// The label was set.
    Relabeled,
    /// Agent visibility changed.
    Visibility,
}

/// Components affected since a given revision.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ChangeSet {
    /// Current registry revision.
    pub revision: u64,
    /// Mounted components changed since the supplied revision.
    pub changed: Vec<ComponentId>,
    /// Removed components among the last 1024 unmounts, plus newly hidden components for agent changes.
    pub removed: Vec<ComponentId>,
}

pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

trait Erased: Behavior {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn checkpoint(&self) -> Option<Box<dyn Any + Send>>;
    fn restore(&mut self, saved: Box<dyn Any + Send>);
}

impl<P: Primitive> Erased for P {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn checkpoint(&self) -> Option<Box<dyn Any + Send>> {
        Primitive::checkpoint(self)
    }

    fn restore(&mut self, saved: Box<dyn Any + Send>) {
        Primitive::restore(self, saved);
    }
}

struct CellState {
    primitive: Box<dyn Erased>,
    reserved: bool,
}

type Cell = Arc<Mutex<CellState>>;

struct Entry {
    hidden: bool,
    // Revision at which agents last lost access, or zero if never visible.
    hidden_since: u64,
    name: &'static str,
    label: Option<String>,
    revision: u64,
    cell: Cell,
}

impl Entry {
    fn accessible(&self, source: ChangeSource) -> bool {
        source != ChangeSource::Agent || !self.hidden
    }
}

#[derive(Default)]
struct Registry {
    entries: BTreeMap<ComponentId, Entry>,
    last_id: u32,
    revision: u64,
    removed: VecDeque<(ComponentId, u64, u64)>,
}

impl Registry {
    fn event(&mut self, id: ComponentId, kind: ChangeKind, source: ChangeSource) -> ChangeEvent {
        self.revision += 1;
        if let Some(entry) = self.entries.get_mut(&id) {
            entry.revision = self.revision;
        }
        ChangeEvent {
            id,
            kind,
            source,
            revision: self.revision,
        }
    }

    fn resolve(&self, r: Ref, source: ChangeSource) -> Result<ComponentId, ResolveError> {
        let query = match r {
            Ref::Id(id) => {
                return self
                    .entries
                    .get(&id)
                    .is_some_and(|e| e.accessible(source))
                    .then_some(id)
                    .ok_or_else(|| missing(id));
            }
            Ref::Label(query) => query,
        };
        let folded = query.to_lowercase();
        let mut matches = [Vec::new(), Vec::new(), Vec::new()];
        for (&id, e) in &self.entries {
            if !e.accessible(source) {
                continue;
            }
            if e.label.as_ref() == Some(&query) {
                matches[0].push(id);
            } else if e.label.as_ref().is_some_and(|s| s.to_lowercase() == folded) {
                matches[1].push(id);
            } else if e.name == query {
                matches[2].push(id);
            }
        }
        for candidates in matches {
            match candidates.as_slice() {
                [] => (),
                [id] => return Ok(*id),
                _ => return Err(ResolveError::Ambiguous { query, candidates }),
            }
        }
        Err(ResolveError::NotFound(query))
    }
}

fn missing(id: ComponentId) -> ResolveError {
    ResolveError::NotFound(id.to_string())
}

pub(crate) type Hooks<F> = Arc<Mutex<Vec<Arc<F>>>>;

type Interceptor = dyn Fn(&ActionRequest<'_>) -> Decision + Send + Sync;

type Subscriber = dyn Fn(&ChangeEvent) + Send + Sync;

struct Inner {
    session: u64,
    state: Mutex<Registry>,
    approver: Mutex<Option<Arc<Registration>>>,
    interceptors: Hooks<Interceptor>,
    subscribers: Hooks<Subscriber>,
}

impl Default for Inner {
    fn default() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let session = RandomState::new()
            .hash_one((
                SystemTime::now(),
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::Relaxed),
            ))
            .max(1);
        Self {
            session,
            state: Mutex::default(),
            approver: Mutex::default(),
            interceptors: Arc::default(),
            subscribers: Arc::default(),
        }
    }
}

/// Thread-safe registry shared by all its clones.
///
/// ```
/// use rust_use_core::{Ui, Behavior, Describe, Primitive};
/// struct Counter(u32);
/// impl Behavior for Counter { fn describe(&self, out: &mut Describe) { out.value(self.0); } }
/// impl Primitive for Counter { const NAME: &'static str = "counter"; }
/// let ui = Ui::new();
/// let counter = ui.mount(Counter(0)).label("Score");
/// assert_eq!(ui.resolve("Score")?, counter.id());
/// # Ok::<(), rust_use_core::ResolveError>(())
/// ```
#[derive(Clone, Default)]
pub struct Ui {
    inner: Arc<Inner>,
}

impl Ui {
    /// Create an empty registry at revision zero.
    pub fn new() -> Self {
        Self::default()
    }

    /// Mount a primitive and return a typed handle.
    ///
    /// # Panics
    /// Panics if all 32-bit component ids have been allocated.
    ///
    /// ```
    /// use rust_use_core::{Ui, Behavior, Describe, Primitive};
    /// struct Counter(u32);
    /// impl Behavior for Counter { fn describe(&self, out: &mut Describe) { out.value(self.0); } }
    /// impl Primitive for Counter { const NAME: &'static str = "counter"; }
    /// let counter = Ui::new().mount(Counter(3));
    /// assert_eq!(counter.read(|c| c.0), Some(3));
    /// ```
    pub fn mount<P: Primitive>(&self, primitive: P) -> Handle<P> {
        let mut state = lock(&self.inner.state);
        assert!(state.last_id < u32::MAX, "component id space exhausted");
        state.last_id += 1;
        let id = ComponentId(state.last_id);
        state.entries.insert(
            id,
            Entry {
                name: P::NAME,
                hidden: P::HIDDEN,
                hidden_since: 0,
                label: None,
                revision: 0,
                cell: Arc::new(Mutex::new(CellState {
                    primitive: Box::new(primitive),
                    reserved: false,
                })),
            },
        );
        let event = state.event(id, ChangeKind::Mounted, ChangeSource::Program);
        drop(state);
        self.notify(&event);
        Handle {
            ui: self.clone(),
            id,
            marker: PhantomData,
        }
    }

    /// Return the nonzero session id assigned to this registry at creation.
    ///
    /// It identifies one `Ui` instance and is shared by its clones. It carries no user identity.
    pub fn session(&self) -> u64 {
        self.inner.session
    }

    /// Read the latest global revision.
    pub fn revision(&self) -> u64 {
        lock(&self.inner.state).revision
    }

    /// Describe mounted components in ascending id order.
    pub fn components(&self) -> Vec<ComponentInfo> {
        self.components_for(ChangeSource::Program)
    }

    /// Describe only components accessible to the given source, in id order.
    pub fn components_for(&self, source: ChangeSource) -> Vec<ComponentInfo> {
        let ids: Vec<_> = lock(&self.inner.state)
            .entries
            .iter()
            .filter(|(_, e)| e.accessible(source))
            .map(|(&id, _)| id)
            .collect();
        ids.into_iter()
            .filter_map(|id| self.component_for(id, source).ok())
            .collect()
    }

    fn target(&self, r: Ref, source: ChangeSource) -> Result<(ComponentId, Cell), ResolveError> {
        let state = lock(&self.inner.state);
        let id = state.resolve(r, source)?;
        Ok((id, state.entries[&id].cell.clone()))
    }

    /// Describe one mounted component and its enabled actions.
    ///
    /// # Errors
    /// Returns a resolution error for missing or ambiguous targets.
    pub fn component(&self, r: impl Into<Ref>) -> Result<ComponentInfo, ResolveError> {
        self.component_for(r, ChangeSource::Program)
    }

    /// Describe an accessible component and its enabled actions.
    ///
    /// # Errors
    /// Returns a resolution error for missing or ambiguous accessible targets.
    pub fn component_for(
        &self,
        r: impl Into<Ref>,
        source: ChangeSource,
    ) -> Result<ComponentInfo, ResolveError> {
        let (id, cell) = self.target(r.into(), source)?;
        let guard = lock(&cell);
        let state = lock(&self.inner.state);
        let e = state
            .entries
            .get(&id)
            .filter(|e| e.accessible(source))
            .ok_or_else(|| missing(id))?;
        let hidden = e.hidden;
        let (name, label, revision) = (e.name, e.label.clone(), e.revision);
        drop(state);
        let mut describe = Describe::new();
        guard.primitive.describe(&mut describe);
        let actions = enabled(&*guard.primitive);
        Ok(ComponentInfo {
            hidden,
            id,
            name,
            label,
            revision,
            describe,
            actions,
            queryable: guard.primitive.queryable(),
        })
    }

    /// Resolve a component id, label, or kind.
    ///
    /// # Errors
    /// Returns a resolution error for missing or ambiguous targets.
    pub fn resolve(&self, r: impl Into<Ref>) -> Result<ComponentId, ResolveError> {
        self.resolve_for(r, ChangeSource::Program)
    }

    /// Resolve a reference among components accessible to this source.
    ///
    /// # Errors
    /// Returns a resolution error for missing or ambiguous accessible targets.
    pub fn resolve_for(
        &self,
        r: impl Into<Ref>,
        source: ChangeSource,
    ) -> Result<ComponentId, ResolveError> {
        lock(&self.inner.state).resolve(r.into(), source)
    }

    /// Collect changed ids and retained removals after a revision.
    pub fn changes_since(&self, revision: u64) -> ChangeSet {
        self.collect_changes(revision, false)
    }

    /// Collect changes visible to agents, including removals caused by hiding components.
    pub fn agent_changes_since(&self, revision: u64) -> ChangeSet {
        self.collect_changes(revision, true)
    }

    fn collect_changes(&self, revision: u64, agent: bool) -> ChangeSet {
        let state = lock(&self.inner.state);
        let mut removed: Vec<_> = state
            .removed
            .iter()
            .filter(|(_, unmount, visible_end)| {
                if agent {
                    *visible_end > revision
                } else {
                    *unmount > revision
                }
            })
            .map(|(id, _, _)| *id)
            .collect();
        if agent {
            removed.extend(
                state
                    .entries
                    .iter()
                    .filter(|(_, e)| e.hidden && e.hidden_since > revision)
                    .map(|(&id, _)| id),
            );
        }
        ChangeSet {
            revision: state.revision,
            changed: state
                .entries
                .iter()
                .filter(|(_, e)| e.revision > revision && (!agent || !e.hidden))
                .map(|(&id, _)| id)
                .collect(),
            removed,
        }
    }

    /// Dispatch an enabled action after applying registered interceptors.
    ///
    /// # Errors
    /// Returns resolution, availability, veto, or behavior errors.
    ///
    /// ```
    /// use rust_use_core::*;
    /// struct Counter(u32);
    /// impl Behavior for Counter {
    ///     fn describe(&self, out: &mut Describe) { out.value(self.0); }
    ///     fn actions(&self) -> Vec<ActionSpec> {
    ///         vec![ActionSpec::new("increment", "Add one", &[])]
    ///     }
    ///     fn invoke(&mut self, action: &str, _: &Args) -> Option<Result<Value, ActionError>> {
    ///         (action == "increment").then(|| { self.0 += 1; Ok(self.0.into()) })
    ///     }
    /// }
    /// impl Primitive for Counter { const NAME: &'static str = "counter"; }
    /// let ui = Ui::new();
    /// ui.mount(Counter(0));
    /// assert_eq!(ui.invoke("counter", "increment", (), ChangeSource::Agent)?, Value::from(1));
    /// # Ok::<(), InvokeError>(())
    /// ```
    pub fn invoke(
        &self,
        r: impl Into<Ref>,
        action: &str,
        args: impl Into<Args>,
        source: ChangeSource,
    ) -> Result<Value, InvokeError> {
        self.invoke_with(r.into(), action, &args.into(), source, None)
    }

    fn invoke_with(
        &self,
        r: Ref,
        action: &str,
        args: &Args,
        source: ChangeSource,
        tx: Option<&mut Tx<'_>>,
    ) -> Result<Value, InvokeError> {
        let (id, cell) = self.target(r, source)?;
        let mut guard = lock(&cell);
        if tx.is_none() && guard.reserved {
            return Err(InvokeError::TransactionActive(id));
        }
        let state = lock(&self.inner.state);
        let entry = state
            .entries
            .get(&id)
            .filter(|e| e.accessible(source))
            .ok_or_else(|| missing(id))?;
        let (name, label) = (entry.name, entry.label.clone());
        drop(state);
        let confirm = check_action(&*guard.primitive, name, action)?;
        let hooks = lock(&self.inner.interceptors).clone();
        if !hooks.is_empty() || (source == ChangeSource::Agent && confirm.is_some()) {
            drop(guard);
            let request = ActionRequest {
                id,
                component: name,
                label: label.as_deref(),
                action,
                args,
                source,
                confirm,
            };
            for hook in hooks {
                if let Decision::Deny(reason) = hook(&request) {
                    return Err(InvokeError::Denied(reason));
                }
            }
            if source == ChangeSource::Agent
                && let Some(text) = confirm
            {
                self.resolve_for(id, source)?;
                let approver = lock(&self.inner.approver)
                    .clone()
                    .ok_or_else(|| InvokeError::Denied(format!("needs a person: {text}")))?;
                approver.wait(&request, text).map_err(InvokeError::Denied)?;
            }
            guard = lock(&cell);
            self.resolve_for(id, source)?;
            if tx.is_none() && guard.reserved {
                return Err(InvokeError::TransactionActive(id));
            }
            let updated = check_action(&*guard.primitive, name, action)?;
            if source == ChangeSource::Agent
                && updated != confirm
                && let Some(text) = updated
            {
                return Err(InvokeError::Denied(format!("needs a person: {text}")));
            }
        }
        if let Some(tx) = tx {
            tx.touch(id);
        }
        let result = guard
            .primitive
            .invoke(action, args)
            .ok_or_else(|| unknown(&*guard.primitive, name, action))??;
        let event = lock(&self.inner.state).event(id, ChangeKind::Action(action.into()), source);
        drop(guard);
        self.notify(&event);
        Ok(result)
    }

    /// Run a batch of checkpointed actions from the given source.
    ///
    /// Actions publish their usual change events as they run. If the closure returns
    /// an error, touched primitives are restored in reverse order of first invocation.
    /// Each restoration advances the revision and publishes one [`ChangeKind::Restored`]
    /// event after the state has been restored.
    ///
    /// Prepared primitives remain readable. Other invocations and direct value updates
    /// on them are rejected until the batch ends, including during approval waits.
    /// Registry operations such as relabeling, hiding, and unmounting still apply.
    ///
    /// # Errors
    /// Returns the error from the closure after restoring the touched primitives.
    pub fn transaction<T, E>(
        &self,
        source: ChangeSource,
        f: impl FnOnce(&mut Tx<'_>) -> Result<T, E>,
    ) -> Result<T, E> {
        let mut tx = Tx {
            ui: self,
            source,
            checkpoints: BTreeMap::new(),
            touched: Vec::new(),
        };
        let result = f(&mut tx);
        let events = if result.is_err() {
            tx.restore()
        } else {
            Vec::new()
        };
        drop(tx);
        for event in events {
            self.notify(&event);
        }
        result
    }

    /// Query a component's optional paginated list.
    ///
    /// # Errors
    /// Returns a resolution error for missing or ambiguous targets.
    pub fn query(&self, r: impl Into<Ref>, query: &Query) -> Result<Option<Page>, ResolveError> {
        self.query_for(r, query, ChangeSource::Program)
    }

    /// Query an accessible component using source-aware resolution.
    ///
    /// # Errors
    /// Returns a resolution error for missing or ambiguous accessible targets.
    pub fn query_for(
        &self,
        r: impl Into<Ref>,
        query: &Query,
        source: ChangeSource,
    ) -> Result<Option<Page>, ResolveError> {
        let (id, cell) = self.target(r.into(), source)?;
        let guard = lock(&cell);
        self.resolve_for(id, source)?;
        Ok(guard.primitive.query(query))
    }

    /// Install a queue for agent actions requiring a person. Replacing or dropping
    /// this registration releases its pending requests. See [`Approvals`].
    pub fn require_approval(&self, approvals: &Approvals) -> Subscription {
        let registration = Arc::new(Registration::new(approvals));
        let mut slot = lock(&self.inner.approver);
        if let Some(previous) = slot.replace(registration.clone()) {
            previous.close();
        }
        let inner = Arc::downgrade(&self.inner);
        Subscription {
            unregister: Some(Box::new(move || {
                if let Some(inner) = inner.upgrade() {
                    let mut slot = lock(&inner.approver);
                    if slot.as_ref().is_some_and(|r| Arc::ptr_eq(r, &registration)) {
                        slot.take();
                    }
                }
                registration.close();
            })),
        }
    }

    /// Register a veto hook for every source, in registration order.
    pub fn intercept(
        &self,
        f: impl Fn(&ActionRequest<'_>) -> Decision + Send + Sync + 'static,
    ) -> Subscription {
        register(&self.inner.interceptors, Arc::new(f))
    }

    /// Subscribe to committed changes.
    pub fn subscribe(&self, f: impl Fn(&ChangeEvent) + Send + Sync + 'static) -> Subscription {
        register(&self.inner.subscribers, Arc::new(f))
    }

    fn notify(&self, event: &ChangeEvent) {
        let hooks = lock(&self.inner.subscribers).clone();
        for hook in hooks {
            hook(event);
        }
    }
}

struct Checkpoint {
    cell: Cell,
    saved: Option<Box<dyn Any + Send>>,
    touched: bool,
}

/// Action dispatch within a [`Ui::transaction`].
pub struct Tx<'a> {
    ui: &'a Ui,
    source: ChangeSource,
    checkpoints: BTreeMap<ComponentId, Checkpoint>,
    touched: Vec<ComponentId>,
}

impl Tx<'_> {
    /// Resolve a target and save its checkpoint if this batch has yet to prepare it.
    ///
    /// Call this for all targets before invoking any steps to validate the whole batch.
    /// Use the returned ids for dispatch to keep targets stable across relabeling.
    ///
    /// # Errors
    /// Returns an error if the target cannot be resolved, reserved, or checkpointed.
    pub fn prepare(&mut self, target: impl Into<Ref>) -> Result<ComponentId, InvokeError> {
        let (id, cell) = self.ui.target(target.into(), self.source)?;
        if self.checkpoints.contains_key(&id) {
            return Ok(id);
        }
        let mut guard = lock(&cell);
        let name = lock(&self.ui.inner.state)
            .entries
            .get(&id)
            .filter(|entry| entry.accessible(self.source))
            .ok_or_else(|| missing(id))?
            .name;
        if guard.reserved {
            return Err(InvokeError::TransactionActive(id));
        }
        let saved = guard
            .primitive
            .checkpoint()
            .ok_or(InvokeError::NotCheckpointable {
                id,
                component: name,
            })?;
        guard.reserved = true;
        drop(guard);
        self.checkpoints.insert(
            id,
            Checkpoint {
                cell,
                saved: Some(saved),
                touched: false,
            },
        );
        Ok(id)
    }

    /// Prepare the target if needed and invoke an action with the registry's usual checks.
    ///
    /// # Errors
    /// Returns errors from preparation, action checks, approvals, or the action itself.
    pub fn invoke(
        &mut self,
        target: impl Into<Ref>,
        action: &str,
        args: impl Into<Args>,
    ) -> Result<Value, InvokeError> {
        let id = self.prepare(target)?;
        self.ui
            .invoke_with(id.into(), action, &args.into(), self.source, Some(self))
    }

    fn touch(&mut self, id: ComponentId) {
        if let Some(checkpoint) = self.checkpoints.get_mut(&id)
            && !checkpoint.touched
        {
            checkpoint.touched = true;
            self.touched.push(id);
        }
    }

    fn restore(&mut self) -> Vec<ChangeEvent> {
        self.touched
            .iter()
            .rev()
            .filter_map(|id| {
                let checkpoint = self.checkpoints.get_mut(id)?;
                let mut guard = lock(&checkpoint.cell);
                guard.primitive.restore(checkpoint.saved.take()?);
                Some(lock(&self.ui.inner.state).event(*id, ChangeKind::Restored, self.source))
            })
            .collect()
    }
}

impl Drop for Tx<'_> {
    fn drop(&mut self) {
        for checkpoint in self.checkpoints.values() {
            lock(&checkpoint.cell).reserved = false;
        }
    }
}

fn enabled(p: &dyn Erased) -> Vec<ActionSpec> {
    p.actions()
        .into_iter()
        .filter(|a| p.is_enabled(a.name))
        .collect()
}

fn unknown(p: &dyn Erased, name: &str, action: &str) -> InvokeError {
    InvokeError::UnknownAction {
        component: name.into(),
        action: action.into(),
        available: enabled(p)
            .iter()
            .map(|a| a.name)
            .collect::<Vec<_>>()
            .join(","),
    }
}

fn check_action(
    p: &dyn Erased,
    name: &str,
    action: &str,
) -> Result<Option<&'static str>, InvokeError> {
    if !p.owns(action) {
        return Err(unknown(p, name, action));
    }
    if !p.is_enabled(action) {
        return Err(ActionError::Disabled(action.into()).into());
    }
    Ok(p.actions()
        .iter()
        .find(|a| a.name == action)
        .and_then(|a| a.confirm))
}

/// Typed access to a mounted primitive. Dropping a handle leaves it mounted.
pub struct Handle<P: Primitive> {
    ui: Ui,
    id: ComponentId,
    marker: PhantomData<fn() -> P>,
}

impl<P: Primitive> Clone for Handle<P> {
    fn clone(&self) -> Self {
        Self {
            ui: self.ui.clone(),
            id: self.id,
            marker: PhantomData,
        }
    }
}

impl<P: Primitive> Handle<P> {
    /// Stable component id.
    pub fn id(&self) -> ComponentId {
        self.id
    }

    /// Registry owning the component.
    pub fn ui(&self) -> &Ui {
        &self.ui
    }

    /// Set the human label and return the handle.
    ///
    /// Not `#[must_use]`: `ui.mount(x).label("Mute");` without keeping the handle is a normal way
    /// to mount a component, since dropping a handle leaves the component mounted.
    #[allow(clippy::return_self_not_must_use)]
    pub fn label(self, label: impl Into<String>) -> Self {
        self.set_label(label);
        self
    }

    /// Set the human label and emit a relabel event if mounted.
    pub fn set_label(&self, label: impl Into<String>) {
        let label = label.into();
        let mut state = lock(&self.ui.inner.state);
        let Some(entry) = state.entries.get_mut(&self.id) else {
            return;
        };
        entry.label = Some(label);
        let event = state.event(self.id, ChangeKind::Relabeled, ChangeSource::Program);
        drop(state);
        self.ui.notify(&event);
    }

    /// Hide the component from agent discovery and dispatch.
    pub fn hide_from_agents(&self) {
        self.set_hidden(true);
    }

    /// Allow agents to discover and use this component.
    pub fn show_to_agents(&self) {
        self.set_hidden(false);
    }

    fn set_hidden(&self, hidden: bool) {
        let Ok((_, cell)) = self.ui.target(self.id.into(), ChangeSource::Program) else {
            return;
        };
        let guard = lock(&cell);
        let mut state = lock(&self.ui.inner.state);
        let next = state.revision + 1;
        let Some(entry) = state.entries.get_mut(&self.id) else {
            return;
        };
        if entry.hidden == hidden {
            return;
        }
        entry.hidden = hidden;
        if hidden {
            entry.hidden_since = next;
        }
        let event = state.event(self.id, ChangeKind::Visibility, ChangeSource::Program);
        drop(state);
        drop(guard);
        self.ui.notify(&event);
    }

    /// Read under the component lock, returning `None` after unmount.
    pub fn read<R>(&self, f: impl FnOnce(&P) -> R) -> Option<R> {
        let (_, cell) = self.ui.target(self.id.into(), ChangeSource::Program).ok()?;
        let guard = lock(&cell);
        self.ui.resolve(self.id).ok()?;
        Some(f(guard.primitive.as_any().downcast_ref()?))
    }

    /// Mutate directly under the component lock and report an update.
    /// Returns `None` if unmounted or reserved by an atomic batch.
    pub fn update<R>(&self, source: ChangeSource, f: impl FnOnce(&mut P) -> R) -> Option<R> {
        let (_, cell) = self.ui.target(self.id.into(), ChangeSource::Program).ok()?;
        let mut guard = lock(&cell);
        if guard.reserved {
            return None;
        }
        self.ui.resolve(self.id).ok()?;
        let result = f(guard.primitive.as_any_mut().downcast_mut()?);
        let event = lock(&self.ui.inner.state).event(self.id, ChangeKind::Updated, source);
        drop(guard);
        self.ui.notify(&event);
        Some(result)
    }

    /// Dispatch a named action through the owning registry.
    ///
    /// # Errors
    /// Propagates errors from [`Ui::invoke`].
    pub fn invoke(
        &self,
        action: &str,
        args: impl Into<Args>,
        source: ChangeSource,
    ) -> Result<Value, InvokeError> {
        self.ui.invoke(self.id, action, args, source)
    }

    /// Remove the component, invalidating all its handles.
    pub fn unmount(self) {
        let Ok((_, cell)) = self.ui.target(self.id.into(), ChangeSource::Program) else {
            return;
        };
        let guard = lock(&cell);
        let mut state = lock(&self.ui.inner.state);
        let Some(entry) = state.entries.remove(&self.id) else {
            return;
        };
        let event = state.event(self.id, ChangeKind::Unmounted, ChangeSource::Program);
        if state.removed.len() == 1024 {
            state.removed.pop_front();
        }
        state.removed.push_back((
            self.id,
            event.revision,
            if entry.hidden {
                entry.hidden_since
            } else {
                event.revision
            },
        ));
        drop(state);
        drop(guard);
        self.ui.notify(&event);
    }

    /// Whether the component is still mounted.
    pub fn is_mounted(&self) -> bool {
        self.ui.resolve(self.id).is_ok()
    }
}

/// Registration guard that unregisters its callback when dropped.
#[must_use]
pub struct Subscription {
    unregister: Option<Box<dyn FnOnce() + Send + Sync>>,
}

impl Subscription {
    /// Leave the callback registered for the registry's lifetime.
    pub fn detach(mut self) {
        self.unregister = None;
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        if let Some(f) = self.unregister.take() {
            f();
        }
    }
}

pub(crate) fn register<F: ?Sized + Send + Sync + 'static>(
    hooks: &Hooks<F>,
    f: Arc<F>,
) -> Subscription {
    lock(hooks).push(f.clone());
    let hooks = Arc::downgrade(hooks);
    Subscription {
        unregister: Some(Box::new(move || {
            if let Some(hooks) = hooks.upgrade() {
                lock(&hooks).retain(|hook| !Arc::ptr_eq(hook, &f));
            }
        })),
    }
}

#[cfg(test)]
mod tests {
    use super::Ui;

    #[test]
    fn session_identifies_the_registry() {
        let first = Ui::new();
        let second = Ui::new();
        assert_ne!(first.session(), 0);
        assert_ne!(second.session(), 0);
        assert_ne!(first.session(), second.session());
        assert_eq!(first.session(), first.clone().session());
    }
}
