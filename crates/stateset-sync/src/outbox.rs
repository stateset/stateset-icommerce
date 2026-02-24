use std::collections::VecDeque;

use crate::error::SyncError;
use crate::event::SyncEvent;

/// Default maximum capacity for the outbox.
const DEFAULT_MAX_CAPACITY: usize = 10_000;

/// An in-memory, append-only event outbox.
///
/// Events are appended and assigned monotonically increasing sequence numbers.
/// The outbox can be drained (consumed) in order for push operations, or
/// peeked without consuming.
///
/// This mirrors the JS `Outbox` class but without SQLite persistence
/// (persistence is delegated to the transport layer or a storage backend).
///
/// # Examples
///
/// ```
/// use stateset_sync::{Outbox, SyncEvent};
/// use serde_json::json;
///
/// let mut outbox = Outbox::new(100);
/// let seq = outbox.append(SyncEvent::new("order.created", "order", "ORD-1", json!({}))).unwrap();
/// assert_eq!(seq, 1);
/// assert_eq!(outbox.count(), 1);
/// ```
#[derive(Debug)]
pub struct Outbox {
    events: VecDeque<SyncEvent>,
    max_capacity: usize,
    next_sequence: u64,
}

impl Outbox {
    /// Create a new `Outbox` with the given maximum capacity.
    #[must_use]
    pub const fn new(max_capacity: usize) -> Self {
        Self { events: VecDeque::new(), max_capacity, next_sequence: 1 }
    }

    /// Create a new `Outbox` with the default maximum capacity (10,000).
    #[must_use]
    pub const fn with_default_capacity() -> Self {
        Self::new(DEFAULT_MAX_CAPACITY)
    }

    /// Append an event to the outbox, assigning it the next sequence number.
    ///
    /// Returns the assigned sequence number.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::OutboxFull`] if the outbox is at capacity.
    pub fn append(&mut self, event: SyncEvent) -> Result<u64, SyncError> {
        if self.events.len() >= self.max_capacity {
            return Err(SyncError::OutboxFull {
                capacity: self.max_capacity,
                current: self.events.len(),
            });
        }

        let seq = self.next_sequence;
        self.next_sequence += 1;
        let event = event.with_sequence(seq);
        self.events.push_back(event);
        Ok(seq)
    }

    /// Drain up to `count` events from the front of the outbox (FIFO order).
    ///
    /// Drained events are removed from the outbox.
    pub fn drain(&mut self, count: usize) -> Vec<SyncEvent> {
        let n = count.min(self.events.len());
        self.events.drain(..n).collect()
    }

    /// Peek at up to `count` events from the front of the outbox without consuming them.
    #[must_use]
    pub fn peek(&self, count: usize) -> Vec<&SyncEvent> {
        self.events.iter().take(count).collect()
    }

    /// Retain only events matching the given predicate.
    ///
    /// Preserves FIFO order and does not modify sequence allocation.
    pub fn retain<F>(&mut self, mut predicate: F)
    where
        F: FnMut(&SyncEvent) -> bool,
    {
        self.events.retain(|event| predicate(event));
    }

    /// Return the number of events currently in the outbox.
    #[must_use]
    pub fn count(&self) -> usize {
        self.events.len()
    }

    /// Whether the outbox is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Whether the outbox is at maximum capacity.
    #[must_use]
    pub fn is_full(&self) -> bool {
        self.events.len() >= self.max_capacity
    }

    /// Remove all events from the outbox.
    pub fn clear(&mut self) {
        self.events.clear();
    }

    /// Return the maximum capacity of the outbox.
    #[must_use]
    pub const fn max_capacity(&self) -> usize {
        self.max_capacity
    }

    /// Return the next sequence number that will be assigned.
    #[must_use]
    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }
}

impl Default for Outbox {
    fn default() -> Self {
        Self::with_default_capacity()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_event(event_type: &str) -> SyncEvent {
        SyncEvent::new(event_type, "order", "ORD-1", json!({}))
    }

    #[test]
    fn append_increments_sequence() {
        let mut outbox = Outbox::new(100);
        let s1 = outbox.append(make_event("a")).unwrap();
        let s2 = outbox.append(make_event("b")).unwrap();
        let s3 = outbox.append(make_event("c")).unwrap();
        assert_eq!(s1, 1);
        assert_eq!(s2, 2);
        assert_eq!(s3, 3);
    }

    #[test]
    fn append_updates_count() {
        let mut outbox = Outbox::new(100);
        assert_eq!(outbox.count(), 0);
        outbox.append(make_event("a")).unwrap();
        assert_eq!(outbox.count(), 1);
        outbox.append(make_event("b")).unwrap();
        assert_eq!(outbox.count(), 2);
    }

    #[test]
    fn append_at_capacity_fails() {
        let mut outbox = Outbox::new(2);
        outbox.append(make_event("a")).unwrap();
        outbox.append(make_event("b")).unwrap();
        let result = outbox.append(make_event("c"));
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SyncError::OutboxFull { capacity: 2, current: 2 }));
    }

    #[test]
    fn drain_partial() {
        let mut outbox = Outbox::new(100);
        outbox.append(make_event("a")).unwrap();
        outbox.append(make_event("b")).unwrap();
        outbox.append(make_event("c")).unwrap();

        let drained = outbox.drain(2);
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].sequence, 1);
        assert_eq!(drained[1].sequence, 2);
        assert_eq!(outbox.count(), 1);
    }

    #[test]
    fn drain_all() {
        let mut outbox = Outbox::new(100);
        outbox.append(make_event("a")).unwrap();
        outbox.append(make_event("b")).unwrap();

        let drained = outbox.drain(10);
        assert_eq!(drained.len(), 2);
        assert!(outbox.is_empty());
    }

    #[test]
    fn drain_empty() {
        let mut outbox = Outbox::new(100);
        let drained = outbox.drain(10);
        assert!(drained.is_empty());
    }

    #[test]
    fn peek_without_consuming() {
        let mut outbox = Outbox::new(100);
        outbox.append(make_event("a")).unwrap();
        outbox.append(make_event("b")).unwrap();

        let peeked = outbox.peek(1);
        assert_eq!(peeked.len(), 1);
        assert_eq!(peeked[0].sequence, 1);
        assert_eq!(outbox.count(), 2); // still there
    }

    #[test]
    fn peek_more_than_available() {
        let mut outbox = Outbox::new(100);
        outbox.append(make_event("a")).unwrap();

        let peeked = outbox.peek(10);
        assert_eq!(peeked.len(), 1);
    }

    #[test]
    fn retain_filters_events_and_preserves_order() {
        let mut outbox = Outbox::new(10);
        outbox.append(make_event("a")).unwrap();
        outbox.append(make_event("b")).unwrap();
        outbox.append(make_event("c")).unwrap();

        outbox.retain(|event| event.event_type != "b");

        let remaining = outbox.peek(10);
        assert_eq!(remaining.len(), 2);
        assert_eq!(remaining[0].event_type, "a");
        assert_eq!(remaining[1].event_type, "c");
    }

    #[test]
    fn clear_removes_all() {
        let mut outbox = Outbox::new(100);
        outbox.append(make_event("a")).unwrap();
        outbox.append(make_event("b")).unwrap();
        outbox.clear();
        assert!(outbox.is_empty());
        assert_eq!(outbox.count(), 0);
    }

    #[test]
    fn clear_does_not_reset_sequence() {
        let mut outbox = Outbox::new(100);
        outbox.append(make_event("a")).unwrap();
        outbox.append(make_event("b")).unwrap();
        outbox.clear();

        let seq = outbox.append(make_event("c")).unwrap();
        assert_eq!(seq, 3); // sequence continues from where it left off
    }

    #[test]
    fn is_empty_and_is_full() {
        let mut outbox = Outbox::new(2);
        assert!(outbox.is_empty());
        assert!(!outbox.is_full());

        outbox.append(make_event("a")).unwrap();
        assert!(!outbox.is_empty());
        assert!(!outbox.is_full());

        outbox.append(make_event("b")).unwrap();
        assert!(!outbox.is_empty());
        assert!(outbox.is_full());
    }

    #[test]
    fn fifo_ordering() {
        let mut outbox = Outbox::new(100);
        outbox.append(make_event("first")).unwrap();
        outbox.append(make_event("second")).unwrap();
        outbox.append(make_event("third")).unwrap();

        let drained = outbox.drain(3);
        assert_eq!(drained[0].event_type, "first");
        assert_eq!(drained[1].event_type, "second");
        assert_eq!(drained[2].event_type, "third");
    }

    #[test]
    fn drain_then_append_works() {
        let mut outbox = Outbox::new(2);
        outbox.append(make_event("a")).unwrap();
        outbox.append(make_event("b")).unwrap();
        assert!(outbox.is_full());

        outbox.drain(1);
        assert!(!outbox.is_full());

        let seq = outbox.append(make_event("c")).unwrap();
        assert_eq!(seq, 3);
    }

    #[test]
    fn default_capacity() {
        let outbox = Outbox::with_default_capacity();
        assert_eq!(outbox.max_capacity(), DEFAULT_MAX_CAPACITY);
    }

    #[test]
    fn default_trait() {
        let outbox = Outbox::default();
        assert_eq!(outbox.max_capacity(), DEFAULT_MAX_CAPACITY);
        assert!(outbox.is_empty());
    }

    #[test]
    fn next_sequence_accessor() {
        let mut outbox = Outbox::new(100);
        assert_eq!(outbox.next_sequence(), 1);
        outbox.append(make_event("a")).unwrap();
        assert_eq!(outbox.next_sequence(), 2);
    }
}
