//! Protocol error types.
//!
//! [`ProtocolError`] covers all failure modes that can occur when constructing,
//! validating, or verifying protocol wire types.
//!
//! # Example
//!
//! ```rust
//! use stateset_protocol::ProtocolError;
//!
//! let err = ProtocolError::InvalidEnvelope("missing entity_type".into());
//! assert!(err.to_string().contains("missing entity_type"));
//! ```


/// Errors that can occur in protocol operations.
#[derive(Debug, Clone, thiserror::Error)]
#[non_exhaustive]
pub enum ProtocolError {
    /// The event envelope failed validation.
    #[error("invalid envelope: {0}")]
    InvalidEnvelope(String),

    /// The sync batch failed validation.
    #[error("invalid batch: {0}")]
    InvalidBatch(String),

    /// Merkle proof verification failed.
    #[error("merkle verification failed: {0}")]
    MerkleVerificationFailed(String),

    /// A cryptographic signature is invalid.
    #[error("invalid signature: {0}")]
    InvalidSignature(String),

    /// The protocol or schema version is not supported.
    #[error("unsupported version: {0}")]
    UnsupportedVersion(String),

    /// Serialization or deserialization failed.
    #[error("serialization error: {0}")]
    SerializationError(String),
}

impl From<serde_json::Error> for ProtocolError {
    fn from(err: serde_json::Error) -> Self {
        Self::SerializationError(err.to_string())
    }
}

/// Type alias for protocol results.
pub type Result<T> = std::result::Result<T, ProtocolError>;

/// Human-readable protocol error category for logging.
impl ProtocolError {
    /// Returns a short category label for this error variant.
    ///
    /// ```rust
    /// use stateset_protocol::ProtocolError;
    ///
    /// let err = ProtocolError::MerkleVerificationFailed("bad root".into());
    /// assert_eq!(err.category(), "merkle_verification_failed");
    /// ```
    #[must_use]
    pub const fn category(&self) -> &'static str {
        match self {
            Self::InvalidEnvelope(_) => "invalid_envelope",
            Self::InvalidBatch(_) => "invalid_batch",
            Self::MerkleVerificationFailed(_) => "merkle_verification_failed",
            Self::InvalidSignature(_) => "invalid_signature",
            Self::UnsupportedVersion(_) => "unsupported_version",
            Self::SerializationError(_) => "serialization_error",
        }
    }
}

/// Display implementation is derived via `thiserror`, but we verify it in tests.
impl ProtocolError {
    /// Returns the inner message of this error.
    ///
    /// ```rust
    /// use stateset_protocol::ProtocolError;
    ///
    /// let err = ProtocolError::InvalidEnvelope("bad field".into());
    /// assert_eq!(err.message(), "bad field");
    /// ```
    #[must_use]
    pub fn message(&self) -> &str {
        match self {
            Self::InvalidEnvelope(msg)
            | Self::InvalidBatch(msg)
            | Self::MerkleVerificationFailed(msg)
            | Self::InvalidSignature(msg)
            | Self::UnsupportedVersion(msg)
            | Self::SerializationError(msg) => msg,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_invalid_envelope() {
        let err = ProtocolError::InvalidEnvelope("missing id".into());
        assert_eq!(err.to_string(), "invalid envelope: missing id");
    }

    #[test]
    fn display_invalid_batch() {
        let err = ProtocolError::InvalidBatch("empty leaves".into());
        assert_eq!(err.to_string(), "invalid batch: empty leaves");
    }

    #[test]
    fn display_merkle_verification_failed() {
        let err = ProtocolError::MerkleVerificationFailed("root mismatch".into());
        assert_eq!(
            err.to_string(),
            "merkle verification failed: root mismatch"
        );
    }

    #[test]
    fn display_invalid_signature() {
        let err = ProtocolError::InvalidSignature("bad sig".into());
        assert_eq!(err.to_string(), "invalid signature: bad sig");
    }

    #[test]
    fn display_unsupported_version() {
        let err = ProtocolError::UnsupportedVersion("v99".into());
        assert_eq!(err.to_string(), "unsupported version: v99");
    }

    #[test]
    fn display_serialization_error() {
        let err = ProtocolError::SerializationError("bad json".into());
        assert_eq!(err.to_string(), "serialization error: bad json");
    }

    #[test]
    fn from_serde_json_error() {
        let json_err = serde_json::from_str::<serde_json::Value>("{{bad}}").unwrap_err();
        let proto_err: ProtocolError = json_err.into();
        assert!(matches!(proto_err, ProtocolError::SerializationError(_)));
    }

    #[test]
    fn category_labels() {
        assert_eq!(
            ProtocolError::InvalidEnvelope(String::new()).category(),
            "invalid_envelope"
        );
        assert_eq!(
            ProtocolError::InvalidBatch(String::new()).category(),
            "invalid_batch"
        );
        assert_eq!(
            ProtocolError::MerkleVerificationFailed(String::new()).category(),
            "merkle_verification_failed"
        );
        assert_eq!(
            ProtocolError::InvalidSignature(String::new()).category(),
            "invalid_signature"
        );
        assert_eq!(
            ProtocolError::UnsupportedVersion(String::new()).category(),
            "unsupported_version"
        );
        assert_eq!(
            ProtocolError::SerializationError(String::new()).category(),
            "serialization_error"
        );
    }

    #[test]
    fn message_extraction() {
        let err = ProtocolError::InvalidEnvelope("test msg".into());
        assert_eq!(err.message(), "test msg");
    }

    #[test]
    fn error_is_clone() {
        let err = ProtocolError::InvalidEnvelope("clone me".into());
        let cloned = err.clone();
        assert_eq!(err.to_string(), cloned.to_string());
    }

    #[test]
    fn error_is_debug() {
        let err = ProtocolError::InvalidBatch("debug me".into());
        let debug = format!("{err:?}");
        assert!(debug.contains("InvalidBatch"));
        assert!(debug.contains("debug me"));
    }
}
