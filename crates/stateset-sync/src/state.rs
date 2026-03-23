use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Tracks the synchronization state between local and remote stores.
///
/// This mirrors the JS `_ves_sync_state` table, keeping track of
/// local outbox progress, canonical remote progress, push/pull timestamps,
/// and pending event count.
///
/// # Examples
///
/// ```
/// use stateset_sync::SyncState;
///
/// let state = SyncState::default();
/// assert_eq!(state.local_head, 0);
/// assert_eq!(state.remote_head, 0);
/// assert_eq!(state.remote_cursor, 0);
/// assert_eq!(state.pending_count, 0);
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncState {
    /// The local outbox head sequence number (last locally recorded provisional event).
    pub local_head: u64,
    /// The highest canonical remote sequence number currently known on the sequencer.
    pub remote_head: u64,
    /// Optional remote state root associated with the latest known head.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_state_root: Option<String>,
    /// Optional latest commitment id associated with the latest known head.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_commitment_id: Option<String>,
    /// The highest canonical remote sequence number fully observed by the local pull cursor.
    #[serde(default)]
    pub remote_cursor: u64,
    /// The latest canonical remote sequence acknowledged for one of this node's pushed events.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_acknowledged_remote_sequence: Option<u64>,
    /// Timestamp of the last successful push, if any.
    pub last_push: Option<DateTime<Utc>>,
    /// Timestamp of the last successful pull, if any.
    pub last_pull: Option<DateTime<Utc>>,
    /// Number of events pending push.
    pub pending_count: usize,
}

impl SyncState {
    /// Compute the lag in canonical remote events between the known remote head and the local pull cursor.
    #[must_use]
    pub const fn lag(&self) -> u64 {
        self.remote_head.saturating_sub(self.remote_cursor)
    }

    /// Whether the local store is in sync with the known remote head.
    #[must_use]
    pub const fn is_synced(&self) -> bool {
        self.pending_count == 0 && self.remote_cursor >= self.remote_head
    }
}

/// Overall sync status reported by the engine.
///
/// This is the Rust equivalent of the JS `SyncStatus` typedef.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStatus {
    /// Whether the engine has been initialized.
    pub initialized: bool,
    /// Current local outbox head sequence.
    pub local_head: u64,
    /// Current remote head sequence.
    pub remote_head: u64,
    /// Optional remote state root associated with the latest known head.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_state_root: Option<String>,
    /// Optional latest commitment id associated with the latest known head.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_commitment_id: Option<String>,
    /// Current canonical remote cursor applied by pull pagination.
    pub remote_cursor: u64,
    /// Continuation cursor to use for the next paginated pull request, if a pull is in progress.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_pull_cursor: Option<u64>,
    /// Latest canonical remote sequence acknowledged for a local push, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_acknowledged_remote_sequence: Option<u64>,
    /// Number of events pending push.
    pub pending: usize,
    /// Number of dead-lettered events retained by the engine.
    pub dead_letters: usize,
    /// Number of retained push confirmations available for inspection.
    #[serde(default)]
    pub retained_confirmations: usize,
    /// Canonical remote events not yet observed by the local cursor.
    pub lag: u64,
    /// Whether local pending events are drained and the remote cursor has reached the known head.
    #[serde(default)]
    pub caught_up: bool,
    /// Timestamp of last push.
    pub last_push: Option<DateTime<Utc>>,
    /// Timestamp of last pull.
    pub last_pull: Option<DateTime<Utc>>,
    /// Number of events in the buffer.
    pub buffered_events: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state() {
        let state = SyncState::default();
        assert_eq!(state.local_head, 0);
        assert_eq!(state.remote_head, 0);
        assert_eq!(state.remote_state_root, None);
        assert_eq!(state.last_commitment_id, None);
        assert_eq!(state.remote_cursor, 0);
        assert_eq!(state.last_acknowledged_remote_sequence, None);
        assert!(state.last_push.is_none());
        assert!(state.last_pull.is_none());
        assert_eq!(state.pending_count, 0);
    }

    #[test]
    fn lag_calculation_uses_remote_cursor() {
        let state = SyncState { remote_cursor: 5, remote_head: 15, ..Default::default() };
        assert_eq!(state.lag(), 10);
    }

    #[test]
    fn lag_when_remote_cursor_is_ahead() {
        let state = SyncState { remote_cursor: 20, remote_head: 15, ..Default::default() };
        assert_eq!(state.lag(), 0);
    }

    #[test]
    fn is_synced_when_cursor_reaches_remote_head_and_no_pending() {
        let state = SyncState {
            remote_cursor: 10,
            remote_head: 10,
            pending_count: 0,
            ..Default::default()
        };
        assert!(state.is_synced());
    }

    #[test]
    fn not_synced_with_pending() {
        let state = SyncState {
            remote_cursor: 10,
            remote_head: 10,
            pending_count: 3,
            ..Default::default()
        };
        assert!(!state.is_synced());
    }

    #[test]
    fn not_synced_when_cursor_is_behind() {
        let state =
            SyncState { remote_cursor: 5, remote_head: 10, pending_count: 0, ..Default::default() };
        assert!(!state.is_synced());
    }

    #[test]
    fn state_serde_roundtrip() {
        let state = SyncState {
            local_head: 42,
            remote_head: 100,
            remote_state_root: Some("root-100".into()),
            last_commitment_id: Some("BATCH-100".into()),
            remote_cursor: 95,
            last_acknowledged_remote_sequence: Some(88),
            last_push: Some(Utc::now()),
            last_pull: Some(Utc::now()),
            pending_count: 7,
        };
        let json = serde_json::to_string(&state).unwrap();
        let deserialized: SyncState = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.local_head, state.local_head);
        assert_eq!(deserialized.remote_head, state.remote_head);
        assert_eq!(deserialized.remote_state_root, state.remote_state_root);
        assert_eq!(deserialized.last_commitment_id, state.last_commitment_id);
        assert_eq!(deserialized.remote_cursor, state.remote_cursor);
        assert_eq!(
            deserialized.last_acknowledged_remote_sequence,
            state.last_acknowledged_remote_sequence
        );
        assert_eq!(deserialized.pending_count, state.pending_count);
    }

    #[test]
    fn state_clone_eq() {
        let state = SyncState {
            local_head: 10,
            remote_head: 20,
            remote_cursor: 15,
            pending_count: 5,
            ..Default::default()
        };
        let cloned = state.clone();
        assert_eq!(state, cloned);
    }

    #[test]
    fn sync_status_debug() {
        let status = SyncStatus {
            initialized: true,
            local_head: 10,
            remote_head: 20,
            remote_state_root: Some("root-20".into()),
            last_commitment_id: Some("BATCH-20".into()),
            remote_cursor: 12,
            next_pull_cursor: Some(13),
            last_acknowledged_remote_sequence: Some(11),
            pending: 5,
            dead_letters: 1,
            retained_confirmations: 2,
            lag: 8,
            caught_up: false,
            last_push: None,
            last_pull: None,
            buffered_events: 3,
        };
        let debug = format!("{status:?}");
        assert!(debug.contains("SyncStatus"));
    }
}
