//! VES v1.0 Cryptographic Operations
//!
//! Implements:
//! - RFC 8785 JSON Canonicalization Scheme (JCS) via `serde_jcs`
//! - Domain-separated hashing per VES spec
//! - Ed25519 signing for agent signatures
//! - AES-256-GCM payload encryption (VES-ENC-1)
//! - X25519 ECDH key wrapping
//! - Merkle tree hashing

pub mod canonicalize;
pub mod encrypt;
pub mod hash;
pub mod merkle;
pub mod sign;

mod encoding;
mod error;

pub use encoding::{bytes_to_hex, encode_string, hex_to_bytes, u32_be, u64_be, uuid_to_bytes};
pub use error::CryptoError;

/// Domain separation prefixes (must match sequencer)
pub mod domain {
    /// Payload plain hash domain prefix
    pub const PAYLOAD_PLAIN: &[u8] = b"VES_PAYLOAD_PLAIN_V1";
    /// Payload AAD domain prefix
    pub const PAYLOAD_AAD: &[u8] = b"VES_PAYLOAD_AAD_V1";
    /// Payload cipher hash domain prefix
    pub const PAYLOAD_CIPHER: &[u8] = b"VES_PAYLOAD_CIPHER_V1";
    /// Recipients hash domain prefix
    pub const RECIPIENTS: &[u8] = b"VES_RECIPIENTS_V1";
    /// Event signing hash domain prefix
    pub const EVENTSIG: &[u8] = b"VES_EVENTSIG_V1";
    /// Merkle leaf hash domain prefix
    pub const LEAF: &[u8] = b"VES_LEAF_V1";
    /// Merkle node hash domain prefix
    pub const NODE: &[u8] = b"VES_NODE_V1";
    /// Merkle padding leaf domain prefix
    pub const PAD_LEAF: &[u8] = b"VES_PAD_LEAF_V1";
    /// Stream ID domain prefix
    pub const STREAM: &[u8] = b"VES_STREAM_V1";
    /// Receipt hash domain prefix
    pub const RECEIPT: &[u8] = b"VES_RECEIPT_V1";
}

/// 32 bytes of zeros -- used for plaintext payloads in cipher hash field
pub const ZERO_HASH: [u8; 32] = [0u8; 32];
