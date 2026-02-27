use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// A sync event representing a state change in the system.
///
/// This is the Rust equivalent of the JS `OutboxEvent` and the VES v1.0
/// event envelope. Events are immutable once created.
///
/// # Examples
///
/// ```
/// use stateset_sync::SyncEvent;
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
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncEvent {
    /// Unique event identifier.
    pub id: Uuid,
    /// Monotonically increasing sequence number (0 = unassigned).
    pub sequence: u64,
    /// The type of event (e.g. `order.created`, `inventory.adjusted`).
    pub event_type: String,
    /// The entity type this event applies to (e.g. `order`, `customer`).
    pub entity_type: String,
    /// The identifier of the entity.
    pub entity_id: String,
    /// The event payload as a JSON value.
    pub payload: Value,
    /// SHA-256 hash of the canonicalized payload (hex-encoded).
    pub hash: String,
    /// Optional cryptographic signature (hex-encoded Ed25519).
    pub signature: Option<String>,
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
            event_type: event_type.into(),
            entity_type: entity_type.into(),
            entity_id: entity_id.into(),
            payload,
            hash,
            signature: None,
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
            event_type: event_type.into(),
            entity_type: entity_type.into(),
            entity_id: entity_id.into(),
            payload,
            hash,
            signature: None,
            timestamp,
        }
    }

    /// Compute the SHA-256 hash of a JSON payload, hex-encoded.
    #[must_use]
    pub fn compute_hash(payload: &Value) -> String {
        let canonical = canonicalize_json(payload);
        let bytes = serde_json::to_vec(&canonical).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        hex::encode(hasher.finalize())
    }

    /// Assign a sequence number to this event, returning a new event.
    #[must_use]
    pub const fn with_sequence(mut self, sequence: u64) -> Self {
        self.sequence = sequence;
        self
    }

    /// Set the signature on this event.
    #[must_use]
    pub fn with_signature(mut self, signature: impl Into<String>) -> Self {
        self.signature = Some(signature.into());
        self
    }
}

impl PartialOrd for SyncEvent {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SyncEvent {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.sequence
            .cmp(&other.sequence)
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
        assert!(event.signature.is_none());
    }

    #[test]
    fn event_serde_roundtrip() {
        let event =
            SyncEvent::new("product.updated", "product", "PROD-1", json!({"name": "Widget"}));
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: SyncEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, event.id);
        assert_eq!(deserialized.event_type, event.event_type);
        assert_eq!(deserialized.hash, event.hash);
        assert_eq!(deserialized.payload, event.payload);
    }

    #[test]
    fn event_with_sequence() {
        let event = SyncEvent::new("order.created", "order", "ORD-1", json!({})).with_sequence(42);
        assert_eq!(event.sequence, 42);
    }

    #[test]
    fn event_with_signature() {
        let event =
            SyncEvent::new("order.created", "order", "ORD-1", json!({})).with_signature("deadbeef");
        assert_eq!(event.signature, Some("deadbeef".to_string()));
    }

    #[test]
    fn event_ordering_by_sequence() {
        let e1 = SyncEvent::new("a", "x", "1", json!({})).with_sequence(1);
        let e2 = SyncEvent::new("b", "x", "2", json!({})).with_sequence(2);
        let e3 = SyncEvent::new("c", "x", "3", json!({})).with_sequence(3);

        let mut events = vec![e3, e1, e2];
        events.sort();
        assert_eq!(events[0].sequence, 1);
        assert_eq!(events[1].sequence, 2);
        assert_eq!(events[2].sequence, 3);
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
    fn with_id_constructor() {
        let id = Uuid::new_v4();
        let ts = Utc::now();
        let event = SyncEvent::with_id(id, 10, "order.created", "order", "ORD-1", json!({}), ts);
        assert_eq!(event.id, id);
        assert_eq!(event.sequence, 10);
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
