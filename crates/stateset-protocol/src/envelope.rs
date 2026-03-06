//! Event envelope wire type.
//!
//! [`EventEnvelope`] is the canonical on-the-wire representation of a single
//! domain event. It carries metadata (correlation, causation, versioning) plus
//! a codec-tagged payload with its SHA-256 hash.
//!
//! # Example
//!
//! ```rust
//! use stateset_protocol::{EventEnvelope, PayloadCodec};
//!
//! let envelope = EventEnvelope::builder()
//!     .event_type("order.created")
//!     .entity_type("order")
//!     .entity_id("ord_123")
//!     .payload(br#"{"total": 99.99}"#.to_vec())
//!     .build()
//!     .unwrap();
//!
//! assert_eq!(envelope.event_type, "order.created");
//! assert_eq!(envelope.payload_codec, PayloadCodec::Json);
//! assert!(envelope.validate().is_ok());
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::error::{ProtocolError, Result};

/// The codec used to encode the event payload bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PayloadCodec {
    /// JSON encoding (UTF-8).
    Json,
    /// CBOR binary encoding.
    Cbor,
    /// `MessagePack` binary encoding.
    MessagePack,
}

impl std::fmt::Display for PayloadCodec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Json => write!(f, "json"),
            Self::Cbor => write!(f, "cbor"),
            Self::MessagePack => write!(f, "message_pack"),
        }
    }
}

/// A single event in wire format.
///
/// Contains all metadata needed for ordering, correlation, and integrity
/// verification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct EventEnvelope {
    /// Unique event identifier.
    pub id: Uuid,
    /// Monotonically increasing sequence number.
    pub sequence: u64,
    /// When the event occurred.
    pub timestamp: DateTime<Utc>,
    /// Optional correlation ID for tracing related events.
    pub correlation_id: Option<Uuid>,
    /// Optional causation ID linking to the causing event.
    pub causation_id: Option<Uuid>,
    /// The type of event (e.g., `"order.created"`).
    pub event_type: String,
    /// The type of entity this event applies to (e.g., `"order"`).
    pub entity_type: String,
    /// The identifier of the entity.
    pub entity_id: String,
    /// SHA-256 hash of the payload bytes.
    pub payload_hash: [u8; 32],
    /// The codec used to encode the payload.
    pub payload_codec: PayloadCodec,
    /// The raw payload bytes.
    pub payload: Vec<u8>,
    /// Protocol version for forward compatibility.
    pub protocol_version: u16,
    /// Schema version for the event type.
    pub schema_version: u16,
}

impl EventEnvelope {
    /// Create a new builder for constructing an [`EventEnvelope`].
    ///
    /// ```rust
    /// use stateset_protocol::EventEnvelope;
    ///
    /// let builder = EventEnvelope::builder();
    /// ```
    #[must_use]
    pub fn builder() -> EventEnvelopeBuilder {
        EventEnvelopeBuilder::default()
    }

    /// Validate that the envelope has all required fields populated.
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolError::InvalidEnvelope`] if any required field is
    /// empty or if the payload hash does not match.
    ///
    /// ```rust
    /// use stateset_protocol::EventEnvelope;
    ///
    /// let envelope = EventEnvelope::builder()
    ///     .event_type("test.event")
    ///     .entity_type("test")
    ///     .entity_id("t_1")
    ///     .payload(b"{}".to_vec())
    ///     .build()
    ///     .unwrap();
    /// assert!(envelope.validate().is_ok());
    /// ```
    pub fn validate(&self) -> Result<()> {
        if self.protocol_version != 1 {
            return Err(ProtocolError::UnsupportedVersion(format!(
                "unsupported envelope protocol_version {} (expected 1)",
                self.protocol_version
            )));
        }
        if self.schema_version == 0 {
            return Err(ProtocolError::InvalidEnvelope("schema_version must be >= 1".into()));
        }
        validate_required_str("event_type", &self.event_type)?;
        validate_required_str("entity_type", &self.entity_type)?;
        validate_required_str("entity_id", &self.entity_id)?;
        if self.payload.is_empty() {
            return Err(ProtocolError::InvalidEnvelope("payload must not be empty".into()));
        }

        // Verify payload hash
        let computed = Self::compute_payload_hash(&self.payload);
        if computed != self.payload_hash {
            return Err(ProtocolError::InvalidEnvelope(
                "payload_hash does not match payload".into(),
            ));
        }

        Ok(())
    }

    /// Compute the SHA-256 hash of a payload.
    ///
    /// ```rust
    /// use stateset_protocol::EventEnvelope;
    ///
    /// let hash = EventEnvelope::compute_payload_hash(b"hello");
    /// assert_eq!(hash.len(), 32);
    /// ```
    #[must_use]
    pub fn compute_payload_hash(payload: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(payload);
        hasher.finalize().into()
    }

    /// Compute the Merkle leaf hash that binds full envelope integrity.
    ///
    /// This hash covers event metadata, payload hash, payload bytes, codec, and
    /// protocol/schema versions. Any envelope mutation changes the leaf hash.
    #[must_use]
    pub fn merkle_leaf_hash(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(b"stateset:event-envelope-leaf:v2");
        hasher.update([0x00]);
        hasher.update(self.id.as_bytes());
        hasher.update(self.sequence.to_be_bytes());
        hasher.update(self.timestamp.timestamp().to_be_bytes());
        hasher.update(self.timestamp.timestamp_subsec_nanos().to_be_bytes());
        update_optional_uuid(&mut hasher, self.correlation_id);
        update_optional_uuid(&mut hasher, self.causation_id);
        update_len_prefixed(&mut hasher, self.event_type.as_bytes());
        update_len_prefixed(&mut hasher, self.entity_type.as_bytes());
        update_len_prefixed(&mut hasher, self.entity_id.as_bytes());
        hasher.update(self.payload_hash);
        hasher.update([payload_codec_tag(self.payload_codec)]);
        hasher.update(self.protocol_version.to_be_bytes());
        hasher.update(self.schema_version.to_be_bytes());
        update_len_prefixed(&mut hasher, &self.payload);
        hasher.finalize().into()
    }
}

fn validate_required_str(field: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        return Err(ProtocolError::InvalidEnvelope(format!("{field} must not be empty")));
    }
    Ok(())
}

impl PartialOrd for EventEnvelope {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for EventEnvelope {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.sequence
            .cmp(&other.sequence)
            .then_with(|| self.timestamp.cmp(&other.timestamp))
            .then_with(|| self.id.cmp(&other.id))
            .then_with(|| self.correlation_id.cmp(&other.correlation_id))
            .then_with(|| self.causation_id.cmp(&other.causation_id))
            .then_with(|| self.event_type.cmp(&other.event_type))
            .then_with(|| self.entity_type.cmp(&other.entity_type))
            .then_with(|| self.entity_id.cmp(&other.entity_id))
            .then_with(|| self.payload_hash.cmp(&other.payload_hash))
            .then_with(|| self.payload_codec.cmp(&other.payload_codec))
            .then_with(|| self.payload.cmp(&other.payload))
            .then_with(|| self.protocol_version.cmp(&other.protocol_version))
            .then_with(|| self.schema_version.cmp(&other.schema_version))
    }
}

fn update_len_prefixed(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn update_optional_uuid(hasher: &mut Sha256, value: Option<Uuid>) {
    match value {
        Some(id) => {
            hasher.update([1]);
            hasher.update(id.as_bytes());
        }
        None => hasher.update([0]),
    }
}

const fn payload_codec_tag(codec: PayloadCodec) -> u8 {
    match codec {
        PayloadCodec::Json => 1,
        PayloadCodec::Cbor => 2,
        PayloadCodec::MessagePack => 3,
    }
}

/// Builder for constructing [`EventEnvelope`] instances.
///
/// Uses the builder pattern to ensure all required fields are set before
/// construction. Optional fields have sensible defaults.
///
/// # Example
///
/// ```rust
/// use stateset_protocol::{EventEnvelope, PayloadCodec};
/// use uuid::Uuid;
///
/// let envelope = EventEnvelope::builder()
///     .event_type("inventory.adjusted")
///     .entity_type("inventory")
///     .entity_id("inv_456")
///     .payload(br#"{"qty": 10}"#.to_vec())
///     .codec(PayloadCodec::Json)
///     .correlation_id(Uuid::new_v4())
///     .sequence(42)
///     .build()
///     .unwrap();
/// ```
#[derive(Debug, Default)]
pub struct EventEnvelopeBuilder {
    id: Option<Uuid>,
    sequence: Option<u64>,
    timestamp: Option<DateTime<Utc>>,
    correlation_id: Option<Uuid>,
    causation_id: Option<Uuid>,
    event_type: Option<String>,
    entity_type: Option<String>,
    entity_id: Option<String>,
    payload_codec: Option<PayloadCodec>,
    payload: Option<Vec<u8>>,
    protocol_version: Option<u16>,
    schema_version: Option<u16>,
}

impl EventEnvelopeBuilder {
    /// Set the envelope ID. Defaults to a new `UUIDv4` if not set.
    #[must_use]
    pub const fn id(mut self, id: Uuid) -> Self {
        self.id = Some(id);
        self
    }

    /// Set the sequence number. Defaults to 0 if not set.
    #[must_use]
    pub const fn sequence(mut self, seq: u64) -> Self {
        self.sequence = Some(seq);
        self
    }

    /// Set the timestamp. Defaults to `Utc::now()` if not set.
    #[must_use]
    pub const fn timestamp(mut self, ts: DateTime<Utc>) -> Self {
        self.timestamp = Some(ts);
        self
    }

    /// Set the correlation ID for tracing.
    #[must_use]
    pub const fn correlation_id(mut self, id: Uuid) -> Self {
        self.correlation_id = Some(id);
        self
    }

    /// Set the causation ID linking to the causing event.
    #[must_use]
    pub const fn causation_id(mut self, id: Uuid) -> Self {
        self.causation_id = Some(id);
        self
    }

    /// Set the event type (required).
    #[must_use]
    pub fn event_type(mut self, et: &str) -> Self {
        self.event_type = Some(et.to_owned());
        self
    }

    /// Set the entity type (required).
    #[must_use]
    pub fn entity_type(mut self, et: &str) -> Self {
        self.entity_type = Some(et.to_owned());
        self
    }

    /// Set the entity ID (required).
    #[must_use]
    pub fn entity_id(mut self, id: &str) -> Self {
        self.entity_id = Some(id.to_owned());
        self
    }

    /// Set the payload codec. Defaults to [`PayloadCodec::Json`].
    #[must_use]
    pub const fn codec(mut self, codec: PayloadCodec) -> Self {
        self.payload_codec = Some(codec);
        self
    }

    /// Set the payload bytes (required).
    #[must_use]
    pub fn payload(mut self, payload: Vec<u8>) -> Self {
        self.payload = Some(payload);
        self
    }

    /// Set the protocol version. Defaults to 1.
    #[must_use]
    pub const fn protocol_version(mut self, v: u16) -> Self {
        self.protocol_version = Some(v);
        self
    }

    /// Set the schema version. Defaults to 1.
    #[must_use]
    pub const fn schema_version(mut self, v: u16) -> Self {
        self.schema_version = Some(v);
        self
    }

    /// Build the [`EventEnvelope`].
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolError::InvalidEnvelope`] if required fields are missing.
    pub fn build(self) -> Result<EventEnvelope> {
        let event_type = self
            .event_type
            .ok_or_else(|| ProtocolError::InvalidEnvelope("event_type is required".into()))?;
        let entity_type = self
            .entity_type
            .ok_or_else(|| ProtocolError::InvalidEnvelope("entity_type is required".into()))?;
        let entity_id = self
            .entity_id
            .ok_or_else(|| ProtocolError::InvalidEnvelope("entity_id is required".into()))?;
        let payload = self
            .payload
            .ok_or_else(|| ProtocolError::InvalidEnvelope("payload is required".into()))?;

        let payload_hash = EventEnvelope::compute_payload_hash(&payload);

        Ok(EventEnvelope {
            id: self.id.unwrap_or_else(Uuid::new_v4),
            sequence: self.sequence.unwrap_or(0),
            timestamp: self.timestamp.unwrap_or_else(Utc::now),
            correlation_id: self.correlation_id,
            causation_id: self.causation_id,
            event_type,
            entity_type,
            entity_id,
            payload_hash,
            payload_codec: self.payload_codec.unwrap_or(PayloadCodec::Json),
            payload,
            protocol_version: self.protocol_version.unwrap_or(1),
            schema_version: self.schema_version.unwrap_or(1),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_envelope() -> EventEnvelope {
        EventEnvelope::builder()
            .event_type("order.created")
            .entity_type("order")
            .entity_id("ord_123")
            .payload(br#"{"total": 99.99}"#.to_vec())
            .build()
            .unwrap()
    }

    // --- PayloadCodec tests ---

    #[test]
    fn payload_codec_display() {
        assert_eq!(PayloadCodec::Json.to_string(), "json");
        assert_eq!(PayloadCodec::Cbor.to_string(), "cbor");
        assert_eq!(PayloadCodec::MessagePack.to_string(), "message_pack");
    }

    #[test]
    fn payload_codec_serde_roundtrip() {
        for codec in [PayloadCodec::Json, PayloadCodec::Cbor, PayloadCodec::MessagePack] {
            let json = serde_json::to_string(&codec).unwrap();
            let deserialized: PayloadCodec = serde_json::from_str(&json).unwrap();
            assert_eq!(deserialized, codec);
        }
    }

    #[test]
    fn payload_codec_json_serde_value() {
        let json = serde_json::to_string(&PayloadCodec::Json).unwrap();
        assert_eq!(json, r#""json""#);
    }

    #[test]
    fn payload_codec_cbor_serde_value() {
        let json = serde_json::to_string(&PayloadCodec::Cbor).unwrap();
        assert_eq!(json, r#""cbor""#);
    }

    #[test]
    fn payload_codec_msgpack_serde_value() {
        let json = serde_json::to_string(&PayloadCodec::MessagePack).unwrap();
        assert_eq!(json, r#""message_pack""#);
    }

    // --- Builder tests ---

    #[test]
    fn builder_minimal() {
        let env = sample_envelope();
        assert_eq!(env.event_type, "order.created");
        assert_eq!(env.entity_type, "order");
        assert_eq!(env.entity_id, "ord_123");
        assert_eq!(env.payload_codec, PayloadCodec::Json);
        assert_eq!(env.protocol_version, 1);
        assert_eq!(env.schema_version, 1);
        assert_eq!(env.sequence, 0);
        assert!(env.correlation_id.is_none());
        assert!(env.causation_id.is_none());
    }

    #[test]
    fn builder_all_fields() {
        let id = Uuid::new_v4();
        let corr = Uuid::new_v4();
        let cause = Uuid::new_v4();
        let ts = Utc::now();

        let env = EventEnvelope::builder()
            .id(id)
            .sequence(42)
            .timestamp(ts)
            .correlation_id(corr)
            .causation_id(cause)
            .event_type("return.requested")
            .entity_type("return")
            .entity_id("ret_1")
            .codec(PayloadCodec::Cbor)
            .payload(vec![0xA0]) // CBOR empty map
            .protocol_version(2)
            .schema_version(3)
            .build()
            .unwrap();

        assert_eq!(env.id, id);
        assert_eq!(env.sequence, 42);
        assert_eq!(env.timestamp, ts);
        assert_eq!(env.correlation_id, Some(corr));
        assert_eq!(env.causation_id, Some(cause));
        assert_eq!(env.event_type, "return.requested");
        assert_eq!(env.entity_type, "return");
        assert_eq!(env.entity_id, "ret_1");
        assert_eq!(env.payload_codec, PayloadCodec::Cbor);
        assert_eq!(env.payload, vec![0xA0]);
        assert_eq!(env.protocol_version, 2);
        assert_eq!(env.schema_version, 3);
    }

    #[test]
    fn builder_missing_event_type() {
        let result = EventEnvelope::builder()
            .entity_type("order")
            .entity_id("o_1")
            .payload(b"{}".to_vec())
            .build();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ProtocolError::InvalidEnvelope(_)));
    }

    #[test]
    fn builder_missing_entity_type() {
        let result = EventEnvelope::builder()
            .event_type("x.y")
            .entity_id("o_1")
            .payload(b"{}".to_vec())
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn builder_missing_entity_id() {
        let result = EventEnvelope::builder()
            .event_type("x.y")
            .entity_type("x")
            .payload(b"{}".to_vec())
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn builder_missing_payload() {
        let result =
            EventEnvelope::builder().event_type("x.y").entity_type("x").entity_id("x_1").build();
        assert!(result.is_err());
    }

    #[test]
    fn builder_auto_computes_hash() {
        let payload = b"test payload data".to_vec();
        let expected_hash = EventEnvelope::compute_payload_hash(&payload);
        let env = EventEnvelope::builder()
            .event_type("t")
            .entity_type("t")
            .entity_id("1")
            .payload(payload)
            .build()
            .unwrap();
        assert_eq!(env.payload_hash, expected_hash);
    }

    #[test]
    fn builder_auto_generates_id() {
        let e1 = sample_envelope();
        let e2 = sample_envelope();
        // UUIDs should be different (extremely high probability)
        assert_ne!(e1.id, e2.id);
    }

    // --- Validation tests ---

    #[test]
    fn validate_valid_envelope() {
        let env = sample_envelope();
        assert!(env.validate().is_ok());
    }

    #[test]
    fn validate_empty_event_type() {
        let mut env = sample_envelope();
        env.event_type = String::new();
        assert!(env.validate().is_err());
    }

    #[test]
    fn validate_whitespace_event_type() {
        let mut env = sample_envelope();
        env.event_type = "   ".to_string();
        assert!(env.validate().is_err());
    }

    #[test]
    fn validate_empty_entity_type() {
        let mut env = sample_envelope();
        env.entity_type = String::new();
        assert!(env.validate().is_err());
    }

    #[test]
    fn validate_whitespace_entity_type() {
        let mut env = sample_envelope();
        env.entity_type = "\t".to_string();
        assert!(env.validate().is_err());
    }

    #[test]
    fn validate_empty_entity_id() {
        let mut env = sample_envelope();
        env.entity_id = String::new();
        assert!(env.validate().is_err());
    }

    #[test]
    fn validate_whitespace_entity_id() {
        let mut env = sample_envelope();
        env.entity_id = "  ".to_string();
        assert!(env.validate().is_err());
    }

    #[test]
    fn validate_empty_payload() {
        let mut env = sample_envelope();
        env.payload = Vec::new();
        assert!(env.validate().is_err());
    }

    #[test]
    fn validate_rejects_unsupported_protocol_version() {
        let mut env = sample_envelope();
        env.protocol_version = 2;
        assert!(matches!(env.validate(), Err(ProtocolError::UnsupportedVersion(_))));
    }

    #[test]
    fn validate_rejects_zero_schema_version() {
        let mut env = sample_envelope();
        env.schema_version = 0;
        assert!(matches!(env.validate(), Err(ProtocolError::InvalidEnvelope(_))));
    }

    #[test]
    fn validate_wrong_hash() {
        let mut env = sample_envelope();
        env.payload_hash = [0xFF; 32]; // tamper with hash
        let result = env.validate();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("payload_hash"));
    }

    // --- compute_payload_hash tests ---

    #[test]
    fn compute_payload_hash_deterministic() {
        let h1 = EventEnvelope::compute_payload_hash(b"test");
        let h2 = EventEnvelope::compute_payload_hash(b"test");
        assert_eq!(h1, h2);
    }

    #[test]
    fn compute_payload_hash_different_data() {
        let h1 = EventEnvelope::compute_payload_hash(b"data1");
        let h2 = EventEnvelope::compute_payload_hash(b"data2");
        assert_ne!(h1, h2);
    }

    #[test]
    fn compute_payload_hash_length() {
        let h = EventEnvelope::compute_payload_hash(b"anything");
        assert_eq!(h.len(), 32);
    }

    #[test]
    fn merkle_leaf_hash_deterministic() {
        let env = sample_envelope();
        let h1 = env.merkle_leaf_hash();
        let h2 = env.merkle_leaf_hash();
        assert_eq!(h1, h2);
    }

    #[test]
    fn merkle_leaf_hash_changes_when_metadata_changes() {
        let mut env1 = sample_envelope();
        let mut env2 = env1.clone();
        env2.event_type = "order.cancelled".to_string();
        assert_ne!(env1.merkle_leaf_hash(), env2.merkle_leaf_hash());

        env1.payload = b"{\"total\":1}".to_vec();
        env1.payload_hash = EventEnvelope::compute_payload_hash(&env1.payload);
        assert_ne!(env1.merkle_leaf_hash(), env2.merkle_leaf_hash());
    }

    // --- Serde round-trip tests ---

    #[test]
    fn serde_roundtrip_json() {
        let env = sample_envelope();
        let json = serde_json::to_string(&env).unwrap();
        let deserialized: EventEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, env);
    }

    #[test]
    fn serde_roundtrip_with_optionals() {
        let env = EventEnvelope::builder()
            .event_type("test")
            .entity_type("test")
            .entity_id("1")
            .payload(b"data".to_vec())
            .correlation_id(Uuid::new_v4())
            .causation_id(Uuid::new_v4())
            .build()
            .unwrap();

        let json = serde_json::to_string(&env).unwrap();
        let deserialized: EventEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.correlation_id, env.correlation_id);
        assert_eq!(deserialized.causation_id, env.causation_id);
    }

    #[test]
    fn serde_roundtrip_without_optionals() {
        let env = sample_envelope();
        let json = serde_json::to_string(&env).unwrap();
        let deserialized: EventEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.correlation_id, None);
        assert_eq!(deserialized.causation_id, None);
    }

    // --- Ordering tests ---

    #[test]
    fn ordering_by_sequence() {
        let mut e1 = sample_envelope();
        let mut e2 = sample_envelope();
        e1.sequence = 1;
        e2.sequence = 2;
        assert!(e1 < e2);
    }

    #[test]
    fn ordering_same_sequence_by_timestamp() {
        let ts1 = DateTime::parse_from_rfc3339("2025-01-01T00:00:00Z").unwrap().with_timezone(&Utc);
        let ts2 = DateTime::parse_from_rfc3339("2025-01-02T00:00:00Z").unwrap().with_timezone(&Utc);

        let mut e1 = sample_envelope();
        let mut e2 = sample_envelope();
        e1.sequence = 1;
        e1.timestamp = ts1;
        e2.sequence = 1;
        e2.timestamp = ts2;
        assert!(e1 < e2);
    }

    #[test]
    fn ordering_equal() {
        let ts = Utc::now();
        let id = Uuid::new_v4();
        let mut e1 = sample_envelope();
        let mut e2 = sample_envelope();
        e1.sequence = 5;
        e1.timestamp = ts;
        e1.id = id;
        e2.sequence = 5;
        e2.timestamp = ts;
        e2.id = id;
        // Same sequence+timestamp => Equal ordering
        assert_eq!(e1.cmp(&e2), std::cmp::Ordering::Equal);
    }

    #[test]
    fn ordering_tiebreakers_keep_cmp_consistent_with_eq() {
        let e1 = sample_envelope();
        let mut e2 = e1.clone();
        e2.payload = br#"{"different":true}"#.to_vec();
        e2.payload_hash = EventEnvelope::compute_payload_hash(&e2.payload);
        assert_ne!(e1, e2);
        assert_ne!(e1.cmp(&e2), std::cmp::Ordering::Equal);
    }

    #[test]
    fn sorting_multiple_envelopes() {
        let mut envelopes: Vec<EventEnvelope> = (0..5)
            .rev()
            .map(|i| {
                let mut e = sample_envelope();
                e.sequence = i;
                e
            })
            .collect();

        envelopes.sort();

        for (i, env) in envelopes.iter().enumerate() {
            assert_eq!(env.sequence, i as u64);
        }
    }

    // --- Clone & Debug tests ---

    #[test]
    fn envelope_is_clone() {
        let env = sample_envelope();
        let cloned = env.clone();
        assert_eq!(env, cloned);
    }

    #[test]
    fn envelope_is_debug() {
        let env = sample_envelope();
        let debug = format!("{env:?}");
        assert!(debug.contains("EventEnvelope"));
        assert!(debug.contains("order.created"));
    }

    #[test]
    fn builder_is_debug() {
        let builder = EventEnvelope::builder().event_type("test");
        let debug = format!("{builder:?}");
        assert!(debug.contains("EventEnvelopeBuilder"));
    }
}
