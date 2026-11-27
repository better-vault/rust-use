use crate::{
    ActionRequest, Args, ComponentId, Subscription,
    ui::{Hooks, lock, register},
};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Condvar, Mutex, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

/// An agent action awaiting a person's decision.
#[derive(Debug, Clone)]
pub struct PendingApproval {
    /// Unique request id in this queue.
    pub id: u64,
    /// Target component id.
    pub component: ComponentId,
    /// Component label at dispatch.
    pub label: Option<String>,
    /// Requested action.
    pub action: String,
    /// Positional arguments supplied by the agent.
    pub args: Args,
    /// Prompt supplied by the action author.
    pub confirm: &'static str,
    /// Time at which the request entered the queue.
    pub requested: Instant,
}

struct Waiting {
    request: PendingApproval,
    active: Arc<AtomicBool>,
    decision: Option<Result<(), String>>,
    timeout: Duration,
}

#[derive(Default)]
struct Queue {
    next: u64,
    waiting: BTreeMap<u64, Waiting>,
}

#[derive(Default)]
struct Shared {
    queue: Mutex<Queue>,
    wake: Condvar,
    callbacks: Hooks<dyn Fn(&PendingApproval) + Send + Sync>,
}

/// Pending agent actions waiting for a person. Clones share the queue.
///
/// ```
/// use rust_use_core::{Approvals, Ui};
/// let ui = Ui::new();
/// let approvals = Approvals::new();
/// let registration = ui.require_approval(&approvals);
/// // In a UI event handler, after the person accepts:
/// if let Some(request) = approvals.pending().first() {
///     approvals.approve(request.id);
/// }
/// ```
#[derive(Clone)]
pub struct Approvals {
    shared: Arc<Shared>,
    timeout: Duration,
}

impl Default for Approvals {
    fn default() -> Self {
        Self {
            shared: Arc::default(),
            timeout: Duration::from_secs(300),
        }
    }
}

impl Approvals {
    /// Create an empty queue with a five-minute timeout.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum wait for registrations made with this handle.
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// List unanswered requests, oldest first.
    pub fn pending(&self) -> Vec<PendingApproval> {
        lock(&self.shared.queue)
            .waiting
            .values()
            .filter(|w| w.decision.is_none() && w.request.requested.elapsed() < w.timeout)
            .map(|w| w.request.clone())
            .collect()
    }

    /// Approve a pending request. Returns false for expired, decided or unknown ids.
    pub fn approve(&self, id: u64) -> bool {
        self.decide(id, Ok(()))
    }

    /// Deny a pending request with a reason for the agent.
    pub fn deny(&self, id: u64, reason: impl Into<String>) -> bool {
        self.decide(id, Err(reason.into()))
    }

    fn decide(&self, id: u64, decision: Result<(), String>) -> bool {
        let mut queue = lock(&self.shared.queue);
        let Some(w) = queue.waiting.get_mut(&id) else {
            return false;
        };
        if w.decision.is_some() || w.request.requested.elapsed() >= w.timeout {
            return false;
        }
        w.decision = Some(decision);
        self.shared.wake.notify_all();
        true
    }

    /// Register a callback to wake the application's approval view when a request arrives.
    pub fn on_request(&self, f: impl Fn(&PendingApproval) + Send + Sync + 'static) -> Subscription {
        register(&self.shared.callbacks, Arc::new(f))
    }
}

// Each installation has its own lifetime even when several registries share a queue.
pub(crate) struct Registration {
    approvals: Approvals,
    active: Arc<AtomicBool>,
}

impl Registration {
    pub(crate) fn new(approvals: &Approvals) -> Self {
        Self {
            approvals: approvals.clone(),
            active: Arc::new(AtomicBool::new(true)),
        }
    }

    pub(crate) fn close(&self) {
        let shared = &self.approvals.shared;
        let mut queue = lock(&shared.queue);
        self.active.store(false, Ordering::Relaxed);
        for w in queue.waiting.values_mut() {
            if Arc::ptr_eq(&w.active, &self.active) && w.decision.is_none() {
                w.decision = Some(Err("approvals were turned off".into()));
            }
        }
        shared.wake.notify_all();
    }

    pub(crate) fn wait(
        &self,
        req: &ActionRequest<'_>,
        confirm: &'static str,
    ) -> Result<(), String> {
        let shared = &self.approvals.shared;
        let timeout = self.approvals.timeout;
        let mut queue = lock(&shared.queue);
        if !self.active.load(Ordering::Relaxed) {
            return Err("approvals were turned off".into());
        }
        queue.next += 1;
        let id = queue.next;
        let request = PendingApproval {
            id,
            component: req.id,
            label: req.label.map(str::to_owned),
            action: req.action.into(),
            args: req.args.clone(),
            confirm,
            requested: Instant::now(),
        };
        queue.waiting.insert(
            id,
            Waiting {
                request: request.clone(),
                decision: None,
                active: self.active.clone(),
                timeout,
            },
        );
        drop(queue);
        // Also retire the request if an application callback unwinds.
        let _cleanup = Cleanup { shared, id };
        let callbacks = lock(&shared.callbacks).clone();
        for callback in callbacks {
            callback(&request);
        }

        let mut queue = lock(&shared.queue);
        loop {
            if let Some(decision) = queue.waiting.get_mut(&id).and_then(|w| w.decision.take()) {
                queue.waiting.remove(&id);
                return decision;
            }
            let remaining = timeout.saturating_sub(request.requested.elapsed());
            if remaining.is_zero() {
                return Err(format!("no answer from a person within {timeout:?}"));
            }
            (queue, _) = shared
                .wake
                .wait_timeout(queue, remaining)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }
}

struct Cleanup<'a> {
    shared: &'a Shared,
    id: u64,
}
impl Drop for Cleanup<'_> {
    fn drop(&mut self) {
        lock(&self.shared.queue).waiting.remove(&self.id);
    }
}
