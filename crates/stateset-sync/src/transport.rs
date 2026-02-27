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

/// Result of a paginated pull operation with an explicit continuation cursor.
///
/// This wraps [`PullResult`] and carries a cursor that can differ from
/// `remote_head` when the remote reports global head metadata separately from
/// page progression semantics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullPage {
    /// Page payload from the transport.
    pub result: PullResult,
    /// Cursor to use for the next page request, if known.
    ///
    /// `None` means the transport could not infer/provide a safe cursor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<u64>,
}

/// Derive a conservative next cursor from pulled events.
///
/// Uses the highest pulled sequence greater than `since`. This avoids event
/// skips when `remote_head` is an independent global-head watermark.
#[must_use]
pub fn derive_next_cursor(since: u64, events: &[SyncEvent]) -> Option<u64> {
    events.iter().map(|e| e.sequence).filter(|seq| *seq > since).max()
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

    /// Pull events with explicit pagination cursor metadata.
    ///
    /// Default behavior wraps [`Transport::pull_events`] and derives a cursor
    /// from pulled event sequence numbers when `has_more` is true.
    ///
    /// Transport implementations can override this to return a server-provided
    /// continuation cursor that is independent from `remote_head`.
    async fn pull_events_page(&self, since: u64, limit: usize) -> Result<PullPage, SyncError> {
        let result = self.pull_events(since, limit).await?;
        let next_cursor =
            if result.has_more { derive_next_cursor(since, &result.events) } else { None };
        Ok(PullPage { result, next_cursor })
    }
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

    #[test]
    fn derive_next_cursor_uses_max_sequence() {
        let events = vec![
            SyncEvent::new("a", "x", "1", json!({})).with_sequence(2),
            SyncEvent::new("b", "x", "2", json!({})).with_sequence(5),
            SyncEvent::new("c", "x", "3", json!({})).with_sequence(4),
        ];
        assert_eq!(derive_next_cursor(1, &events), Some(5));
    }

    #[test]
    fn derive_next_cursor_returns_none_without_progress() {
        let events = vec![
            SyncEvent::new("a", "x", "1", json!({})).with_sequence(0),
            SyncEvent::new("b", "x", "2", json!({})).with_sequence(1),
        ];
        assert_eq!(derive_next_cursor(1, &events), None);
    }

    #[tokio::test]
    async fn default_pull_events_page_derives_cursor() {
        #[derive(Debug)]
        struct MockTransport;

        #[async_trait::async_trait]
        impl Transport for MockTransport {
            async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
                Ok(PushResult { accepted: events.len(), remote_head: 10 })
            }

            async fn pull_events(
                &self,
                since: u64,
                _limit: usize,
            ) -> Result<PullResult, SyncError> {
                if since == 0 {
                    Ok(PullResult {
                        events: vec![
                            SyncEvent::new("a", "x", "1", json!({})).with_sequence(1),
                            SyncEvent::new("b", "x", "2", json!({})).with_sequence(2),
                        ],
                        remote_head: 99,
                        has_more: true,
                    })
                } else {
                    Ok(PullResult { events: vec![], remote_head: 99, has_more: false })
                }
            }
        }

        let transport = MockTransport;
        let page = transport.pull_events_page(0, 100).await.unwrap();
        assert!(page.result.has_more);
        assert_eq!(page.result.remote_head, 99);
        assert_eq!(page.next_cursor, Some(2));
    }

    #[test]
    fn pull_page_serde_roundtrip() {
        let page = PullPage {
            result: PullResult {
                events: vec![SyncEvent::new("a", "b", "c", json!({})).with_sequence(42)],
                remote_head: 100,
                has_more: true,
            },
            next_cursor: Some(42),
        };
        let json = serde_json::to_string(&page).unwrap();
        let de: PullPage = serde_json::from_str(&json).unwrap();
        assert_eq!(de.result.remote_head, 100);
        assert_eq!(de.next_cursor, Some(42));
        assert_eq!(de.result.events.len(), 1);
    }
}
