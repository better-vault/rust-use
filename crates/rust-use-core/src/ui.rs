use std::{
    any::Any,
    collections::{BTreeMap, VecDeque},
    fmt,
    marker::PhantomData,
    num::ParseIntError,
    str::FromStr,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use crate::{
    ActionError, ActionRequest, ActionSpec, Args, Behavior, ChangeSource, Decision, Describe,
    InvokeError, Page, Primitive, Query, ResolveError, Value,
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
    /// App code directly updated the primitive.
    Updated,
    /// The label was set.
    Relabeled,
}
/// Components affected since a given revision.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ChangeSet {
    /// Current registry revision.
    pub revision: u64,
    /// Mounted components changed since the supplied revision.
    pub changed: Vec<ComponentId>,
    /// Removed components among the last 1024 unmounts.
    pub removed: Vec<ComponentId>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

trait Erased: Behavior {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}
impl<P: Primitive> Erased for P {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
type Cell = Arc<Mutex<Box<dyn Erased>>>;
struct Entry {
    name: &'static str,
    label: Option<String>,
    revision: u64,
    cell: Cell,
}
#[derive(Default)]
struct Registry {
    entries: BTreeMap<ComponentId, Entry>,
    last_id: u32,
    revision: u64,
    removed: VecDeque<(ComponentId, u64)>,
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
    fn resolve(&self, r: Ref) -> Result<ComponentId, ResolveError> {
        let query = match r {
            Ref::Id(id) => {
                return self
                    .entries
                    .contains_key(&id)
                    .then_some(id)
                    .ok_or_else(|| missing(id));
            }
            Ref::Label(query) => query,
        };
        let folded = query.to_lowercase();
        let mut matches = [Vec::new(), Vec::new(), Vec::new()];
        for (&id, e) in &self.entries {
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

type Hooks<F> = Arc<Mutex<Vec<Arc<F>>>>;
type Interceptor = dyn Fn(&ActionRequest<'_>) -> Decision + Send + Sync;
type Subscriber = dyn Fn(&ChangeEvent) + Send + Sync;
#[derive(Default)]
struct Inner {
    state: Mutex<Registry>,
    interceptors: Hooks<Interceptor>,
    subscribers: Hooks<Subscriber>,
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
                label: None,
                revision: 0,
                cell: Arc::new(Mutex::new(Box::new(primitive))),
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
    /// Read the latest global revision.
    pub fn revision(&self) -> u64 {
        lock(&self.inner.state).revision
    }
    /// Describe mounted components in ascending id order.
    pub fn components(&self) -> Vec<ComponentInfo> {
        let ids: Vec<_> = lock(&self.inner.state).entries.keys().copied().collect();
        ids.into_iter()
            .filter_map(|id| self.component(id).ok())
            .collect()
    }
    fn target(&self, r: Ref) -> Result<(ComponentId, Cell), ResolveError> {
        let state = lock(&self.inner.state);
        let id = state.resolve(r)?;
        Ok((id, state.entries[&id].cell.clone()))
    }
    /// Describe one mounted component and its enabled actions.
    ///
    /// # Errors
    /// Returns a resolution error for missing or ambiguous targets.
    pub fn component(&self, r: impl Into<Ref>) -> Result<ComponentInfo, ResolveError> {
        let (id, cell) = self.target(r.into())?;
        let guard = lock(&cell);
        let state = lock(&self.inner.state);
        let e = state.entries.get(&id).ok_or_else(|| missing(id))?;
        let (name, label, revision) = (e.name, e.label.clone(), e.revision);
        drop(state);
        let mut describe = Describe::new();
        guard.describe(&mut describe);
        let actions = enabled(&**guard);
        Ok(ComponentInfo {
            id,
            name,
            label,
            revision,
            describe,
            actions,
        })
    }
    /// Resolve a component id, label, or kind.
    ///
    /// # Errors
    /// Returns a resolution error for missing or ambiguous targets.
    pub fn resolve(&self, r: impl Into<Ref>) -> Result<ComponentId, ResolveError> {
        lock(&self.inner.state).resolve(r.into())
    }
    /// Collect changed ids and retained removals after a revision.
    pub fn changes_since(&self, revision: u64) -> ChangeSet {
        let state = lock(&self.inner.state);
        ChangeSet {
            revision: state.revision,
            changed: state
                .entries
                .iter()
                .filter(|(_, e)| e.revision > revision)
                .map(|(&id, _)| id)
                .collect(),
            removed: state
                .removed
                .iter()
                .filter(|(_, r)| *r > revision)
                .map(|(id, _)| *id)
                .collect(),
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
    ///         vec![ActionSpec { name: "increment", doc: "Add one", params: &[] }]
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
        let args = args.into();
        let (id, cell) = self.target(r.into())?;
        let mut guard = lock(&cell);
        let state = lock(&self.inner.state);
        let entry = state.entries.get(&id).ok_or_else(|| missing(id))?;
        let (name, label, revision) = (entry.name, entry.label.clone(), entry.revision);
        drop(state);
        check_action(&**guard, name, action)?;
        let hooks = lock(&self.inner.interceptors).clone();
        if !hooks.is_empty() {
            drop(guard);
            let request = ActionRequest {
                id,
                component: name,
                label: label.as_deref(),
                action,
                args: &args,
                source,
            };
            for hook in hooks {
                if let Decision::Deny(reason) = hook(&request) {
                    return Err(InvokeError::Denied(reason));
                }
            }
            guard = lock(&cell);
            let current = lock(&self.inner.state)
                .entries
                .get(&id)
                .ok_or_else(|| missing(id))?
                .revision;
            if current != revision {
                check_action(&**guard, name, action)?;
            }
        }
        let result = guard
            .invoke(action, &args)
            .ok_or_else(|| unknown(&**guard, name, action))??;
        let event = lock(&self.inner.state).event(id, ChangeKind::Action(action.into()), source);
        drop(guard);
        self.notify(&event);
        Ok(result)
    }
    /// Query a component's optional paginated list.
    ///
    /// # Errors
    /// Returns a resolution error for missing or ambiguous targets.
    pub fn query(&self, r: impl Into<Ref>, query: &Query) -> Result<Option<Page>, ResolveError> {
        let (id, cell) = self.target(r.into())?;
        let guard = lock(&cell);
        self.resolve(id)?;
        Ok(guard.query(query))
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
fn check_action(p: &dyn Erased, name: &str, action: &str) -> Result<(), InvokeError> {
    if !p.actions().iter().any(|a| a.name == action) {
        return Err(unknown(p, name, action));
    }
    if !p.is_enabled(action) {
        return Err(ActionError::Disabled(action.into()).into());
    }
    Ok(())
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
    #[must_use]
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
    /// Read under the component lock, returning `None` after unmount.
    pub fn read<R>(&self, f: impl FnOnce(&P) -> R) -> Option<R> {
        let (_, cell) = self.ui.target(self.id.into()).ok()?;
        let guard = lock(&cell);
        self.ui.resolve(self.id).ok()?;
        Some(f(guard.as_any().downcast_ref()?))
    }
    /// Mutate directly under the component lock and report an update.
    pub fn update<R>(&self, source: ChangeSource, f: impl FnOnce(&mut P) -> R) -> Option<R> {
        let (_, cell) = self.ui.target(self.id.into()).ok()?;
        let mut guard = lock(&cell);
        self.ui.resolve(self.id).ok()?;
        let result = f(guard.as_any_mut().downcast_mut()?);
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
        let Ok((_, cell)) = self.ui.target(self.id.into()) else {
            return;
        };
        let guard = lock(&cell);
        let mut state = lock(&self.ui.inner.state);
        if state.entries.remove(&self.id).is_none() {
            return;
        }
        let event = state.event(self.id, ChangeKind::Unmounted, ChangeSource::Program);
        if state.removed.len() == 1024 {
            state.removed.pop_front();
        }
        state.removed.push_back((self.id, event.revision));
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
fn register<F: ?Sized + Send + Sync + 'static>(hooks: &Hooks<F>, f: Arc<F>) -> Subscription {
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
