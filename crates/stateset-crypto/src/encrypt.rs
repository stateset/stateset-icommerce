//! Payload encryption per VES-ENC-1
//!
//! - AES-256-GCM for payload encryption
//! - X25519 ECDH + HKDF + AES-256-GCM for key wrapping

use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, Key, KeyInit, Nonce};
use base64::Engine;
use hkdf::Hkdf;
use rand::RngCore;
use sha2::Sha256;
use x25519_dalek::{EphemeralSecret, PublicKey, StaticSecret};

use crate::canonicalize::canonicalize_json;
use crate::hash::{
    compute_payload_aad, compute_payload_cipher_hash, compute_payload_plain_hash,
    compute_recipients_hash, PayloadAadParams, PayloadCipherParams,
};
use crate::CryptoError;

const NONCE_SIZE: usize = 12;
const SALT_SIZE: usize = 16;
const KEY_SIZE: usize = 32;
const TAG_SIZE: usize = 16;

/// Recipient key for encryption
#[derive(Debug, Clone)]
pub struct RecipientKey {
    /// Recipient key identifier
    pub kid: u32,
    /// 32-byte X25519 public key
    pub public_key: [u8; 32],
}

/// Result of payload encryption
#[derive(Debug)]
pub struct EncryptionResult {
    /// The encrypted payload structure (serializable to JSON)
    pub payload_encrypted: serde_json::Value,
    /// 16-byte salt used
    pub salt: [u8; 16],
    /// 32-byte `payload_plain_hash`
    pub payload_plain_hash: [u8; 32],
    /// 32-byte `payload_cipher_hash`
    pub payload_cipher_hash: [u8; 32],
}

/// Encrypt payload per VES-ENC-1
///
/// # Errors
///
/// Returns an error if:
/// - No recipients are provided ([`CryptoError::NoRecipients`])
/// - The payload cannot be canonicalized ([`CryptoError::SerializationError`])
/// - Encryption or key wrapping fails ([`CryptoError::EncryptionError`], [`CryptoError::KeyWrapError`])
pub fn encrypt_payload(
    payload: &serde_json::Value,
    aad_params: &PayloadAadParams<'_>,
    recipient_keys: &[RecipientKey],
) -> Result<EncryptionResult, CryptoError> {
    if recipient_keys.is_empty() {
        return Err(CryptoError::NoRecipients);
    }

    let mut rng = rand::thread_rng();
    let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;

    // Generate random values
    let mut salt = [0u8; SALT_SIZE];
    let mut dek = [0u8; KEY_SIZE];
    let mut nonce_bytes = [0u8; NONCE_SIZE];
    rng.fill_bytes(&mut salt);
    rng.fill_bytes(&mut dek);
    rng.fill_bytes(&mut nonce_bytes);

    // Compute payload_plain_hash with salt
    let payload_plain_hash = compute_payload_plain_hash(payload, Some(&salt))?;

    // Compute AAD with updated plain hash
    let updated_aad_params = PayloadAadParams {
        payload_plain_hash: &payload_plain_hash,
        ..*aad_params
    };
    let payload_aad = compute_payload_aad(&updated_aad_params)?;

    // Prepare plaintext: salt || JCS(payload)
    let canonical = canonicalize_json(payload)?;
    let mut plaintext = Vec::with_capacity(SALT_SIZE + canonical.len());
    plaintext.extend_from_slice(&salt);
    plaintext.extend_from_slice(canonical.as_bytes());

    // Encrypt with AES-256-GCM
    let key = Key::<Aes256Gcm>::from_slice(&dek);
    let cipher = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(&nonce_bytes);

    // Use aes_gcm with AAD
    let aead_payload = aes_gcm::aead::Payload {
        msg: &plaintext,
        aad: &payload_aad,
    };
    let ciphertext_with_tag = cipher
        .encrypt(nonce, aead_payload)
        .map_err(|e| CryptoError::EncryptionError(e.to_string()))?;

    // aes-gcm appends the tag to the ciphertext
    let ct_len = ciphertext_with_tag.len() - TAG_SIZE;
    let ciphertext = &ciphertext_with_tag[..ct_len];
    let tag = &ciphertext_with_tag[ct_len..];

    // Wrap DEK for each recipient
    let mut recipients = Vec::with_capacity(recipient_keys.len());
    for rk in recipient_keys {
        let (enc, wrapped_key) = wrap_dek(&dek, &rk.public_key, &payload_aad)?;
        recipients.push(serde_json::json!({
            "recipient_kid": rk.kid,
            "enc_b64u": b64.encode(&enc),
            "ct_b64u": b64.encode(&wrapped_key),
        }));
    }

    // Sort by kid
    recipients.sort_by(|a, b| {
        let a_kid = a
            .get("recipient_kid")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let b_kid = b
            .get("recipient_kid")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        a_kid.cmp(&b_kid)
    });

    // Compute recipients hash
    let recipients_hash = compute_recipients_hash(&recipients)?;

    // Compute payload_cipher_hash
    let cipher_params = PayloadCipherParams {
        nonce: &nonce_bytes,
        payload_aad: &payload_aad,
        ciphertext,
        tag,
        recipients_hash: &recipients_hash,
    };
    let payload_cipher_hash = compute_payload_cipher_hash(Some(&cipher_params));

    let payload_encrypted = serde_json::json!({
        "enc_version": 1,
        "aead": "AES-256-GCM",
        "nonce_b64u": b64.encode(nonce_bytes),
        "ciphertext_b64u": b64.encode(ciphertext),
        "tag_b64u": b64.encode(tag),
        "hpke": {
            "mode": "base",
            "kem": "X25519-HKDF-SHA256",
            "kdf": "HKDF-SHA256",
            "aead": "AES-256-GCM"
        },
        "recipients": recipients,
    });

    Ok(EncryptionResult {
        payload_encrypted,
        salt,
        payload_plain_hash,
        payload_cipher_hash,
    })
}

/// Decrypt payload per VES-ENC-1
///
/// # Errors
///
/// Returns an error if:
/// - The recipient is not found ([`CryptoError::RecipientNotFound`])
/// - Decryption fails ([`CryptoError::DecryptionError`])
/// - The payload hash does not match ([`CryptoError::PayloadHashMismatch`])
#[allow(clippy::missing_panics_doc)]
pub fn decrypt_payload(
    payload_encrypted: &serde_json::Value,
    payload_aad: &[u8; 32],
    recipient_kid: u32,
    recipient_private_key: &[u8; 32],
    expected_plain_hash: &[u8; 32],
) -> Result<serde_json::Value, CryptoError> {
    let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;

    // Find recipient entry
    let recipients = payload_encrypted
        .get("recipients")
        .and_then(|v| v.as_array())
        .ok_or_else(|| CryptoError::DecryptionError("Missing recipients".to_string()))?;

    let recipient = recipients
        .iter()
        .find(|r| {
            r.get("recipient_kid").and_then(|v| v.as_u64())
                == Some(u64::from(recipient_kid))
        })
        .ok_or(CryptoError::RecipientNotFound(recipient_kid))?;

    // Unwrap DEK
    let enc = b64
        .decode(
            recipient
                .get("enc_b64u")
                .and_then(|v| v.as_str())
                .ok_or_else(|| CryptoError::DecryptionError("Missing enc_b64u".to_string()))?,
        )
        .map_err(|e| CryptoError::DecryptionError(e.to_string()))?;

    let wrapped_key = b64
        .decode(
            recipient
                .get("ct_b64u")
                .and_then(|v| v.as_str())
                .ok_or_else(|| CryptoError::DecryptionError("Missing ct_b64u".to_string()))?,
        )
        .map_err(|e| CryptoError::DecryptionError(e.to_string()))?;

    let mut enc_arr = [0u8; 32];
    enc_arr.copy_from_slice(&enc);
    let dek = unwrap_dek(&enc_arr, &wrapped_key, recipient_private_key, payload_aad)?;

    // Decode encrypted payload
    let nonce_bytes = b64
        .decode(
            payload_encrypted
                .get("nonce_b64u")
                .and_then(|v| v.as_str())
                .ok_or_else(|| CryptoError::DecryptionError("Missing nonce_b64u".to_string()))?,
        )
        .map_err(|e| CryptoError::DecryptionError(e.to_string()))?;

    let ciphertext = b64
        .decode(
            payload_encrypted
                .get("ciphertext_b64u")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    CryptoError::DecryptionError("Missing ciphertext_b64u".to_string())
                })?,
        )
        .map_err(|e| CryptoError::DecryptionError(e.to_string()))?;

    let tag = b64
        .decode(
            payload_encrypted
                .get("tag_b64u")
                .and_then(|v| v.as_str())
                .ok_or_else(|| CryptoError::DecryptionError("Missing tag_b64u".to_string()))?,
        )
        .map_err(|e| CryptoError::DecryptionError(e.to_string()))?;

    // Decrypt with AES-256-GCM
    let key = Key::<Aes256Gcm>::from_slice(&dek);
    let cipher_obj = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(&nonce_bytes);

    // Reconstruct ciphertext+tag as aes-gcm expects
    let mut ct_with_tag = Vec::with_capacity(ciphertext.len() + tag.len());
    ct_with_tag.extend_from_slice(&ciphertext);
    ct_with_tag.extend_from_slice(&tag);

    let aead_payload = aes_gcm::aead::Payload {
        msg: &ct_with_tag,
        aad: payload_aad,
    };
    let plaintext = cipher_obj
        .decrypt(nonce, aead_payload)
        .map_err(|e| CryptoError::DecryptionError(e.to_string()))?;

    // Extract salt and JSON
    if plaintext.len() < SALT_SIZE {
        return Err(CryptoError::DecryptionError(
            "Plaintext too short".to_string(),
        ));
    }
    let salt: [u8; 16] = plaintext[..SALT_SIZE]
        .try_into()
        .expect("salt slice is exactly 16 bytes");
    let json_bytes = &plaintext[SALT_SIZE..];
    let payload: serde_json::Value = serde_json::from_slice(json_bytes)
        .map_err(|e| CryptoError::DecryptionError(e.to_string()))?;

    // Verify payload_plain_hash
    let computed_hash = compute_payload_plain_hash(&payload, Some(&salt))?;
    if computed_hash != *expected_plain_hash {
        return Err(CryptoError::PayloadHashMismatch);
    }

    Ok(payload)
}

/// Wrap DEK using X25519 ECDH + HKDF + AES-256-GCM
fn wrap_dek(
    dek: &[u8; 32],
    recipient_public_key: &[u8; 32],
    info: &[u8],
) -> Result<(Vec<u8>, Vec<u8>), CryptoError> {
    // Generate ephemeral X25519 key pair
    let mut rng = rand::thread_rng();
    let ephemeral_secret = EphemeralSecret::random_from_rng(&mut rng);
    let ephemeral_public = PublicKey::from(&ephemeral_secret);
    let enc = ephemeral_public.as_bytes().to_vec();

    // Compute shared secret via ECDH
    let recipient_pk = PublicKey::from(*recipient_public_key);
    let shared_secret = ephemeral_secret.diffie_hellman(&recipient_pk);

    // Derive wrapping key using HKDF
    let hk = Hkdf::<Sha256>::new(None, shared_secret.as_bytes());
    let mut wrapping_key = [0u8; 32];
    hk.expand(info, &mut wrapping_key)
        .map_err(|e| CryptoError::KeyWrapError(e.to_string()))?;

    // Wrap DEK with AES-256-GCM
    let mut wrap_nonce_bytes = [0u8; NONCE_SIZE];
    rng.fill_bytes(&mut wrap_nonce_bytes);

    let key = Key::<Aes256Gcm>::from_slice(&wrapping_key);
    let wrap_cipher = Aes256Gcm::new(key);
    let wrap_nonce = Nonce::from_slice(&wrap_nonce_bytes);

    let wrapped = wrap_cipher
        .encrypt(wrap_nonce, dek.as_ref())
        .map_err(|e| CryptoError::KeyWrapError(e.to_string()))?;

    // wrappedKey = nonce || ciphertext_with_tag
    let mut wrapped_key = Vec::with_capacity(NONCE_SIZE + wrapped.len());
    wrapped_key.extend_from_slice(&wrap_nonce_bytes);
    wrapped_key.extend_from_slice(&wrapped);

    Ok((enc, wrapped_key))
}

/// Unwrap DEK using X25519 ECDH + HKDF + AES-256-GCM
fn unwrap_dek(
    enc: &[u8; 32],
    wrapped_key: &[u8],
    recipient_private_key: &[u8; 32],
    info: &[u8],
) -> Result<[u8; 32], CryptoError> {
    // Compute shared secret
    let ephemeral_pk = PublicKey::from(*enc);
    let recipient_sk = StaticSecret::from(*recipient_private_key);
    let shared_secret = recipient_sk.diffie_hellman(&ephemeral_pk);

    // Derive wrapping key
    let hk = Hkdf::<Sha256>::new(None, shared_secret.as_bytes());
    let mut wrapping_key = [0u8; 32];
    hk.expand(info, &mut wrapping_key)
        .map_err(|e| CryptoError::KeyWrapError(e.to_string()))?;

    // Unwrap DEK
    if wrapped_key.len() < NONCE_SIZE {
        return Err(CryptoError::KeyWrapError(
            "Wrapped key too short".to_string(),
        ));
    }
    let wrap_nonce = Nonce::from_slice(&wrapped_key[..NONCE_SIZE]);
    let ciphertext_tag = &wrapped_key[NONCE_SIZE..];

    let key = Key::<Aes256Gcm>::from_slice(&wrapping_key);
    let wrap_cipher = Aes256Gcm::new(key);

    let dek_bytes = wrap_cipher
        .decrypt(wrap_nonce, ciphertext_tag)
        .map_err(|e| CryptoError::KeyWrapError(e.to_string()))?;

    let mut dek = [0u8; KEY_SIZE];
    dek.copy_from_slice(&dek_bytes);
    Ok(dek)
}

/// Generate an X25519 keypair for key wrapping
///
/// Returns (`private_key`, `public_key`) as 32-byte arrays.
pub fn generate_x25519_keypair() -> ([u8; 32], [u8; 32]) {
    let mut rng = rand::thread_rng();
    let secret = StaticSecret::random_from_rng(&mut rng);
    let public = PublicKey::from(&secret);
    (secret.to_bytes(), *public.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::PayloadAadParams;
    use serde_json::json;

    const TEST_UUID: &str = "550e8400-e29b-41d4-a716-446655440000";

    fn test_aad_params(plain_hash: &[u8; 32]) -> PayloadAadParams<'_> {
        PayloadAadParams {
            ves_version: 1,
            tenant_id: TEST_UUID,
            store_id: TEST_UUID,
            event_id: TEST_UUID,
            source_agent_id: TEST_UUID,
            agent_key_id: 1,
            entity_type: "order",
            entity_id: "ord_001",
            event_type: "order.created",
            created_at: "2026-02-21T00:00:00Z",
            payload_plain_hash: plain_hash,
        }
    }

    #[test]
    fn dek_wrap_unwrap_roundtrip() {
        let (private_key, public_key) = generate_x25519_keypair();
        let dek = [42u8; 32];
        let info = b"test_info";

        let (enc, wrapped) = wrap_dek(&dek, &public_key, info).unwrap();

        let mut enc_arr = [0u8; 32];
        enc_arr.copy_from_slice(&enc);
        let recovered = unwrap_dek(&enc_arr, &wrapped, &private_key, info).unwrap();
        assert_eq!(recovered, dek);
    }

    #[test]
    fn dek_unwrap_wrong_key_fails() {
        let (_, public_key) = generate_x25519_keypair();
        let (wrong_private, _) = generate_x25519_keypair();
        let dek = [42u8; 32];
        let info = b"test_info";

        let (enc, wrapped) = wrap_dek(&dek, &public_key, info).unwrap();
        let mut enc_arr = [0u8; 32];
        enc_arr.copy_from_slice(&enc);
        assert!(unwrap_dek(&enc_arr, &wrapped, &wrong_private, info).is_err());
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let payload = json!({"order_id": "ord_001", "amount": 99.99});
        let plain_hash = [0u8; 32]; // placeholder, will be computed inside
        let aad_params = test_aad_params(&plain_hash);

        let (private_key, public_key) = generate_x25519_keypair();
        let recipients = vec![RecipientKey {
            kid: 1,
            public_key,
        }];

        let enc_result = encrypt_payload(&payload, &aad_params, &recipients).unwrap();

        // Compute AAD for decryption
        let dec_aad_params = PayloadAadParams {
            payload_plain_hash: &enc_result.payload_plain_hash,
            ..aad_params
        };
        let dec_payload_aad = crate::hash::compute_payload_aad(&dec_aad_params).unwrap();

        let decrypted = decrypt_payload(
            &enc_result.payload_encrypted,
            &dec_payload_aad,
            1,
            &private_key,
            &enc_result.payload_plain_hash,
        )
        .unwrap();

        assert_eq!(decrypted, payload);
    }

    #[test]
    fn encrypt_no_recipients_fails() {
        let payload = json!({"key": "value"});
        let plain_hash = [0u8; 32];
        let aad_params = test_aad_params(&plain_hash);
        assert!(encrypt_payload(&payload, &aad_params, &[]).is_err());
    }

    #[test]
    fn encrypt_multiple_recipients() {
        let payload = json!({"key": "value"});
        let plain_hash = [0u8; 32];
        let aad_params = test_aad_params(&plain_hash);

        let (priv1, pub1) = generate_x25519_keypair();
        let (priv2, pub2) = generate_x25519_keypair();
        let recipients = vec![
            RecipientKey {
                kid: 1,
                public_key: pub1,
            },
            RecipientKey {
                kid: 2,
                public_key: pub2,
            },
        ];

        let enc_result = encrypt_payload(&payload, &aad_params, &recipients).unwrap();

        let dec_aad_params = PayloadAadParams {
            payload_plain_hash: &enc_result.payload_plain_hash,
            ..aad_params
        };
        let dec_payload_aad = crate::hash::compute_payload_aad(&dec_aad_params).unwrap();

        // Both recipients can decrypt
        let d1 = decrypt_payload(
            &enc_result.payload_encrypted,
            &dec_payload_aad,
            1,
            &priv1,
            &enc_result.payload_plain_hash,
        );
        let d2 = decrypt_payload(
            &enc_result.payload_encrypted,
            &dec_payload_aad,
            2,
            &priv2,
            &enc_result.payload_plain_hash,
        );
        assert!(d1.is_ok());
        assert!(d2.is_ok());
        assert_eq!(d1.unwrap(), payload);
        assert_eq!(d2.unwrap(), payload);
    }

    #[test]
    fn decrypt_wrong_recipient_fails() {
        let payload = json!({"key": "value"});
        let plain_hash = [0u8; 32];
        let aad_params = test_aad_params(&plain_hash);

        let (_, pub_key) = generate_x25519_keypair();
        let recipients = vec![RecipientKey {
            kid: 1,
            public_key: pub_key,
        }];

        let enc_result = encrypt_payload(&payload, &aad_params, &recipients).unwrap();
        let dec_aad_params = PayloadAadParams {
            payload_plain_hash: &enc_result.payload_plain_hash,
            ..aad_params
        };
        let dec_payload_aad = crate::hash::compute_payload_aad(&dec_aad_params).unwrap();

        // Try with wrong kid
        let (wrong_priv, _) = generate_x25519_keypair();
        let result = decrypt_payload(
            &enc_result.payload_encrypted,
            &dec_payload_aad,
            99,
            &wrong_priv,
            &enc_result.payload_plain_hash,
        );
        assert!(result.is_err());
    }
}
