use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::SyncError;
use crate::event::SyncEvent;

/// Default maximum capacity for the outbox.
const DEFAULT_MAX_CAPACITY: usize = 10_000;

/// Snapshot schema for durable outbox persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct OutboxSnapshot {
    events: Vec<SyncEvent>,
    next_sequence: u64,
}

/// An event outbox.
///
/// Events are appended and assigned monotonically increasing sequence numbers.
/// The outbox can be drained (consumed) in order for push operations, or
/// peeked without consuming.
///
/// By default this is in-memory, but it can be backed by a durable JSON
/// snapshot via [`Outbox::with_persistence`].
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
    persistence_path: Option<PathBuf>,
}

impl Outbox {
    /// Create a new in-memory `Outbox` with the given maximum capacity.
    #[must_use]
    pub const fn new(max_capacity: usize) -> Self {
        Self { events: VecDeque::new(), max_capacity, next_sequence: 1, persistence_path: None }
    }

    /// Create a new `Outbox` with the default maximum capacity (10,000).
    #[must_use]
    pub const fn with_default_capacity() -> Self {
        Self::new(DEFAULT_MAX_CAPACITY)
    }

    /// Create a durable outbox persisted to `path`.
    ///
    /// If the snapshot already exists, it is loaded and reused.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::Storage`] if snapshot I/O fails.
    pub fn with_persistence(
        max_capacity: usize,
        path: impl AsRef<Path>,
    ) -> Result<Self, SyncError> {
        let path = path.as_ref().to_path_buf();
        let mut outbox = Self {
            events: VecDeque::new(),
            max_capacity,
            next_sequence: 1,
            persistence_path: Some(path.clone()),
        };

        if path.exists() {
            let contents = fs::read_to_string(&path)
                .map_err(|e| SyncError::Storage(format!("read outbox snapshot failed: {e}")))?;
            if !contents.trim().is_empty() {
                let snapshot: OutboxSnapshot = serde_json::from_str(&contents)?;
                outbox.events = snapshot.events.into();
                outbox.next_sequence = snapshot.next_sequence.max(
                    outbox.events.back().map(|event| event.sequence.saturating_add(1)).unwrap_or(1),
                );
                while outbox.events.len() > outbox.max_capacity {
                    let _ = outbox.events.pop_front();
                }
            }
        } else {
            outbox.persist()?;
        }

        Ok(outbox)
    }

    /// Append an event to the outbox, assigning it the next sequence number.
    ///
    /// Returns the assigned sequence number.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::OutboxFull`] if the outbox is at capacity or
    /// [`SyncError::Storage`] if durable persistence fails.
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

        if let Err(err) = self.persist() {
            let _ = self.events.pop_back();
            self.next_sequence = self.next_sequence.saturating_sub(1);
            return Err(err);
        }

        Ok(seq)
    }

    /// Drain up to `count` events from the front of the outbox (FIFO order).
    ///
    /// Drained events are removed from the outbox.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::Storage`] if durable persistence cannot be updated
    /// after the in-memory drain succeeds.
    pub fn drain(&mut self, count: usize) -> Result<Vec<SyncEvent>, SyncError> {
        let n = count.min(self.events.len());
        let drained: Vec<SyncEvent> = self.events.drain(..n).collect();

        if let Err(err) = self.persist() {
            return Err(err);
        }

        Ok(drained)
    }

    /// Peek at up to `count` events from the front of the outbox without consuming them.
    #[must_use]
    pub fn peek(&self, count: usize) -> Vec<&SyncEvent> {
        self.events.iter().take(count).collect()
    }

    /// Retain only events matching the given predicate.
    ///
    /// Preserves FIFO order and does not modify sequence allocation.
    pub fn retain<F>(&mut self, predicate: F)
    where
        F: FnMut(&SyncEvent) -> bool,
    {
        let _ = self.try_retain(predicate);
    }

    /// Retain only events matching the given predicate and persist the result.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::Storage`] if durable persistence fails. In that
    /// case, the original in-memory ordering is restored.
    pub fn try_retain<F>(&mut self, mut predicate: F) -> Result<(), SyncError>
    where
        F: FnMut(&SyncEvent) -> bool,
    {
        let before = self.events.clone();
        self.events.retain(|event| predicate(event));
        if let Err(err) = self.persist() {
            self.events = before;
            return Err(err);
        }
        Ok(())
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
        let before = self.events.clone();
        self.events.clear();
        if self.persist().is_err() {
            self.events = before;
        }
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

    fn persist(&self) -> Result<(), SyncError> {
        let Some(path) = &self.persistence_path else {
            return Ok(());
        };

        let snapshot = OutboxSnapshot {
            events: self.events.iter().cloned().collect(),
            next_sequence: self.next_sequence,
        };

        let serialized = serde_json::to_string_pretty(&snapshot)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                SyncError::Storage(format!("create outbox snapshot directory failed: {e}"))
            })?;
        }

        let tmp_path = path.with_extension("tmp");
        fs::write(&tmp_path, serialized)
            .map_err(|e| SyncError::Storage(format!("write outbox snapshot failed: {e}")))?;
        fs::rename(&tmp_path, path)
            .map_err(|e| SyncError::Storage(format!("replace outbox snapshot failed: {e}")))?;

        Ok(())
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
    use tempfile::tempdir;

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

        let drained = outbox.drain(2).unwrap();
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

        let drained = outbox.drain(10).unwrap();
        assert_eq!(drained.len(), 2);
        assert!(outbox.is_empty());
    }

    #[test]
    fn drain_empty() {
        let mut outbox = Outbox::new(100);
        let drained = outbox.drain(10).unwrap();
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

        let drained = outbox.drain(3).unwrap();
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

        outbox.drain(1).unwrap();
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

    #[test]
    fn persistent_outbox_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("outbox.json");

        {
            let mut outbox = Outbox::with_persistence(10, &path).unwrap();
            outbox.append(make_event("a")).unwrap();
            outbox.append(make_event("b")).unwrap();
            assert_eq!(outbox.count(), 2);
        }

        let outbox = Outbox::with_persistence(10, &path).unwrap();
        assert_eq!(outbox.count(), 2);
        assert_eq!(outbox.next_sequence(), 3);
    }

    #[test]
    fn drain_returns_storage_error_after_in_memory_removal() {
        let dir = tempdir().unwrap();
        let mut outbox = Outbox::new(10);
        outbox.append(make_event("a")).unwrap();
        outbox.append(make_event("b")).unwrap();
        outbox.persistence_path = Some(dir.path().join("outbox.json"));
        outbox.persist().unwrap();
        std::fs::remove_file(dir.path().join("outbox.json")).unwrap();
        std::fs::create_dir(dir.path().join("outbox.json")).unwrap();

        let err = outbox.drain(1).unwrap_err();
        assert!(matches!(err, SyncError::Storage(_)));
        assert_eq!(outbox.count(), 1);
        assert_eq!(outbox.peek(10)[0].event_type, "b");
    }
}
