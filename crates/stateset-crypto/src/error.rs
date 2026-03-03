use thiserror::Error;

/// Errors that can occur during VES cryptographic operations
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CryptoError {
    /// JCS does not support Infinity or `NaN` values
    #[error("JCS does not support Infinity or NaN")]
    JcsInvalidNumber,
    /// Cannot canonicalize the given type
    #[error("Cannot canonicalize type")]
    JcsUnsupportedType,
    /// The provided UUID string is invalid
    #[error("Invalid UUID: {0}")]
    InvalidUuid(String),
    /// Invalid hex string
    #[error("Invalid hex: {0}")]
    InvalidHex(String),
    /// Salt must be exactly 16 bytes
    #[error("Salt must be 16 bytes")]
    InvalidSalt,
    /// At least one recipient is required for encryption
    #[error("At least one recipient required")]
    NoRecipients,
    /// The specified recipient was not found in the encrypted payload
    #[error("Recipient {0} not found")]
    RecipientNotFound(u32),
    /// The computed payload hash does not match the expected hash
    #[error("Payload hash mismatch")]
    PayloadHashMismatch,
    /// Ed25519 signature verification failed
    #[error("Signature verification failed: {0}")]
    SignatureError(String),
    /// AES-256-GCM encryption failed
    #[error("Encryption error: {0}")]
    EncryptionError(String),
    /// AES-256-GCM decryption failed
    #[error("Decryption error: {0}")]
    DecryptionError(String),
    /// X25519 ECDH key wrapping failed
    #[error("Key wrapping error: {0}")]
    KeyWrapError(String),
    /// JSON serialization or canonicalization failed
    #[error("Serialization error: {0}")]
    SerializationError(String),
}
