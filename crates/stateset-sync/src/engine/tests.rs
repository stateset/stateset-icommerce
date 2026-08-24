use super::*;
use crate::transport::NullTransport;
use proptest::prelude::*;
use serde_json::json;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use tempfile::tempdir;

fn make_config() -> SyncConfig {
    SyncConfig::new("agent-1", "tenant-1", "store-1")
}

fn make_event(event_type: &str) -> SyncEvent {
    SyncEvent::new(event_type, "order", "ORD-1", json!({}))
}

#[test]
fn new_engine() {
    let engine = SyncEngine::new(make_config()).unwrap();
    assert_eq!(engine.pending_count(), 0);
    assert_eq!(engine.buffered_count(), 0);
    assert!(engine.status().initialized);
}

#[test]
fn record_event() {
    let mut engine = SyncEngine::new(make_config()).unwrap();
    let seq = engine.record(make_event("order.created")).unwrap();
    assert_eq!(seq, 1);
    assert_eq!(engine.pending_count(), 1);
    assert_eq!(engine.state().local_head, 1);
}

#[test]
fn record_multiple_events() {
    let mut engine = SyncEngine::new(make_config()).unwrap();
    engine.record(make_event("a")).unwrap();
    engine.record(make_event("b")).unwrap();
    engine.record(make_event("c")).unwrap();
    assert_eq!(engine.pending_count(), 3);
    assert_eq!(engine.state().local_head, 3);
}

#[test]
fn record_kernel_transaction_attaches_policy_budget_and_returns_pending_receipt() {
    let mut engine = SyncEngine::new(make_config()).unwrap();
    let receipt = engine
        .record_kernel_transaction(
            crate::kernel::KernelTransaction::new(SyncEvent::new(
                "order.created",
                "order",
                "ORD-1",
                json!({"total": 99}),
            ))
            .with_policy_checkpoint(crate::event::PolicyCheckpoint::new(
                "orders",
                crate::event::PolicyDecision::Allowed,
            ))
            .with_budget_authorization(crate::kernel::BudgetAuthorization::new(
                "budget-1", 9900, 10000, "USD",
            )),
        )
        .unwrap();

    assert_eq!(receipt.status, KernelReceiptStatus::LocalPending);
    assert_eq!(receipt.local_sequence, Some(1));
    assert_eq!(
        receipt
            .kernel
            .as_ref()
            .and_then(|kernel| kernel.policy.as_ref())
            .map(|policy| policy.domain.as_str()),
        Some("orders")
    );
    assert_eq!(
        receipt
            .kernel
            .as_ref()
            .and_then(|kernel| kernel.budget.as_ref())
            .map(|budget| budget.remaining_amount_minor),
        Some(Some(100))
    );
}

#[test]
fn record_kernel_transaction_rejects_denied_policy() {
    let mut engine = SyncEngine::new(make_config()).unwrap();
    let error = engine
        .record_kernel_transaction(
            crate::kernel::KernelTransaction::new(SyncEvent::new(
                "order.created",
                "order",
                "ORD-1",
                json!({}),
            ))
            .with_policy_checkpoint(
                crate::event::PolicyCheckpoint::new("orders", crate::event::PolicyDecision::Denied)
                    .with_reason("blocked"),
            ),
        )
        .unwrap_err();

    assert_eq!(
        error,
        crate::kernel::KernelExecutionError::PolicyDenied {
            domain: "orders".into(),
            reason: Some("blocked".into())
        }
    );
    assert_eq!(engine.pending_count(), 0);
}

#[test]
fn record_kernel_transaction_rejects_budget_overrun() {
    let mut engine = SyncEngine::new(make_config()).unwrap();
    let error = engine
        .record_kernel_transaction(
            crate::kernel::KernelTransaction::new(SyncEvent::new(
                "order.created",
                "order",
                "ORD-1",
                json!({}),
            ))
            .with_budget_authorization(crate::kernel::BudgetAuthorization::new(
                "budget-1", 150, 100, "USD",
            )),
        )
        .unwrap_err();

    assert_eq!(
        error,
        crate::kernel::KernelExecutionError::BudgetExceeded {
            budget_id: "budget-1".into(),
            requested_amount_minor: 150,
            available_amount_minor: 100,
            currency: "USD".into()
        }
    );
    assert_eq!(engine.pending_count(), 0);
}

#[tokio::test]
async fn push_with_null_transport() {
    let mut engine = SyncEngine::new(make_config()).unwrap();
    engine.record(make_event("a")).unwrap();
    engine.record(make_event("b")).unwrap();

    let transport = NullTransport::new();
    let result = engine.push(&transport).await.unwrap();
    assert_eq!(result.accepted, 2);
    assert_eq!(engine.pending_count(), 0);
    assert!(engine.state().last_push.is_some());
}

#[tokio::test]
async fn push_empty_outbox() {
    let mut engine = SyncEngine::new(make_config()).unwrap();
    let transport = NullTransport::new();
    let result = engine.push(&transport).await.unwrap();
    assert_eq!(result.accepted, 0);
}

#[tokio::test]
async fn pull_with_null_transport() {
    let mut engine = SyncEngine::new(make_config()).unwrap();
    let transport = NullTransport::new();
    let result = engine.pull(&transport).await.unwrap();
    assert!(result.events.is_empty());
    assert!(!result.has_more);
    assert!(engine.state().last_pull.is_some());
}

#[tokio::test]
async fn pull_buffers_events() {
    /// Mock transport that returns predefined events on pull.
    #[derive(Debug)]
    struct MockPullTransport {
        events: Vec<SyncEvent>,
        head: u64,
    }

    #[async_trait::async_trait]
    impl Transport for MockPullTransport {
        async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
            Ok(PushResult::accepted_only(events.len(), self.head))
        }
        async fn pull_events(&self, _since: u64, _limit: usize) -> Result<PullResult, SyncError> {
            Ok(PullResult { events: self.events.clone(), remote_head: self.head, has_more: false })
        }
    }

    let transport = MockPullTransport {
        events: vec![
            make_event("pulled-1").with_remote_sequence(1),
            make_event("pulled-2").with_remote_sequence(2),
        ],
        head: 2,
    };

    let mut engine = SyncEngine::new(make_config()).unwrap();
    let result = engine.pull(&transport).await.unwrap();
    assert_eq!(result.events.len(), 2);
    assert_eq!(engine.buffered_count(), 2);
    assert_eq!(engine.state().remote_head, 2);
    assert_eq!(engine.state().remote_cursor, 2);
}

#[tokio::test]
async fn full_sync() {
    let mut engine = SyncEngine::new(make_config()).unwrap();
    engine.record(make_event("local")).unwrap();

    let transport = NullTransport::new();
    let (push_result, pull_result) = engine.full_sync(&transport).await.unwrap();
    assert_eq!(push_result.accepted, 1);
    assert!(pull_result.events.is_empty());
    assert_eq!(engine.pending_count(), 0);
}

#[tokio::test]
async fn refresh_remote_head_updates_known_head_without_advancing_cursor() {
    #[derive(Debug)]
    struct HeadTransport;

    #[async_trait::async_trait]
    impl Transport for HeadTransport {
        async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
            Ok(PushResult::accepted_only(events.len(), 0))
        }

        async fn pull_events(&self, _since: u64, _limit: usize) -> Result<PullResult, SyncError> {
            Ok(PullResult { events: vec![], remote_head: 0, has_more: false })
        }

        async fn fetch_head(&self) -> Result<RemoteHead, SyncError> {
            Ok(RemoteHead::new(15).with_state_root("root-15").with_last_commitment_id("BATCH-15"))
        }
    }

    // This test exercises bare (manifest-absent) metadata, which the
    // default fail-closed policy rejects; opt out explicitly.
    let mut engine =
        SyncEngine::new(make_config().with_unauthenticated_remote_head_allowed()).unwrap();
    engine.record(make_event("a")).unwrap();

    let head = engine.refresh_remote_head(&HeadTransport).await.unwrap();
    assert_eq!(head.remote_head, 15);
    assert_eq!(head.state_root.as_deref(), Some("root-15"));
    assert_eq!(head.last_commitment_id.as_deref(), Some("BATCH-15"));
    assert_eq!(engine.state().remote_head, 15);
    assert_eq!(engine.state().remote_state_root.as_deref(), Some("root-15"));
    assert_eq!(engine.state().last_commitment_id.as_deref(), Some("BATCH-15"));
    assert_eq!(engine.state().remote_cursor, 0);
    assert_eq!(engine.state().lag(), 15);
}

#[tokio::test]
async fn refresh_remote_head_ignores_stale_metadata_from_lower_head() {
    #[derive(Debug)]
    struct StaleHeadTransport;

    #[async_trait::async_trait]
    impl Transport for StaleHeadTransport {
        async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
            Ok(PushResult::accepted_only(events.len(), 0))
        }

        async fn pull_events(&self, _since: u64, _limit: usize) -> Result<PullResult, SyncError> {
            Ok(PullResult { events: vec![], remote_head: 0, has_more: false })
        }

        async fn fetch_head(&self) -> Result<RemoteHead, SyncError> {
            Ok(RemoteHead::new(10)
                .with_state_root("stale-root")
                .with_last_commitment_id("STALE-BATCH"))
        }
    }

    let mut engine = SyncEngine::new(make_config()).unwrap();
    engine.state.remote_head = 12;
    engine.state.remote_state_root = Some("root-12".into());
    engine.state.last_commitment_id = Some("BATCH-12".into());

    let head = engine.refresh_remote_head(&StaleHeadTransport).await.unwrap();
    assert_eq!(head.remote_head, 12);
    assert_eq!(head.state_root.as_deref(), Some("root-12"));
    assert_eq!(head.last_commitment_id.as_deref(), Some("BATCH-12"));
    assert_eq!(engine.state().remote_head, 12);
    assert_eq!(engine.state().remote_state_root.as_deref(), Some("root-12"));
    assert_eq!(engine.state().last_commitment_id.as_deref(), Some("BATCH-12"));
}

#[tokio::test]
async fn refresh_remote_head_persists_state_snapshot() {
    #[derive(Debug)]
    struct HeadTransport;

    #[async_trait::async_trait]
    impl Transport for HeadTransport {
        async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
            Ok(PushResult::accepted_only(events.len(), 0))
        }

        async fn pull_events(&self, _since: u64, _limit: usize) -> Result<PullResult, SyncError> {
            Ok(PullResult { events: vec![], remote_head: 0, has_more: false })
        }

        async fn fetch_head(&self) -> Result<RemoteHead, SyncError> {
            Ok(RemoteHead::new(21).with_state_root("root-21").with_last_commitment_id("BATCH-21"))
        }
    }

    let dir = tempdir().unwrap();
    let state_path = dir.path().join("sync-state.json");
    // Bare metadata (no manifest): opt out of the fail-closed default.
    let config = make_config()
        .with_state_path(state_path.to_string_lossy().into_owned())
        .with_unauthenticated_remote_head_allowed();

    {
        let mut engine = SyncEngine::new(config.clone()).unwrap();
        engine.refresh_remote_head(&HeadTransport).await.unwrap();
        assert_eq!(engine.state().remote_head, 21);
        assert_eq!(engine.state().remote_state_root.as_deref(), Some("root-21"));
        assert_eq!(engine.state().last_commitment_id.as_deref(), Some("BATCH-21"));
    }

    let engine = SyncEngine::new(config).unwrap();
    assert_eq!(engine.state().remote_head, 21);
    assert_eq!(engine.state().remote_state_root.as_deref(), Some("root-21"));
    assert_eq!(engine.state().last_commitment_id.as_deref(), Some("BATCH-21"));
}

#[tokio::test]
async fn refresh_remote_head_imports_verified_commitment_manifest() {
    let state_root = "30".repeat(32);

    #[derive(Debug)]
    struct HeadTransport {
        manifest: CommitmentManifest,
        state_root: String,
    }

    #[async_trait::async_trait]
    impl Transport for HeadTransport {
        async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
            Ok(PushResult::accepted_only(events.len(), 0))
        }

        async fn pull_events(&self, _since: u64, _limit: usize) -> Result<PullResult, SyncError> {
            Ok(PullResult { events: vec![], remote_head: 0, has_more: false })
        }

        async fn fetch_head(&self) -> Result<RemoteHead, SyncError> {
            Ok(RemoteHead::new(30)
                .with_commitment_manifest(self.manifest.clone())
                .with_state_root(self.state_root.clone())
                .with_last_commitment_id("BATCH-30"))
        }
    }

    let (private_key, public_key) = stateset_crypto::sign::generate_keypair();
    let manifest = crate::commitment::sign_commitment_manifest(
        crate::commitment::CommitmentManifest::new(
            "BATCH-30",
            state_root.clone(),
            30,
            "sequencer-a",
        ),
        &private_key,
        &public_key,
    )
    .unwrap();

    let mut engine = SyncEngine::new(
        make_config()
            .with_require_commitment_manifest(true)
            .with_trusted_commitment_signer("sequencer-a")
            .with_trusted_commitment_signer_public_key(hex::encode(public_key)),
    )
    .unwrap();
    let head = engine.refresh_remote_head(&HeadTransport { manifest, state_root }).await.unwrap();

    assert_eq!(head.remote_head, 30);
    assert_eq!(
        head.commitment_manifest.as_ref().map(|manifest| manifest.signer_id.as_str()),
        Some("sequencer-a")
    );
    assert_eq!(engine.state().remote_head, 30);
    assert_eq!(engine.state().last_commitment_id.as_deref(), Some("BATCH-30"));
    assert_eq!(
        engine.verified_commitment_manifest("BATCH-30").map(|manifest| manifest.signer_id.as_str()),
        Some("sequencer-a")
    );
}

#[tokio::test]
async fn refresh_remote_head_rejects_untrusted_commitment_manifest_signer() {
    let state_root = "31".repeat(32);

    #[derive(Debug)]
    struct HeadTransport {
        manifest: CommitmentManifest,
        state_root: String,
    }

    #[async_trait::async_trait]
    impl Transport for HeadTransport {
        async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
            Ok(PushResult::accepted_only(events.len(), 0))
        }

        async fn pull_events(&self, _since: u64, _limit: usize) -> Result<PullResult, SyncError> {
            Ok(PullResult { events: vec![], remote_head: 0, has_more: false })
        }

        async fn fetch_head(&self) -> Result<RemoteHead, SyncError> {
            Ok(RemoteHead::new(31)
                .with_commitment_manifest(self.manifest.clone())
                .with_state_root(self.state_root.clone())
                .with_last_commitment_id("BATCH-31"))
        }
    }

    let (private_key, public_key) = stateset_crypto::sign::generate_keypair();
    let manifest = crate::commitment::sign_commitment_manifest(
        crate::commitment::CommitmentManifest::new(
            "BATCH-31",
            state_root.clone(),
            31,
            "sequencer-a",
        ),
        &private_key,
        &public_key,
    )
    .unwrap();

    let mut engine = SyncEngine::new(
        make_config()
            .with_require_commitment_manifest(true)
            .with_trusted_commitment_signer("sequencer-b")
            .with_trusted_commitment_signer_public_key(hex::encode(public_key)),
    )
    .unwrap();
    let error =
        engine.refresh_remote_head(&HeadTransport { manifest, state_root }).await.unwrap_err();

    assert!(matches!(error, SyncError::Trust(_)));
    assert_eq!(engine.state().remote_head, 0);
    assert!(engine.verified_commitment_manifests().is_empty());
}

#[derive(Debug)]
struct BareMetadataTransport;

#[async_trait::async_trait]
impl Transport for BareMetadataTransport {
    async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
        Ok(PushResult::accepted_only(events.len(), 0))
    }

    async fn pull_events(&self, _since: u64, _limit: usize) -> Result<PullResult, SyncError> {
        Ok(PullResult { events: vec![], remote_head: 0, has_more: false })
    }

    async fn fetch_head(&self) -> Result<RemoteHead, SyncError> {
        Ok(RemoteHead::new(40).with_state_root("40".repeat(32)).with_last_commitment_id("BATCH-40"))
    }
}

#[tokio::test]
async fn refresh_remote_head_rejects_unauthenticated_metadata_by_default() {
    let mut engine = SyncEngine::new(make_config()).unwrap();

    let error = engine.refresh_remote_head(&BareMetadataTransport).await.unwrap_err();

    assert!(
        matches!(error, SyncError::Trust(_)),
        "expected SyncError::Trust for unsigned remote metadata, got: {error:?}"
    );
    // Unauthenticated metadata must not leak into engine state.
    assert_eq!(engine.state().remote_head, 0);
    assert_eq!(engine.state().remote_state_root, None);
    assert_eq!(engine.state().last_commitment_id, None);
}

#[tokio::test]
async fn refresh_remote_head_allows_unauthenticated_metadata_only_with_opt_out() {
    let mut engine =
        SyncEngine::new(make_config().with_unauthenticated_remote_head_allowed()).unwrap();

    let head = engine.refresh_remote_head(&BareMetadataTransport).await.unwrap();

    assert_eq!(head.remote_head, 40);
    assert_eq!(head.state_root.as_deref(), Some(&*"40".repeat(32)));
    assert_eq!(engine.state().remote_head, 40);
    assert_eq!(engine.state().last_commitment_id.as_deref(), Some("BATCH-40"));
}

#[test]
fn status_reporting() {
    let mut engine = SyncEngine::new(make_config()).unwrap();
    engine.record(make_event("a")).unwrap();
    engine.record(make_event("b")).unwrap();

    let status = engine.status();
    assert!(status.initialized);
    assert_eq!(status.pending, 2);
    assert_eq!(status.dead_letters, 0);
    assert_eq!(status.retained_confirmations, 0);
    assert_eq!(status.local_head, 2);
    assert_eq!(status.remote_head, 0);
    assert_eq!(status.remote_state_root, None);
    assert_eq!(status.last_commitment_id, None);
    assert_eq!(status.remote_cursor, 0);
    assert_eq!(status.next_pull_cursor, None);
    assert_eq!(status.last_acknowledged_remote_sequence, None);
    assert_eq!(status.lag, 0);
    assert!(!status.caught_up);
    assert!(status.last_push.is_none());
}

#[test]
fn status_reports_continuation_cursor_when_pull_is_in_progress() {
    let mut engine = SyncEngine::new(make_config()).unwrap();
    engine.state.remote_head = 10;
    engine.state.remote_cursor = 8;
    engine.next_pull_cursor = Some(9);

    let status = engine.status();
    assert_eq!(status.next_pull_cursor, Some(9));
    assert!(!status.caught_up);

    engine.state.remote_cursor = 10;
    engine.next_pull_cursor = None;
    let status = engine.status();
    assert!(status.caught_up);
}

#[test]
fn drain_confirmations_clears_engine_log() {
    let mut engine = SyncEngine::new(make_config()).unwrap();
    let event = make_event("confirmed").with_command_id("cmd-1");
    let acknowledgement =
        crate::transport::PushAcknowledgement::new(event.id, 42).with_receipt("receipt-42");
    engine.confirmations.push(PushConfirmation::from_ack(&event, &acknowledgement));

    let drained = engine.drain_confirmations().unwrap();
    assert_eq!(drained.len(), 1);
    assert_eq!(drained[0].event_id, event.id);
    assert_eq!(drained[0].receipt.as_deref(), Some("receipt-42"));
    assert_eq!(engine.confirmation_count(), 0);
}

#[test]
fn confirmation_lookup_supports_event_sequence_and_receipt_queries() {
    let mut engine = SyncEngine::new(make_config()).unwrap();
    let first = make_event("confirmed-a").with_command_id("cmd-a").with_local_sequence(1);
    let second = make_event("confirmed-b").with_command_id("cmd-a").with_local_sequence(2);
    let first_ack =
        crate::transport::PushAcknowledgement::new(first.id, 42).with_receipt("batch-1");
    let second_ack =
        crate::transport::PushAcknowledgement::new(second.id, 43).with_receipt("batch-1");
    engine.confirmations.push(PushConfirmation::from_ack(&first, &first_ack));
    engine.confirmations.push(PushConfirmation::from_ack(&second, &second_ack));

    let by_event = engine.confirmation_for_event(first.id).unwrap();
    assert_eq!(by_event.command_id.as_deref(), Some("cmd-a"));
    assert_eq!(by_event.remote_sequence, 42);

    let by_sequence = engine.confirmation_for_remote_sequence(43).unwrap();
    assert_eq!(by_sequence.event_id, second.id);

    let by_receipt = engine.confirmations_for_receipt("batch-1");
    assert_eq!(by_receipt.len(), 2);

    let by_command = engine.confirmations_for_command("cmd-a");
    assert_eq!(by_command.len(), 2);
    assert_eq!(by_command[0].event_id, first.id);
    assert_eq!(by_command[1].event_id, second.id);

    let latest_by_command = engine.latest_confirmation_for_command("cmd-a").unwrap();
    assert_eq!(latest_by_command.event_id, second.id);
    assert_eq!(latest_by_command.remote_sequence, 43);

    let by_entity = engine.confirmations_for_entity("order", "ORD-1");
    assert_eq!(by_entity.len(), 2);

    let latest_by_entity = engine.latest_confirmation_for_entity("order", "ORD-1").unwrap();
    assert_eq!(latest_by_entity.event_id, second.id);
    assert_eq!(latest_by_entity.remote_sequence, 43);

    assert!(engine.confirmation_for_event(uuid::Uuid::new_v4()).is_none());
    assert!(engine.confirmation_for_remote_sequence(99).is_none());
    assert!(engine.confirmations_for_command("missing").is_empty());
    assert!(engine.confirmations_for_entity("order", "missing").is_empty());
    assert!(engine.latest_confirmation_for_command("missing").is_none());
    assert!(engine.latest_confirmation_for_entity("order", "missing").is_none());
}

#[test]
fn kernel_receipts_span_pending_confirmed_and_rejected_events() {
    let mut engine = SyncEngine::new(make_config()).unwrap();
    let pending = SyncEvent::new("order.created", "order", "ORD-1", json!({"total": 99}))
        .with_command_id("cmd-kernel")
        .with_policy_checkpoint(
            crate::event::PolicyCheckpoint::new("orders", crate::event::PolicyDecision::Allowed)
                .with_reason("within budget"),
        )
        .with_budget_checkpoint(
            crate::event::BudgetCheckpoint::new("budget-1", 9900, "USD")
                .with_remaining_amount_minor(100),
        );
    let confirmed =
        SyncEvent::new("order.confirmed", "order", "ORD-1", json!({"status": "confirmed"}))
            .with_command_id("cmd-kernel")
            .with_policy_checkpoint(crate::event::PolicyCheckpoint::new(
                "orders",
                crate::event::PolicyDecision::Allowed,
            ));
    let rejected =
        SyncEvent::new("order.rejected", "order", "ORD-1", json!({"status": "rejected"}))
            .with_command_id("cmd-kernel")
            .with_budget_checkpoint(crate::event::BudgetCheckpoint::new("budget-2", 5000, "USD"));

    let pending_id = pending.id;
    let confirmed_id = confirmed.id;
    let rejected_id = rejected.id;

    engine.record(confirmed.clone()).unwrap();
    engine.record(rejected.clone()).unwrap();
    engine.record(pending).unwrap();

    let confirmed_ack =
        crate::transport::PushAcknowledgement::new(confirmed_id, 42).with_receipt("rcpt-42");
    engine.confirmations.push(PushConfirmation::from_ack(&confirmed, &confirmed_ack));
    engine.dead_letters.push(DeadLetter::new(
        rejected,
        crate::transport::PushRejection::new(rejected_id)
            .with_code("budget_exceeded")
            .with_reason("limit exceeded")
            .with_retryable(false),
    ));
    engine.outbox.try_retain(|event| event.id == pending_id).unwrap();

    let receipts = engine.kernel_receipts();
    assert_eq!(receipts.len(), 3);

    let pending_receipt = engine.kernel_receipt_for_event(pending_id).unwrap();
    assert_eq!(pending_receipt.status, KernelReceiptStatus::LocalPending);
    assert_eq!(
        pending_receipt
            .kernel
            .as_ref()
            .and_then(|kernel| kernel.policy.as_ref())
            .map(|policy| policy.reason.as_deref()),
        Some(Some("within budget"))
    );
    assert_eq!(
        pending_receipt
            .kernel
            .as_ref()
            .and_then(|kernel| kernel.budget.as_ref())
            .map(|budget| budget.remaining_amount_minor),
        Some(Some(100))
    );

    let confirmed_receipt = engine.kernel_receipt_for_event(confirmed_id).unwrap();
    assert_eq!(confirmed_receipt.status, KernelReceiptStatus::ConfirmedRemote);
    assert_eq!(confirmed_receipt.remote_sequence, Some(42));
    assert_eq!(confirmed_receipt.remote_receipt.as_deref(), Some("rcpt-42"));

    let rejected_receipt = engine.kernel_receipt_for_event(rejected_id).unwrap();
    assert_eq!(rejected_receipt.status, KernelReceiptStatus::RejectedRemote);
    assert_eq!(rejected_receipt.rejection_code.as_deref(), Some("budget_exceeded"));
    assert_eq!(rejected_receipt.rejection_reason.as_deref(), Some("limit exceeded"));
    assert_eq!(rejected_receipt.retryable, Some(false));

    let by_command = engine.kernel_receipts_for_command("cmd-kernel");
    assert_eq!(by_command.len(), 3);
    assert_eq!(
        engine.latest_kernel_receipt_for_command("cmd-kernel").unwrap().event_id,
        pending_id
    );

    let by_entity = engine.kernel_receipts_for_entity("order", "ORD-1");
    assert_eq!(by_entity.len(), 3);
    assert_eq!(
        engine.latest_kernel_receipt_for_entity("order", "ORD-1").unwrap().event_id,
        pending_id
    );
}

#[test]
fn command_convergence_tracks_pending_confirmed_committed_settled_and_rejected_states() {
    let mut engine = SyncEngine::new(make_config()).unwrap();

    let pending = SyncEvent::new("order.created", "order", "ORD-pending", json!({}))
        .with_command_id("cmd-pending");
    let confirmed = SyncEvent::new("order.created", "order", "ORD-confirmed", json!({}))
        .with_command_id("cmd-confirmed");
    let committed = SyncEvent::new("order.created", "order", "ORD-committed", json!({}))
        .with_command_id("cmd-committed");
    let settled = SyncEvent::new("order.created", "order", "ORD-settled", json!({}))
        .with_command_id("cmd-settled");
    let rejected = SyncEvent::new("order.created", "order", "ORD-rejected", json!({}))
        .with_command_id("cmd-rejected");

    let pending_id = pending.id;
    let confirmed_id = confirmed.id;
    let committed_id = committed.id;
    let settled_id = settled.id;
    let rejected_id = rejected.id;

    engine.record(pending).unwrap();
    engine.record(confirmed.clone()).unwrap();
    engine.record(committed.clone()).unwrap();
    engine.record(settled.clone()).unwrap();
    engine.record(rejected.clone()).unwrap();

    engine.confirmations.push(PushConfirmation::from_ack(
        &confirmed,
        &crate::transport::PushAcknowledgement::new(confirmed_id, 10)
            .with_receipt("receipt-confirmed"),
    ));
    engine.confirmations.push(PushConfirmation::from_ack(
        &committed,
        &crate::transport::PushAcknowledgement::new(committed_id, 11)
            .with_receipt("receipt-committed"),
    ));
    engine.confirmations.push(PushConfirmation::from_ack(
        &settled,
        &crate::transport::PushAcknowledgement::new(settled_id, 12).with_receipt("receipt-settled"),
    ));
    engine.dead_letters.push(DeadLetter::new(
        rejected,
        crate::transport::PushRejection::new(rejected_id)
            .with_code("rejected")
            .with_reason("remote rejected"),
    ));
    engine.outbox.try_retain(|event| event.id == pending_id).unwrap();

    let confirmed_convergence = engine.command_convergence("cmd-confirmed").unwrap();
    assert_eq!(
        confirmed_convergence.status,
        crate::convergence::CounterpartyConvergenceStatus::ConfirmedRemote
    );
    assert_eq!(confirmed_convergence.remote_receipts, vec!["receipt-confirmed".to_string()]);

    engine.state.remote_head = 12;
    engine.state.remote_state_root = Some("root-12".into());
    engine.state.last_commitment_id = Some("BATCH-12".into());
    engine.state.remote_cursor = 11;

    let pending_convergence = engine.command_convergence("cmd-pending").unwrap();
    assert_eq!(
        pending_convergence.status,
        crate::convergence::CounterpartyConvergenceStatus::LocalPending
    );

    let committed_convergence = engine.command_convergence("cmd-committed").unwrap();
    assert_eq!(
        committed_convergence.status,
        crate::convergence::CounterpartyConvergenceStatus::Settled
    );
    assert_eq!(committed_convergence.max_remote_sequence, Some(11));
    assert_eq!(
        committed_convergence
            .commitment
            .as_ref()
            .and_then(|commitment| commitment.commitment_id.as_deref()),
        Some("BATCH-12")
    );

    let settled_convergence = engine.command_convergence("cmd-settled").unwrap();
    assert_eq!(
        settled_convergence.status,
        crate::convergence::CounterpartyConvergenceStatus::CommittedRemote
    );
    assert_eq!(settled_convergence.max_remote_sequence, Some(12));

    let rejected_convergence = engine.command_convergence("cmd-rejected").unwrap();
    assert_eq!(
        rejected_convergence.status,
        crate::convergence::CounterpartyConvergenceStatus::RejectedRemote
    );
    assert_eq!(rejected_convergence.rejection_codes, vec!["rejected".to_string()]);

    let all = engine.command_convergences();
    assert_eq!(all.len(), 5);
    assert_eq!(
        all.first().map(|convergence| convergence.command_id.as_str()),
        Some("cmd-committed")
    );
    assert!(all.iter().any(|convergence| convergence.command_id == "cmd-pending"));
    assert!(engine.command_convergence("missing").is_none());
}

#[test]
fn attest_command_verifies_and_retains_commitment_proof() {
    let mut engine = SyncEngine::new(make_config()).unwrap();
    let event =
        SyncEvent::new("order.created", "order", "ORD-1", json!({})).with_command_id("cmd-attest");
    let event_id = event.id;
    engine.record(event.clone()).unwrap();
    engine.confirmations.push(PushConfirmation::from_ack(
        &event,
        &crate::transport::PushAcknowledgement::new(event_id, 7).with_receipt("receipt-7"),
    ));
    engine.outbox.try_retain(|pending| pending.id != event_id).unwrap();

    let receipts = engine.kernel_receipts_for_command("cmd-attest");
    let leaf_hash =
        crate::attestation::compute_command_settlement_leaf("cmd-attest", &receipts).unwrap();
    let sibling_hash = [3_u8; 32];
    let root = stateset_crypto::merkle::compute_merkle_root(&[leaf_hash, sibling_hash]);

    engine.state.remote_head = 7;
    engine.state.remote_cursor = 7;
    engine.state.remote_state_root = Some(hex::encode(root));
    engine.state.last_commitment_id = Some("BATCH-7".into());

    let attestation = engine
        .attest_command(
            crate::attestation::CommandInclusionProof::new("cmd-attest", hex::encode(root), 0, 2)
                .with_commitment_id("BATCH-7")
                .with_sibling_hashes(vec![hex::encode(sibling_hash)]),
        )
        .unwrap();

    assert_eq!(attestation.command_id, "cmd-attest");
    assert_eq!(attestation.max_remote_sequence, 7);
    assert!(attestation.settled);
    assert_eq!(engine.command_attestations().len(), 1);
    let expected_leaf_hash = hex::encode(leaf_hash);
    assert_eq!(
        engine.command_attestation("cmd-attest").map(|stored| stored.leaf_hash.as_str()),
        Some(expected_leaf_hash.as_str())
    );
}

#[test]
fn attest_command_persists_across_restart() {
    let dir = tempdir().unwrap();
    let state_path = dir.path().join("sync-state.json");
    let config = make_config().with_state_path(state_path.to_string_lossy().into_owned());

    {
        let mut engine = SyncEngine::new(config.clone()).unwrap();
        let event = SyncEvent::new("order.created", "order", "ORD-2", json!({}))
            .with_command_id("cmd-persist-attest");
        let event_id = event.id;
        engine.record(event.clone()).unwrap();
        engine.confirmations.push(PushConfirmation::from_ack(
            &event,
            &crate::transport::PushAcknowledgement::new(event_id, 8).with_receipt("receipt-8"),
        ));
        engine.outbox.try_retain(|pending| pending.id != event_id).unwrap();

        let receipts = engine.kernel_receipts_for_command("cmd-persist-attest");
        let leaf_hash =
            crate::attestation::compute_command_settlement_leaf("cmd-persist-attest", &receipts)
                .unwrap();
        let sibling_hash = [4_u8; 32];
        let root = stateset_crypto::merkle::compute_merkle_root(&[leaf_hash, sibling_hash]);

        engine.state.remote_head = 8;
        engine.state.remote_cursor = 8;
        engine.state.remote_state_root = Some(hex::encode(root));
        engine.state.last_commitment_id = Some("BATCH-8".into());

        engine
            .attest_command(
                crate::attestation::CommandInclusionProof::new(
                    "cmd-persist-attest",
                    hex::encode(root),
                    0,
                    2,
                )
                .with_commitment_id("BATCH-8")
                .with_sibling_hashes(vec![hex::encode(sibling_hash)]),
            )
            .unwrap();
    }

    let restored = SyncEngine::new(config).unwrap();
    let attestation = restored.command_attestation("cmd-persist-attest").unwrap();
    assert_eq!(attestation.commitment_id.as_deref(), Some("BATCH-8"));
    assert_eq!(attestation.max_remote_sequence, 8);
}

#[test]
fn verify_commitment_manifest_persists_across_restart() {
    let dir = tempdir().unwrap();
    let state_path = dir.path().join("sync-state.json");
    let (private_key, public_key) = stateset_crypto::sign::generate_keypair();
    let config = make_config()
        .with_state_path(state_path.to_string_lossy().into_owned())
        .with_trusted_commitment_signer_public_key(hex::encode(public_key));

    {
        let mut engine = SyncEngine::new(config.clone()).unwrap();
        engine.state.remote_head = 10;
        engine.state.remote_state_root = Some("22".repeat(32));
        engine.state.last_commitment_id = Some("BATCH-10".into());

        let manifest = crate::commitment::sign_commitment_manifest(
            crate::commitment::CommitmentManifest::new(
                "BATCH-10",
                "22".repeat(32),
                10,
                "sequencer-persist",
            ),
            &private_key,
            &public_key,
        )
        .unwrap();

        let verified = engine.verify_commitment_manifest(manifest).unwrap();
        assert_eq!(verified.commitment_id, "BATCH-10");
        assert_eq!(verified.signer_id, "sequencer-persist");
    }

    let restored = SyncEngine::new(config).unwrap();
    let manifest = restored.verified_commitment_manifest("BATCH-10").unwrap();
    assert_eq!(manifest.state_root, "22".repeat(32));
    assert_eq!(manifest.signer_id, "sequencer-persist");
}

#[test]
fn verify_commitment_manifest_rejects_unpinned_signer_by_default() {
    let mut engine = SyncEngine::new(make_config()).unwrap();
    engine.state.remote_head = 12;
    engine.state.remote_state_root = Some("33".repeat(32));
    engine.state.last_commitment_id = Some("BATCH-12".into());

    let (private_key, public_key) = stateset_crypto::sign::generate_keypair();
    let manifest = crate::commitment::sign_commitment_manifest(
        crate::commitment::CommitmentManifest::new(
            "BATCH-12",
            "33".repeat(32),
            12,
            "sequencer-any",
        ),
        &private_key,
        &public_key,
    )
    .unwrap();

    let error = engine.verify_commitment_manifest(manifest).unwrap_err();
    assert!(
        matches!(error, ManifestVerificationError::TrustPolicyViolation { .. }),
        "expected TrustPolicyViolation, got: {error:?}"
    );
    assert!(engine.verified_commitment_manifests().is_empty());
}

#[test]
fn verify_commitment_manifest_accepts_pinned_signer_key() {
    let (private_key, public_key) = stateset_crypto::sign::generate_keypair();
    let mut engine = SyncEngine::new(
        make_config().with_trusted_commitment_signer_public_key(hex::encode(public_key)),
    )
    .unwrap();

    let manifest = crate::commitment::sign_commitment_manifest(
        crate::commitment::CommitmentManifest::new(
            "BATCH-13",
            "34".repeat(32),
            13,
            "sequencer-pinned",
        ),
        &private_key,
        &public_key,
    )
    .unwrap();

    let verified = engine.verify_commitment_manifest(manifest).unwrap();
    assert_eq!(verified.commitment_id, "BATCH-13");
    assert_eq!(verified.signer_public_key, hex::encode(public_key));
    assert_eq!(engine.verified_commitment_manifests().len(), 1);
}

#[test]
fn trust_on_first_use_pins_first_key_and_rejects_rotation() {
    let dir = tempdir().unwrap();
    let state_path = dir.path().join("sync-state.json");
    let config = make_config()
        .with_state_path(state_path.to_string_lossy().into_owned())
        .with_commitment_trust_on_first_use();
    let (private_key_a, public_key_a) = stateset_crypto::sign::generate_keypair();
    let (private_key_b, public_key_b) = stateset_crypto::sign::generate_keypair();

    let sign = |commitment_id: &str, private_key: &[u8; 32], public_key: &[u8; 32]| {
        crate::commitment::sign_commitment_manifest(
            crate::commitment::CommitmentManifest::new(
                commitment_id,
                "35".repeat(32),
                0,
                "sequencer-tofu",
            ),
            private_key,
            public_key,
        )
        .unwrap()
    };

    {
        let mut engine = SyncEngine::new(config.clone()).unwrap();

        // First-seen key is accepted and pinned.
        let verified = engine
            .verify_commitment_manifest(sign("BATCH-A", &private_key_a, &public_key_a))
            .unwrap();
        assert_eq!(verified.signer_public_key, hex::encode(public_key_a));

        // A different key for the same signer id is rejected.
        let error = engine
            .verify_commitment_manifest(sign("BATCH-B", &private_key_b, &public_key_b))
            .unwrap_err();
        assert!(
            matches!(error, ManifestVerificationError::TrustPolicyViolation { .. }),
            "expected TrustPolicyViolation, got: {error:?}"
        );

        // The pinned key keeps working.
        engine.verify_commitment_manifest(sign("BATCH-C", &private_key_a, &public_key_a)).unwrap();
    }

    // The first-use pin is durable: a different key is still rejected
    // after a restart, and the pinned key is still accepted.
    let mut restored = SyncEngine::new(config).unwrap();
    let error = restored
        .verify_commitment_manifest(sign("BATCH-D", &private_key_b, &public_key_b))
        .unwrap_err();
    assert!(
        matches!(error, ManifestVerificationError::TrustPolicyViolation { .. }),
        "expected TrustPolicyViolation after restart, got: {error:?}"
    );
    restored.verify_commitment_manifest(sign("BATCH-E", &private_key_a, &public_key_a)).unwrap();
}

#[test]
fn trust_on_first_use_rejects_new_signer_id_after_pin() {
    let mut engine = SyncEngine::new(make_config().with_commitment_trust_on_first_use()).unwrap();
    let (private_key_a, public_key_a) = stateset_crypto::sign::generate_keypair();
    let (private_key_b, public_key_b) = stateset_crypto::sign::generate_keypair();

    engine
        .verify_commitment_manifest(
            crate::commitment::sign_commitment_manifest(
                crate::commitment::CommitmentManifest::new(
                    "BATCH-A",
                    "36".repeat(32),
                    0,
                    "sequencer-first",
                ),
                &private_key_a,
                &public_key_a,
            )
            .unwrap(),
        )
        .unwrap();

    // Once a first-use key is pinned, a brand-new signer id with a fresh
    // key must not silently establish a second trust anchor.
    let error = engine
        .verify_commitment_manifest(
            crate::commitment::sign_commitment_manifest(
                crate::commitment::CommitmentManifest::new(
                    "BATCH-B",
                    "36".repeat(32),
                    0,
                    "sequencer-second",
                ),
                &private_key_b,
                &public_key_b,
            )
            .unwrap(),
        )
        .unwrap_err();
    assert!(
        matches!(error, ManifestVerificationError::TrustPolicyViolation { .. }),
        "expected TrustPolicyViolation, got: {error:?}"
    );
}

#[test]
fn allow_any_signer_requires_explicit_opt_in() {
    let (private_key, public_key) = stateset_crypto::sign::generate_keypair();
    let sign = || {
        crate::commitment::sign_commitment_manifest(
            crate::commitment::CommitmentManifest::new(
                "BATCH-ANY",
                "37".repeat(32),
                0,
                "sequencer-any",
            ),
            &private_key,
            &public_key,
        )
        .unwrap()
    };

    // Default policy: rejected.
    let mut strict = SyncEngine::new(make_config()).unwrap();
    let error = strict.verify_commitment_manifest(sign()).unwrap_err();
    assert!(matches!(error, ManifestVerificationError::TrustPolicyViolation { .. }));

    // Explicit opt-in: the same manifest is accepted.
    let mut permissive = SyncEngine::new(make_config().with_allow_any_commitment_signer()).unwrap();
    let verified = permissive.verify_commitment_manifest(sign()).unwrap();
    assert_eq!(verified.commitment_id, "BATCH-ANY");
}

#[test]
fn attest_command_uses_verified_manifest_signer_metadata() {
    let (private_key, public_key) = stateset_crypto::sign::generate_keypair();
    let mut engine = SyncEngine::new(
        make_config().with_trusted_commitment_signer_public_key(hex::encode(public_key)),
    )
    .unwrap();
    let event = SyncEvent::new("order.created", "order", "ORD-9", json!({}))
        .with_command_id("cmd-attest-signed");
    let event_id = event.id;
    engine.record(event.clone()).unwrap();
    engine.confirmations.push(PushConfirmation::from_ack(
        &event,
        &crate::transport::PushAcknowledgement::new(event_id, 9).with_receipt("receipt-9"),
    ));
    engine.outbox.try_retain(|pending| pending.id != event_id).unwrap();

    let receipts = engine.kernel_receipts_for_command("cmd-attest-signed");
    let leaf_hash =
        crate::attestation::compute_command_settlement_leaf("cmd-attest-signed", &receipts)
            .unwrap();
    let root = stateset_crypto::merkle::compute_merkle_root(&[leaf_hash]);

    engine.state.remote_head = 9;
    engine.state.remote_cursor = 9;
    engine.state.remote_state_root = Some(hex::encode(root));
    engine.state.last_commitment_id = Some("BATCH-9".into());

    let manifest = crate::commitment::sign_commitment_manifest(
        crate::commitment::CommitmentManifest::new("BATCH-9", hex::encode(root), 9, "sequencer-a"),
        &private_key,
        &public_key,
    )
    .unwrap();
    engine.verify_commitment_manifest(manifest).unwrap();

    let attestation = engine
        .attest_command(
            crate::attestation::CommandInclusionProof::new(
                "cmd-attest-signed",
                hex::encode(root),
                0,
                1,
            )
            .with_commitment_id("BATCH-9"),
        )
        .unwrap();

    assert_eq!(attestation.manifest_signer_id.as_deref(), Some("sequencer-a"));
    assert!(attestation.manifest_verified_at.is_some());
    assert_eq!(
        engine
            .command_attestation("cmd-attest-signed")
            .and_then(|stored| stored.manifest_signer_id.as_deref()),
        Some("sequencer-a")
    );
}

#[test]
fn drain_dead_letters_clears_engine_queue() {
    let mut engine = SyncEngine::new(make_config()).unwrap();
    let dead_letter = DeadLetter::new(
        make_event("dead-letter"),
        crate::transport::PushRejection::new(uuid::Uuid::new_v4()).with_reason("invalid signature"),
    );
    engine.dead_letters.push(dead_letter.clone());

    let drained = engine.drain_dead_letters().unwrap();
    assert_eq!(drained, vec![dead_letter]);
    assert_eq!(engine.dead_letter_count(), 0);
}

#[test]
fn dead_letter_lookup_supports_event_command_and_entity_queries() {
    let mut engine = SyncEngine::new(make_config()).unwrap();
    let base = Utc::now();

    let first_event = make_event("dead-a").with_command_id("cmd-a");
    let first_id = first_event.id;
    let mut first = DeadLetter::new(
        first_event,
        crate::transport::PushRejection::new(first_id).with_reason("invalid signature"),
    );
    first.rejected_at = base;

    let second_event = make_event("dead-b").with_command_id("cmd-a");
    let second_id = second_event.id;
    let mut second = DeadLetter::new(
        second_event,
        crate::transport::PushRejection::new(second_id).with_reason("invalid schema"),
    );
    second.rejected_at = base + chrono::Duration::seconds(1);

    let third_event =
        SyncEvent::new("dead-c", "order", "ORD-2", json!({})).with_command_id("cmd-b");
    let third_id = third_event.id;
    let mut third = DeadLetter::new(
        third_event,
        crate::transport::PushRejection::new(third_id).with_reason("tenant mismatch"),
    );
    third.rejected_at = base + chrono::Duration::seconds(2);

    engine.dead_letters.push(first);
    engine.dead_letters.push(second);
    engine.dead_letters.push(third);

    let by_event = engine.dead_letter_for_event(first_id).unwrap();
    assert_eq!(by_event.event.command_id.as_deref(), Some("cmd-a"));
    assert_eq!(by_event.event.entity_id, "ORD-1");

    let by_command = engine.dead_letters_for_command("cmd-a");
    assert_eq!(by_command.len(), 2);
    assert_eq!(by_command[0].event.id, first_id);
    assert_eq!(by_command[1].event.id, second_id);

    let latest_by_command = engine.latest_dead_letter_for_command("cmd-a").unwrap();
    assert_eq!(latest_by_command.event.id, second_id);
    assert_eq!(latest_by_command.rejection.reason.as_deref(), Some("invalid schema"));

    let by_entity = engine.dead_letters_for_entity("order", "ORD-1");
    assert_eq!(by_entity.len(), 2);

    let latest_by_entity = engine.latest_dead_letter_for_entity("order", "ORD-1").unwrap();
    assert_eq!(latest_by_entity.event.id, second_id);

    assert!(engine.dead_letter_for_event(uuid::Uuid::new_v4()).is_none());
    assert!(engine.dead_letters_for_command("missing").is_empty());
    assert!(engine.dead_letters_for_entity("order", "missing").is_empty());
    assert!(engine.latest_dead_letter_for_command("missing").is_none());
    assert!(engine.latest_dead_letter_for_entity("order", "missing").is_none());
}

#[test]
fn requeue_dead_letter_moves_event_back_to_outbox() {
    let mut engine = SyncEngine::new(make_config()).unwrap();
    let event = make_event("dead-letter");
    let event_id = event.id;
    engine.dead_letters.push(DeadLetter::new(
        event,
        crate::transport::PushRejection::new(event_id).with_reason("invalid signature"),
    ));

    let sequence = engine.requeue_dead_letter(event_id).unwrap();
    assert_eq!(sequence, 1);
    assert_eq!(engine.pending_count(), 1);
    assert_eq!(engine.dead_letter_count(), 0);
    let pending = engine.outbox.peek(10);
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, event_id);
    assert_eq!(pending[0].local_sequence(), Some(1));
}

#[test]
fn requeue_dead_letter_rejects_duplicate_pending_event() {
    let mut engine = SyncEngine::new(make_config()).unwrap();
    let event = make_event("duplicate");
    let event_id = event.id;
    engine.record(event.clone()).unwrap();
    engine.dead_letters.push(DeadLetter::new(
        event,
        crate::transport::PushRejection::new(event_id).with_reason("invalid signature"),
    ));

    let err = engine.requeue_dead_letter(event_id).unwrap_err();
    assert!(matches!(err, SyncError::DuplicateEvent(_)));
    assert_eq!(engine.pending_count(), 1);
    assert_eq!(engine.dead_letter_count(), 1);
}

#[test]
fn requeue_dead_letter_returns_not_found_for_unknown_id() {
    let mut engine = SyncEngine::new(make_config()).unwrap();
    let err = engine.requeue_dead_letter(uuid::Uuid::new_v4()).unwrap_err();
    assert!(matches!(err, SyncError::NotFound(_)));
}

#[test]
fn discard_dead_letter_removes_entry() {
    let mut engine = SyncEngine::new(make_config()).unwrap();
    let event = make_event("discarded");
    let event_id = event.id;
    engine.dead_letters.push(DeadLetter::new(
        event,
        crate::transport::PushRejection::new(event_id).with_reason("invalid signature"),
    ));

    let discarded = engine.discard_dead_letter(event_id).unwrap();
    assert_eq!(discarded.event.id, event_id);
    assert_eq!(engine.dead_letter_count(), 0);
}

#[test]
fn drain_buffer() {
    let mut engine = SyncEngine::new(make_config()).unwrap();
    // Manually push to buffer via engine internals
    engine.buffer.push(make_event("buffered"));
    assert_eq!(engine.buffered_count(), 1);

    let drained = engine.drain_buffer();
    assert_eq!(drained.len(), 1);
    assert_eq!(engine.buffered_count(), 0);
}

#[test]
fn engine_with_strategy() {
    let engine = SyncEngine::with_strategy(make_config(), ConflictStrategy::LocalWins).unwrap();
    assert_eq!(engine.resolver().strategy(), ConflictStrategy::LocalWins);
}

#[test]
fn config_accessor() {
    let config = make_config();
    let engine = SyncEngine::new(config).unwrap();
    assert_eq!(engine.config().agent_id, "agent-1");
}

#[test]
fn try_new_rejects_invalid_config() {
    let bad = SyncConfig::new("", "tenant", "store");
    assert!(SyncEngine::try_new(bad).is_err());
}

#[test]
fn try_new_with_persistent_outbox_restores_pending_events() {
    let dir = tempdir().unwrap();
    let outbox_path = dir.path().join("sync-outbox.json");
    let path_str = outbox_path.to_string_lossy().to_string();

    {
        let mut engine = SyncEngine::try_new(
            SyncConfig::new("agent-1", "tenant-1", "store-1").with_outbox_path(path_str.clone()),
        )
        .unwrap();
        engine.record(make_event("persisted-a")).unwrap();
        engine.record(make_event("persisted-b")).unwrap();
        assert_eq!(engine.pending_count(), 2);
    }

    let engine = SyncEngine::try_new(
        SyncConfig::new("agent-1", "tenant-1", "store-1").with_outbox_path(path_str),
    )
    .unwrap();
    assert_eq!(engine.pending_count(), 2);
}

#[test]
fn default_state_path_for_outbox_derives_sibling_file() {
    let derived =
        SyncEngine::default_state_path_for_outbox(std::path::Path::new("/tmp/sync-outbox.json"));
    assert_eq!(derived, std::path::PathBuf::from("/tmp/sync-outbox.state.json"));
}

#[tokio::test]
async fn try_new_with_persistent_sync_state_restores_remote_progress() {
    #[derive(Debug)]
    struct PersistedStateTransport;

    #[async_trait::async_trait]
    impl Transport for PersistedStateTransport {
        async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
            let acknowledgements = events
                .iter()
                .enumerate()
                .map(|(index, event)| {
                    crate::transport::PushAcknowledgement::new(event.id, 7 + index as u64)
                })
                .collect();
            Ok(PushResult::accepted_only(events.len(), 9).with_acknowledgements(acknowledgements))
        }

        async fn pull_events(&self, since: u64, _limit: usize) -> Result<PullResult, SyncError> {
            if since == 0 {
                Ok(PullResult {
                    events: vec![make_event("remote").with_remote_sequence(10)],
                    remote_head: 12,
                    has_more: true,
                })
            } else {
                Ok(PullResult { events: vec![], remote_head: 12, has_more: false })
            }
        }
    }

    let dir = tempdir().unwrap();
    let outbox_path = dir.path().join("sync-outbox.json");
    let state_path = dir.path().join("sync-state.json");
    let config = SyncConfig::new("agent-1", "tenant-1", "store-1")
        .with_outbox_path(outbox_path.to_string_lossy().into_owned())
        .with_state_path(state_path.to_string_lossy().into_owned());

    {
        let mut engine = SyncEngine::try_new(config.clone()).unwrap();
        engine.record(make_event("local")).unwrap();
        engine.push(&PersistedStateTransport).await.unwrap();
        let result = engine.pull(&PersistedStateTransport).await.unwrap();
        assert_eq!(result.events.len(), 1);
        assert_eq!(engine.state().remote_head, 12);
        assert_eq!(engine.state().remote_cursor, 10);
        assert_eq!(engine.state().last_acknowledged_remote_sequence, Some(7));
        assert_eq!(engine.next_pull_cursor, Some(10));
    }

    let restored = SyncEngine::try_new(config).unwrap();
    assert_eq!(restored.state().local_head, 1);
    assert_eq!(restored.state().remote_head, 12);
    assert_eq!(restored.state().remote_cursor, 10);
    assert_eq!(restored.state().last_acknowledged_remote_sequence, Some(7));
    assert_eq!(restored.next_pull_cursor, Some(10));
    assert_eq!(restored.pending_count(), 0);
}

#[tokio::test]
async fn try_new_with_persistent_sync_state_restores_dead_letters() {
    #[derive(Debug)]
    struct RejectingTransport;

    #[async_trait::async_trait]
    impl Transport for RejectingTransport {
        async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
            Ok(PushResult::accepted_only(0, 0).with_rejections(vec![
                crate::transport::PushRejection::new(events[0].id)
                    .with_code("invalid_signature")
                    .with_reason("signature verification failed")
                    .with_retryable(false),
            ]))
        }

        async fn pull_events(&self, _since: u64, _limit: usize) -> Result<PullResult, SyncError> {
            Ok(PullResult { events: vec![], remote_head: 0, has_more: false })
        }
    }

    let dir = tempdir().unwrap();
    let outbox_path = dir.path().join("sync-outbox.json");
    let state_path = dir.path().join("sync-state.json");
    let config = SyncConfig::new("agent-1", "tenant-1", "store-1")
        .with_outbox_path(outbox_path.to_string_lossy().into_owned())
        .with_state_path(state_path.to_string_lossy().into_owned());

    let event_id = {
        let mut engine = SyncEngine::try_new(config.clone()).unwrap();
        let event = make_event("dead-letter");
        let event_id = event.id;
        engine.record(event).unwrap();
        engine.push(&RejectingTransport).await.unwrap();
        assert_eq!(engine.pending_count(), 0);
        assert_eq!(engine.dead_letter_count(), 1);
        event_id
    };

    let restored = SyncEngine::try_new(config).unwrap();
    assert_eq!(restored.pending_count(), 0);
    assert_eq!(restored.dead_letter_count(), 1);
    assert_eq!(restored.dead_letters()[0].event.id, event_id);
    assert_eq!(restored.dead_letters()[0].rejection.code.as_deref(), Some("invalid_signature"));
}

#[tokio::test]
async fn requeue_dead_letter_persists_across_restart() {
    let dir = tempdir().unwrap();
    let outbox_path = dir.path().join("sync-outbox.json");
    let state_path = dir.path().join("sync-state.json");
    let config = SyncConfig::new("agent-1", "tenant-1", "store-1")
        .with_outbox_path(outbox_path.to_string_lossy().into_owned())
        .with_state_path(state_path.to_string_lossy().into_owned());

    let event_id = {
        let mut engine = SyncEngine::try_new(config.clone()).unwrap();
        let event = make_event("requeued");
        let event_id = event.id;
        engine.dead_letters.push(DeadLetter::new(
            event,
            crate::transport::PushRejection::new(event_id).with_reason("invalid signature"),
        ));
        engine.persist_runtime_state().unwrap();

        let sequence = engine.requeue_dead_letter(event_id).unwrap();
        assert_eq!(sequence, 1);
        assert_eq!(engine.pending_count(), 1);
        assert_eq!(engine.dead_letter_count(), 0);
        event_id
    };

    let restored = SyncEngine::try_new(config).unwrap();
    assert_eq!(restored.pending_count(), 1);
    assert_eq!(restored.dead_letter_count(), 0);
    let pending = restored.outbox.peek(10);
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, event_id);
}

#[tokio::test]
async fn push_confirmations_persist_across_restart() {
    #[derive(Debug)]
    struct ReceiptAckTransport;

    #[async_trait::async_trait]
    impl Transport for ReceiptAckTransport {
        async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
            let acknowledgements = events
                .iter()
                .enumerate()
                .map(|(index, event)| {
                    crate::transport::PushAcknowledgement::new(event.id, 30 + index as u64)
                        .with_receipt(format!("receipt-{}", 30 + index as u64))
                })
                .collect();
            Ok(PushResult::accepted_only(events.len(), 31).with_acknowledgements(acknowledgements))
        }

        async fn pull_events(&self, _since: u64, _limit: usize) -> Result<PullResult, SyncError> {
            Ok(PullResult { events: vec![], remote_head: 31, has_more: false })
        }
    }

    let dir = tempdir().unwrap();
    let state_path = dir.path().join("sync-state.json");
    let config = make_config().with_state_path(state_path.to_string_lossy().into_owned());

    let first_id = {
        let mut engine = SyncEngine::try_new(config.clone()).unwrap();
        let first = make_event("persisted-a").with_command_id("cmd-persist");
        let second = make_event("persisted-b");
        let first_id = first.id;
        engine.record(first).unwrap();
        engine.record(second).unwrap();
        engine.push(&ReceiptAckTransport).await.unwrap();
        assert_eq!(engine.confirmation_count(), 2);
        first_id
    };

    let restored = SyncEngine::try_new(config).unwrap();
    assert_eq!(restored.confirmation_count(), 2);
    assert_eq!(restored.confirmations()[0].event_id, first_id);
    assert_eq!(restored.confirmations()[0].command_id.as_deref(), Some("cmd-persist"));
    assert_eq!(restored.confirmations()[0].remote_sequence, 30);
    assert_eq!(restored.confirmations()[0].receipt.as_deref(), Some("receipt-30"));
}

#[tokio::test]
async fn push_confirmation_retention_respects_capacity() {
    #[derive(Debug)]
    struct ReceiptAckTransport;

    #[async_trait::async_trait]
    impl Transport for ReceiptAckTransport {
        async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
            let acknowledgements = events
                .iter()
                .enumerate()
                .map(|(index, event)| {
                    crate::transport::PushAcknowledgement::new(event.id, 40 + index as u64)
                })
                .collect();
            Ok(PushResult::accepted_only(events.len(), 42).with_acknowledgements(acknowledgements))
        }

        async fn pull_events(&self, _since: u64, _limit: usize) -> Result<PullResult, SyncError> {
            Ok(PullResult { events: vec![], remote_head: 42, has_more: false })
        }
    }

    let mut engine = SyncEngine::new(make_config().with_confirmation_capacity(2)).unwrap();
    let first = make_event("a");
    let second = make_event("b");
    let third = make_event("c");
    let second_id = second.id;
    let third_id = third.id;
    engine.record(first).unwrap();
    engine.record(second).unwrap();
    engine.record(third).unwrap();

    engine.push(&ReceiptAckTransport).await.unwrap();

    assert_eq!(engine.confirmation_count(), 2);
    assert_eq!(engine.confirmations()[0].event_id, second_id);
    assert_eq!(engine.confirmations()[0].remote_sequence, 41);
    assert_eq!(engine.confirmations()[1].event_id, third_id);
    assert_eq!(engine.confirmations()[1].remote_sequence, 42);
}

#[tokio::test]
async fn push_confirmations_replace_stale_entries_for_matching_ids() {
    #[derive(Debug)]
    struct ReplacementTransport;

    #[async_trait::async_trait]
    impl Transport for ReplacementTransport {
        async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
            let acknowledgements = vec![
                crate::transport::PushAcknowledgement::new(events[0].id, 55)
                    .with_receipt("receipt-55"),
            ];
            Ok(PushResult::accepted_only(events.len(), 55).with_acknowledgements(acknowledgements))
        }

        async fn pull_events(&self, _since: u64, _limit: usize) -> Result<PullResult, SyncError> {
            Ok(PullResult { events: vec![], remote_head: 55, has_more: false })
        }
    }

    let mut engine = SyncEngine::new(make_config()).unwrap();
    let event = make_event("replace-me");
    let event_id = event.id;
    engine.confirmations.push(PushConfirmation::from_ack(
        &event.clone().with_local_sequence(1),
        &crate::transport::PushAcknowledgement::new(event_id, 11).with_receipt("receipt-11"),
    ));
    engine.record(event).unwrap();

    engine.push(&ReplacementTransport).await.unwrap();

    assert_eq!(engine.confirmation_count(), 1);
    let confirmation = engine.confirmation_for_event(event_id).unwrap();
    assert_eq!(confirmation.remote_sequence, 55);
    assert_eq!(confirmation.receipt.as_deref(), Some("receipt-55"));
}

#[tokio::test]
async fn drain_confirmations_persists_clear_state() {
    #[derive(Debug)]
    struct ReceiptAckTransport;

    #[async_trait::async_trait]
    impl Transport for ReceiptAckTransport {
        async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
            let acknowledgements = events
                .iter()
                .map(|event| crate::transport::PushAcknowledgement::new(event.id, 50))
                .collect();
            Ok(PushResult::accepted_only(events.len(), 50).with_acknowledgements(acknowledgements))
        }

        async fn pull_events(&self, _since: u64, _limit: usize) -> Result<PullResult, SyncError> {
            Ok(PullResult { events: vec![], remote_head: 50, has_more: false })
        }
    }

    let dir = tempdir().unwrap();
    let state_path = dir.path().join("sync-state.json");
    let config = make_config().with_state_path(state_path.to_string_lossy().into_owned());

    {
        let mut engine = SyncEngine::try_new(config.clone()).unwrap();
        engine.record(make_event("drained")).unwrap();
        engine.push(&ReceiptAckTransport).await.unwrap();
        assert_eq!(engine.confirmation_count(), 1);

        let drained = engine.drain_confirmations().unwrap();
        assert_eq!(drained.len(), 1);
        assert_eq!(engine.confirmation_count(), 0);
    }

    let restored = SyncEngine::try_new(config).unwrap();
    assert_eq!(restored.confirmation_count(), 0);
}

#[tokio::test]
async fn push_respects_batch_size() {
    let config = SyncConfig::new("agent-1", "tenant-1", "store-1").with_batch_size(2);
    let mut engine = SyncEngine::new(config).unwrap();
    engine.record(make_event("a")).unwrap();
    engine.record(make_event("b")).unwrap();
    engine.record(make_event("c")).unwrap();

    let transport = NullTransport::new();
    let result = engine.push(&transport).await.unwrap();
    // Should only push 2 due to batch_size
    assert_eq!(result.accepted, 2);
    assert_eq!(engine.pending_count(), 1);
}

#[tokio::test]
async fn push_updates_state() {
    /// Mock transport that returns an increasing remote head.
    #[derive(Debug)]
    struct MockHeadTransport {
        head: Arc<AtomicU64>,
    }

    #[async_trait::async_trait]
    impl Transport for MockHeadTransport {
        async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
            let new_head =
                self.head.fetch_add(events.len() as u64, Ordering::SeqCst) + events.len() as u64;
            Ok(PushResult::accepted_only(events.len(), new_head))
        }
        async fn pull_events(&self, _since: u64, _limit: usize) -> Result<PullResult, SyncError> {
            Ok(PullResult {
                events: vec![],
                remote_head: self.head.load(Ordering::SeqCst),
                has_more: false,
            })
        }
    }

    let transport = MockHeadTransport { head: Arc::new(AtomicU64::new(0)) };

    let mut engine = SyncEngine::new(make_config()).unwrap();
    engine.record(make_event("a")).unwrap();
    engine.record(make_event("b")).unwrap();

    let result = engine.push(&transport).await.unwrap();
    assert_eq!(result.remote_head, 2);
    assert_eq!(engine.state().remote_head, 2);
}

#[tokio::test]
async fn push_tracks_acknowledged_remote_sequence() {
    #[derive(Debug)]
    struct AckTransport;

    #[async_trait::async_trait]
    impl Transport for AckTransport {
        async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
            let acknowledgements = events
                .iter()
                .enumerate()
                .map(|(index, event)| {
                    crate::transport::PushAcknowledgement::new(event.id, 10 + index as u64)
                })
                .collect();
            Ok(PushResult::accepted_only(events.len(), 11).with_acknowledgements(acknowledgements))
        }

        async fn pull_events(&self, _since: u64, _limit: usize) -> Result<PullResult, SyncError> {
            Ok(PullResult { events: vec![], remote_head: 11, has_more: false })
        }
    }

    let mut engine = SyncEngine::new(make_config()).unwrap();
    engine.record(make_event("a")).unwrap();
    engine.record(make_event("b")).unwrap();

    let result = engine.push(&AckTransport).await.unwrap();
    assert_eq!(result.acknowledged_head(), Some(11));
    assert_eq!(engine.state().last_acknowledged_remote_sequence, Some(11));
    assert_eq!(engine.pending_count(), 0);
    assert_eq!(engine.confirmation_count(), 2);
    assert_eq!(engine.status().retained_confirmations, 2);
    assert_eq!(engine.confirmations()[0].local_sequence, Some(1));
    assert_eq!(engine.confirmations()[1].local_sequence, Some(2));
}

#[tokio::test]
async fn push_retains_exact_confirmations_with_receipts() {
    #[derive(Debug)]
    struct ReceiptAckTransport;

    #[async_trait::async_trait]
    impl Transport for ReceiptAckTransport {
        async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
            let acknowledgements = events
                .iter()
                .enumerate()
                .map(|(index, event)| {
                    crate::transport::PushAcknowledgement::new(event.id, 20 + index as u64)
                        .with_receipt(format!("receipt-{}", 20 + index as u64))
                })
                .collect();
            Ok(PushResult::accepted_only(events.len(), 21).with_acknowledgements(acknowledgements))
        }

        async fn pull_events(&self, _since: u64, _limit: usize) -> Result<PullResult, SyncError> {
            Ok(PullResult { events: vec![], remote_head: 21, has_more: false })
        }
    }

    let mut engine = SyncEngine::new(make_config()).unwrap();
    let first = make_event("a").with_command_id("cmd-a");
    let second = make_event("b");
    let first_id = first.id;
    let second_id = second.id;
    engine.record(first).unwrap();
    engine.record(second).unwrap();

    engine.push(&ReceiptAckTransport).await.unwrap();

    let confirmations = engine.confirmations();
    assert_eq!(confirmations.len(), 2);
    assert_eq!(confirmations[0].event_id, first_id);
    assert_eq!(confirmations[0].command_id.as_deref(), Some("cmd-a"));
    assert_eq!(confirmations[0].local_sequence, Some(1));
    assert_eq!(confirmations[0].remote_sequence, 20);
    assert_eq!(confirmations[0].receipt.as_deref(), Some("receipt-20"));
    assert_eq!(confirmations[1].event_id, second_id);
    assert_eq!(confirmations[1].local_sequence, Some(2));
    assert_eq!(confirmations[1].remote_sequence, 21);
    assert_eq!(confirmations[1].receipt.as_deref(), Some("receipt-21"));
}

#[tokio::test]
async fn push_acknowledgements_remove_non_prefix_events() {
    #[derive(Debug)]
    struct SparseAckTransport;

    #[async_trait::async_trait]
    impl Transport for SparseAckTransport {
        async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
            Ok(PushResult::accepted_only(2, 12).with_acknowledgements(vec![
                crate::transport::PushAcknowledgement::new(events[0].id, 10),
                crate::transport::PushAcknowledgement::new(events[2].id, 12),
            ]))
        }

        async fn pull_events(&self, _since: u64, _limit: usize) -> Result<PullResult, SyncError> {
            Ok(PullResult { events: vec![], remote_head: 12, has_more: false })
        }
    }

    let mut engine = SyncEngine::new(make_config()).unwrap();
    let first = make_event("a");
    let second = make_event("b");
    let third = make_event("c");
    engine.record(first.clone()).unwrap();
    engine.record(second.clone()).unwrap();
    engine.record(third.clone()).unwrap();

    let result = engine.push(&SparseAckTransport).await.unwrap();
    assert_eq!(result.accepted, 2);
    assert_eq!(engine.pending_count(), 1);
    let remaining = engine.outbox.peek(10);
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].id, second.id);
    assert_eq!(engine.state().last_acknowledged_remote_sequence, Some(12));
}

#[tokio::test]
async fn push_dead_letters_non_retryable_rejections() {
    #[derive(Debug)]
    struct RejectingTransport;

    #[async_trait::async_trait]
    impl Transport for RejectingTransport {
        async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
            Ok(PushResult::accepted_only(1, 20).with_rejections(vec![
                crate::transport::PushRejection::new(events[2].id)
                    .with_code("invalid_signature")
                    .with_reason("signature verification failed")
                    .with_retryable(false),
            ]))
        }

        async fn pull_events(&self, _since: u64, _limit: usize) -> Result<PullResult, SyncError> {
            Ok(PullResult { events: vec![], remote_head: 20, has_more: false })
        }
    }

    let mut engine = SyncEngine::new(make_config()).unwrap();
    let first = make_event("a");
    let second = make_event("b");
    let third = make_event("c");
    engine.record(first.clone()).unwrap();
    engine.record(second.clone()).unwrap();
    engine.record(third.clone()).unwrap();

    let result = engine.push(&RejectingTransport).await.unwrap();
    assert_eq!(result.accepted, 1);
    assert_eq!(result.rejections.len(), 1);
    assert_eq!(engine.pending_count(), 1);
    assert_eq!(engine.dead_letter_count(), 1);
    let remaining = engine.outbox.peek(10);
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].id, second.id);
    let dead_letter = &engine.dead_letters()[0];
    assert_eq!(dead_letter.event.id, third.id);
    assert_eq!(dead_letter.rejection.code.as_deref(), Some("invalid_signature"));
    assert_eq!(engine.status().dead_letters, 1);
}

#[tokio::test]
async fn push_retryable_rejections_stay_pending() {
    #[derive(Debug)]
    struct RetryableRejectTransport;

    #[async_trait::async_trait]
    impl Transport for RetryableRejectTransport {
        async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
            Ok(PushResult::accepted_only(0, 0).with_rejections(vec![
                crate::transport::PushRejection::new(events[0].id)
                    .with_reason("sequencer is rebalancing")
                    .with_retryable(true),
            ]))
        }

        async fn pull_events(&self, _since: u64, _limit: usize) -> Result<PullResult, SyncError> {
            Ok(PullResult { events: vec![], remote_head: 0, has_more: false })
        }
    }

    let mut engine = SyncEngine::new(make_config()).unwrap();
    let event = make_event("retryable");
    let event_id = event.id;
    engine.record(event).unwrap();

    let result = engine.push(&RetryableRejectTransport).await.unwrap();
    assert_eq!(result.rejections.len(), 1);
    assert_eq!(engine.pending_count(), 1);
    assert_eq!(engine.dead_letter_count(), 0);
    assert_eq!(engine.outbox.peek(10)[0].id, event_id);
}

#[tokio::test]
async fn push_rejects_prefix_overlap_without_acknowledgements() {
    #[derive(Debug)]
    struct InvalidRejectionTransport;

    #[async_trait::async_trait]
    impl Transport for InvalidRejectionTransport {
        async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
            Ok(PushResult::accepted_only(1, 10).with_rejections(vec![
                crate::transport::PushRejection::new(events[0].id)
                    .with_reason("cannot overlap accepted prefix"),
            ]))
        }

        async fn pull_events(&self, _since: u64, _limit: usize) -> Result<PullResult, SyncError> {
            Ok(PullResult { events: vec![], remote_head: 10, has_more: false })
        }
    }

    let mut engine = SyncEngine::new(make_config()).unwrap();
    engine.record(make_event("a")).unwrap();
    engine.record(make_event("b")).unwrap();

    let err = engine.push(&InvalidRejectionTransport).await.unwrap_err();
    assert!(matches!(err, SyncError::Transport(_)));
    assert_eq!(engine.pending_count(), 2);
    assert_eq!(engine.dead_letter_count(), 0);
}

#[tokio::test]
async fn push_rejects_acknowledgement_for_unknown_event() {
    #[derive(Debug)]
    struct InvalidAckTransport;

    #[async_trait::async_trait]
    impl Transport for InvalidAckTransport {
        async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
            Ok(PushResult::accepted_only(events.len(), 99).with_acknowledgements(vec![
                crate::transport::PushAcknowledgement::new(uuid::Uuid::new_v4(), 99),
            ]))
        }

        async fn pull_events(&self, _since: u64, _limit: usize) -> Result<PullResult, SyncError> {
            Ok(PullResult { events: vec![], remote_head: 99, has_more: false })
        }
    }

    let mut engine = SyncEngine::new(make_config()).unwrap();
    engine.record(make_event("a")).unwrap();

    let err = engine.push(&InvalidAckTransport).await.unwrap_err();
    assert!(matches!(err, SyncError::Transport(_)));
    assert_eq!(engine.pending_count(), 1);
    assert!(engine.state().last_push.is_none());
}

#[tokio::test]
async fn transport_error_propagates() {
    /// Transport that always fails.
    #[derive(Debug)]
    struct FailTransport;

    #[async_trait::async_trait]
    impl Transport for FailTransport {
        async fn push_events(&self, _events: &[SyncEvent]) -> Result<PushResult, SyncError> {
            Err(SyncError::Transport("network down".into()))
        }
        async fn pull_events(&self, _since: u64, _limit: usize) -> Result<PullResult, SyncError> {
            Err(SyncError::Transport("network down".into()))
        }
    }

    let mut engine = SyncEngine::new(make_config()).unwrap();
    engine.record(make_event("a")).unwrap();

    let transport = FailTransport;
    let result = engine.push(&transport).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), SyncError::Transport(_)));
    // Failed push must not drop local events.
    assert_eq!(engine.pending_count(), 1);

    let pull_result = engine.pull(&transport).await;
    assert!(pull_result.is_err());
}

#[tokio::test]
async fn push_only_drains_accepted_events() {
    /// Transport that only accepts one event from each batch.
    #[derive(Debug)]
    struct PartialAcceptTransport;

    #[async_trait::async_trait]
    impl Transport for PartialAcceptTransport {
        async fn push_events(&self, _events: &[SyncEvent]) -> Result<PushResult, SyncError> {
            Ok(PushResult::accepted_only(1, 1))
        }

        async fn pull_events(&self, _since: u64, _limit: usize) -> Result<PullResult, SyncError> {
            Ok(PullResult { events: vec![], remote_head: 1, has_more: false })
        }
    }

    let mut engine = SyncEngine::new(make_config()).unwrap();
    engine.record(make_event("a")).unwrap();
    engine.record(make_event("b")).unwrap();
    engine.record(make_event("c")).unwrap();

    let result = engine.push(&PartialAcceptTransport).await.unwrap();
    assert_eq!(result.accepted, 1);
    assert_eq!(engine.pending_count(), 2);
}

#[tokio::test]
async fn push_returns_storage_error_when_ack_persist_fails() {
    #[derive(Debug)]
    struct AcceptAllTransport;

    #[async_trait::async_trait]
    impl Transport for AcceptAllTransport {
        async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
            Ok(PushResult::accepted_only(events.len(), events.len() as u64))
        }

        async fn pull_events(&self, _since: u64, _limit: usize) -> Result<PullResult, SyncError> {
            Ok(PullResult { events: vec![], remote_head: 0, has_more: false })
        }
    }

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("outbox.json");
    let config = make_config().with_outbox_path(path.to_string_lossy().into_owned());
    let mut engine = SyncEngine::new(config).unwrap();
    engine.record(make_event("a")).unwrap();

    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();

    let err = engine.push(&AcceptAllTransport).await.unwrap_err();
    assert!(matches!(err, SyncError::Storage(_)));
    assert_eq!(engine.pending_count(), 1);
    assert_eq!(engine.state().remote_head, 0);
    assert!(engine.state().last_push.is_none());
}

#[tokio::test]
async fn pull_conflict_resolution() {
    /// Transport that returns events conflicting with local outbox.
    #[derive(Debug)]
    struct ConflictTransport;

    #[async_trait::async_trait]
    impl Transport for ConflictTransport {
        async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
            Ok(PushResult::accepted_only(events.len(), 10))
        }
        async fn pull_events(&self, _since: u64, _limit: usize) -> Result<PullResult, SyncError> {
            // Return an event for the same entity as the pending local event
            let remote_event =
                SyncEvent::new("order.updated", "order", "ORD-1", json!({"status": "remote"}))
                    .with_remote_sequence(5);
            Ok(PullResult { events: vec![remote_event], remote_head: 5, has_more: false })
        }
    }

    let mut engine =
        SyncEngine::with_strategy(make_config(), ConflictStrategy::RemoteWins).unwrap();
    engine
        .record(SyncEvent::new("order.updated", "order", "ORD-1", json!({"status": "local"})))
        .unwrap();

    let transport = ConflictTransport;
    let result = engine.pull(&transport).await.unwrap();
    assert_eq!(result.events.len(), 1);
    // RemoteWins removes conflicting local outbox events and keeps pulled event.
    assert_eq!(engine.buffered_count(), 1);
    assert_eq!(engine.pending_count(), 0);
}

#[tokio::test]
async fn pull_conflict_remote_wins_only_drops_latest_pending_event() {
    #[derive(Debug)]
    struct ConflictTransport;

    #[async_trait::async_trait]
    impl Transport for ConflictTransport {
        async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
            Ok(PushResult::accepted_only(events.len(), 10))
        }

        async fn pull_events(&self, _since: u64, _limit: usize) -> Result<PullResult, SyncError> {
            let remote_event =
                SyncEvent::new("order.updated", "order", "ORD-1", json!({"status": "remote"}))
                    .with_remote_sequence(5);
            Ok(PullResult { events: vec![remote_event], remote_head: 5, has_more: false })
        }
    }

    let mut engine =
        SyncEngine::with_strategy(make_config(), ConflictStrategy::RemoteWins).unwrap();
    engine
        .record(SyncEvent::new("order.note_added", "order", "ORD-1", json!({"note": "a"})))
        .unwrap();
    engine
        .record(SyncEvent::new("order.updated", "order", "ORD-1", json!({"status": "local"})))
        .unwrap();

    engine.pull(&ConflictTransport).await.unwrap();

    let pending: Vec<_> = engine.outbox.peek(10).into_iter().cloned().collect();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].event_type, "order.note_added");
    assert_eq!(engine.buffered_count(), 1);
}

#[tokio::test]
async fn pull_conflict_local_wins_keeps_pending_and_skips_remote() {
    #[derive(Debug)]
    struct ConflictTransport;

    #[async_trait::async_trait]
    impl Transport for ConflictTransport {
        async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
            Ok(PushResult::accepted_only(events.len(), 10))
        }
        async fn pull_events(&self, _since: u64, _limit: usize) -> Result<PullResult, SyncError> {
            let remote_event =
                SyncEvent::new("order.updated", "order", "ORD-1", json!({"status": "remote"}))
                    .with_remote_sequence(5);
            Ok(PullResult { events: vec![remote_event], remote_head: 5, has_more: false })
        }
    }

    let mut engine = SyncEngine::with_strategy(make_config(), ConflictStrategy::LocalWins).unwrap();
    engine
        .record(SyncEvent::new("order.updated", "order", "ORD-1", json!({"status": "local"})))
        .unwrap();

    let result = engine.pull(&ConflictTransport).await.unwrap();
    assert_eq!(result.events.len(), 1);
    assert_eq!(engine.pending_count(), 1);
    assert_eq!(engine.buffered_count(), 0);
}

#[tokio::test]
async fn full_sync_paginates_pull_until_complete() {
    #[derive(Debug)]
    struct PagingTransport {
        pulls: Arc<AtomicU64>,
        since_args: Arc<Mutex<Vec<u64>>>,
    }

    #[async_trait::async_trait]
    impl Transport for PagingTransport {
        async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
            Ok(PushResult::accepted_only(events.len(), 0))
        }

        async fn pull_events(&self, since: u64, _limit: usize) -> Result<PullResult, SyncError> {
            self.since_args.lock().unwrap().push(since);
            let call = self.pulls.fetch_add(1, Ordering::SeqCst);
            if call == 0 {
                Ok(PullResult {
                    events: vec![
                        SyncEvent::new("order.updated", "order", "ORD-1", json!({}))
                            .with_remote_sequence(1),
                    ],
                    // Simulate remote_head as global head watermark, not page cursor.
                    remote_head: 999,
                    has_more: true,
                })
            } else {
                Ok(PullResult {
                    events: vec![
                        SyncEvent::new("order.updated", "order", "ORD-2", json!({}))
                            .with_remote_sequence(2),
                    ],
                    remote_head: 999,
                    has_more: false,
                })
            }
        }
    }

    let since_args = Arc::new(Mutex::new(Vec::new()));
    let transport =
        PagingTransport { pulls: Arc::new(AtomicU64::new(0)), since_args: Arc::clone(&since_args) };
    let mut engine = SyncEngine::new(make_config()).unwrap();
    let (_push_result, pull_result) = engine.full_sync(&transport).await.unwrap();

    assert_eq!(pull_result.events.len(), 2);
    assert!(!pull_result.has_more);
    assert_eq!(engine.buffered_count(), 2);
    assert_eq!(engine.state().remote_head, 999);
    assert_eq!(transport.pulls.load(Ordering::SeqCst), 2);
    assert_eq!(&*since_args.lock().unwrap(), &[0, 1]);
}

#[tokio::test]
async fn pull_cursor_does_not_advance_from_push_remote_head() {
    #[derive(Debug)]
    struct HeadSkewTransport {
        since_args: Arc<Mutex<Vec<u64>>>,
    }

    #[async_trait::async_trait]
    impl Transport for HeadSkewTransport {
        async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
            Ok(PushResult::accepted_only(events.len(), 1000))
        }

        async fn pull_events(&self, since: u64, _limit: usize) -> Result<PullResult, SyncError> {
            self.since_args.lock().unwrap().push(since);
            Ok(PullResult {
                events: vec![
                    SyncEvent::new("order.updated", "order", "ORD-1", json!({}))
                        .with_remote_sequence(7),
                ],
                remote_head: 1000,
                has_more: false,
            })
        }
    }

    let since_args = Arc::new(Mutex::new(Vec::new()));
    let transport = HeadSkewTransport { since_args: Arc::clone(&since_args) };
    let mut engine = SyncEngine::new(make_config()).unwrap();

    engine.record(make_event("local.pending")).unwrap();
    let (_push, pull) = engine.full_sync(&transport).await.unwrap();

    assert_eq!(pull.events.len(), 1);
    assert_eq!(&*since_args.lock().unwrap(), &[0]);
}

#[tokio::test]
async fn pull_tracks_observed_cursor_separately_from_continuation_cursor() {
    #[derive(Debug)]
    struct ContinuationCursorTransport;

    #[async_trait::async_trait]
    impl Transport for ContinuationCursorTransport {
        async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
            Ok(PushResult::accepted_only(events.len(), 0))
        }

        async fn pull_events(&self, _since: u64, _limit: usize) -> Result<PullResult, SyncError> {
            Ok(PullResult { events: vec![], remote_head: 12, has_more: true })
        }

        async fn pull_events_page(&self, since: u64, _limit: usize) -> Result<PullPage, SyncError> {
            assert_eq!(since, 0);
            Ok(PullPage {
                result: PullResult {
                    events: vec![
                        SyncEvent::new("order.updated", "order", "ORD-1", json!({}))
                            .with_remote_sequence(10),
                    ],
                    remote_head: 12,
                    has_more: true,
                },
                next_cursor: Some(11),
                observed_cursor: Some(10),
            })
        }
    }

    let mut engine = SyncEngine::new(make_config()).unwrap();
    let result = engine.pull(&ContinuationCursorTransport).await.unwrap();
    assert_eq!(result.remote_head, 12);
    assert_eq!(engine.state().remote_cursor, 10);
    assert_eq!(engine.next_pull_cursor, Some(11));
}

#[tokio::test]
async fn pull_errors_when_has_more_but_cursor_cannot_advance() {
    #[derive(Debug)]
    struct StalledPagingTransport;

    #[async_trait::async_trait]
    impl Transport for StalledPagingTransport {
        async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
            Ok(PushResult::accepted_only(events.len(), 0))
        }

        async fn pull_events(&self, _since: u64, _limit: usize) -> Result<PullResult, SyncError> {
            // has_more=true but no event sequence progress, so default cursor
            // derivation cannot safely continue.
            Ok(PullResult {
                events: vec![
                    SyncEvent::new("order.updated", "order", "ORD-1", json!({}))
                        .with_remote_sequence(0),
                ],
                remote_head: 100,
                has_more: true,
            })
        }
    }

    let mut engine = SyncEngine::new(make_config()).unwrap();
    let err = engine.pull(&StalledPagingTransport).await.unwrap_err();
    assert!(matches!(err, SyncError::Transport(_)));
}

proptest! {
    #[test]
    fn resolve_next_cursor_enforces_monotonic_progress(
        since in 0u64..20_000,
        transport_cursor in prop::option::of(0u64..20_000),
        sequences in prop::collection::vec(0u64..20_000, 0..64),
    ) {
        let events: Vec<SyncEvent> = sequences
            .iter()
            .enumerate()
            .map(|(i, seq)| {
                SyncEvent::new(
                    format!("evt-{i}"),
                    "entity",
                    format!("id-{i}"),
                    json!({ "s": seq }),
                )
                .with_remote_sequence(*seq)
            })
            .collect();

        let result = SyncEngine::resolve_next_cursor(since, &events, true, transport_cursor);
        if let Some(cursor) = transport_cursor {
            if cursor > since {
                prop_assert_eq!(result.unwrap(), Some(cursor));
            } else {
                prop_assert!(matches!(result, Err(SyncError::Transport(_))));
            }
        } else if let Some(expected) = derive_next_cursor(since, &events) {
            prop_assert_eq!(result.unwrap(), Some(expected));
        } else {
            prop_assert!(matches!(result, Err(SyncError::Transport(_))));
        }
    }
}

proptest! {
    #[test]
    fn resolve_next_cursor_returns_none_when_transport_signals_no_more(
        since in 0u64..20_000,
        transport_cursor in prop::option::of(0u64..20_000),
        sequences in prop::collection::vec(0u64..20_000, 0..64),
    ) {
        let events: Vec<SyncEvent> = sequences
            .iter()
            .enumerate()
            .map(|(i, seq)| {
                SyncEvent::new(
                    format!("evt-{i}"),
                    "entity",
                    format!("id-{i}"),
                    json!({}),
                )
                .with_remote_sequence(*seq)
            })
            .collect();

        let result = SyncEngine::resolve_next_cursor(since, &events, false, transport_cursor)
            .unwrap();
        prop_assert_eq!(result, None);
    }
}

#[test]
fn engine_debug() {
    let engine = SyncEngine::new(make_config()).unwrap();
    let debug = format!("{engine:?}");
    assert!(debug.contains("SyncEngine"));
}
