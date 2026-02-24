use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Tracks the synchronization state between local and remote stores.
///
/// This mirrors the JS `_ves_sync_state` table, keeping track of
/// local/remote heads, push/pull timestamps, and pending event count.
///
/// # Examples
///
/// ```
/// use stateset_sync::SyncState;
///
/// let state = SyncState::default();
/// assert_eq!(state.local_head, 0);
/// assert_eq!(state.remote_head, 0);
/// assert_eq!(state.pending_count, 0);
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncState {
    /// The local head sequence number (last locally recorded event).
    pub local_head: u64,
    /// The remote head sequence number (last known remote sequence).
    pub remote_head: u64,
    /// Timestamp of the last successful push, if any.
    pub last_push: Option<DateTime<Utc>>,
    /// Timestamp of the last successful pull, if any.
    pub last_pull: Option<DateTime<Utc>>,
    /// Number of events pending push.
    pub pending_count: usize,
}

impl SyncState {
    /// Compute the lag (events behind remote).
    #[must_use]
    pub const fn lag(&self) -> u64 {
        self.remote_head.saturating_sub(self.local_head)
    }

    /// Whether the local store is in sync with remote.
    #[must_use]
    pub const fn is_synced(&self) -> bool {
        self.local_head >= self.remote_head && self.pending_count == 0
    }
}

/// Overall sync status reported by the engine.
///
/// This is the Rust equivalent of the JS `SyncStatus` typedef.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStatus {
    /// Whether the engine has been initialized.
    pub initialized: bool,
    /// Current local head sequence.
    pub local_head: u64,
    /// Current remote head sequence.
    pub remote_head: u64,
    /// Number of events pending push.
    pub pending: usize,
    /// Events behind remote.
    pub lag: u64,
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
        assert!(state.last_push.is_none());
        assert!(state.last_pull.is_none());
        assert_eq!(state.pending_count, 0);
    }

    #[test]
    fn lag_calculation() {
        let state = SyncState { local_head: 5, remote_head: 15, ..Default::default() };
        assert_eq!(state.lag(), 10);
    }

    #[test]
    fn lag_when_local_ahead() {
        let state = SyncState { local_head: 20, remote_head: 15, ..Default::default() };
        assert_eq!(state.lag(), 0);
    }

    #[test]
    fn is_synced_when_equal() {
        let state =
            SyncState { local_head: 10, remote_head: 10, pending_count: 0, ..Default::default() };
        assert!(state.is_synced());
    }

    #[test]
    fn not_synced_with_pending() {
        let state =
            SyncState { local_head: 10, remote_head: 10, pending_count: 3, ..Default::default() };
        assert!(!state.is_synced());
    }

    #[test]
    fn not_synced_when_behind() {
        let state =
            SyncState { local_head: 5, remote_head: 10, pending_count: 0, ..Default::default() };
        assert!(!state.is_synced());
    }

    #[test]
    fn state_serde_roundtrip() {
        let state = SyncState {
            local_head: 42,
            remote_head: 100,
            last_push: Some(Utc::now()),
            last_pull: Some(Utc::now()),
            pending_count: 7,
        };
        let json = serde_json::to_string(&state).unwrap();
        let deserialized: SyncState = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.local_head, state.local_head);
        assert_eq!(deserialized.remote_head, state.remote_head);
        assert_eq!(deserialized.pending_count, state.pending_count);
    }

    #[test]
    fn state_clone_eq() {
        let state =
            SyncState { local_head: 10, remote_head: 20, pending_count: 5, ..Default::default() };
        let cloned = state.clone();
        assert_eq!(state, cloned);
    }

    #[test]
    fn sync_status_debug() {
        let status = SyncStatus {
            initialized: true,
            local_head: 10,
            remote_head: 20,
            pending: 5,
            lag: 10,
            last_push: None,
            last_pull: None,
            buffered_events: 3,
        };
        let debug = format!("{status:?}");
        assert!(debug.contains("SyncStatus"));
    }
}
