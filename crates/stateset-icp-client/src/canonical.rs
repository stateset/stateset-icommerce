//! Canonical JSON per RFC 8785 (JCS), thin wrapper around `serde_jcs`.

use crate::Error;
use serde::Serialize;

/// Serialize a value to canonical JSON bytes (RFC 8785 JCS).
///
/// This is the same routine used by the merchant handler and the
/// conformance IUT — produces byte-identical output across JS, Rust,
/// Go, and Python implementations.
pub fn canonical_json<T: Serialize>(value: &T) -> Result<String, Error> {
    serde_jcs::to_string(value).map_err(|e| Error::Canonicalization(e.to_string()))
}
