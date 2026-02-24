use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::SyncError;
use crate::event::SyncEvent;

/// Result of a push operation.
///
/// Maps to the JS `PushResult` typedef.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushResult {
    /// Number of events accepted by the remote.
    pub accepted: usize,
    /// The new remote head sequence after the push.
    pub remote_head: u64,
}

/// Result of a pull operation.
///
/// Maps to the JS `PullResult` typedef.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullResult {
    /// Events pulled from the remote.
    pub events: Vec<SyncEvent>,
    /// The current remote head sequence.
    pub remote_head: u64,
    /// Whether there are more events available beyond this batch.
    pub has_more: bool,
}

/// Abstraction over the network transport used to push and pull events.
///
/// The JS codebase uses a `UnifiedClient` that supports both REST and gRPC.
/// In Rust we define this as an async trait so that users can provide
/// any transport implementation (HTTP, gRPC, in-memory for testing).
///
/// # Examples
///
/// ```ignore
/// use stateset_sync::{Transport, PushResult, PullResult, SyncError, SyncEvent};
///
/// struct HttpTransport { base_url: String }
///
/// #[async_trait::async_trait]
/// impl Transport for HttpTransport {
///     async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
///         // ... HTTP POST to /events/push
///         todo!()
///     }
///     async fn pull_events(&self, since: u64, limit: usize) -> Result<PullResult, SyncError> {
///         // ... HTTP GET /events?since=...&limit=...
///         todo!()
///     }
/// }
/// ```
#[async_trait]
pub trait Transport: Send + Sync + std::fmt::Debug {
    /// Push a batch of events to the remote sequencer.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::Transport`] if the transport operation fails.
    async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError>;

    /// Pull events from the remote sequencer starting after `since`.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::Transport`] if the transport operation fails.
    async fn pull_events(&self, since: u64, limit: usize) -> Result<PullResult, SyncError>;
}

/// A no-op transport that always succeeds with empty results.
///
/// Useful for testing scenarios where transport behavior is irrelevant.
#[derive(Debug, Clone, Default)]
pub struct NullTransport;

impl NullTransport {
    /// Create a new `NullTransport`.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Transport for NullTransport {
    async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
        Ok(PushResult { accepted: events.len(), remote_head: 0 })
    }

    async fn pull_events(&self, _since: u64, _limit: usize) -> Result<PullResult, SyncError> {
        Ok(PullResult { events: Vec::new(), remote_head: 0, has_more: false })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn push_result_serde_roundtrip() {
        let result = PushResult { accepted: 5, remote_head: 100 };
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: PushResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.accepted, 5);
        assert_eq!(deserialized.remote_head, 100);
    }

    #[test]
    fn pull_result_serde_roundtrip() {
        let result = PullResult {
            events: vec![SyncEvent::new("a", "b", "c", json!({}))],
            remote_head: 50,
            has_more: true,
        };
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: PullResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.events.len(), 1);
        assert_eq!(deserialized.remote_head, 50);
        assert!(deserialized.has_more);
    }

    #[tokio::test]
    async fn null_transport_push() {
        let transport = NullTransport::new();
        let events = vec![SyncEvent::new("a", "b", "c", json!({}))];
        let result = transport.push_events(&events).await.unwrap();
        assert_eq!(result.accepted, 1);
        assert_eq!(result.remote_head, 0);
    }

    #[tokio::test]
    async fn null_transport_pull() {
        let transport = NullTransport::new();
        let result = transport.pull_events(0, 100).await.unwrap();
        assert!(result.events.is_empty());
        assert!(!result.has_more);
    }

    #[test]
    fn null_transport_debug() {
        let transport = NullTransport::new();
        let debug = format!("{transport:?}");
        assert!(debug.contains("NullTransport"));
    }

    #[test]
    fn push_result_debug() {
        let result = PushResult { accepted: 0, remote_head: 0 };
        let debug = format!("{result:?}");
        assert!(debug.contains("PushResult"));
    }

    #[test]
    fn pull_result_debug() {
        let result = PullResult { events: vec![], remote_head: 0, has_more: false };
        let debug = format!("{result:?}");
        assert!(debug.contains("PullResult"));
    }
}
