//! Reliable agent-to-agent messaging.
//!
//! Provides typed message envelopes, an in-memory delivery queue with
//! automatic retry scheduling (exponential backoff capped at 16 s), and
//! conversation threading.
//!
//! ## Delivery lifecycle
//!
//! ```text
//! Pending → Delivered → Acknowledged
//!         ↘ Failed (retry until max_attempts) → Expired
//! ```

use std::collections::VecDeque;

use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{A2AError, A2AResult};

// ────────────────────────────────────────────────────────────────────
// Enums
// ────────────────────────────────────────────────────────────────────

/// Lifecycle status of an [`A2AMessage`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum MessageStatus {
    /// Enqueued but not yet sent.
    Pending,
    /// Successfully delivered to the remote agent.
    Delivered,
    /// Remote agent acknowledged receipt.
    Acknowledged,
    /// Delivery failed (may be retried).
    Failed,
    /// All retry attempts exhausted or TTL exceeded.
    Expired,
}

/// Discriminant for the purpose of an [`A2AMessage`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum MessageType {
    /// Request for a price quote.
    QuoteRequest,
    /// Response to a quote request.
    QuoteResponse,
    /// Counter-offer during negotiation.
    CounterOffer,
    /// Declaration of intent to purchase.
    PurchaseIntent,
    /// Notification about a delivery event.
    DeliveryNotification,
    /// Formal dispute notice.
    DisputeNotice,
    /// Application-defined message type.
    Custom(String),
}

// ────────────────────────────────────────────────────────────────────
// Core structs
// ────────────────────────────────────────────────────────────────────

/// A single message exchanged between two agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2AMessage {
    /// Unique message identifier.
    pub id: Uuid,
    /// Conversation this message belongs to.
    pub conversation_id: Uuid,
    /// Sender agent identifier.
    pub from_agent_id: Uuid,
    /// Recipient agent identifier.
    pub to_agent_id: Uuid,
    /// Semantic type of this message.
    pub message_type: MessageType,
    /// JSON-encoded payload.
    pub payload: String,
    /// Current delivery status.
    pub status: MessageStatus,
    /// Monotonically increasing position within the conversation.
    pub sequence_number: u64,
    /// Number of delivery attempts made so far.
    pub attempts: u32,
    /// Maximum delivery attempts before expiry.
    pub max_attempts: u32,
    /// When the next retry should be attempted (if failed).
    pub next_retry_at: Option<DateTime<Utc>>,
    /// When the remote agent acknowledged this message.
    pub acknowledged_at: Option<DateTime<Utc>>,
    /// Human-readable error from the most recent failure.
    pub error: Option<String>,
    /// When this message was created.
    pub created_at: DateTime<Utc>,
}

/// A conversation thread between two agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    /// Unique conversation identifier.
    pub id: Uuid,
    /// First participant.
    pub agent_a: Uuid,
    /// Second participant.
    pub agent_b: Uuid,
    /// Ordered list of messages in this conversation.
    pub messages: Vec<A2AMessage>,
    /// When this conversation was created.
    pub created_at: DateTime<Utc>,
}

// ────────────────────────────────────────────────────────────────────
// MessageQueue
// ────────────────────────────────────────────────────────────────────

/// Maximum backoff interval (in seconds) for retry scheduling.
const MAX_BACKOFF_SECS: i64 = 16;

/// In-memory outbound message queue with retry scheduling.
#[derive(Debug)]
pub struct MessageQueue {
    /// Internal FIFO storage.
    queue: VecDeque<A2AMessage>,
}

impl MessageQueue {
    /// Create an empty queue.
    #[must_use]
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
        }
    }

    /// Add a message to the queue and return its id.
    pub fn enqueue(&mut self, message: A2AMessage) -> Uuid {
        let id = message.id;
        self.queue.push_back(message);
        id
    }

    /// Mark a message as acknowledged.
    ///
    /// # Errors
    ///
    /// Returns [`A2AError::NotFound`] if no message with the given id exists.
    pub fn acknowledge(&mut self, message_id: Uuid) -> A2AResult<()> {
        let msg = self
            .queue
            .iter_mut()
            .find(|m| m.id == message_id)
            .ok_or(A2AError::NotFound { entity: "message" })?;

        msg.status = MessageStatus::Acknowledged;
        msg.acknowledged_at = Some(Utc::now());
        Ok(())
    }

    /// Record a delivery failure for a message.
    ///
    /// If the message has remaining attempts the status is set to
    /// [`MessageStatus::Failed`] and a retry is scheduled using exponential
    /// backoff.  Otherwise the status moves to [`MessageStatus::Expired`].
    ///
    /// # Errors
    ///
    /// Returns [`A2AError::NotFound`] if no message with the given id exists.
    pub fn mark_failed(&mut self, message_id: Uuid, error: String) -> A2AResult<()> {
        let msg = self
            .queue
            .iter_mut()
            .find(|m| m.id == message_id)
            .ok_or(A2AError::NotFound { entity: "message" })?;

        msg.attempts += 1;
        msg.error = Some(error);

        if msg.attempts >= msg.max_attempts {
            msg.status = MessageStatus::Expired;
            msg.next_retry_at = None;
        } else {
            msg.status = MessageStatus::Failed;
            msg.next_retry_at = Some(Self::compute_next_retry(msg.attempts));
        }

        Ok(())
    }

    /// Return all messages that are [`MessageStatus::Pending`].
    #[must_use]
    pub fn get_pending(&self) -> Vec<A2AMessage> {
        self.queue
            .iter()
            .filter(|m| m.status == MessageStatus::Pending)
            .cloned()
            .collect()
    }

    /// Return failed messages whose `next_retry_at` is at or before `now`.
    #[must_use]
    pub fn get_for_retry(&self) -> Vec<A2AMessage> {
        let now = Utc::now();
        self.queue
            .iter()
            .filter(|m| {
                m.status == MessageStatus::Failed
                    && m.next_retry_at.is_some_and(|t| t <= now)
            })
            .cloned()
            .collect()
    }

    /// Compute the next retry timestamp using exponential backoff.
    ///
    /// The delay doubles with each attempt (1 s, 2 s, 4 s, 8 s, 16 s) and is
    /// capped at [`MAX_BACKOFF_SECS`] seconds.
    #[must_use]
    pub fn compute_next_retry(attempts: u32) -> DateTime<Utc> {
        let secs = 1i64
            .checked_shl(attempts.saturating_sub(1))
            .unwrap_or(MAX_BACKOFF_SECS)
            .min(MAX_BACKOFF_SECS);
        Utc::now()
            + TimeDelta::try_seconds(secs)
                .unwrap_or_else(|| TimeDelta::try_seconds(MAX_BACKOFF_SECS).expect("16s is valid"))
    }
}

impl Default for MessageQueue {
    fn default() -> Self {
        Self::new()
    }
}

// ────────────────────────────────────────────────────────────────────
// Tests
// ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a minimal pending message.
    fn make_message() -> A2AMessage {
        A2AMessage {
            id: Uuid::new_v4(),
            conversation_id: Uuid::new_v4(),
            from_agent_id: Uuid::new_v4(),
            to_agent_id: Uuid::new_v4(),
            message_type: MessageType::QuoteRequest,
            payload: r#"{"item":"widget","qty":10}"#.into(),
            status: MessageStatus::Pending,
            sequence_number: 1,
            attempts: 0,
            max_attempts: 5,
            next_retry_at: None,
            acknowledged_at: None,
            error: None,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn enqueue_and_get_pending() {
        let mut q = MessageQueue::new();
        let msg = make_message();
        let id = q.enqueue(msg);

        let pending = q.get_pending();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, id);
        assert_eq!(pending[0].status, MessageStatus::Pending);
    }

    #[test]
    fn acknowledge_marks_delivered() {
        let mut q = MessageQueue::new();
        let msg = make_message();
        let id = q.enqueue(msg);

        q.acknowledge(id).expect("acknowledge should succeed");

        // After acknowledgement the message is no longer pending.
        assert!(q.get_pending().is_empty());

        // And its status is Acknowledged.
        let acked = q.queue.iter().find(|m| m.id == id).expect("message exists");
        assert_eq!(acked.status, MessageStatus::Acknowledged);
        assert!(acked.acknowledged_at.is_some());
    }

    #[test]
    fn failed_message_schedules_retry() {
        let mut q = MessageQueue::new();
        let msg = make_message();
        let id = q.enqueue(msg);

        q.mark_failed(id, "connection refused".into())
            .expect("mark_failed should succeed");

        let entry = q.queue.iter().find(|m| m.id == id).expect("message exists");
        assert_eq!(entry.status, MessageStatus::Failed);
        assert_eq!(entry.attempts, 1);
        assert!(entry.next_retry_at.is_some());
        assert_eq!(entry.error.as_deref(), Some("connection refused"));
    }

    #[test]
    fn exponential_backoff_capped_at_16s() {
        // attempts=1 → 2^0 = 1s
        let t1 = MessageQueue::compute_next_retry(1);
        // attempts=2 → 2^1 = 2s
        let t2 = MessageQueue::compute_next_retry(2);
        // attempts=3 → 2^2 = 4s
        let t3 = MessageQueue::compute_next_retry(3);
        // attempts=4 → 2^3 = 8s
        let t4 = MessageQueue::compute_next_retry(4);
        // attempts=5 → 2^4 = 16s (cap)
        let t5 = MessageQueue::compute_next_retry(5);
        // attempts=6 → still 16s (cap)
        let t6 = MessageQueue::compute_next_retry(6);

        let now = Utc::now();

        // Each successive retry should be further in the future (within a
        // tolerance window of 2 s to account for wall-clock jitter).
        let delta = |t: DateTime<Utc>| (t - now).num_milliseconds();

        assert!(delta(t1) >= 800, "attempt 1 should be ~1 s");
        assert!(delta(t2) >= 1_800, "attempt 2 should be ~2 s");
        assert!(delta(t3) >= 3_800, "attempt 3 should be ~4 s");
        assert!(delta(t4) >= 7_800, "attempt 4 should be ~8 s");
        assert!(delta(t5) >= 15_800, "attempt 5 should be ~16 s");
        // Cap check: attempt 6 should NOT exceed 16 s.
        assert!(delta(t6) <= 18_000, "attempt 6 should be capped at ~16 s");
        assert!(delta(t6) >= 15_800, "attempt 6 should still be ~16 s");
    }
}
