//! Monotonic event ids and bounded replay for the SSE event stream.
//!
//! Server-Sent Events clients can recover from a dropped connection by sending
//! a `Last-Event-ID` header (or `?last_event_id=` query parameter). To support
//! that, every emitted [`CommerceEvent`] is assigned a process-monotonic `id`
//! and buffered in a bounded ring. On reconnect the server replays buffered
//! events whose id is greater than the supplied `Last-Event-ID`, then resumes
//! the live stream.
//!
//! ## Buffer semantics and overflow
//!
//! The ring retains the most recent `capacity` events. When it overflows, the
//! oldest events are discarded. If a client requests replay from an id that has
//! already been evicted (i.e. there is a gap between `Last-Event-ID` and the
//! oldest retained id), the server cannot guarantee a complete replay. In that
//! case it emits a single documented **reset marker** event
//! (`event: stream_reset`) before replaying whatever remains in the buffer, so
//! the client knows it must reconcile state out-of-band (e.g. via a REST list).
//!
//! Each distinct [`Commerce`](stateset_embedded::Commerce) instance gets its own
//! buffer and background pump, created lazily on first subscription and keyed by
//! the `Arc<Commerce>` pointer in [`EventReplayRegistry`].

use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use stateset_core::CommerceEvent;
use stateset_embedded::Commerce;
use tokio::sync::broadcast;

/// Default number of events retained in the replay ring buffer.
pub(crate) const DEFAULT_REPLAY_CAPACITY: usize = 1024;
/// Capacity of the live re-broadcast channel that carries `(id, event)` frames.
const LIVE_CHANNEL_CAPACITY: usize = 1024;

/// The synthetic event type emitted when a replay gap is detected.
pub(crate) const STREAM_RESET_EVENT: &str = "stream_reset";

/// An event paired with its process-monotonic id.
#[derive(Clone, Debug)]
pub(crate) struct SequencedEvent {
    pub(crate) id: u64,
    pub(crate) event: CommerceEvent,
}

/// Inner mutable ring state.
#[derive(Debug)]
struct Ring {
    buf: VecDeque<SequencedEvent>,
    capacity: usize,
    /// Highest id that has been assigned so far (0 means "none yet").
    last_id: u64,
}

impl Ring {
    fn new(capacity: usize) -> Self {
        Self {
            buf: VecDeque::with_capacity(capacity.min(1024)),
            capacity: capacity.max(1),
            last_id: 0,
        }
    }

    fn push(&mut self, event: SequencedEvent) {
        self.last_id = event.id;
        if self.buf.len() == self.capacity {
            self.buf.pop_front();
        }
        self.buf.push_back(event);
    }

    /// The smallest id currently retained, if any.
    fn oldest_id(&self) -> Option<u64> {
        self.buf.front().map(|e| e.id)
    }
}

/// A bounded replay buffer plus a live re-broadcast channel for one event bus.
#[derive(Debug)]
pub(crate) struct EventReplayBuffer {
    ring: Mutex<Ring>,
    live_tx: broadcast::Sender<SequencedEvent>,
}

/// Outcome of preparing a replay for a reconnecting client.
pub(crate) struct ReplayPlan {
    /// Buffered events to replay (already filtered to `id > last_event_id`).
    pub(crate) events: Vec<SequencedEvent>,
    /// `true` when a gap was detected and a reset marker should precede replay.
    pub(crate) gap_detected: bool,
}

impl EventReplayBuffer {
    fn new(capacity: usize) -> Self {
        let (live_tx, _live_rx) = broadcast::channel(LIVE_CHANNEL_CAPACITY);
        Self { ring: Mutex::new(Ring::new(capacity)), live_tx }
    }

    /// Subscribe to the live `(id, event)` stream.
    pub(crate) fn subscribe_live(&self) -> broadcast::Receiver<SequencedEvent> {
        self.live_tx.subscribe()
    }

    /// Record an event: assign the next id, push to the ring, re-broadcast live.
    fn record(&self, event: CommerceEvent) {
        let sequenced = {
            let mut ring = self.ring.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            let id = ring.last_id + 1;
            let sequenced = SequencedEvent { id, event };
            ring.push(sequenced.clone());
            sequenced
        };
        // A send error simply means there are no live subscribers; ignore it.
        let _ = self.live_tx.send(sequenced);
    }

    /// Build a replay plan for a reconnecting client.
    ///
    /// * `last_event_id == None` → no replay (fresh subscription).
    /// * `last_event_id == Some(n)` → replay buffered events with `id > n`.
    ///
    /// A gap is reported when the buffer's oldest retained id is greater than
    /// `n + 1`, i.e. some events after `n` have already been evicted.
    pub(crate) fn replay_after(&self, last_event_id: Option<u64>) -> ReplayPlan {
        let Some(last) = last_event_id else {
            return ReplayPlan { events: Vec::new(), gap_detected: false };
        };
        let ring = self.ring.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let gap_detected = ring.oldest_id().is_some_and(|oldest| oldest > last + 1);
        let events = ring.buf.iter().filter(|e| e.id > last).cloned().collect::<Vec<_>>();
        ReplayPlan { events, gap_detected }
    }

    /// Snapshot the highest id assigned so far (test/inspection helper).
    #[cfg(test)]
    pub(crate) fn last_id(&self) -> u64 {
        self.ring.lock().unwrap_or_else(std::sync::PoisonError::into_inner).last_id
    }
}

/// Lazily-created, per-`Commerce` registry of replay buffers.
///
/// Keyed by the `Arc<Commerce>` pointer address so that a tenant-routed
/// deployment maintains an independent buffer (and background pump) per tenant
/// engine. Cloning the registry shares the underlying map.
#[derive(Clone, Debug, Default)]
pub(crate) struct EventReplayRegistry {
    buffers: Arc<Mutex<HashMap<usize, Arc<EventReplayBuffer>>>>,
    capacity: usize,
}

impl EventReplayRegistry {
    /// Create a registry with the given per-buffer ring capacity.
    pub(crate) fn new(capacity: usize) -> Self {
        Self { buffers: Arc::new(Mutex::new(HashMap::new())), capacity: capacity.max(1) }
    }

    /// Get (or lazily create) the replay buffer for a `Commerce` engine,
    /// spawning its background pump on first creation.
    pub(crate) fn buffer_for(&self, commerce: &Arc<Commerce>) -> Arc<EventReplayBuffer> {
        let key = Arc::as_ptr(commerce) as usize;
        let mut buffers = self.buffers.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(existing) = buffers.get(&key) {
            return Arc::clone(existing);
        }
        let buffer = Arc::new(EventReplayBuffer::new(self.capacity));
        spawn_pump(commerce, &buffer);
        buffers.insert(key, Arc::clone(&buffer));
        buffer
    }
}

/// Spawn a background task that drains the engine's event subscription into the
/// replay buffer, assigning monotonic ids as events arrive.
///
/// The subscription to the engine's broadcast bus is created **synchronously**
/// (before the task is spawned) so that no event emitted after `buffer_for`
/// returns can slip past the pump while the task is still being scheduled.
fn spawn_pump(commerce: &Arc<Commerce>, buffer: &Arc<EventReplayBuffer>) {
    use tokio_stream::StreamExt as _;

    let mut subscription = commerce.subscribe_events();
    let buffer = Arc::clone(buffer);
    // The subscription holds a receiver on the engine's broadcast bus; the task
    // ends naturally when the engine (and thus the sender) is dropped.
    tokio::spawn(async move {
        while let Some(event) = subscription.next().await {
            buffer.record(event);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use stateset_core::{CommerceEvent, CustomerId};

    fn customer_event(email: &str) -> CommerceEvent {
        CommerceEvent::CustomerCreated {
            customer_id: CustomerId::new(),
            email: email.to_string(),
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn ring_assigns_monotonic_ids() {
        let buffer = EventReplayBuffer::new(8);
        buffer.record(customer_event("a@example.com"));
        buffer.record(customer_event("b@example.com"));
        assert_eq!(buffer.last_id(), 2);
    }

    #[test]
    fn replay_after_filters_by_id() {
        let buffer = EventReplayBuffer::new(8);
        for i in 0..5 {
            buffer.record(customer_event(&format!("user{i}@example.com")));
        }
        let plan = buffer.replay_after(Some(2));
        let ids: Vec<u64> = plan.events.iter().map(|e| e.id).collect();
        assert_eq!(ids, vec![3, 4, 5]);
        assert!(!plan.gap_detected);
    }

    #[test]
    fn replay_after_none_is_empty() {
        let buffer = EventReplayBuffer::new(8);
        buffer.record(customer_event("a@example.com"));
        let plan = buffer.replay_after(None);
        assert!(plan.events.is_empty());
        assert!(!plan.gap_detected);
    }

    #[test]
    fn overflow_evicts_oldest_and_reports_gap() {
        let buffer = EventReplayBuffer::new(3);
        // Assign ids 1..=5; capacity 3 retains ids {3,4,5}.
        for i in 0..5 {
            buffer.record(customer_event(&format!("user{i}@example.com")));
        }
        // Client last saw id 1, but ids 2 was evicted (oldest retained is 3).
        let plan = buffer.replay_after(Some(1));
        assert!(plan.gap_detected, "evicted ids after last_event_id must report a gap");
        let ids: Vec<u64> = plan.events.iter().map(|e| e.id).collect();
        assert_eq!(ids, vec![3, 4, 5]);
    }

    #[test]
    fn contiguous_replay_reports_no_gap() {
        let buffer = EventReplayBuffer::new(3);
        for i in 0..5 {
            buffer.record(customer_event(&format!("user{i}@example.com")));
        }
        // Oldest retained id is 3; client last saw id 3 → ids 4,5 with no gap.
        let plan = buffer.replay_after(Some(3));
        assert!(!plan.gap_detected);
        let ids: Vec<u64> = plan.events.iter().map(|e| e.id).collect();
        assert_eq!(ids, vec![4, 5]);
    }

    #[test]
    fn registry_returns_same_buffer_per_commerce() {
        let commerce = Arc::new(Commerce::new(":memory:").expect("commerce"));
        let registry = EventReplayRegistry::new(16);
        // Spawning the pump requires a runtime; use a current-thread one.
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let _guard = rt.enter();
        let a = registry.buffer_for(&commerce);
        let b = registry.buffer_for(&commerce);
        assert!(Arc::ptr_eq(&a, &b));
    }
}
