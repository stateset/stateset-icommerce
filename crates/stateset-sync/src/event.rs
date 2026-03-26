use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use stateset_crypto::hash::compute_payload_plain_hash;
use uuid::Uuid;

/// Identifies which system assigned [`SyncEvent::sequence`].
///
/// Local outbox ordering is only meaningful within one agent's pending queue.
/// Canonical remote ordering is assigned by the sequencer and is the only
/// sequence that should drive cross-agent pagination or replication cursors.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Default)]
#[serde(rename_all = "snake_case")]
pub enum SequenceAuthority {
    /// Provisional FIFO ordering assigned by the local outbox.
    #[default]
    LocalOutbox,
    /// Canonical ordering assigned by the remote sequencer.
    CanonicalRemote,
}

/// A sync event representing a state change in the system.
///
/// This is the Rust equivalent of the JS `OutboxEvent` and the VES v1.0
/// event envelope. Events are immutable once created.
///
/// # Examples
///
/// ```
/// use stateset_sync::{SequenceAuthority, SyncEvent};
/// use serde_json::json;
///
/// let event = SyncEvent::new(
///     "order.created",
///     "order",
///     "ORD-123",
///     json!({"total": 99.99}),
/// );
/// assert_eq!(event.event_type, "order.created");
/// assert_eq!(event.entity_type, "order");
/// assert!(!event.hash.is_empty());
/// assert_eq!(event.sequence_authority, SequenceAuthority::LocalOutbox);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncEvent {
    /// Unique event identifier.
    pub id: Uuid,
    /// Monotonically increasing sequence number (0 = unassigned).
    ///
    /// Its meaning depends on [`SyncEvent::sequence_authority`]:
    /// - [`SequenceAuthority::LocalOutbox`]: provisional FIFO position in the local outbox
    /// - [`SequenceAuthority::CanonicalRemote`]: canonical sequencer ordering for replication
    pub sequence: u64,
    /// Which system assigned [`SyncEvent::sequence`].
    #[serde(default)]
    pub sequence_authority: SequenceAuthority,
    /// The type of event (e.g. `order.created`, `inventory.adjusted`).
    pub event_type: String,
    /// The entity type this event applies to (e.g. `order`, `customer`).
    pub entity_type: String,
    /// The identifier of the entity.
    pub entity_id: String,
    /// The event payload as a JSON value.
    pub payload: Value,
    /// VES payload-plain hash for the payload (hex-encoded).
    pub hash: String,
    /// Optional cryptographic signature (hex-encoded Ed25519).
    pub signature: Option<String>,
    /// Optional upstream command identifier associated with the event.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command_id: Option<String>,
    /// Optional optimistic concurrency base version for the mutated entity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_version: Option<u64>,
    /// Optional source agent id recorded in the VES envelope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_agent_id: Option<String>,
    /// Optional agent key id used for the recorded signature.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_key_id: Option<u32>,
    /// Timestamp when the event was created.
    pub timestamp: DateTime<Utc>,
}

impl SyncEvent {
    /// Create a new `SyncEvent` with an auto-generated id, hash, and timestamp.
    #[must_use]
    pub fn new(
        event_type: impl Into<String>,
        entity_type: impl Into<String>,
        entity_id: impl Into<String>,
        payload: Value,
    ) -> Self {
        let hash = Self::compute_hash(&payload);
        Self {
            id: Uuid::new_v4(),
            sequence: 0,
            sequence_authority: SequenceAuthority::LocalOutbox,
            event_type: event_type.into(),
            entity_type: entity_type.into(),
            entity_id: entity_id.into(),
            payload,
            hash,
            signature: None,
            command_id: None,
            base_version: None,
            source_agent_id: None,
            agent_key_id: None,
            timestamp: Utc::now(),
        }
    }

    /// Create a `SyncEvent` with an explicit id and sequence.
    #[must_use]
    pub fn with_id(
        id: Uuid,
        sequence: u64,
        event_type: impl Into<String>,
        entity_type: impl Into<String>,
        entity_id: impl Into<String>,
        payload: Value,
        timestamp: DateTime<Utc>,
    ) -> Self {
        let hash = Self::compute_hash(&payload);
        Self {
            id,
            sequence,
            sequence_authority: SequenceAuthority::LocalOutbox,
            event_type: event_type.into(),
            entity_type: entity_type.into(),
            entity_id: entity_id.into(),
            payload,
            hash,
            signature: None,
            command_id: None,
            base_version: None,
            source_agent_id: None,
            agent_key_id: None,
            timestamp,
        }
    }

    /// Compute the VES payload-plain hash of a JSON payload, hex-encoded.
    #[must_use]
    pub fn compute_hash(payload: &Value) -> String {
        if let Ok(hash) = compute_payload_plain_hash(payload, None) { hex::encode(hash) } else {
            let canonical = canonicalize_json(payload);
            let bytes = serde_json::to_vec(&canonical).unwrap_or_default();
            let mut hasher = Sha256::new();
            hasher.update(&bytes);
            hex::encode(hasher.finalize())
        }
    }

    /// Assign a provisional local outbox sequence number to this event.
    #[must_use]
    pub const fn with_local_sequence(mut self, sequence: u64) -> Self {
        self.sequence = sequence;
        self.sequence_authority = SequenceAuthority::LocalOutbox;
        self
    }

    /// Assign a canonical remote sequencer number to this event.
    #[must_use]
    pub const fn with_remote_sequence(mut self, sequence: u64) -> Self {
        self.sequence = sequence;
        self.sequence_authority = SequenceAuthority::CanonicalRemote;
        self
    }

    /// Assign a sequence number using local-outbox semantics.
    ///
    /// Prefer [`SyncEvent::with_local_sequence`] or
    /// [`SyncEvent::with_remote_sequence`] for new code.
    #[must_use]
    pub const fn with_sequence(self, sequence: u64) -> Self {
        self.with_local_sequence(sequence)
    }

    /// Set the signature on this event.
    #[must_use]
    pub fn with_signature(mut self, signature: impl Into<String>) -> Self {
        self.signature = Some(signature.into());
        self
    }

    /// Attach an upstream command identifier to the event.
    #[must_use]
    pub fn with_command_id(mut self, command_id: impl Into<String>) -> Self {
        self.command_id = Some(command_id.into());
        self
    }

    /// Attach the optimistic-concurrency base version used to create the event.
    #[must_use]
    pub const fn with_base_version(mut self, base_version: u64) -> Self {
        self.base_version = Some(base_version);
        self
    }

    /// Attach the source agent id recorded in the VES envelope.
    #[must_use]
    pub fn with_source_agent_id(mut self, source_agent_id: impl Into<String>) -> Self {
        self.source_agent_id = Some(source_agent_id.into());
        self
    }

    /// Attach the signing key id recorded in the VES envelope.
    #[must_use]
    pub const fn with_agent_key_id(mut self, agent_key_id: u32) -> Self {
        self.agent_key_id = Some(agent_key_id);
        self
    }

    /// Return the canonical remote sequence, if this event has one.
    #[must_use]
    pub const fn canonical_sequence(&self) -> Option<u64> {
        match self.sequence_authority {
            SequenceAuthority::CanonicalRemote if self.sequence > 0 => Some(self.sequence),
            SequenceAuthority::CanonicalRemote | SequenceAuthority::LocalOutbox => None,
        }
    }

    /// Return the provisional local outbox sequence, if this event has one.
    #[must_use]
    pub const fn local_sequence(&self) -> Option<u64> {
        match self.sequence_authority {
            SequenceAuthority::LocalOutbox if self.sequence > 0 => Some(self.sequence),
            SequenceAuthority::LocalOutbox | SequenceAuthority::CanonicalRemote => None,
        }
    }

    /// Whether this event has a canonical remote sequence assigned by the sequencer.
    #[must_use]
    pub const fn is_canonical_remote(&self) -> bool {
        matches!(self.sequence_authority, SequenceAuthority::CanonicalRemote)
    }
}

impl PartialOrd for SyncEvent {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SyncEvent {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.sequence_authority
            .cmp(&other.sequence_authority)
            .then_with(|| self.sequence.cmp(&other.sequence))
            .then_with(|| self.timestamp.cmp(&other.timestamp))
            .then_with(|| self.id.cmp(&other.id))
    }
}

fn canonicalize_json(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort_unstable();

            let mut canonical = Map::with_capacity(map.len());
            for key in keys {
                if let Some(inner) = map.get(key) {
                    canonical.insert(key.clone(), canonicalize_json(inner));
                }
            }
            Value::Object(canonical)
        }
        Value::Array(values) => Value::Array(values.iter().map(canonicalize_json).collect()),
        _ => value.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn new_event_has_uuid_and_hash() {
        let event = SyncEvent::new("order.created", "order", "ORD-1", json!({"total": 10}));
        assert!(!event.id.is_nil());
        assert!(!event.hash.is_empty());
        assert_eq!(event.hash.len(), 64); // SHA-256 hex
        assert_eq!(event.sequence, 0);
        assert_eq!(event.sequence_authority, SequenceAuthority::LocalOutbox);
        assert!(event.signature.is_none());
        assert!(event.command_id.is_none());
        assert!(event.base_version.is_none());
        assert!(event.source_agent_id.is_none());
        assert!(event.agent_key_id.is_none());
    }

    #[test]
    fn event_serde_roundtrip() {
        let event =
            SyncEvent::new("product.updated", "product", "PROD-1", json!({"name": "Widget"}))
                .with_command_id("cmd-1")
                .with_base_version(7)
                .with_source_agent_id("agent-7")
                .with_agent_key_id(11);
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: SyncEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, event.id);
        assert_eq!(deserialized.event_type, event.event_type);
        assert_eq!(deserialized.hash, event.hash);
        assert_eq!(deserialized.payload, event.payload);
        assert_eq!(deserialized.sequence_authority, SequenceAuthority::LocalOutbox);
        assert_eq!(deserialized.command_id.as_deref(), Some("cmd-1"));
        assert_eq!(deserialized.base_version, Some(7));
        assert_eq!(deserialized.source_agent_id.as_deref(), Some("agent-7"));
        assert_eq!(deserialized.agent_key_id, Some(11));
    }

    #[test]
    fn compute_hash_matches_ves_payload_plain_hash() {
        let payload = json!({"b": 2, "a": 1});
        let expected = hex::encode(compute_payload_plain_hash(&payload, None).unwrap());
        assert_eq!(SyncEvent::compute_hash(&payload), expected);
    }

    #[test]
    fn with_local_sequence_marks_provisional_ordering() {
        let event =
            SyncEvent::new("order.created", "order", "ORD-1", json!({})).with_local_sequence(42);
        assert_eq!(event.sequence, 42);
        assert_eq!(event.local_sequence(), Some(42));
        assert_eq!(event.canonical_sequence(), None);
    }

    #[test]
    fn with_remote_sequence_marks_canonical_ordering() {
        let event =
            SyncEvent::new("order.created", "order", "ORD-1", json!({})).with_remote_sequence(7);
        assert_eq!(event.sequence, 7);
        assert_eq!(event.sequence_authority, SequenceAuthority::CanonicalRemote);
        assert_eq!(event.canonical_sequence(), Some(7));
        assert_eq!(event.local_sequence(), None);
        assert!(event.is_canonical_remote());
    }

    #[test]
    fn event_with_signature() {
        let event =
            SyncEvent::new("order.created", "order", "ORD-1", json!({})).with_signature("deadbeef");
        assert_eq!(event.signature, Some("deadbeef".to_string()));
    }

    #[test]
    fn event_ordering_by_authority_then_sequence() {
        let local = SyncEvent::new("a", "x", "1", json!({})).with_local_sequence(99);
        let remote_low = SyncEvent::new("b", "x", "2", json!({})).with_remote_sequence(1);
        let remote_high = SyncEvent::new("c", "x", "3", json!({})).with_remote_sequence(2);

        let mut events = vec![remote_high, local, remote_low];
        events.sort();
        assert_eq!(events[0].sequence_authority, SequenceAuthority::LocalOutbox);
        assert_eq!(events[1].canonical_sequence(), Some(1));
        assert_eq!(events[2].canonical_sequence(), Some(2));
    }

    #[test]
    fn same_payload_same_hash() {
        let payload = json!({"key": "value"});
        let e1 = SyncEvent::new("a", "x", "1", payload.clone());
        let e2 = SyncEvent::new("b", "y", "2", payload);
        assert_eq!(e1.hash, e2.hash);
    }

    #[test]
    fn different_payload_different_hash() {
        let e1 = SyncEvent::new("a", "x", "1", json!({"key": "value1"}));
        let e2 = SyncEvent::new("a", "x", "1", json!({"key": "value2"}));
        assert_ne!(e1.hash, e2.hash);
    }

    #[test]
    fn with_id_constructor_defaults_to_local_authority() {
        let id = Uuid::new_v4();
        let ts = Utc::now();
        let event = SyncEvent::with_id(id, 10, "order.created", "order", "ORD-1", json!({}), ts);
        assert_eq!(event.id, id);
        assert_eq!(event.sequence, 10);
        assert_eq!(event.sequence_authority, SequenceAuthority::LocalOutbox);
        assert_eq!(event.timestamp, ts);
    }

    #[test]
    fn event_eq() {
        let id = Uuid::new_v4();
        let ts = Utc::now();
        let e1 = SyncEvent::with_id(id, 1, "a", "b", "c", json!({}), ts);
        let e2 = SyncEvent::with_id(id, 1, "a", "b", "c", json!({}), ts);
        assert_eq!(e1, e2);
    }

    #[test]
    fn event_debug() {
        let event = SyncEvent::new("test", "entity", "id", json!({}));
        let debug = format!("{event:?}");
        assert!(debug.contains("SyncEvent"));
    }

    #[test]
    fn compute_hash_deterministic() {
        let payload = json!({"a": 1, "b": 2});
        let h1 = SyncEvent::compute_hash(&payload);
        let h2 = SyncEvent::compute_hash(&payload);
        assert_eq!(h1, h2);
    }

    #[test]
    fn compute_hash_is_canonical_for_object_key_order() {
        let p1 = json!({"a": 1, "b": 2, "c": {"x": 1, "y": 2}});
        let p2 = json!({"c": {"y": 2, "x": 1}, "b": 2, "a": 1});
        assert_eq!(SyncEvent::compute_hash(&p1), SyncEvent::compute_hash(&p2));
    }
}
