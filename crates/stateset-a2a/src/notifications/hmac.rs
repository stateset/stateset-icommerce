//! HMAC-SHA256 webhook signing and verification.
//!
//! Matches the `JavaScript` implementation: `createHmac('sha256', secret).update(payload).digest('hex')`.

use ::hmac::{Hmac, Mac};
use sha2::Sha256;
use thiserror::Error;

type HmacSha256 = Hmac<Sha256>;

/// Error type for webhook HMAC operations.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum WebhookHmacError {
    /// Failed to initialize HMAC with the provided secret.
    #[error("failed to initialize HMAC key")]
    InvalidKey,
}

fn new_hmac(secret: &[u8]) -> Result<HmacSha256, WebhookHmacError> {
    HmacSha256::new_from_slice(secret).map_err(|_| WebhookHmacError::InvalidKey)
}

/// Sign a webhook payload with HMAC-SHA256.
///
/// Returns the hex-encoded signature string.
///
/// # Example
///
/// ```
/// use stateset_a2a::notifications::sign_webhook;
///
/// let sig = sign_webhook(b"my-secret", b"hello world");
/// assert_eq!(sig.len(), 64); // 32 bytes hex-encoded
/// ```
///
/// Returns an empty string if HMAC initialization fails.
#[must_use]
pub fn sign_webhook(secret: &[u8], payload: &[u8]) -> String {
    try_sign_webhook(secret, payload).unwrap_or_default()
}

/// Fallible variant of [`sign_webhook`].
///
/// # Errors
///
/// Returns [`WebhookHmacError::InvalidKey`] if HMAC initialization fails.
pub fn try_sign_webhook(secret: &[u8], payload: &[u8]) -> Result<String, WebhookHmacError> {
    let mut mac = new_hmac(secret)?;
    mac.update(payload);
    Ok(hex::encode(mac.finalize().into_bytes()))
}

/// Verify a webhook payload against an HMAC-SHA256 signature.
///
/// Uses constant-time comparison to prevent timing attacks.
///
/// # Example
///
/// ```
/// use stateset_a2a::notifications::{sign_webhook, verify_webhook};
///
/// let sig = sign_webhook(b"secret", b"payload");
/// assert!(verify_webhook(b"secret", b"payload", &sig));
/// assert!(!verify_webhook(b"wrong-secret", b"payload", &sig));
/// ```
#[must_use]
pub fn verify_webhook(secret: &[u8], payload: &[u8], signature: &str) -> bool {
    try_verify_webhook(secret, payload, signature).unwrap_or(false)
}

/// Fallible variant of [`verify_webhook`].
///
/// # Errors
///
/// Returns [`WebhookHmacError::InvalidKey`] if HMAC initialization fails.
pub fn try_verify_webhook(
    secret: &[u8],
    payload: &[u8],
    signature: &str,
) -> Result<bool, WebhookHmacError> {
    let mut mac = new_hmac(secret)?;
    mac.update(payload);

    // Decode the provided hex signature
    let Ok(sig_bytes) = hex::decode(signature) else {
        return Ok(false);
    };

    // Use the `verify_slice` method for constant-time comparison
    Ok(mac.verify_slice(&sig_bytes).is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_produces_64_char_hex() {
        let sig = sign_webhook(b"secret", b"payload");
        assert_eq!(sig.len(), 64);
        // Verify it's valid hex
        assert!(hex::decode(&sig).is_ok());
    }

    #[test]
    fn sign_deterministic() {
        let sig1 = sign_webhook(b"key", b"data");
        let sig2 = sign_webhook(b"key", b"data");
        assert_eq!(sig1, sig2);
    }

    #[test]
    fn sign_different_secrets_differ() {
        let sig1 = sign_webhook(b"secret1", b"payload");
        let sig2 = sign_webhook(b"secret2", b"payload");
        assert_ne!(sig1, sig2);
    }

    #[test]
    fn sign_different_payloads_differ() {
        let sig1 = sign_webhook(b"secret", b"payload1");
        let sig2 = sign_webhook(b"secret", b"payload2");
        assert_ne!(sig1, sig2);
    }

    #[test]
    fn verify_correct_signature() {
        let sig = sign_webhook(b"secret", b"payload");
        assert!(verify_webhook(b"secret", b"payload", &sig));
    }

    #[test]
    fn verify_wrong_secret() {
        let sig = sign_webhook(b"secret", b"payload");
        assert!(!verify_webhook(b"wrong", b"payload", &sig));
    }

    #[test]
    fn verify_wrong_payload() {
        let sig = sign_webhook(b"secret", b"payload");
        assert!(!verify_webhook(b"secret", b"tampered", &sig));
    }

    #[test]
    fn verify_invalid_hex() {
        assert!(!verify_webhook(b"secret", b"payload", "not-hex!@#$"));
    }

    #[test]
    fn verify_wrong_length() {
        assert!(!verify_webhook(b"secret", b"payload", "abcd"));
    }

    #[test]
    fn sign_empty_secret() {
        let sig = sign_webhook(b"", b"payload");
        assert!(verify_webhook(b"", b"payload", &sig));
    }

    #[test]
    fn sign_empty_payload() {
        let sig = sign_webhook(b"secret", b"");
        assert!(verify_webhook(b"secret", b"", &sig));
    }

    #[test]
    fn sign_both_empty() {
        let sig = sign_webhook(b"", b"");
        assert!(verify_webhook(b"", b"", &sig));
    }

    #[test]
    fn sign_json_payload() {
        let payload = br#"{"event_type":"payment.completed","amount":100}"#;
        let sig = sign_webhook(b"whsec_abc123", payload);
        assert!(verify_webhook(b"whsec_abc123", payload, &sig));
    }

    #[test]
    fn known_vector() {
        // Known HMAC-SHA256 test vector
        // HMAC-SHA256("key", "The quick brown fox jumps over the lazy dog")
        let sig = sign_webhook(b"key", b"The quick brown fox jumps over the lazy dog");
        assert_eq!(sig, "f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8");
    }

    #[test]
    fn try_sign_and_verify_success() {
        let sig = try_sign_webhook(b"secret", b"payload").unwrap();
        assert!(try_verify_webhook(b"secret", b"payload", &sig).unwrap());
    }
}
