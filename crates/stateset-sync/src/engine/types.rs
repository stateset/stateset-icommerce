//! Public value types produced by the engine: dead letters, push confirmations,
//! and unified kernel receipts.

use super::*;

/// A non-retryable pushed event that the sequencer explicitly rejected.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeadLetter {
    /// The local event that was rejected.
    pub event: SyncEvent,
    /// The rejection metadata reported by the remote.
    pub rejection: PushRejection,
    /// Timestamp when the event was moved out of the outbox.
    pub rejected_at: DateTime<Utc>,
}

impl DeadLetter {
    /// Create a new dead-letter entry for a rejected local event.
    #[must_use]
    pub fn new(event: SyncEvent, rejection: PushRejection) -> Self {
        Self { event, rejection, rejected_at: Utc::now() }
    }
}

/// Durable record that a local event received a canonical remote sequence.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PushConfirmation {
    /// Local event id that was accepted by the sequencer.
    pub event_id: Uuid,
    /// Optional upstream command identifier associated with the event.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command_id: Option<String>,
    /// Event type confirmed by the sequencer.
    pub event_type: String,
    /// Entity type confirmed by the sequencer.
    pub entity_type: String,
    /// Entity id confirmed by the sequencer.
    pub entity_id: String,
    /// Provisional local outbox sequence that originally carried the event, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_sequence: Option<u64>,
    /// Canonical remote sequence assigned by the sequencer.
    pub remote_sequence: u64,
    /// VES payload hash for the confirmed event.
    pub hash: String,
    /// Optional source agent id recorded for the event.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_agent_id: Option<String>,
    /// Optional kernel metadata captured alongside the local transaction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kernel: Option<KernelMetadata>,
    /// Timestamp when the local event was originally recorded.
    #[serde(default = "current_timestamp")]
    pub event_timestamp: DateTime<Utc>,
    /// Optional sequencer receipt handle or hash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receipt: Option<String>,
    /// Timestamp when the confirmation was retained locally.
    pub confirmed_at: DateTime<Utc>,
}

impl PushConfirmation {
    /// Create a retained confirmation from a local event and sequencer acknowledgement.
    #[must_use]
    pub fn from_ack(event: &SyncEvent, acknowledgement: &PushAcknowledgement) -> Self {
        Self {
            event_id: event.id,
            command_id: event.command_id.clone(),
            event_type: event.event_type.clone(),
            entity_type: event.entity_type.clone(),
            entity_id: event.entity_id.clone(),
            local_sequence: event.local_sequence(),
            remote_sequence: acknowledgement.remote_sequence,
            hash: event.hash.clone(),
            source_agent_id: event.source_agent_id.clone(),
            kernel: event.kernel.clone().filter(|kernel| !kernel.is_empty()),
            event_timestamp: event.timestamp,
            receipt: acknowledgement.receipt.clone(),
            confirmed_at: Utc::now(),
        }
    }
}

/// Status of a local-kernel receipt as it converges with the remote sequencer.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum KernelReceiptStatus {
    /// The event exists only in the local outbox and has not reached the sequencer yet.
    LocalPending,
    /// The event was accepted by the remote sequencer and assigned a canonical sequence.
    ConfirmedRemote,
    /// The remote sequencer rejected the event and the rejection was retained locally.
    RejectedRemote,
}

/// Unified receipt view spanning pending, confirmed, and rejected local events.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KernelReceipt {
    /// Local event id associated with this receipt.
    pub event_id: Uuid,
    /// Current convergence state for the event.
    pub status: KernelReceiptStatus,
    /// Optional upstream command identifier associated with the event.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command_id: Option<String>,
    /// Event type captured in the local transaction kernel.
    pub event_type: String,
    /// Entity type captured in the local transaction kernel.
    pub entity_type: String,
    /// Entity id captured in the local transaction kernel.
    pub entity_id: String,
    /// Provisional local outbox sequence assigned to the event, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_sequence: Option<u64>,
    /// Canonical remote sequence assigned by the sequencer, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_sequence: Option<u64>,
    /// Stable event hash.
    pub hash: String,
    /// Optional source agent id recorded in the event envelope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_agent_id: Option<String>,
    /// Optional kernel metadata captured before the mutation was recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kernel: Option<KernelMetadata>,
    /// Timestamp when the local event was created.
    pub event_timestamp: DateTime<Utc>,
    /// Timestamp when the current receipt status was observed locally.
    pub observed_at: DateTime<Utc>,
    /// Optional remote receipt handle associated with sequencer confirmation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_receipt: Option<String>,
    /// Optional remote rejection code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rejection_code: Option<String>,
    /// Optional remote rejection reason.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rejection_reason: Option<String>,
    /// Whether the remote marked the rejection as retryable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retryable: Option<bool>,
}

impl KernelReceipt {
    pub(super) fn from_pending(event: &SyncEvent) -> Self {
        Self {
            event_id: event.id,
            status: KernelReceiptStatus::LocalPending,
            command_id: event.command_id.clone(),
            event_type: event.event_type.clone(),
            entity_type: event.entity_type.clone(),
            entity_id: event.entity_id.clone(),
            local_sequence: event.local_sequence(),
            remote_sequence: None,
            hash: event.hash.clone(),
            source_agent_id: event.source_agent_id.clone(),
            kernel: event.kernel.clone().filter(|kernel| !kernel.is_empty()),
            event_timestamp: event.timestamp,
            observed_at: event.timestamp,
            remote_receipt: None,
            rejection_code: None,
            rejection_reason: None,
            retryable: None,
        }
    }

    pub(super) fn from_confirmation(confirmation: &PushConfirmation) -> Self {
        Self {
            event_id: confirmation.event_id,
            status: KernelReceiptStatus::ConfirmedRemote,
            command_id: confirmation.command_id.clone(),
            event_type: confirmation.event_type.clone(),
            entity_type: confirmation.entity_type.clone(),
            entity_id: confirmation.entity_id.clone(),
            local_sequence: confirmation.local_sequence,
            remote_sequence: Some(confirmation.remote_sequence),
            hash: confirmation.hash.clone(),
            source_agent_id: confirmation.source_agent_id.clone(),
            kernel: confirmation.kernel.clone().filter(|kernel| !kernel.is_empty()),
            event_timestamp: confirmation.event_timestamp,
            observed_at: confirmation.confirmed_at,
            remote_receipt: confirmation.receipt.clone(),
            rejection_code: None,
            rejection_reason: None,
            retryable: None,
        }
    }

    pub(super) fn from_dead_letter(dead_letter: &DeadLetter) -> Self {
        Self {
            event_id: dead_letter.event.id,
            status: KernelReceiptStatus::RejectedRemote,
            command_id: dead_letter.event.command_id.clone(),
            event_type: dead_letter.event.event_type.clone(),
            entity_type: dead_letter.event.entity_type.clone(),
            entity_id: dead_letter.event.entity_id.clone(),
            local_sequence: dead_letter.event.local_sequence(),
            remote_sequence: None,
            hash: dead_letter.event.hash.clone(),
            source_agent_id: dead_letter.event.source_agent_id.clone(),
            kernel: dead_letter.event.kernel.clone().filter(|kernel| !kernel.is_empty()),
            event_timestamp: dead_letter.event.timestamp,
            observed_at: dead_letter.rejected_at,
            remote_receipt: None,
            rejection_code: dead_letter.rejection.code.clone(),
            rejection_reason: dead_letter.rejection.reason.clone(),
            retryable: dead_letter.rejection.retryable,
        }
    }

    pub(crate) fn ordering_key(&self) -> (u64, u64, i64, Uuid) {
        (
            self.local_sequence.unwrap_or(0),
            self.remote_sequence.unwrap_or(0),
            self.observed_at.timestamp_millis(),
            self.event_id,
        )
    }
}

fn current_timestamp() -> DateTime<Utc> {
    Utc::now()
}
