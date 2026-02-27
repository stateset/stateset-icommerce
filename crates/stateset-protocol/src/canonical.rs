//! Canonical serialization utilities.
//!
//! Provides deterministic serialization for protocol types, following
//! [RFC 8785 JSON Canonicalization Scheme (JCS)](https://www.rfc-editor.org/rfc/rfc8785)
//! and domain-separated SHA-256 hashing.
//!
//! # Example
//!
//! ```rust
//! use stateset_protocol::canonical::{canonical_json, domain_hash};
//! use serde_json::json;
//!
//! let value = json!({"b": 2, "a": 1});
//! let canonical = canonical_json(&value).unwrap();
//! assert_eq!(canonical, r#"{"a":1,"b":2}"#);
//!
//! let hash = domain_hash("MY_DOMAIN", b"hello");
//! assert_eq!(hash.len(), 32);
//! ```

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

use crate::error::{ProtocolError, Result};

/// Produce RFC 8785 JCS canonical JSON from a [`serde_json::Value`].
///
/// Keys are sorted lexicographically; numbers use shortest representation.
///
/// # Errors
///
/// Returns [`ProtocolError::SerializationError`] if the value cannot be serialized.
///
/// # Example
///
/// ```rust
/// use stateset_protocol::canonical::canonical_json;
/// use serde_json::json;
///
/// let v = json!({"z": 1, "a": 2});
/// assert_eq!(canonical_json(&v).unwrap(), r#"{"a":2,"z":1}"#);
/// ```
pub fn canonical_json(value: &serde_json::Value) -> Result<String> {
    serde_jcs::to_string(value).map_err(|e| ProtocolError::SerializationError(e.to_string()))
}

/// Compute a domain-separated SHA-256 hash.
///
/// The hash is computed as:
/// `SHA256("stateset-domain-hash-v1" || 0x00 || len(domain) || 0x1F || domain || 0x1E || len(data) || 0x1D || data)`.
///
/// This length-prefix + delimiter framing avoids concatenation ambiguity.
///
/// # Example
///
/// ```rust
/// use stateset_protocol::canonical::domain_hash;
///
/// let h1 = domain_hash("DOMAIN_A", b"data");
/// let h2 = domain_hash("DOMAIN_B", b"data");
/// assert_ne!(h1, h2); // different domains produce different hashes
/// ```
#[must_use]
pub fn domain_hash(domain: &str, data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"stateset-domain-hash-v1");
    hasher.update([0x00]);
    hasher.update((domain.len() as u64).to_be_bytes());
    hasher.update([0x1F]);
    hasher.update(domain.as_bytes());
    hasher.update([0x1E]);
    hasher.update((data.len() as u64).to_be_bytes());
    hasher.update([0x1D]);
    hasher.update(data);
    hasher.finalize().into()
}

/// Protocol version newtype.
///
/// Wraps a `u16` to provide type safety and prevent accidental confusion
/// with [`SchemaVersion`].
///
/// # Example
///
/// ```rust
/// use stateset_protocol::ProtocolVersion;
///
/// let v = ProtocolVersion::new(1);
/// assert_eq!(v.as_u16(), 1);
/// assert_eq!(v.to_string(), "1");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProtocolVersion(u16);

impl ProtocolVersion {
    /// The current protocol version.
    pub const CURRENT: Self = Self(1);

    /// Create a new protocol version.
    #[must_use]
    pub const fn new(version: u16) -> Self {
        Self(version)
    }

    /// Return the inner `u16` value.
    #[must_use]
    pub const fn as_u16(self) -> u16 {
        self.0
    }
}

impl fmt::Display for ProtocolVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u16> for ProtocolVersion {
    fn from(v: u16) -> Self {
        Self(v)
    }
}

impl From<ProtocolVersion> for u16 {
    fn from(v: ProtocolVersion) -> Self {
        v.0
    }
}

/// Schema version newtype.
///
/// Wraps a `u16` to provide type safety and prevent accidental confusion
/// with [`ProtocolVersion`].
///
/// # Example
///
/// ```rust
/// use stateset_protocol::SchemaVersion;
///
/// let v = SchemaVersion::new(3);
/// assert_eq!(v.as_u16(), 3);
/// assert_eq!(v.to_string(), "3");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SchemaVersion(u16);

impl SchemaVersion {
    /// The current schema version.
    pub const CURRENT: Self = Self(1);

    /// Create a new schema version.
    #[must_use]
    pub const fn new(version: u16) -> Self {
        Self(version)
    }

    /// Return the inner `u16` value.
    #[must_use]
    pub const fn as_u16(self) -> u16 {
        self.0
    }
}

impl fmt::Display for SchemaVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u16> for SchemaVersion {
    fn from(v: u16) -> Self {
        Self(v)
    }
}

impl From<SchemaVersion> for u16 {
    fn from(v: SchemaVersion) -> Self {
        v.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use serde_json::json;
    use serde_json::{Map, Value};

    fn arb_json_value() -> impl Strategy<Value = Value> {
        let leaf = prop_oneof![
            Just(Value::Null),
            any::<bool>().prop_map(Value::Bool),
            any::<i64>().prop_map(|n| Value::Number(n.into())),
            proptest::string::string_regex("[a-zA-Z0-9_]{0,16}").unwrap().prop_map(Value::String),
        ];

        leaf.prop_recursive(4, 128, 10, |inner| {
            prop_oneof![
                prop::collection::vec(inner.clone(), 0..6).prop_map(Value::Array),
                prop::collection::btree_map(
                    proptest::string::string_regex("[a-zA-Z0-9_]{1,8}").unwrap(),
                    inner,
                    0..6
                )
                .prop_map(|entries| {
                    let mut map = Map::new();
                    for (k, v) in entries {
                        map.insert(k, v);
                    }
                    Value::Object(map)
                }),
            ]
        })
    }

    fn reorder_object_keys(value: &Value) -> Value {
        match value {
            Value::Object(map) => {
                let mut entries: Vec<_> = map.iter().collect();
                entries.reverse();
                let mut reordered = Map::with_capacity(map.len());
                for (key, nested) in entries {
                    reordered.insert(key.clone(), reorder_object_keys(nested));
                }
                Value::Object(reordered)
            }
            Value::Array(items) => Value::Array(items.iter().map(reorder_object_keys).collect()),
            _ => value.clone(),
        }
    }

    // --- canonical_json tests ---

    #[test]
    fn jcs_sorts_keys() {
        let v = json!({"b": 2, "a": 1});
        assert_eq!(canonical_json(&v).unwrap(), r#"{"a":1,"b":2}"#);
    }

    #[test]
    fn jcs_nested_sorts_keys() {
        let v = json!({"z": {"b": 2, "a": 1}, "a": 0});
        assert_eq!(canonical_json(&v).unwrap(), r#"{"a":0,"z":{"a":1,"b":2}}"#);
    }

    #[test]
    fn jcs_empty_object() {
        let v = json!({});
        assert_eq!(canonical_json(&v).unwrap(), "{}");
    }

    #[test]
    fn jcs_array() {
        let v = json!([3, 1, 2]);
        assert_eq!(canonical_json(&v).unwrap(), "[3,1,2]");
    }

    #[test]
    fn jcs_string() {
        let v = json!("hello");
        assert_eq!(canonical_json(&v).unwrap(), r#""hello""#);
    }

    #[test]
    fn jcs_null() {
        let v = json!(null);
        assert_eq!(canonical_json(&v).unwrap(), "null");
    }

    #[test]
    fn jcs_boolean() {
        assert_eq!(canonical_json(&json!(true)).unwrap(), "true");
        assert_eq!(canonical_json(&json!(false)).unwrap(), "false");
    }

    #[test]
    fn jcs_deterministic() {
        let v = json!({"x": [1, 2], "a": {"nested": true}});
        let s1 = canonical_json(&v).unwrap();
        let s2 = canonical_json(&v).unwrap();
        assert_eq!(s1, s2);
    }

    // --- domain_hash tests ---

    #[test]
    fn domain_hash_deterministic() {
        let h1 = domain_hash("TEST", b"data");
        let h2 = domain_hash("TEST", b"data");
        assert_eq!(h1, h2);
    }

    #[test]
    fn domain_hash_different_domains() {
        let h1 = domain_hash("DOMAIN_A", b"data");
        let h2 = domain_hash("DOMAIN_B", b"data");
        assert_ne!(h1, h2);
    }

    #[test]
    fn domain_hash_different_data() {
        let h1 = domain_hash("DOMAIN", b"data1");
        let h2 = domain_hash("DOMAIN", b"data2");
        assert_ne!(h1, h2);
    }

    #[test]
    fn domain_hash_empty_data() {
        let h = domain_hash("DOMAIN", b"");
        assert_eq!(h.len(), 32);
    }

    #[test]
    fn domain_hash_empty_domain() {
        let h = domain_hash("", b"data");
        assert_eq!(h.len(), 32);
    }

    #[test]
    fn domain_hash_not_ambiguous_under_concatenation() {
        // Previously ambiguous under naive `domain || data` framing.
        let h1 = domain_hash("ab", b"c");
        let h2 = domain_hash("a", b"bc");
        assert_ne!(h1, h2);
    }

    // --- ProtocolVersion tests ---

    #[test]
    fn protocol_version_new() {
        let v = ProtocolVersion::new(42);
        assert_eq!(v.as_u16(), 42);
    }

    #[test]
    fn protocol_version_display() {
        let v = ProtocolVersion::new(7);
        assert_eq!(v.to_string(), "7");
    }

    #[test]
    fn protocol_version_from_u16() {
        let v: ProtocolVersion = 5u16.into();
        assert_eq!(v.as_u16(), 5);
    }

    #[test]
    fn protocol_version_into_u16() {
        let v = ProtocolVersion::new(10);
        let n: u16 = v.into();
        assert_eq!(n, 10);
    }

    #[test]
    fn protocol_version_ord() {
        let v1 = ProtocolVersion::new(1);
        let v2 = ProtocolVersion::new(2);
        assert!(v1 < v2);
    }

    #[test]
    fn protocol_version_eq() {
        let v1 = ProtocolVersion::new(3);
        let v2 = ProtocolVersion::new(3);
        assert_eq!(v1, v2);
    }

    #[test]
    fn protocol_version_serde_roundtrip() {
        let v = ProtocolVersion::new(1);
        let json = serde_json::to_string(&v).unwrap();
        assert_eq!(json, "1");
        let deserialized: ProtocolVersion = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, v);
    }

    #[test]
    fn protocol_version_current() {
        assert_eq!(ProtocolVersion::CURRENT.as_u16(), 1);
    }

    // --- SchemaVersion tests ---

    #[test]
    fn schema_version_new() {
        let v = SchemaVersion::new(42);
        assert_eq!(v.as_u16(), 42);
    }

    #[test]
    fn schema_version_display() {
        let v = SchemaVersion::new(7);
        assert_eq!(v.to_string(), "7");
    }

    #[test]
    fn schema_version_from_u16() {
        let v: SchemaVersion = 5u16.into();
        assert_eq!(v.as_u16(), 5);
    }

    #[test]
    fn schema_version_into_u16() {
        let v = SchemaVersion::new(10);
        let n: u16 = v.into();
        assert_eq!(n, 10);
    }

    #[test]
    fn schema_version_ord() {
        let v1 = SchemaVersion::new(1);
        let v2 = SchemaVersion::new(2);
        assert!(v1 < v2);
    }

    #[test]
    fn schema_version_serde_roundtrip() {
        let v = SchemaVersion::new(5);
        let json = serde_json::to_string(&v).unwrap();
        let deserialized: SchemaVersion = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, v);
    }

    #[test]
    fn schema_version_current() {
        assert_eq!(SchemaVersion::CURRENT.as_u16(), 1);
    }

    #[test]
    fn protocol_version_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(ProtocolVersion::new(1));
        set.insert(ProtocolVersion::new(1));
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn schema_version_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(SchemaVersion::new(1));
        set.insert(SchemaVersion::new(1));
        assert_eq!(set.len(), 1);
    }

    proptest! {
        #[test]
        fn canonical_json_stable_under_object_key_reordering(value in arb_json_value()) {
            let reordered = reorder_object_keys(&value);
            let canonical_original = canonical_json(&value).unwrap();
            let canonical_reordered = canonical_json(&reordered).unwrap();
            prop_assert_eq!(canonical_original, canonical_reordered);
        }
    }

    proptest! {
        #[test]
        fn canonical_json_roundtrip_preserves_semantics(value in arb_json_value()) {
            let canonical = canonical_json(&value).unwrap();
            let reparsed: Value = serde_json::from_str(&canonical).unwrap();
            prop_assert_eq!(reparsed, value);
        }
    }

    proptest! {
        #[test]
        fn domain_hash_changes_when_domain_or_payload_changes(
            domain in proptest::string::string_regex("[A-Z0-9_]{0,16}").unwrap(),
            payload in prop::collection::vec(any::<u8>(), 0..64),
            other in prop::collection::vec(any::<u8>(), 0..64),
        ) {
            let base = domain_hash(&domain, &payload);
            let same = domain_hash(&domain, &payload);
            prop_assert_eq!(base, same);

            let domain_variant = format!("{domain}X");
            if !domain_variant.is_empty() {
                prop_assert_ne!(base, domain_hash(&domain_variant, &payload));
            }
            if payload != other {
                prop_assert_ne!(base, domain_hash(&domain, &other));
            }
        }
    }
}
