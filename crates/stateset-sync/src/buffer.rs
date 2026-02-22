use std::collections::VecDeque;

use crate::event::SyncEvent;

/// A bounded, FIFO event buffer.
///
/// When the buffer is at capacity and a new event is pushed, the oldest
/// event is evicted and returned. This mirrors the JS `_eventBuffer`
/// behavior in `SyncEngine` where old events are shifted when the
/// buffer exceeds `_eventBufferSize`.
///
/// # Examples
///
/// ```
/// use stateset_sync::{EventBuffer, SyncEvent};
/// use serde_json::json;
///
/// let mut buffer = EventBuffer::new(2);
/// buffer.push(SyncEvent::new("a", "x", "1", json!({})));
/// buffer.push(SyncEvent::new("b", "x", "2", json!({})));
///
/// // Third push evicts the oldest event
/// let evicted = buffer.push(SyncEvent::new("c", "x", "3", json!({})));
/// assert!(evicted.is_some());
/// assert_eq!(evicted.unwrap().event_type, "a");
/// assert_eq!(buffer.len(), 2);
/// ```
#[derive(Debug)]
pub struct EventBuffer {
    buffer: VecDeque<SyncEvent>,
    capacity: usize,
}

impl EventBuffer {
    /// Create a new `EventBuffer` with the given capacity.
    ///
    /// A capacity of 0 means the buffer will evict every event immediately.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// Push an event into the buffer.
    ///
    /// If the buffer is at capacity, the oldest event is evicted and returned.
    /// Returns `None` if no eviction was needed.
    pub fn push(&mut self, event: SyncEvent) -> Option<SyncEvent> {
        let evicted = if self.buffer.len() >= self.capacity {
            self.buffer.pop_front()
        } else {
            None
        };
        // For zero-capacity buffers, don't actually store the event
        if self.capacity > 0 {
            self.buffer.push_back(event);
        }
        evicted
    }

    /// Drain all events from the buffer, returning them in FIFO order.
    pub fn drain_all(&mut self) -> Vec<SyncEvent> {
        self.buffer.drain(..).collect()
    }

    /// Return the number of events in the buffer.
    #[must_use]
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Whether the buffer contains no events.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Whether the buffer is at capacity.
    #[must_use]
    pub fn is_full(&self) -> bool {
        self.buffer.len() >= self.capacity
    }

    /// Return the maximum capacity.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Peek at the most recent `count` events (from the back of the buffer).
    #[must_use]
    pub fn recent(&self, count: usize) -> Vec<&SyncEvent> {
        let start = self.buffer.len().saturating_sub(count);
        self.buffer.iter().skip(start).collect()
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_event(name: &str) -> SyncEvent {
        SyncEvent::new(name, "entity", "id", json!({}))
    }

    #[test]
    fn push_within_capacity() {
        let mut buffer = EventBuffer::new(5);
        let evicted = buffer.push(make_event("a"));
        assert!(evicted.is_none());
        assert_eq!(buffer.len(), 1);
    }

    #[test]
    fn push_at_capacity_evicts() {
        let mut buffer = EventBuffer::new(2);
        buffer.push(make_event("a"));
        buffer.push(make_event("b"));
        assert!(buffer.is_full());

        let evicted = buffer.push(make_event("c"));
        assert!(evicted.is_some());
        assert_eq!(evicted.unwrap().event_type, "a");
        assert_eq!(buffer.len(), 2);
    }

    #[test]
    fn eviction_preserves_fifo_order() {
        let mut buffer = EventBuffer::new(3);
        buffer.push(make_event("a"));
        buffer.push(make_event("b"));
        buffer.push(make_event("c"));

        // Push d evicts a
        let evicted = buffer.push(make_event("d"));
        assert_eq!(evicted.unwrap().event_type, "a");

        let drained = buffer.drain_all();
        assert_eq!(drained[0].event_type, "b");
        assert_eq!(drained[1].event_type, "c");
        assert_eq!(drained[2].event_type, "d");
    }

    #[test]
    fn drain_all_empties_buffer() {
        let mut buffer = EventBuffer::new(10);
        buffer.push(make_event("a"));
        buffer.push(make_event("b"));

        let drained = buffer.drain_all();
        assert_eq!(drained.len(), 2);
        assert!(buffer.is_empty());
    }

    #[test]
    fn drain_all_empty_buffer() {
        let mut buffer = EventBuffer::new(10);
        let drained = buffer.drain_all();
        assert!(drained.is_empty());
    }

    #[test]
    fn is_empty_and_is_full() {
        let mut buffer = EventBuffer::new(2);
        assert!(buffer.is_empty());
        assert!(!buffer.is_full());

        buffer.push(make_event("a"));
        assert!(!buffer.is_empty());
        assert!(!buffer.is_full());

        buffer.push(make_event("b"));
        assert!(!buffer.is_empty());
        assert!(buffer.is_full());
    }

    #[test]
    fn zero_capacity_buffer() {
        let mut buffer = EventBuffer::new(0);
        assert!(buffer.is_full());

        // Push to zero-capacity buffer: nothing stored
        let evicted = buffer.push(make_event("a"));
        assert!(evicted.is_none()); // No event to evict from empty buffer
        assert!(buffer.is_empty());
        assert_eq!(buffer.len(), 0);
    }

    #[test]
    fn capacity_accessor() {
        let buffer = EventBuffer::new(42);
        assert_eq!(buffer.capacity(), 42);
    }

    #[test]
    fn recent_events() {
        let mut buffer = EventBuffer::new(10);
        buffer.push(make_event("a"));
        buffer.push(make_event("b"));
        buffer.push(make_event("c"));

        let recent = buffer.recent(2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].event_type, "b");
        assert_eq!(recent[1].event_type, "c");
    }

    #[test]
    fn recent_more_than_available() {
        let mut buffer = EventBuffer::new(10);
        buffer.push(make_event("a"));

        let recent = buffer.recent(5);
        assert_eq!(recent.len(), 1);
    }

    #[test]
    fn clear_buffer() {
        let mut buffer = EventBuffer::new(10);
        buffer.push(make_event("a"));
        buffer.push(make_event("b"));
        buffer.clear();
        assert!(buffer.is_empty());
        assert_eq!(buffer.len(), 0);
    }

    #[test]
    fn debug_impl() {
        let buffer = EventBuffer::new(10);
        let debug = format!("{buffer:?}");
        assert!(debug.contains("EventBuffer"));
    }

    #[test]
    fn multiple_evictions() {
        let mut buffer = EventBuffer::new(2);
        buffer.push(make_event("a"));
        buffer.push(make_event("b"));

        let e1 = buffer.push(make_event("c"));
        assert_eq!(e1.unwrap().event_type, "a");

        let e2 = buffer.push(make_event("d"));
        assert_eq!(e2.unwrap().event_type, "b");

        let remaining = buffer.drain_all();
        assert_eq!(remaining.len(), 2);
        assert_eq!(remaining[0].event_type, "c");
        assert_eq!(remaining[1].event_type, "d");
    }
}
