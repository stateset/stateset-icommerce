//! VES v1.0 Cryptographic Operations (stateset-crypto).
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]
// `#[napi]` free functions and their object types are reached only through
// napi's registration, which rustc cannot see from inside a private module.
#![allow(dead_code)]

use super::*;

// =============================================================================
// VES v1.0 Cryptographic Operations (stateset-crypto)
// =============================================================================

#[napi(object)]
pub struct HybridSigningKeypairOutput {
    pub ed25519_public_key: Buffer,
    pub ed25519_private_key: Buffer,
    pub ml_dsa_65_public_key: Buffer,
    pub ml_dsa_65_seed: Buffer,
}

#[napi(object)]
pub struct HybridSignatureBundleOutput {
    pub ed25519_signature: Buffer,
    pub ml_dsa_65_signature: Buffer,
}

#[napi(object)]
pub struct HybridRecipientKeypairOutput {
    pub kid: u32,
    pub x25519_public_key: Buffer,
    pub x25519_private_key: Buffer,
    pub ml_kem_768_public_key: Buffer,
    pub ml_kem_768_seed: Buffer,
}

#[napi(object)]
pub struct HybridPayloadAadParamsInput {
    pub ves_version: u32,
    pub tenant_id: String,
    pub store_id: String,
    pub event_id: String,
    pub source_agent_id: String,
    pub agent_key_id: u32,
    pub entity_type: String,
    pub entity_id: String,
    pub event_type: String,
    pub created_at: String,
    pub payload_plain_hash: Buffer,
}

#[napi(object)]
pub struct HybridRecipientPublicKeyInput {
    pub kid: u32,
    pub x25519_public_key: Buffer,
    pub ml_kem_768_public_key: Buffer,
}

#[napi(object)]
pub struct HybridRecipientPrivateKeyInput {
    pub x25519_private_key: Buffer,
    pub ml_kem_768_seed: Buffer,
}

#[napi(object)]
pub struct HybridEncryptionResultOutput {
    pub payload_encrypted_json: String,
    pub salt: Buffer,
    pub payload_plain_hash: Buffer,
    pub payload_cipher_hash: Buffer,
}

/// Canonicalize a JSON string per RFC 8785 JCS
#[napi]
pub fn jcs_canonicalize(json_str: String) -> Result<String> {
    guard(|| {
        let value: serde_json::Value = serde_json::from_str(&json_str)
            .map_err(|e| wrap(ErrCode::Validation, "Invalid JSON", e))?;
        stateset_crypto::canonicalize::canonicalize_json(&value)
            .map_err(|e| wrap(ErrCode::Internal, "JCS error", e))
    })
}

/// Compute domain-separated SHA-256 hash
///
/// domain: one of "PAYLOAD_PLAIN", "PAYLOAD_AAD", "PAYLOAD_CIPHER", "RECIPIENTS",
///         "EVENTSIG", "LEAF", "NODE", "PAD_LEAF", "STREAM", "RECEIPT"
/// data: hex-encoded data to hash (after the domain prefix)
#[napi(ts_args_type = "domain: VesHashDomain, data: Buffer")]
pub fn domain_hash(domain: String, data: Buffer) -> Result<Buffer> {
    guard(|| {
        use sha2::{Digest, Sha256};

        let prefix: &[u8] = match domain.as_str() {
            "PAYLOAD_PLAIN" => stateset_crypto::domain::PAYLOAD_PLAIN,
            "PAYLOAD_AAD" => stateset_crypto::domain::PAYLOAD_AAD,
            "PAYLOAD_CIPHER" => stateset_crypto::domain::PAYLOAD_CIPHER,
            "RECIPIENTS" => stateset_crypto::domain::RECIPIENTS,
            "EVENTSIG" => stateset_crypto::domain::EVENTSIG,
            "LEAF" => stateset_crypto::domain::LEAF,
            "NODE" => stateset_crypto::domain::NODE,
            "PAD_LEAF" => stateset_crypto::domain::PAD_LEAF,
            "STREAM" => stateset_crypto::domain::STREAM,
            "RECEIPT" => stateset_crypto::domain::RECEIPT,
            _ => return Err(wrap(ErrCode::Validation, "Unknown domain", domain)),
        };

        let mut hasher = Sha256::new();
        hasher.update(prefix);
        hasher.update(data.as_ref());
        let result: [u8; 32] = hasher.finalize().into();
        Ok(Buffer::from(result.as_slice()))
    })
}

/// Sign a 32-byte hash with Ed25519
///
/// Returns 64-byte signature
#[napi]
pub fn ed25519_sign(hash: Buffer, private_key: Buffer) -> Result<Buffer> {
    guard(|| {
        if hash.len() != 32 {
            return Err(coded(ErrCode::Validation, "Hash must be 32 bytes"));
        }
        if private_key.len() != 32 {
            return Err(coded(ErrCode::Validation, "Private key must be 32 bytes"));
        }
        let mut hash_arr = [0u8; 32];
        hash_arr.copy_from_slice(hash.as_ref());
        let mut key_arr = [0u8; 32];
        key_arr.copy_from_slice(private_key.as_ref());

        let sig = stateset_crypto::sign::sign_event_hash(&hash_arr, &key_arr)
            .map_err(|e| wrap(ErrCode::Internal, "Sign error", e))?;
        Ok(Buffer::from(sig.as_slice()))
    })
}

/// Verify an Ed25519 signature
///
/// Returns true if signature is valid
#[napi]
pub fn ed25519_verify(hash: Buffer, signature: Buffer, public_key: Buffer) -> Result<bool> {
    guard(|| {
        if hash.len() != 32 || signature.len() != 64 || public_key.len() != 32 {
            return Ok(false);
        }
        let mut hash_arr = [0u8; 32];
        hash_arr.copy_from_slice(hash.as_ref());
        let mut sig_arr = [0u8; 64];
        sig_arr.copy_from_slice(signature.as_ref());
        let mut key_arr = [0u8; 32];
        key_arr.copy_from_slice(public_key.as_ref());

        Ok(stateset_crypto::sign::verify_event_signature(&hash_arr, &sig_arr, &key_arr))
    })
}

/// Generate a hybrid `Ed25519 + ML-DSA-65` signing keypair.
#[napi]
pub fn ves_hybrid_generate_signing_keypair() -> Result<HybridSigningKeypairOutput> {
    guard(|| {
        let keypair = stateset_crypto::pqc::generate_hybrid_signing_keypair()
            .map_err(|e| wrap(ErrCode::Internal, "Hybrid signing key generation failed", e))?;

        Ok(HybridSigningKeypairOutput {
            ed25519_public_key: Buffer::from(keypair.public.ed25519_public_key.as_slice()),
            ed25519_private_key: Buffer::from(keypair.private.ed25519_private_key.as_slice()),
            ml_dsa_65_public_key: Buffer::from(keypair.public.ml_dsa_65_public_key.as_slice()),
            ml_dsa_65_seed: Buffer::from(keypair.private.ml_dsa_65_seed.as_slice()),
        })
    })
}

/// Sign a 32-byte hash with the hybrid `Ed25519 + ML-DSA-65` profile.
#[napi]
pub fn ves_hybrid_sign_event_hash(
    hash: Buffer,
    ed25519_private_key: Buffer,
    ml_dsa_65_seed: Buffer,
) -> Result<HybridSignatureBundleOutput> {
    guard(|| {
        if hash.len() != 32 {
            return Err(coded(ErrCode::Validation, "Hash must be 32 bytes"));
        }
        if ed25519_private_key.len() != 32 {
            return Err(coded(ErrCode::Validation, "Ed25519 private key must be 32 bytes"));
        }
        if ml_dsa_65_seed.len() != 32 {
            return Err(coded(ErrCode::Validation, "ML-DSA-65 seed must be 32 bytes"));
        }

        let mut hash_arr = [0u8; 32];
        hash_arr.copy_from_slice(hash.as_ref());
        let mut ed25519_private_key_arr = [0u8; 32];
        ed25519_private_key_arr.copy_from_slice(ed25519_private_key.as_ref());
        let mut ml_dsa_65_seed_arr = [0u8; 32];
        ml_dsa_65_seed_arr.copy_from_slice(ml_dsa_65_seed.as_ref());

        let signature = stateset_crypto::pqc::hybrid_sign_event_hash(
            &hash_arr,
            &stateset_crypto::pqc::HybridSigningPrivateKey {
                ed25519_private_key: ed25519_private_key_arr,
                ml_dsa_65_seed: ml_dsa_65_seed_arr,
            },
        )
        .map_err(|e| wrap(ErrCode::Internal, "Hybrid signing failed", e))?;

        Ok(HybridSignatureBundleOutput {
            ed25519_signature: Buffer::from(signature.ed25519_signature.as_slice()),
            ml_dsa_65_signature: Buffer::from(signature.ml_dsa_65_signature.as_slice()),
        })
    })
}

/// Verify a 32-byte hash with the hybrid `Ed25519 + ML-DSA-65` profile.
#[napi]
pub fn ves_hybrid_verify_event_signature(
    hash: Buffer,
    ed25519_signature: Buffer,
    ml_dsa_65_signature: Buffer,
    ed25519_public_key: Buffer,
    ml_dsa_65_public_key: Buffer,
) -> Result<bool> {
    guard(|| {
        if hash.len() != 32 || ed25519_signature.len() != 64 || ed25519_public_key.len() != 32 {
            return Ok(false);
        }

        let mut hash_arr = [0u8; 32];
        hash_arr.copy_from_slice(hash.as_ref());
        let mut ed25519_signature_arr = [0u8; 64];
        ed25519_signature_arr.copy_from_slice(ed25519_signature.as_ref());
        let mut ed25519_public_key_arr = [0u8; 32];
        ed25519_public_key_arr.copy_from_slice(ed25519_public_key.as_ref());

        Ok(stateset_crypto::pqc::hybrid_verify_event_signature(
            &hash_arr,
            &stateset_crypto::pqc::HybridSignatureBundle {
                ed25519_signature: ed25519_signature_arr,
                ml_dsa_65_signature: ml_dsa_65_signature.as_ref().to_vec(),
            },
            &stateset_crypto::pqc::HybridSigningPublicKey {
                ed25519_public_key: ed25519_public_key_arr,
                ml_dsa_65_public_key: ml_dsa_65_public_key.as_ref().to_vec(),
            },
        ))
    })
}

/// Return the fixed-seed ML-DSA-65 public key used by cross-language test vectors.
#[napi]
pub fn ves_test_vector_ml_dsa_public_key() -> Result<Buffer> {
    guard(|| {
        Ok(Buffer::from(
            stateset_crypto::pqc::test_vector_ml_dsa_public_key(
                &stateset_crypto::pqc::TEST_VECTOR_SIGNING_SEED,
            )
            .as_slice(),
        ))
    })
}

/// Generate a hybrid `X25519 + ML-KEM-768` recipient keypair.
#[napi]
pub fn ves_hybrid_generate_recipient_keypair(kid: u32) -> Result<HybridRecipientKeypairOutput> {
    guard(|| {
        let keypair = stateset_crypto::pqc::generate_hybrid_recipient_keypair(kid)
            .map_err(|e| wrap(ErrCode::Internal, "Hybrid recipient key generation failed", e))?;

        Ok(HybridRecipientKeypairOutput {
            kid: keypair.public.kid,
            x25519_public_key: Buffer::from(keypair.public.x25519_public_key.as_slice()),
            x25519_private_key: Buffer::from(keypair.private.x25519_private_key.as_slice()),
            ml_kem_768_public_key: Buffer::from(keypair.public.ml_kem_768_public_key.as_slice()),
            ml_kem_768_seed: Buffer::from(keypair.private.ml_kem_768_seed.as_slice()),
        })
    })
}

/// Return the fixed-seed ML-KEM-768 public key used by cross-language test vectors.
#[napi]
pub fn ves_test_vector_ml_kem_public_key() -> Result<Buffer> {
    guard(|| {
        Ok(Buffer::from(
            stateset_crypto::pqc::test_vector_ml_kem_public_key(
                &stateset_crypto::pqc::TEST_VECTOR_KEM_SEED,
            )
            .as_slice(),
        ))
    })
}

/// Encrypt a JSON payload using hybrid `X25519 + ML-KEM-768` recipient wrapping.
#[napi]
pub fn ves_hybrid_encrypt_payload(
    payload_json: String,
    aad_params: HybridPayloadAadParamsInput,
    recipients: Vec<HybridRecipientPublicKeyInput>,
) -> Result<HybridEncryptionResultOutput> {
    guard(|| {
        if aad_params.payload_plain_hash.len() != 32 {
            return Err(coded(ErrCode::Validation, "payload_plain_hash must be 32 bytes"));
        }

        let payload: serde_json::Value = serde_json::from_str(&payload_json)
            .map_err(|e| wrap(ErrCode::Validation, "Invalid payload JSON", e))?;

        let mut payload_plain_hash = [0u8; 32];
        payload_plain_hash.copy_from_slice(aad_params.payload_plain_hash.as_ref());

        let recipient_keys = recipients
            .into_iter()
            .map(|recipient| {
                let mut x25519_public_key = [0u8; 32];
                if recipient.x25519_public_key.len() != 32 {
                    return Err(coded(ErrCode::Validation, "x25519_public_key must be 32 bytes"));
                }
                x25519_public_key.copy_from_slice(recipient.x25519_public_key.as_ref());

                Ok(stateset_crypto::pqc::HybridRecipientPublicKey {
                    kid: recipient.kid,
                    x25519_public_key,
                    ml_kem_768_public_key: recipient.ml_kem_768_public_key.as_ref().to_vec(),
                })
            })
            .collect::<Result<Vec<_>>>()?;

        let aad = stateset_crypto::hash::PayloadAadParams {
            ves_version: aad_params.ves_version,
            tenant_id: &aad_params.tenant_id,
            store_id: &aad_params.store_id,
            event_id: &aad_params.event_id,
            source_agent_id: &aad_params.source_agent_id,
            agent_key_id: aad_params.agent_key_id,
            entity_type: &aad_params.entity_type,
            entity_id: &aad_params.entity_id,
            event_type: &aad_params.event_type,
            created_at: &aad_params.created_at,
            payload_plain_hash: &payload_plain_hash,
        };

        let encrypted =
            stateset_crypto::pqc::encrypt_payload_hybrid(&payload, &aad, &recipient_keys)
                .map_err(|e| wrap(ErrCode::Internal, "Hybrid payload encryption failed", e))?;

        Ok(HybridEncryptionResultOutput {
            payload_encrypted_json: serde_json::to_string(&encrypted.payload_encrypted)
                .map_err(|e| wrap(ErrCode::Internal, "Failed to serialize encrypted payload", e))?,
            salt: Buffer::from(encrypted.salt.as_slice()),
            payload_plain_hash: Buffer::from(encrypted.payload_plain_hash.as_slice()),
            payload_cipher_hash: Buffer::from(encrypted.payload_cipher_hash.as_slice()),
        })
    })
}

/// Decrypt a JSON payload using hybrid `X25519 + ML-KEM-768` recipient wrapping.
#[napi]
pub fn ves_hybrid_decrypt_payload(
    payload_encrypted_json: String,
    payload_aad: Buffer,
    recipient_kid: u32,
    recipient_private_key: HybridRecipientPrivateKeyInput,
    expected_plain_hash: Buffer,
) -> Result<String> {
    guard(|| {
        if payload_aad.len() != 32 {
            return Err(coded(ErrCode::Validation, "payload_aad must be 32 bytes"));
        }
        if recipient_private_key.x25519_private_key.len() != 32 {
            return Err(coded(ErrCode::Validation, "x25519_private_key must be 32 bytes"));
        }
        if recipient_private_key.ml_kem_768_seed.len() != 64 {
            return Err(coded(ErrCode::Validation, "ml_kem_768_seed must be 64 bytes"));
        }
        if expected_plain_hash.len() != 32 {
            return Err(coded(ErrCode::Validation, "expected_plain_hash must be 32 bytes"));
        }

        let payload_encrypted: serde_json::Value = serde_json::from_str(&payload_encrypted_json)
            .map_err(|e| wrap(ErrCode::Validation, "Invalid encrypted payload JSON", e))?;

        let mut payload_aad_arr = [0u8; 32];
        payload_aad_arr.copy_from_slice(payload_aad.as_ref());
        let mut x25519_private_key = [0u8; 32];
        x25519_private_key.copy_from_slice(recipient_private_key.x25519_private_key.as_ref());
        let mut ml_kem_768_seed = [0u8; 64];
        ml_kem_768_seed.copy_from_slice(recipient_private_key.ml_kem_768_seed.as_ref());
        let mut expected_plain_hash_arr = [0u8; 32];
        expected_plain_hash_arr.copy_from_slice(expected_plain_hash.as_ref());

        let decrypted = stateset_crypto::pqc::decrypt_payload_hybrid(
            &payload_encrypted,
            &payload_aad_arr,
            recipient_kid,
            &stateset_crypto::pqc::HybridRecipientPrivateKey {
                x25519_private_key,
                ml_kem_768_seed,
            },
            &expected_plain_hash_arr,
        )
        .map_err(|e| wrap(ErrCode::Internal, "Hybrid payload decryption failed", e))?;

        serde_json::to_string(&decrypted)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to serialize decrypted payload", e))
    })
}
