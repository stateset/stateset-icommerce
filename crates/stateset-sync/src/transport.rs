use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::SyncError;
use crate::event::SyncEvent;

/// Per-event acknowledgement returned after a successful push.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PushAcknowledgement {
    /// The local event id that was accepted by the sequencer.
    pub event_id: Uuid,
    /// Canonical remote sequence number assigned by the sequencer.
    pub remote_sequence: u64,
    /// Optional transport-specific receipt handle or hash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receipt: Option<String>,
}

impl PushAcknowledgement {
    /// Create a new acknowledgement with the required canonical sequence mapping.
    #[must_use]
    pub const fn new(event_id: Uuid, remote_sequence: u64) -> Self {
        Self { event_id, remote_sequence, receipt: None }
    }

    /// Attach a transport-specific receipt payload to the acknowledgement.
    #[must_use]
    pub fn with_receipt(mut self, receipt: impl Into<String>) -> Self {
        self.receipt = Some(receipt.into());
        self
    }
}

/// Per-event rejection returned after a push attempt.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PushRejection {
    /// The local event id that was rejected by the sequencer.
    pub event_id: Uuid,
    /// Optional transport-specific rejection code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// Optional human-readable rejection reason.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Whether the remote indicated that retrying the event may succeed later.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retryable: Option<bool>,
}

impl PushRejection {
    /// Create a new rejection for a specific local event id.
    #[must_use]
    pub const fn new(event_id: Uuid) -> Self {
        Self { event_id, code: None, reason: None, retryable: None }
    }

    /// Attach a transport-specific rejection code.
    #[must_use]
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    /// Attach a human-readable rejection reason.
    #[must_use]
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    /// Mark whether the rejection is retryable.
    #[must_use]
    pub const fn with_retryable(mut self, retryable: bool) -> Self {
        self.retryable = Some(retryable);
        self
    }
}

/// Result of a push operation.
///
/// Maps to the JS `PushResult` typedef.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PushResult {
    /// Number of events accepted by the remote.
    pub accepted: usize,
    /// The new remote head sequence after the push.
    pub remote_head: u64,
    /// Optional per-event acknowledgements mapping local ids to canonical remote sequence numbers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub acknowledgements: Vec<PushAcknowledgement>,
    /// Optional per-event rejections returned by the remote.
    ///
    /// Rejections let the engine distinguish permanently invalid events from
    /// transiently unprocessed ones.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rejections: Vec<PushRejection>,
}

impl PushResult {
    /// Create a push result without per-event acknowledgement metadata.
    #[must_use]
    pub const fn accepted_only(accepted: usize, remote_head: u64) -> Self {
        Self { accepted, remote_head, acknowledgements: Vec::new(), rejections: Vec::new() }
    }

    /// Attach per-event acknowledgements to the result.
    #[must_use]
    pub fn with_acknowledgements(mut self, acknowledgements: Vec<PushAcknowledgement>) -> Self {
        self.acknowledgements = acknowledgements;
        self
    }

    /// Attach per-event rejections to the result.
    #[must_use]
    pub fn with_rejections(mut self, rejections: Vec<PushRejection>) -> Self {
        self.rejections = rejections;
        self
    }

    /// Return the highest canonical remote sequence acknowledged by this push, if any.
    #[must_use]
    pub fn acknowledged_head(&self) -> Option<u64> {
        self.acknowledgements.iter().map(|ack| ack.remote_sequence).max()
    }
}

/// Snapshot of the remote sequencer head.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RemoteHead {
    /// Highest canonical sequence currently known on the remote sequencer.
    pub remote_head: u64,
    /// Optional remote state root or commitment root identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_root: Option<String>,
    /// Optional latest commitment id associated with the remote head.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_commitment_id: Option<String>,
}

impl RemoteHead {
    /// Create a minimal remote-head snapshot.
    #[must_use]
    pub const fn new(remote_head: u64) -> Self {
        Self { remote_head, state_root: None, last_commitment_id: None }
    }

    /// Attach an optional state-root value.
    #[must_use]
    pub fn with_state_root(mut self, state_root: impl Into<String>) -> Self {
        self.state_root = Some(state_root.into());
        self
    }

    /// Attach an optional latest commitment id.
    #[must_use]
    pub fn with_last_commitment_id(mut self, commitment_id: impl Into<String>) -> Self {
        self.last_commitment_id = Some(commitment_id.into());
        self
    }
}

/// Result of a pull operation.
///
/// Maps to the JS `PullResult` typedef. Pulled events should carry
/// `CanonicalRemote` sequence authority from the sequencer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PullResult {
    /// Events pulled from the remote.
    pub events: Vec<SyncEvent>,
    /// The current remote head sequence.
    pub remote_head: u64,
    /// Whether there are more events available beyond this batch.
    pub has_more: bool,
}

/// Result of a paginated pull operation with explicit pagination metadata.
///
/// This wraps [`PullResult`] and distinguishes between:
/// - `observed_cursor`: the highest canonical remote sequence actually seen in
///   this page
/// - `next_cursor`: the continuation cursor to use for the next page request
///
/// These can differ when the remote uses request-scoped continuation cursors or
/// reports page progression separately from the highest event sequence in the
/// returned page.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PullPage {
    /// Page payload from the transport.
    pub result: PullResult,
    /// Cursor to use for the next page request, if known.
    ///
    /// `None` means the transport could not infer/provide a safe continuation
    /// cursor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<u64>,
    /// Highest canonical remote sequence actually observed in this page, if
    /// known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_cursor: Option<u64>,
}

/// Derive a conservative next cursor from pulled events.
///
/// Uses the highest canonical remote sequence greater than `since`. This avoids
/// event skips when `remote_head` is an independent global-head watermark and
/// prevents local provisional ordering from advancing a replication cursor.
#[must_use]
pub fn derive_next_cursor(since: u64, events: &[SyncEvent]) -> Option<u64> {
    events.iter().filter_map(SyncEvent::canonical_sequence).filter(|seq| *seq > since).max()
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
    /// Transports should return per-event acknowledgements when the remote
    /// provides canonical sequence numbers or receipt material, and explicit
    /// per-event rejections when the remote rejects individual local events.
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

    /// Fetch the current remote head without pulling events.
    ///
    /// The default implementation returns a transport error because not every
    /// transport exposes a dedicated head endpoint.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::Transport`] if the transport operation fails or the
    /// transport does not support remote-head queries.
    async fn fetch_head(&self) -> Result<RemoteHead, SyncError> {
        Err(SyncError::Transport("fetch_head is not supported by this transport".to_string()))
    }

    /// Pull events with explicit pagination cursor metadata.
    ///
    /// Default behavior wraps [`Transport::pull_events`], derives the highest
    /// observed canonical remote sequence from the page payload, and reuses it
    /// as the continuation cursor when `has_more` is true.
    ///
    /// Transport implementations can override this to return a server-provided
    /// continuation cursor that is independent from the observed canonical
    /// event sequence.
    async fn pull_events_page(&self, since: u64, limit: usize) -> Result<PullPage, SyncError> {
        let result = self.pull_events(since, limit).await?;
        let observed_cursor = derive_next_cursor(since, &result.events);
        let next_cursor = if result.has_more { observed_cursor } else { None };
        Ok(PullPage { result, next_cursor, observed_cursor })
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
        Ok(PushResult::accepted_only(events.len(), 0))
    }

    async fn pull_events(&self, _since: u64, _limit: usize) -> Result<PullResult, SyncError> {
        Ok(PullResult { events: Vec::new(), remote_head: 0, has_more: false })
    }

    async fn fetch_head(&self) -> Result<RemoteHead, SyncError> {
        Ok(RemoteHead::new(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use serde_json::json;

    #[test]
    fn push_acknowledgement_roundtrip() {
        let ack = PushAcknowledgement::new(Uuid::new_v4(), 42).with_receipt("receipt-hash");
        let json = serde_json::to_string(&ack).unwrap();
        let de: PushAcknowledgement = serde_json::from_str(&json).unwrap();
        assert_eq!(de.remote_sequence, 42);
        assert_eq!(de.receipt.as_deref(), Some("receipt-hash"));
    }

    #[test]
    fn push_rejection_roundtrip() {
        let rejection = PushRejection::new(Uuid::new_v4())
            .with_code("invalid_signature")
            .with_reason("signature verification failed")
            .with_retryable(false);
        let json = serde_json::to_string(&rejection).unwrap();
        let de: PushRejection = serde_json::from_str(&json).unwrap();
        assert_eq!(de.code.as_deref(), Some("invalid_signature"));
        assert_eq!(de.reason.as_deref(), Some("signature verification failed"));
        assert_eq!(de.retryable, Some(false));
    }

    #[test]
    fn push_result_serde_roundtrip() {
        let event_id = Uuid::new_v4();
        let rejected_event_id = Uuid::new_v4();
        let result = PushResult::accepted_only(1, 100)
            .with_acknowledgements(vec![
                PushAcknowledgement::new(event_id, 100).with_receipt("receipt-1"),
            ])
            .with_rejections(vec![
                PushRejection::new(rejected_event_id)
                    .with_code("invalid_signature")
                    .with_reason("signature verification failed")
                    .with_retryable(false),
            ]);
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: PushResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.accepted, 1);
        assert_eq!(deserialized.remote_head, 100);
        assert_eq!(deserialized.acknowledgements.len(), 1);
        assert_eq!(deserialized.acknowledged_head(), Some(100));
        assert_eq!(deserialized.acknowledgements[0].event_id, event_id);
        assert_eq!(deserialized.rejections.len(), 1);
        assert_eq!(deserialized.rejections[0].event_id, rejected_event_id);
        assert_eq!(deserialized.rejections[0].retryable, Some(false));
    }

    #[test]
    fn remote_head_serde_roundtrip() {
        let head =
            RemoteHead::new(42).with_state_root("root-abc").with_last_commitment_id("BATCH-7");
        let json = serde_json::to_string(&head).unwrap();
        let de: RemoteHead = serde_json::from_str(&json).unwrap();
        assert_eq!(de.remote_head, 42);
        assert_eq!(de.state_root.as_deref(), Some("root-abc"));
        assert_eq!(de.last_commitment_id.as_deref(), Some("BATCH-7"));
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
        assert!(result.acknowledgements.is_empty());
        assert!(result.rejections.is_empty());
    }

    #[tokio::test]
    async fn null_transport_fetch_head() {
        let transport = NullTransport::new();
        let head = transport.fetch_head().await.unwrap();
        assert_eq!(head.remote_head, 0);
        assert!(head.state_root.is_none());
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
        let result = PushResult::accepted_only(0, 0);
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
            SyncEvent::new("a", "x", "1", json!({})).with_remote_sequence(2),
            SyncEvent::new("b", "x", "2", json!({})).with_remote_sequence(5),
            SyncEvent::new("c", "x", "3", json!({})).with_remote_sequence(4),
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

    #[test]
    fn derive_next_cursor_ignores_local_sequences_even_if_nonzero() {
        let events = vec![
            SyncEvent::new("a", "x", "1", json!({})).with_local_sequence(5),
            SyncEvent::new("b", "x", "2", json!({})).with_remote_sequence(3),
        ];
        assert_eq!(derive_next_cursor(0, &events), Some(3));
    }

    #[tokio::test]
    async fn default_pull_events_page_derives_cursor() {
        #[derive(Debug)]
        struct MockTransport;

        #[async_trait::async_trait]
        impl Transport for MockTransport {
            async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
                Ok(PushResult::accepted_only(events.len(), 10))
            }

            async fn pull_events(
                &self,
                since: u64,
                _limit: usize,
            ) -> Result<PullResult, SyncError> {
                if since == 0 {
                    Ok(PullResult {
                        events: vec![
                            SyncEvent::new("a", "x", "1", json!({})).with_remote_sequence(1),
                            SyncEvent::new("b", "x", "2", json!({})).with_remote_sequence(2),
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
        assert_eq!(page.observed_cursor, Some(2));
    }

    #[test]
    fn pull_page_serde_roundtrip() {
        let page = PullPage {
            result: PullResult {
                events: vec![SyncEvent::new("a", "b", "c", json!({})).with_remote_sequence(42)],
                remote_head: 100,
                has_more: true,
            },
            next_cursor: Some(43),
            observed_cursor: Some(42),
        };
        let json = serde_json::to_string(&page).unwrap();
        let de: PullPage = serde_json::from_str(&json).unwrap();
        assert_eq!(de.result.remote_head, 100);
        assert_eq!(de.next_cursor, Some(43));
        assert_eq!(de.observed_cursor, Some(42));
        assert_eq!(de.result.events.len(), 1);
    }

    proptest! {
        #[test]
        fn derive_next_cursor_matches_highest_sequence_above_since(
            since in 0u64..10_000,
            sequences in prop::collection::vec(0u64..12_000, 0..64),
        ) {
            let events: Vec<SyncEvent> = sequences
                .iter()
                .enumerate()
                .map(|(i, seq)| {
                    SyncEvent::new(
                        format!("evt-{i}"),
                        "entity",
                        format!("id-{i}"),
                        json!({ "i": i }),
                    )
                    .with_remote_sequence(*seq)
                })
                .collect();

            let expected = sequences.iter().copied().filter(|seq| *seq > since).max();
            prop_assert_eq!(derive_next_cursor(since, &events), expected);
        }
    }

    proptest! {
        #[test]
        fn derive_next_cursor_independent_of_event_order(
            since in 0u64..10_000,
            sequences in prop::collection::vec(0u64..12_000, 0..64),
        ) {
            let ordered: Vec<SyncEvent> = sequences
                .iter()
                .enumerate()
                .map(|(i, seq)| {
                    SyncEvent::new(
                        format!("ordered-{i}"),
                        "entity",
                        format!("ordered-id-{i}"),
                        json!({}),
                    )
                    .with_remote_sequence(*seq)
                })
                .collect();
            let reversed: Vec<SyncEvent> = sequences
                .iter()
                .rev()
                .enumerate()
                .map(|(i, seq)| {
                    SyncEvent::new(
                        format!("rev-{i}"),
                        "entity",
                        format!("rev-id-{i}"),
                        json!({}),
                    )
                    .with_remote_sequence(*seq)
                })
                .collect();

            prop_assert_eq!(
                derive_next_cursor(since, &ordered),
                derive_next_cursor(since, &reversed)
            );
        }
    }
}
