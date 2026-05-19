//! Error type for the ICP client.

use thiserror::Error;

/// Errors returned by the ICP client.
#[derive(Debug, Error)]
pub enum Error {
    /// Inputs were invalid (wrong byte length, malformed hex, etc.).
    #[error("invalid input: {0}")]
    InvalidInput(String),

    /// Canonical JSON serialization failed.
    #[error("canonicalization failed: {0}")]
    Canonicalization(String),

    /// Network transport error.
    #[error("network error: {0}")]
    Network(String),

    /// Merchant returned an ICP error envelope.
    #[error("icp error: {code} — {message}")]
    Icp {
        /// Dotted-namespace ICP error code (see `error-codes.md`).
        code: String,
        /// Human-readable error message.
        message: String,
    },

    /// Response could not be parsed as the expected shape.
    #[error("malformed response: {0}")]
    MalformedResponse(String),

    /// Signature verification of a merchant response failed.
    #[error("signature verification failed")]
    SignatureInvalid,
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self::MalformedResponse(e.to_string())
    }
}
