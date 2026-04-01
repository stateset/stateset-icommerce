//! Expanded integration tests for stateset-sync.
//!
//! Covers engine push/pull lifecycle with mock transports, buffer overflow,
//! outbox append/peek/retain/count, conflict resolution with realistic events,
//! transport error handling, and state management.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::{Duration, Utc};
use serde_json::json;
use uuid::Uuid;

use stateset_sync::{
    ConflictResolver, ConflictStrategy, EventBuffer, NullTransport, Outbox, PullResult,
    PushAcknowledgement, PushRejection, PushResult, Resolution, SyncConfig, SyncEngine, SyncError,
    SyncEvent, SyncState, Transport,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_event(event_type: &str, entity_type: &str, entity_id: &str) -> SyncEvent {
    SyncEvent::new(event_type, entity_type, entity_id, json!({"action": event_type}))
}

fn make_config() -> SyncConfig {
    SyncConfig::new("test-agent", "test-tenant", "test-store")
        .with_batch_size(10)
        .with_buffer_capacity(100)
        .with_outbox_capacity(100)
}

// ---------------------------------------------------------------------------
// Mock transports
// ---------------------------------------------------------------------------

/// Transport that records pushed events and returns configurable pull results.
#[derive(Debug)]
struct RecordingTransport {
    pushed: Arc<Mutex<Vec<Vec<SyncEvent>>>>,
    pull_results: Arc<Mutex<Vec<PullResult>>>,
    push_result_override: Arc<Mutex<Option<Result<PushResult, SyncError>>>>,
}

impl RecordingTransport {
    fn new() -> Self {
        Self {
            pushed: Arc::new(Mutex::new(Vec::new())),
            pull_results: Arc::new(Mutex::new(Vec::new())),
            push_result_override: Arc::new(Mutex::new(None)),
        }
    }

    fn with_pull_results(mut self, results: Vec<PullResult>) -> Self {
        self.pull_results = Arc::new(Mutex::new(results));
        self
    }

    fn pushed_batches(&self) -> Vec<Vec<SyncEvent>> {
        self.pushed.lock().unwrap().clone()
    }

    fn set_push_result(&self, result: Result<PushResult, SyncError>) {
        *self.push_result_override.lock().unwrap() = Some(result);
    }
}

#[async_trait]
impl Transport for RecordingTransport {
    async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
        self.pushed.lock().unwrap().push(events.to_vec());
        if let Some(override_result) = self.push_result_override.lock().unwrap().take() {
            return override_result;
        }
        let acks: Vec<PushAcknowledgement> = events
            .iter()
            .enumerate()
            .map(|(i, e)| PushAcknowledgement::new(e.id, (i as u64) + 100))
            .collect();
        let remote_head = acks.last().map_or(0, |a| a.remote_sequence);
        Ok(PushResult::accepted_only(events.len(), remote_head).with_acknowledgements(acks))
    }

    async fn pull_events(&self, _since: u64, _limit: usize) -> Result<PullResult, SyncError> {
        let mut results = self.pull_results.lock().unwrap();
        if results.is_empty() {
            Ok(PullResult { events: vec![], remote_head: 0, has_more: false })
        } else {
            Ok(results.remove(0))
        }
    }
}

/// Transport that always fails.
#[derive(Debug)]
struct FailingTransport;

#[async_trait]
impl Transport for FailingTransport {
    async fn push_events(&self, _events: &[SyncEvent]) -> Result<PushResult, SyncError> {
        Err(SyncError::Transport("network error".to_string()))
    }

    async fn pull_events(&self, _since: u64, _limit: usize) -> Result<PullResult, SyncError> {
        Err(SyncError::Transport("network error".to_string()))
    }
}

// ===========================================================================
// Engine push/pull lifecycle tests
// ===========================================================================

#[tokio::test]
async fn engine_record_and_push_lifecycle() {
    let config = make_config();
    let mut engine = SyncEngine::new(config).unwrap();

    // Record 3 events
    let s1 = engine.record(make_event("order.created", "order", "ORD-1")).unwrap();
    let s2 = engine.record(make_event("order.updated", "order", "ORD-1")).unwrap();
    let s3 = engine.record(make_event("inventory.adjusted", "inventory", "INV-1")).unwrap();
    assert_eq!(s1, 1);
    assert_eq!(s2, 2);
    assert_eq!(s3, 3);
    assert_eq!(engine.pending_count(), 3);

    // Push them
    let transport = RecordingTransport::new();
    let result = engine.push(&transport).await.unwrap();
    assert_eq!(result.accepted, 3);
    assert_eq!(engine.pending_count(), 0);
    assert!(engine.status().last_push.is_some());

    // Verify what was pushed
    let batches = transport.pushed_batches();
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].len(), 3);
    assert_eq!(batches[0][0].event_type, "order.created");
    assert_eq!(batches[0][2].event_type, "inventory.adjusted");
}

#[tokio::test]
async fn engine_push_empty_outbox_succeeds() {
    let config = make_config();
    let mut engine = SyncEngine::new(config).unwrap();
    let transport = RecordingTransport::new();
    let result = engine.push(&transport).await.unwrap();
    assert_eq!(result.accepted, 0);
}

#[tokio::test]
async fn engine_push_transport_error_preserves_outbox() {
    let config = make_config();
    let mut engine = SyncEngine::new(config).unwrap();
    engine.record(make_event("order.created", "order", "ORD-1")).unwrap();
    assert_eq!(engine.pending_count(), 1);

    let transport = FailingTransport;
    let result = engine.push(&transport).await;
    assert!(result.is_err());
    assert_eq!(engine.pending_count(), 1); // events not drained
}

#[tokio::test]
async fn engine_pull_populates_buffer_and_state() {
    let config = make_config();
    let mut engine = SyncEngine::new(config).unwrap();

    let remote_events = vec![
        make_event("order.created", "order", "ORD-10").with_remote_sequence(1),
        make_event("order.updated", "order", "ORD-10").with_remote_sequence(2),
    ];
    let transport = RecordingTransport::new().with_pull_results(vec![PullResult {
        events: remote_events,
        remote_head: 5,
        has_more: false,
    }]);

    let result = engine.pull(&transport).await.unwrap();
    assert_eq!(result.events.len(), 2);
    assert_eq!(result.remote_head, 5);
    assert!(engine.status().last_pull.is_some());

    let status = engine.status();
    assert_eq!(status.remote_head, 5);
    assert_eq!(status.buffered_events, 2);
}

#[tokio::test]
async fn engine_pull_transport_error_propagates() {
    let config = make_config();
    let mut engine = SyncEngine::new(config).unwrap();
    let transport = FailingTransport;
    let result = engine.pull(&transport).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn engine_push_with_rejections_creates_dead_letters() {
    let config = make_config();
    let mut engine = SyncEngine::new(config).unwrap();

    let event = make_event("order.created", "order", "ORD-1");
    let event_id = event.id;
    engine.record(event).unwrap();

    let transport = RecordingTransport::new();
    transport.set_push_result(Ok(PushResult::accepted_only(0, 50).with_rejections(vec![
        PushRejection::new(event_id)
            .with_code("invalid_payload")
            .with_reason("missing required field")
            .with_retryable(false),
    ])));

    let _result = engine.push(&transport).await.unwrap();
    assert_eq!(engine.dead_letter_count(), 1);
    assert_eq!(engine.pending_count(), 0);

    let dead_letter = engine.dead_letter_for_event(event_id).unwrap();
    assert_eq!(dead_letter.event.event_type, "order.created");
    assert_eq!(dead_letter.rejection.code.as_deref(), Some("invalid_payload"));
}

#[tokio::test]
async fn engine_push_with_acknowledgements_creates_confirmations() {
    let config = make_config();
    let mut engine = SyncEngine::new(config).unwrap();

    let event = make_event("order.created", "order", "ORD-1").with_command_id("cmd-1");
    let event_id = event.id;
    engine.record(event).unwrap();

    let transport = RecordingTransport::new();
    let result = engine.push(&transport).await.unwrap();
    assert_eq!(result.acknowledgements.len(), 1);
    assert_eq!(engine.confirmation_count(), 1);

    let conf = engine.confirmation_for_event(event_id).unwrap();
    assert_eq!(conf.event_type, "order.created");
    assert_eq!(conf.remote_sequence, 100);
    assert_eq!(conf.command_id.as_deref(), Some("cmd-1"));
}

#[tokio::test]
async fn engine_full_push_pull_cycle() {
    let config = make_config();
    let mut engine = SyncEngine::new(config).unwrap();

    // Record and push
    engine.record(make_event("order.created", "order", "ORD-1")).unwrap();
    let transport = RecordingTransport::new().with_pull_results(vec![PullResult {
        events: vec![make_event("payment.received", "payment", "PAY-1").with_remote_sequence(5)],
        remote_head: 5,
        has_more: false,
    }]);

    engine.push(&transport).await.unwrap();
    assert_eq!(engine.pending_count(), 0);

    // Pull remote events
    engine.pull(&transport).await.unwrap();
    let status = engine.status();
    assert_eq!(status.remote_head, 5);
    assert_eq!(status.buffered_events, 1);
    assert!(status.last_push.is_some());
    assert!(status.last_pull.is_some());
}

#[tokio::test]
async fn engine_status_reflects_current_state() {
    let config = make_config();
    let mut engine = SyncEngine::new(config).unwrap();

    let status = engine.status();
    assert!(status.initialized);
    assert_eq!(status.pending, 0);
    assert_eq!(status.dead_letters, 0);
    assert_eq!(status.lag, 0);
    assert!(status.caught_up);

    engine.record(make_event("order.created", "order", "ORD-1")).unwrap();
    let status = engine.status();
    assert_eq!(status.pending, 1);
    assert!(!status.caught_up);
}

// ===========================================================================
// Buffer overflow tests
// ===========================================================================

#[test]
fn buffer_overflow_evicts_oldest_fifo() {
    let mut buffer = EventBuffer::new(3);
    buffer.push(make_event("a", "x", "1"));
    buffer.push(make_event("b", "x", "2"));
    buffer.push(make_event("c", "x", "3"));
    assert!(buffer.is_full());

    let evicted = buffer.push(make_event("d", "x", "4"));
    assert_eq!(evicted.unwrap().event_type, "a");

    let evicted = buffer.push(make_event("e", "x", "5"));
    assert_eq!(evicted.unwrap().event_type, "b");

    let snapshot = buffer.snapshot();
    assert_eq!(snapshot.len(), 3);
    assert_eq!(snapshot[0].event_type, "c");
    assert_eq!(snapshot[1].event_type, "d");
    assert_eq!(snapshot[2].event_type, "e");
}

#[test]
fn buffer_capacity_one_cycles_correctly() {
    let mut buffer = EventBuffer::new(1);
    let e1 = buffer.push(make_event("a", "x", "1"));
    assert!(e1.is_none());
    assert_eq!(buffer.len(), 1);

    let e2 = buffer.push(make_event("b", "x", "2"));
    assert_eq!(e2.unwrap().event_type, "a");
    assert_eq!(buffer.len(), 1);
    assert_eq!(buffer.snapshot()[0].event_type, "b");
}

#[test]
fn buffer_drain_then_refill() {
    let mut buffer = EventBuffer::new(3);
    buffer.push(make_event("a", "x", "1"));
    buffer.push(make_event("b", "x", "2"));

    let drained = buffer.drain_all();
    assert_eq!(drained.len(), 2);
    assert!(buffer.is_empty());

    buffer.push(make_event("c", "x", "3"));
    assert_eq!(buffer.len(), 1);
    assert_eq!(buffer.snapshot()[0].event_type, "c");
}

#[test]
fn buffer_recent_after_overflow() {
    let mut buffer = EventBuffer::new(3);
    for i in 0..10 {
        buffer.push(make_event(&format!("evt-{i}"), "x", &format!("{i}")));
    }
    let recent = buffer.recent(2);
    assert_eq!(recent.len(), 2);
    assert_eq!(recent[0].event_type, "evt-8");
    assert_eq!(recent[1].event_type, "evt-9");
}

#[test]
fn buffer_snapshot_does_not_drain() {
    let mut buffer = EventBuffer::new(5);
    buffer.push(make_event("a", "x", "1"));
    buffer.push(make_event("b", "x", "2"));

    let snap1 = buffer.snapshot();
    let snap2 = buffer.snapshot();
    assert_eq!(snap1.len(), snap2.len());
    assert_eq!(buffer.len(), 2);
}

// ===========================================================================
// Outbox append/peek/retain/count tests
// ===========================================================================

#[test]
fn outbox_append_assigns_monotonic_sequences() {
    let mut outbox = Outbox::new(100);
    let sequences: Vec<u64> = (0..5)
        .map(|i| outbox.append(make_event(&format!("evt-{i}"), "order", "ORD-1")).unwrap())
        .collect();
    assert_eq!(sequences, vec![1, 2, 3, 4, 5]);
}

#[test]
fn outbox_peek_returns_fifo_without_consuming() {
    let mut outbox = Outbox::new(10);
    outbox.append(make_event("a", "order", "ORD-1")).unwrap();
    outbox.append(make_event("b", "order", "ORD-2")).unwrap();
    outbox.append(make_event("c", "order", "ORD-3")).unwrap();

    let peeked = outbox.peek(2);
    assert_eq!(peeked.len(), 2);
    assert_eq!(peeked[0].event_type, "a");
    assert_eq!(peeked[1].event_type, "b");
    assert_eq!(outbox.count(), 3); // nothing consumed
}

#[test]
fn outbox_retain_removes_matching_events() {
    let mut outbox = Outbox::new(10);
    outbox.append(make_event("keep", "order", "ORD-1")).unwrap();
    outbox.append(make_event("drop", "order", "ORD-2")).unwrap();
    outbox.append(make_event("keep", "order", "ORD-3")).unwrap();
    outbox.append(make_event("drop", "order", "ORD-4")).unwrap();

    outbox.retain(|e| e.event_type == "keep");
    assert_eq!(outbox.count(), 2);

    let remaining = outbox.peek(10);
    assert!(remaining.iter().all(|e| e.event_type == "keep"));
}

#[test]
fn outbox_drain_partial_preserves_remainder() {
    let mut outbox = Outbox::new(10);
    outbox.append(make_event("a", "order", "ORD-1")).unwrap();
    outbox.append(make_event("b", "order", "ORD-2")).unwrap();
    outbox.append(make_event("c", "order", "ORD-3")).unwrap();

    let drained = outbox.drain(2).unwrap();
    assert_eq!(drained.len(), 2);
    assert_eq!(drained[0].event_type, "a");
    assert_eq!(drained[1].event_type, "b");
    assert_eq!(outbox.count(), 1);
    assert_eq!(outbox.peek(1)[0].event_type, "c");
}

#[test]
fn outbox_full_rejects_append() {
    let mut outbox = Outbox::new(2);
    outbox.append(make_event("a", "order", "ORD-1")).unwrap();
    outbox.append(make_event("b", "order", "ORD-2")).unwrap();

    let result = outbox.append(make_event("c", "order", "ORD-3"));
    assert!(matches!(result, Err(SyncError::OutboxFull { capacity: 2, current: 2 })));
}

#[test]
fn outbox_contains_event_id() {
    let mut outbox = Outbox::new(10);
    let event = make_event("test", "order", "ORD-1");
    let id = event.id;
    outbox.append(event).unwrap();

    assert!(outbox.contains_event_id(id));
    assert!(!outbox.contains_event_id(Uuid::new_v4()));
}

#[test]
fn outbox_clear_preserves_sequence_counter() {
    let mut outbox = Outbox::new(10);
    outbox.append(make_event("a", "order", "ORD-1")).unwrap();
    outbox.append(make_event("b", "order", "ORD-2")).unwrap();
    assert_eq!(outbox.next_sequence(), 3);

    outbox.clear();
    assert!(outbox.is_empty());
    assert_eq!(outbox.next_sequence(), 3);

    let seq = outbox.append(make_event("c", "order", "ORD-3")).unwrap();
    assert_eq!(seq, 3);
}

#[test]
fn outbox_rejects_canonical_remote_events() {
    let mut outbox = Outbox::new(10);
    let event = make_event("remote", "order", "ORD-1").with_remote_sequence(42);
    let result = outbox.append(event);
    assert!(matches!(result, Err(SyncError::InvalidEvent(_))));
}

// ===========================================================================
// Conflict resolution tests with realistic events
// ===========================================================================

#[test]
fn conflict_remote_wins_discards_local_order_update() {
    let resolver = ConflictResolver::new(ConflictStrategy::RemoteWins);
    let local = make_event("order.updated", "order", "ORD-1");
    let remote = make_event("order.cancelled", "order", "ORD-1");
    assert_eq!(resolver.resolve(&local, &remote), Resolution::KeepRemote);
}

#[test]
fn conflict_local_wins_preserves_local_inventory_adjustment() {
    let resolver = ConflictResolver::new(ConflictStrategy::LocalWins);
    let local = make_event("inventory.adjusted", "inventory", "INV-1");
    let remote = make_event("inventory.adjusted", "inventory", "INV-1");
    assert_eq!(resolver.resolve(&local, &remote), Resolution::KeepLocal);
}

#[test]
fn conflict_last_writer_wins_with_timestamps() {
    let resolver = ConflictResolver::new(ConflictStrategy::LastWriterWins);
    let now = Utc::now();

    let local = SyncEvent::with_id(
        Uuid::new_v4(),
        0,
        "order.updated",
        "order",
        "ORD-1",
        json!({"status": "shipped"}),
        now + Duration::seconds(5),
    );
    let remote = SyncEvent::with_id(
        Uuid::new_v4(),
        0,
        "order.updated",
        "order",
        "ORD-1",
        json!({"status": "cancelled"}),
        now - Duration::seconds(5),
    );

    // Local is newer -> keep local
    assert_eq!(resolver.resolve(&local, &remote), Resolution::KeepLocal);

    // Swap: remote is newer -> keep remote
    assert_eq!(resolver.resolve(&remote, &local), Resolution::KeepRemote);
}

#[test]
fn conflict_batch_resolution_mixed_strategies() {
    let resolver = ConflictResolver::new(ConflictStrategy::RemoteWins);

    let pairs: Vec<(SyncEvent, SyncEvent)> = (0..5)
        .map(|i| {
            let local = make_event(&format!("local-{i}"), "order", &format!("ORD-{i}"));
            let remote = make_event(&format!("remote-{i}"), "order", &format!("ORD-{i}"));
            (local, remote)
        })
        .collect();

    let refs: Vec<(&SyncEvent, &SyncEvent)> = pairs.iter().map(|(l, r)| (l, r)).collect();
    let resolutions = resolver.resolve_batch(&refs);

    assert_eq!(resolutions.len(), 5);
    assert!(resolutions.iter().all(|r| *r == Resolution::KeepRemote));
}

#[test]
fn conflict_merge_variant_works() {
    let merged = make_event("order.merged", "order", "ORD-1");
    let resolution = Resolution::Merge(Box::new(merged));

    match resolution {
        Resolution::Merge(event) => assert_eq!(event.event_type, "order.merged"),
        _ => panic!("expected Merge variant"),
    }
}

#[test]
fn conflict_strategy_serde_roundtrip_all_variants() {
    for strategy in [
        ConflictStrategy::RemoteWins,
        ConflictStrategy::LocalWins,
        ConflictStrategy::LastWriterWins,
    ] {
        let serialized = serde_json::to_string(&strategy).unwrap();
        let deserialized: ConflictStrategy = serde_json::from_str(&serialized).unwrap();
        assert_eq!(strategy, deserialized);
    }
}

// ===========================================================================
// Transport error handling tests
// ===========================================================================

#[tokio::test]
async fn null_transport_push_and_pull() {
    let transport = NullTransport::new();
    let events = vec![make_event("a", "x", "1"), make_event("b", "x", "2")];

    let push_result = transport.push_events(&events).await.unwrap();
    assert_eq!(push_result.accepted, 2);
    assert_eq!(push_result.remote_head, 0);

    let pull_result = transport.pull_events(0, 100).await.unwrap();
    assert!(pull_result.events.is_empty());
    assert!(!pull_result.has_more);

    let head = transport.fetch_head().await.unwrap();
    assert_eq!(head.remote_head, 0);
}

#[tokio::test]
async fn engine_handles_transport_push_failure_gracefully() {
    let config = make_config();
    let mut engine = SyncEngine::new(config).unwrap();
    engine.record(make_event("order.created", "order", "ORD-1")).unwrap();
    engine.record(make_event("order.updated", "order", "ORD-1")).unwrap();

    let failing = FailingTransport;
    let err = engine.push(&failing).await.unwrap_err();
    assert!(matches!(err, SyncError::Transport(_)));
    assert_eq!(engine.pending_count(), 2); // events preserved
}

#[tokio::test]
async fn engine_handles_transport_pull_failure_gracefully() {
    let config = make_config();
    let mut engine = SyncEngine::new(config).unwrap();

    let failing = FailingTransport;
    let err = engine.pull(&failing).await.unwrap_err();
    assert!(matches!(err, SyncError::Transport(_)));
    assert!(engine.status().last_pull.is_none());
}

// ===========================================================================
// State management tests
// ===========================================================================

#[test]
fn sync_state_lag_calculation() {
    let state = SyncState { remote_head: 100, remote_cursor: 75, ..Default::default() };
    assert_eq!(state.lag(), 25);
}

#[test]
fn sync_state_is_synced_when_no_pending_and_caught_up() {
    let state =
        SyncState { remote_head: 50, remote_cursor: 50, pending_count: 0, ..Default::default() };
    assert!(state.is_synced());
}

#[test]
fn sync_state_not_synced_with_lag() {
    let state =
        SyncState { remote_head: 50, remote_cursor: 30, pending_count: 0, ..Default::default() };
    assert!(!state.is_synced());
    assert_eq!(state.lag(), 20);
}

#[test]
fn sync_state_not_synced_with_pending() {
    let state =
        SyncState { remote_head: 50, remote_cursor: 50, pending_count: 3, ..Default::default() };
    assert!(!state.is_synced());
}

#[test]
fn sync_state_serde_roundtrip() {
    let state = SyncState {
        local_head: 42,
        remote_head: 100,
        remote_state_root: Some("state-root-abc".into()),
        last_commitment_id: Some("BATCH-99".into()),
        remote_cursor: 95,
        last_acknowledged_remote_sequence: Some(88),
        last_push: Some(Utc::now()),
        last_pull: Some(Utc::now()),
        pending_count: 5,
    };
    let json = serde_json::to_string(&state).unwrap();
    let deserialized: SyncState = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.local_head, state.local_head);
    assert_eq!(deserialized.remote_head, state.remote_head);
    assert_eq!(deserialized.remote_state_root, state.remote_state_root);
    assert_eq!(deserialized.remote_cursor, state.remote_cursor);
    assert_eq!(
        deserialized.last_acknowledged_remote_sequence,
        state.last_acknowledged_remote_sequence
    );
    assert_eq!(deserialized.pending_count, state.pending_count);
}

#[tokio::test]
async fn engine_state_updates_after_push() {
    let config = make_config();
    let mut engine = SyncEngine::new(config).unwrap();

    engine.record(make_event("order.created", "order", "ORD-1")).unwrap();
    let transport = RecordingTransport::new();
    engine.push(&transport).await.unwrap();

    let status = engine.status();
    assert_eq!(status.pending, 0);
    assert_eq!(status.local_head, 1);
    assert!(status.remote_head > 0);
}

#[tokio::test]
async fn engine_state_updates_after_pull() {
    let config = make_config();
    let mut engine = SyncEngine::new(config).unwrap();

    let transport = RecordingTransport::new().with_pull_results(vec![PullResult {
        events: vec![make_event("order.created", "order", "ORD-1").with_remote_sequence(10)],
        remote_head: 15,
        has_more: false,
    }]);

    engine.pull(&transport).await.unwrap();

    let status = engine.status();
    assert_eq!(status.remote_head, 15);
    assert_eq!(status.remote_cursor, 10);
    assert_eq!(status.buffered_events, 1);
}

#[tokio::test]
async fn engine_requeue_dead_letter() {
    let config = make_config();
    let mut engine = SyncEngine::new(config).unwrap();

    let event = make_event("order.created", "order", "ORD-1");
    let event_id = event.id;
    engine.record(event).unwrap();

    let transport = RecordingTransport::new();
    transport.set_push_result(Ok(PushResult::accepted_only(0, 50)
        .with_rejections(vec![PushRejection::new(event_id).with_retryable(false)])));

    engine.push(&transport).await.unwrap();
    assert_eq!(engine.dead_letter_count(), 1);
    assert_eq!(engine.pending_count(), 0);

    let seq = engine.requeue_dead_letter(event_id).unwrap();
    assert!(seq > 0);
    assert_eq!(engine.dead_letter_count(), 0);
    assert_eq!(engine.pending_count(), 1);
}

// ===========================================================================
// Engine with custom conflict strategy
// ===========================================================================

#[tokio::test]
async fn engine_pull_with_local_wins_strategy_keeps_local() {
    let config = make_config();
    let mut engine = SyncEngine::with_strategy(config, ConflictStrategy::LocalWins).unwrap();

    // Record a local event on the same entity
    engine.record(make_event("order.updated", "order", "ORD-1")).unwrap();
    assert_eq!(engine.pending_count(), 1);

    // Pull a remote event for the same entity
    let transport = RecordingTransport::new().with_pull_results(vec![PullResult {
        events: vec![make_event("order.cancelled", "order", "ORD-1").with_remote_sequence(5)],
        remote_head: 5,
        has_more: false,
    }]);

    engine.pull(&transport).await.unwrap();

    // With LocalWins, local event should stay and remote dropped
    assert_eq!(engine.pending_count(), 1);
}

#[tokio::test]
async fn engine_pull_with_remote_wins_strategy_drops_local() {
    let config = make_config();
    let mut engine = SyncEngine::with_strategy(config, ConflictStrategy::RemoteWins).unwrap();

    engine.record(make_event("order.updated", "order", "ORD-1")).unwrap();
    assert_eq!(engine.pending_count(), 1);

    let transport = RecordingTransport::new().with_pull_results(vec![PullResult {
        events: vec![make_event("order.cancelled", "order", "ORD-1").with_remote_sequence(5)],
        remote_head: 5,
        has_more: false,
    }]);

    engine.pull(&transport).await.unwrap();

    // With RemoteWins, local event should be dropped
    assert_eq!(engine.pending_count(), 0);
}

// ===========================================================================
// Persistent outbox roundtrip
// ===========================================================================

#[test]
fn persistent_outbox_survives_drop_and_reload() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("outbox.json");

    {
        let mut outbox = Outbox::with_persistence(10, &path).unwrap();
        outbox.append(make_event("a", "order", "ORD-1")).unwrap();
        outbox.append(make_event("b", "order", "ORD-2")).unwrap();
        outbox.append(make_event("c", "order", "ORD-3")).unwrap();
        assert_eq!(outbox.count(), 3);
    }

    let outbox = Outbox::with_persistence(10, &path).unwrap();
    assert_eq!(outbox.count(), 3);
    assert_eq!(outbox.next_sequence(), 4);

    let peeked = outbox.peek(10);
    assert_eq!(peeked[0].event_type, "a");
    assert_eq!(peeked[1].event_type, "b");
    assert_eq!(peeked[2].event_type, "c");
}
