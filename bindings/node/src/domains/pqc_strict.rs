//! PQC-Strict Operations (ML-DSA-65 only, ML-KEM-768 only).
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]
// The `#[napi]` free functions here are reached only through napi's
// registration, which rustc cannot see from inside a private module.
#![allow(dead_code)]

use super::*;

// =============================================================================
// PQC-Strict Operations (ML-DSA-65 only, ML-KEM-768 only)
// =============================================================================

#[napi(object)]
pub struct StrictSigningKeypairOutput {
    pub ml_dsa_65_public_key: Buffer,
    pub ml_dsa_65_seed: Buffer,
}

#[napi(object)]
pub struct StrictRecipientKeypairOutput {
    pub kid: u32,
    pub ml_kem_768_public_key: Buffer,
    pub ml_kem_768_seed: Buffer,
}

#[napi(object)]
pub struct StrictRecipientPrivateKeyInput {
    pub ml_kem_768_seed: Buffer,
}

#[napi(object)]
pub struct StrictRecipientPublicKeyInput {
    pub kid: u32,
    pub ml_kem_768_public_key: Buffer,
}

#[napi(object)]
pub struct StrictEncryptionResultOutput {
    pub payload_encrypted_json: String,
    pub salt: Buffer,
    pub payload_plain_hash: Buffer,
    pub payload_cipher_hash: Buffer,
}

/// Generate an ML-DSA-65-only signing keypair for PQC-strict mode.
#[napi]
pub fn ves_strict_generate_signing_keypair() -> Result<StrictSigningKeypairOutput> {
    guard(|| {
        let keypair = stateset_crypto::pqc::generate_strict_signing_keypair()
            .map_err(|e| wrap(ErrCode::Internal, "Strict signing key generation failed", e))?;

        Ok(StrictSigningKeypairOutput {
            ml_dsa_65_public_key: Buffer::from(keypair.public.ml_dsa_65_public_key.as_slice()),
            ml_dsa_65_seed: Buffer::from(keypair.private.ml_dsa_65_seed.as_slice()),
        })
    })
}

/// Sign a 32-byte hash with ML-DSA-65 only (PQC-strict mode).
#[napi]
pub fn ves_strict_sign_event_hash(hash: Buffer, ml_dsa_65_seed: Buffer) -> Result<Buffer> {
    guard(|| {
        if hash.len() != 32 {
            return Err(coded(ErrCode::Validation, "Hash must be 32 bytes"));
        }
        if ml_dsa_65_seed.len() != 32 {
            return Err(coded(ErrCode::Validation, "ML-DSA-65 seed must be 32 bytes"));
        }

        let mut hash_arr = [0u8; 32];
        hash_arr.copy_from_slice(hash.as_ref());
        let mut seed_arr = [0u8; 32];
        seed_arr.copy_from_slice(ml_dsa_65_seed.as_ref());

        let signature = stateset_crypto::pqc::strict_sign_event_hash(
            &hash_arr,
            &stateset_crypto::pqc::StrictSigningPrivateKey { ml_dsa_65_seed: seed_arr },
        )
        .map_err(|e| wrap(ErrCode::Internal, "Strict signing failed", e))?;

        Ok(Buffer::from(signature))
    })
}

/// Verify a 32-byte hash with ML-DSA-65 only (PQC-strict mode).
#[napi]
pub fn ves_strict_verify_event_signature(
    hash: Buffer,
    ml_dsa_65_signature: Buffer,
    ml_dsa_65_public_key: Buffer,
) -> Result<bool> {
    guard(|| {
        if hash.len() != 32 {
            return Ok(false);
        }

        let mut hash_arr = [0u8; 32];
        hash_arr.copy_from_slice(hash.as_ref());

        Ok(stateset_crypto::pqc::strict_verify_event_signature(
            &hash_arr,
            ml_dsa_65_signature.as_ref(),
            &stateset_crypto::pqc::StrictSigningPublicKey {
                ml_dsa_65_public_key: ml_dsa_65_public_key.as_ref().to_vec(),
            },
        ))
    })
}

/// Generate an ML-KEM-768-only recipient keypair for PQC-strict mode.
#[napi]
pub fn ves_strict_generate_recipient_keypair(kid: u32) -> Result<StrictRecipientKeypairOutput> {
    guard(|| {
        let keypair = stateset_crypto::pqc::generate_strict_recipient_keypair(kid)
            .map_err(|e| wrap(ErrCode::Internal, "Strict recipient key generation failed", e))?;

        Ok(StrictRecipientKeypairOutput {
            kid: keypair.public.kid,
            ml_kem_768_public_key: Buffer::from(keypair.public.ml_kem_768_public_key.as_slice()),
            ml_kem_768_seed: Buffer::from(keypair.private.ml_kem_768_seed.as_slice()),
        })
    })
}

/// Encrypt a JSON payload using ML-KEM-768-only recipient wrapping (PQC-strict).
#[napi]
pub fn ves_strict_encrypt_payload(
    payload_json: String,
    aad_params: HybridPayloadAadParamsInput,
    recipients: Vec<StrictRecipientPublicKeyInput>,
) -> Result<StrictEncryptionResultOutput> {
    guard(|| {
        if aad_params.payload_plain_hash.len() != 32 {
            return Err(coded(ErrCode::Validation, "payload_plain_hash must be 32 bytes"));
        }

        let payload: serde_json::Value = serde_json::from_str(&payload_json)
            .map_err(|e| wrap(ErrCode::Validation, "Invalid payload JSON", e))?;

        let mut payload_plain_hash = [0u8; 32];
        payload_plain_hash.copy_from_slice(aad_params.payload_plain_hash.as_ref());

        let recipient_keys: Vec<stateset_crypto::pqc::StrictRecipientPublicKey> = recipients
            .into_iter()
            .map(|r| stateset_crypto::pqc::StrictRecipientPublicKey {
                kid: r.kid,
                ml_kem_768_public_key: r.ml_kem_768_public_key.as_ref().to_vec(),
            })
            .collect();

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
            stateset_crypto::pqc::encrypt_payload_strict(&payload, &aad, &recipient_keys)
                .map_err(|e| wrap(ErrCode::Internal, "Strict payload encryption failed", e))?;

        Ok(StrictEncryptionResultOutput {
            payload_encrypted_json: serde_json::to_string(&encrypted.payload_encrypted)
                .map_err(|e| wrap(ErrCode::Internal, "Failed to serialize", e))?,
            salt: Buffer::from(encrypted.salt.as_slice()),
            payload_plain_hash: Buffer::from(encrypted.payload_plain_hash.as_slice()),
            payload_cipher_hash: Buffer::from(encrypted.payload_cipher_hash.as_slice()),
        })
    })
}

/// Decrypt a JSON payload using ML-KEM-768-only recipient wrapping (PQC-strict).
#[napi]
pub fn ves_strict_decrypt_payload(
    payload_encrypted_json: String,
    payload_aad: Buffer,
    recipient_kid: u32,
    recipient_private_key: StrictRecipientPrivateKeyInput,
    expected_plain_hash: Buffer,
) -> Result<String> {
    guard(|| {
        if payload_aad.len() != 32 {
            return Err(coded(ErrCode::Validation, "payload_aad must be 32 bytes"));
        }
        if recipient_private_key.ml_kem_768_seed.len() != 64 {
            return Err(coded(ErrCode::Validation, "ml_kem_768_seed must be 64 bytes"));
        }
        if expected_plain_hash.len() != 32 {
            return Err(coded(ErrCode::Validation, "expected_plain_hash must be 32 bytes"));
        }

        let payload_encrypted: serde_json::Value = serde_json::from_str(&payload_encrypted_json)
            .map_err(|e| wrap(ErrCode::Validation, "Invalid encrypted payload JSON", e))?;

        let mut aad_arr = [0u8; 32];
        aad_arr.copy_from_slice(payload_aad.as_ref());
        let mut seed = [0u8; 64];
        seed.copy_from_slice(recipient_private_key.ml_kem_768_seed.as_ref());
        let mut hash_arr = [0u8; 32];
        hash_arr.copy_from_slice(expected_plain_hash.as_ref());

        let decrypted = stateset_crypto::pqc::decrypt_payload_strict(
            &payload_encrypted,
            &aad_arr,
            recipient_kid,
            &stateset_crypto::pqc::StrictRecipientPrivateKey { ml_kem_768_seed: seed },
            &hash_arr,
        )
        .map_err(|e| wrap(ErrCode::Internal, "Strict payload decryption failed", e))?;

        serde_json::to_string(&decrypted)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to serialize", e))
    })
}

/// Generate a hybrid signing proof-of-possession bundle.
#[napi]
pub fn ves_hybrid_generate_signing_pop(
    ed25519_private_key: Buffer,
    ml_dsa_65_seed: Buffer,
    ed25519_public_key: Buffer,
    ml_dsa_65_public_key: Buffer,
) -> Result<HybridSignatureBundleOutput> {
    guard(|| {
        if ed25519_private_key.len() != 32
            || ml_dsa_65_seed.len() != 32
            || ed25519_public_key.len() != 32
        {
            return Err(coded(ErrCode::Validation, "Key sizes invalid"));
        }

        let mut ed_priv = [0u8; 32];
        ed_priv.copy_from_slice(ed25519_private_key.as_ref());
        let mut ml_seed = [0u8; 32];
        ml_seed.copy_from_slice(ml_dsa_65_seed.as_ref());
        let mut ed_pub = [0u8; 32];
        ed_pub.copy_from_slice(ed25519_public_key.as_ref());

        let keypair = stateset_crypto::pqc::HybridSigningKeypair {
            public: stateset_crypto::pqc::HybridSigningPublicKey {
                ed25519_public_key: ed_pub,
                ml_dsa_65_public_key: ml_dsa_65_public_key.as_ref().to_vec(),
            },
            private: stateset_crypto::pqc::HybridSigningPrivateKey {
                ed25519_private_key: ed_priv,
                ml_dsa_65_seed: ml_seed,
            },
        };

        let pop = stateset_crypto::pqc::generate_hybrid_signing_pop(&keypair)
            .map_err(|e| wrap(ErrCode::Internal, "PoP generation failed", e))?;

        Ok(HybridSignatureBundleOutput {
            ed25519_signature: Buffer::from(pop.ed25519_signature.as_slice()),
            ml_dsa_65_signature: Buffer::from(pop.ml_dsa_65_signature.as_slice()),
        })
    })
}

/// Verify a hybrid signing proof-of-possession bundle.
#[napi]
pub fn ves_hybrid_verify_signing_pop(
    ed25519_signature: Buffer,
    ml_dsa_65_signature: Buffer,
    ed25519_public_key: Buffer,
    ml_dsa_65_public_key: Buffer,
) -> Result<bool> {
    guard(|| {
        if ed25519_signature.len() != 64 || ed25519_public_key.len() != 32 {
            return Ok(false);
        }

        let mut ed_sig = [0u8; 64];
        ed_sig.copy_from_slice(ed25519_signature.as_ref());
        let mut ed_pub = [0u8; 32];
        ed_pub.copy_from_slice(ed25519_public_key.as_ref());

        Ok(stateset_crypto::pqc::verify_hybrid_signing_pop(
            &stateset_crypto::pqc::HybridSignatureBundle {
                ed25519_signature: ed_sig,
                ml_dsa_65_signature: ml_dsa_65_signature.as_ref().to_vec(),
            },
            &stateset_crypto::pqc::HybridSigningPublicKey {
                ed25519_public_key: ed_pub,
                ml_dsa_65_public_key: ml_dsa_65_public_key.as_ref().to_vec(),
            },
        ))
    })
}

/// Generate a PQC-strict signing proof-of-possession.
#[napi]
pub fn ves_strict_generate_signing_pop(
    ml_dsa_65_seed: Buffer,
    ml_dsa_65_public_key: Buffer,
) -> Result<Buffer> {
    guard(|| {
        if ml_dsa_65_seed.len() != 32 {
            return Err(coded(ErrCode::Validation, "ML-DSA-65 seed must be 32 bytes"));
        }
        let mut seed = [0u8; 32];
        seed.copy_from_slice(ml_dsa_65_seed.as_ref());

        let keypair = stateset_crypto::pqc::StrictSigningKeypair {
            public: stateset_crypto::pqc::StrictSigningPublicKey {
                ml_dsa_65_public_key: ml_dsa_65_public_key.as_ref().to_vec(),
            },
            private: stateset_crypto::pqc::StrictSigningPrivateKey { ml_dsa_65_seed: seed },
        };

        let pop = stateset_crypto::pqc::generate_strict_signing_pop(&keypair)
            .map_err(|e| wrap(ErrCode::Internal, "Strict PoP generation failed", e))?;
        Ok(Buffer::from(pop))
    })
}

/// Verify a PQC-strict signing proof-of-possession.
#[napi]
pub fn ves_strict_verify_signing_pop(
    ml_dsa_65_signature: Buffer,
    ml_dsa_65_public_key: Buffer,
) -> Result<bool> {
    guard(|| {
        Ok(stateset_crypto::pqc::verify_strict_signing_pop(
            ml_dsa_65_signature.as_ref(),
            &stateset_crypto::pqc::StrictSigningPublicKey {
                ml_dsa_65_public_key: ml_dsa_65_public_key.as_ref().to_vec(),
            },
        ))
    })
}

/// Encrypt a buffer with AES-256-GCM
///
/// Returns nonce (12 bytes) || ciphertext || tag (16 bytes)
#[napi]
pub fn aes_gcm_encrypt(plaintext: Buffer, key: Buffer, aad: Buffer) -> Result<Buffer> {
    guard(|| {
        use aes_gcm::aead::Aead;
        use aes_gcm::{Aes256Gcm, Key, KeyInit, Nonce};

        if key.len() != 32 {
            return Err(coded(ErrCode::Validation, "Key must be 32 bytes"));
        }

        let aes_key = Key::<Aes256Gcm>::from_slice(key.as_ref());
        let cipher = Aes256Gcm::new(aes_key);

        let mut nonce_bytes = [0u8; 12];
        use rand::RngCore;
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let payload = aes_gcm::aead::Payload { msg: plaintext.as_ref(), aad: aad.as_ref() };
        let ciphertext_tag = cipher
            .encrypt(nonce, payload)
            .map_err(|e| wrap(ErrCode::Internal, "Encryption failed", e))?;

        let mut result = Vec::with_capacity(12 + ciphertext_tag.len());
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext_tag);
        Ok(Buffer::from(result))
    })
}

/// Decrypt a buffer with AES-256-GCM
///
/// Input: nonce (12 bytes) || ciphertext || tag (16 bytes)
#[napi]
pub fn aes_gcm_decrypt(encrypted: Buffer, key: Buffer, aad: Buffer) -> Result<Buffer> {
    guard(|| {
        use aes_gcm::aead::Aead;
        use aes_gcm::{Aes256Gcm, Key, KeyInit, Nonce};

        if key.len() != 32 {
            return Err(coded(ErrCode::Validation, "Key must be 32 bytes"));
        }
        if encrypted.len() < 28 {
            return Err(coded(
                ErrCode::Validation,
                "Encrypted data too short (need at least nonce + tag)",
            ));
        }

        let nonce = Nonce::from_slice(&encrypted[..12]);
        let ciphertext_tag = &encrypted[12..];

        let aes_key = Key::<Aes256Gcm>::from_slice(key.as_ref());
        let cipher = Aes256Gcm::new(aes_key);

        let payload = aes_gcm::aead::Payload { msg: ciphertext_tag, aad: aad.as_ref() };
        let plaintext = cipher
            .decrypt(nonce, payload)
            .map_err(|e| wrap(ErrCode::Internal, "Decryption failed", e))?;

        Ok(Buffer::from(plaintext))
    })
}

/// Compute Merkle root from an array of 32-byte leaf hashes
#[napi]
pub fn merkle_root(leaves: Vec<Buffer>) -> Result<Buffer> {
    guard(|| {
        let leaf_arrays: std::result::Result<Vec<[u8; 32]>, _> = leaves
            .iter()
            .map(|b| {
                if b.len() != 32 {
                    Err(coded(ErrCode::Validation, "Each leaf must be 32 bytes"))
                } else {
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(b.as_ref());
                    Ok(arr)
                }
            })
            .collect();

        let leaf_arrays = leaf_arrays?;
        let root = stateset_crypto::merkle::compute_merkle_root(&leaf_arrays);
        Ok(Buffer::from(root.as_slice()))
    })
}
