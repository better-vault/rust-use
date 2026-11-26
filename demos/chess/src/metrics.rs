//! Traffic observed at the IPC payload boundary, including failed commands.
use rust_use::prelude::RequestLog;
use std::{
    sync::{Arc, Mutex, MutexGuard},
    time::Duration,
};

/// Automation interface used by a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    /// Agent text protocol.
    Native,
    /// Generic tree and element commands.
    Baseline,
}

/// Cumulative measurements for one transport.
#[derive(Default, Debug, Clone, Copy)]
pub struct Traffic {
    /// Number of answered requests.
    pub calls: u64,
    /// Incoming UTF-8 payload bytes.
    pub request_bytes: usize,
    /// Outgoing UTF-8 payload bytes.
    pub reply_bytes: usize,
    /// Sum of observed dispatch and reply-write times.
    pub total_latency: Duration,
}
impl Traffic {
    /// Estimated tokens for all payloads combined, rounded up.
    pub fn estimated_tokens(&self) -> usize {
        (self.request_bytes + self.reply_bytes).div_ceil(4)
    }
    /// Average observed latency, or zero before the first request.
    #[allow(
        clippy::cast_possible_truncation,
        reason = "the mean cannot exceed the original Duration and its fractional part is below one second"
    )]
    pub fn mean_latency(&self) -> Duration {
        if self.calls == 0 {
            Duration::ZERO
        } else {
            let nanos = self.total_latency.as_nanos() / u128::from(self.calls);
            Duration::new(
                (nanos / 1_000_000_000) as u64,
                (nanos % 1_000_000_000) as u32,
            )
        }
    }
}

/// A consistent snapshot of both transports.
#[derive(Default, Debug, Clone, Copy)]
pub struct MetricsView {
    /// Native protocol totals.
    pub native: Traffic,
    /// Accessibility protocol totals.
    pub baseline: Traffic,
    /// Transport that last completed a request.
    pub last: Option<Transport>,
}

/// Thread-safe collector shared between the server and the GUI.
#[derive(Clone)]
pub struct Metrics {
    view: Arc<Mutex<MetricsView>>,
    changed: async_channel::Sender<()>,
    updates: async_channel::Receiver<()>,
}
impl Default for Metrics {
    fn default() -> Self {
        let (changed, updates) = async_channel::bounded(1);
        Self {
            view: Arc::default(),
            changed,
            updates,
        }
    }
}
impl Metrics {
    fn lock(&self) -> MutexGuard<'_, MetricsView> {
        self.view
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
    /// Coalesced wakeups for the window. Read the latest snapshot on receipt.
    pub fn updates(&self) -> async_channel::Receiver<()> {
        self.updates.clone()
    }
    pub(crate) fn observe(&self, log: &RequestLog<'_>) {
        let mut view = self.lock();
        let kind = if log.request.split_whitespace().next() == Some("a11y") {
            Transport::Baseline
        } else {
            Transport::Native
        };
        view.last = Some(kind);
        let traffic = match kind {
            Transport::Native => &mut view.native,
            Transport::Baseline => &mut view.baseline,
        };
        traffic.calls += 1;
        traffic.request_bytes += log.request.len();
        traffic.reply_bytes += log.reply.len();
        traffic.total_latency += log.elapsed;
        drop(view);
        let _ = self.changed.try_send(());
    }
    /// Read both sets of measurements atomically.
    pub fn snapshot(&self) -> MetricsView {
        *self.lock()
    }
    /// Clear both sets of measurements atomically.
    pub fn reset(&self) {
        *self.lock() = MetricsView::default();
        let _ = self.changed.try_send(());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregate_bytes_and_latency_before_rounding() {
        let metrics = Metrics::default();
        for (request, micros) in [("diff", 1), ("diff", 4), (" a11y tree", 6)] {
            metrics.observe(&RequestLog {
                request,
                reply: "é",
                elapsed: Duration::from_micros(micros),
                connection: 1,
            });
        }
        let view = metrics.snapshot();
        assert_eq!(view.native.calls, 2);
        assert_eq!(view.native.reply_bytes, 4);
        assert_eq!(view.native.estimated_tokens(), 3);
        assert_eq!(view.native.mean_latency(), Duration::from_nanos(2500));
        assert_eq!(view.baseline.calls, 1);
        assert_eq!(view.last, Some(Transport::Baseline));
        metrics.reset();
        assert_eq!(metrics.snapshot().native.mean_latency(), Duration::ZERO);
        assert_eq!(metrics.snapshot().baseline.calls, 0);
    }
}
