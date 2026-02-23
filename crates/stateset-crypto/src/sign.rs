//! Ed25519 signing and verification per VES v1.0
//!
//! Uses ed25519-dalek for cryptographic operations.

use ed25519_dalek::{Signer, SigningKey, Verifier, VerifyingKey};

use crate::CryptoError;

/// Sign an event signing hash with Ed25519
///
/// Takes a 32-byte hash and a 32-byte private key (seed).
/// Returns a 64-byte signature.
///
/// # Errors
///
/// Returns [`CryptoError::SignatureError`] if signing fails.
pub fn sign_event_hash(
    event_signing_hash: &[u8; 32],
    private_key: &[u8; 32],
) -> Result<[u8; 64], CryptoError> {
    let signing_key = SigningKey::from_bytes(private_key);
    let signature = signing_key.sign(event_signing_hash);
    Ok(signature.to_bytes())
}

/// Verify an event signature
///
/// Returns true if the signature is valid, false otherwise.
#[must_use]
pub fn verify_event_signature(
    event_signing_hash: &[u8; 32],
    signature: &[u8; 64],
    public_key: &[u8; 32],
) -> bool {
    let Ok(verifying_key) = VerifyingKey::from_bytes(public_key) else {
        return false;
    };
    let sig = ed25519_dalek::Signature::from_bytes(signature);
    verifying_key.verify(event_signing_hash, &sig).is_ok()
}

/// Generate a new Ed25519 keypair
///
/// Returns (`private_key`, `public_key`) as 32-byte arrays.
pub fn generate_keypair() -> ([u8; 32], [u8; 32]) {
    let mut rng = rand::thread_rng();
    let signing_key = SigningKey::generate(&mut rng);
    let verifying_key = signing_key.verifying_key();
    (signing_key.to_bytes(), verifying_key.to_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_and_verify_roundtrip() {
        let (private_key, public_key) = generate_keypair();
        let hash = [42u8; 32];
        let signature = sign_event_hash(&hash, &private_key).unwrap();
        assert!(verify_event_signature(&hash, &signature, &public_key));
    }

    #[test]
    fn verify_wrong_key_fails() {
        let (private_key, _) = generate_keypair();
        let (_, other_public) = generate_keypair();
        let hash = [42u8; 32];
        let signature = sign_event_hash(&hash, &private_key).unwrap();
        assert!(!verify_event_signature(&hash, &signature, &other_public));
    }

    #[test]
    fn verify_wrong_hash_fails() {
        let (private_key, public_key) = generate_keypair();
        let hash = [42u8; 32];
        let other_hash = [99u8; 32];
        let signature = sign_event_hash(&hash, &private_key).unwrap();
        assert!(!verify_event_signature(&other_hash, &signature, &public_key));
    }

    #[test]
    fn verify_tampered_signature_fails() {
        let (private_key, public_key) = generate_keypair();
        let hash = [42u8; 32];
        let mut signature = sign_event_hash(&hash, &private_key).unwrap();
        signature[0] ^= 0xFF; // Flip bits
        assert!(!verify_event_signature(&hash, &signature, &public_key));
    }

    #[test]
    fn verify_invalid_public_key_returns_false() {
        let (private_key, _) = generate_keypair();
        let hash = [42u8; 32];
        let signature = sign_event_hash(&hash, &private_key).unwrap();
        let bad_key = [0u8; 32]; // Not a valid Ed25519 point
        assert!(!verify_event_signature(&hash, &signature, &bad_key));
    }

    #[test]
    fn signature_is_64_bytes() {
        let (private_key, _) = generate_keypair();
        let hash = [42u8; 32];
        let signature = sign_event_hash(&hash, &private_key).unwrap();
        assert_eq!(signature.len(), 64);
    }

    #[test]
    fn deterministic_signature() {
        // Ed25519 signatures are deterministic for the same key and message
        let (private_key, _) = generate_keypair();
        let hash = [42u8; 32];
        let sig1 = sign_event_hash(&hash, &private_key).unwrap();
        let sig2 = sign_event_hash(&hash, &private_key).unwrap();
        assert_eq!(sig1, sig2);
    }
}
