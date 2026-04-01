//! Post-quantum cryptography helpers for VES.
//!
//! This module keeps the existing Ed25519/X25519 flows intact while adding
//! hybrid and strict lattice-based primitives:
//!
//! **Hybrid mode** (`ed25519+mldsa65` / `x25519+mlkem768`):
//! - `Ed25519 + ML-DSA-65` for event signatures
//! - `X25519 + ML-KEM-768` for payload key wrapping
//!
//! **PQC-strict mode** (`mldsa65` / `mlkem768`):
//! - `ML-DSA-65` only for event signatures
//! - `ML-KEM-768` only for payload key wrapping
//!
//! **Proof-of-possession** for key registration.
//!
//! **Receipt signing** for sequencer non-repudiation.

use std::borrow::Cow;
use std::collections::HashSet;

use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, Key, KeyInit, Nonce};
use base64::Engine;
use hkdf::Hkdf;
use ml_dsa::signature::{Keypair, Signer, Verifier};
use ml_dsa::{
    EncodedVerifyingKey as MlDsaEncodedVerifyingKey, KeyGen, MlDsa65,
    Signature as MlDsaSignature, SigningKey as MlDsaSigningKey,
    VerifyingKey as MlDsaVerifyingKey,
};
use ml_kem::kem::{Decapsulate, KeyExport, TryKeyInit};
use ml_kem::{
    B32 as MlKemB32, DecapsulationKey768, EncapsulationKey768, Seed as MlKemSeed,
    ml_kem_768::Ciphertext as MlKemCiphertext768,
};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use x25519_dalek::{EphemeralSecret, PublicKey as X25519PublicKey, StaticSecret};
use zeroize::Zeroizing;

use crate::CryptoError;
use crate::canonicalize::canonicalize_json;
use crate::hash::{
    PayloadAadParams, PayloadCipherParams, compute_payload_aad, compute_payload_cipher_hash,
    compute_payload_plain_hash, compute_recipients_hash,
};
use crate::sign::{generate_keypair, sign_event_hash, verify_event_signature};

const NONCE_SIZE: usize = 12;
const SALT_SIZE: usize = 16;
const KEY_SIZE: usize = 32;
const TAG_SIZE: usize = 16;
const HYBRID_ENC_VERSION: u64 = 2;
const STRICT_ENC_VERSION: u64 = 3;
const PAYLOAD_AEAD_ALGORITHM: &str = "AES-256-GCM";
const WRAP_AEAD_ALGORITHM: &str = "AES-256-GCM";
const WRAP_SCHEME_INFO_PREFIX: &[u8] = b"VES_PQC_WRAP_HYBRID_V1";
const STRICT_WRAP_SCHEME_INFO_PREFIX: &[u8] = b"VES_PQC_WRAP_STRICT_V1";
const HYBRID_HKDF_SALT: &[u8] = b"VES_PQC_HYBRID_HKDF_SALT_V1";
const STRICT_HKDF_SALT: &[u8] = b"VES_PQC_STRICT_HKDF_SALT_V1";
const POP_DOMAIN: &[u8] = b"VES_POP_V1";

/// Hybrid signature scheme identifier for VES event signatures.
pub const HYBRID_SIGNATURE_SCHEME: &str = "ed25519+mldsa65";
/// Hybrid KEM identifier for payload key wrapping.
pub const HYBRID_KEM_SCHEME: &str = "x25519+mlkem768";
/// Hybrid key-wrap identifier for recipient envelopes.
pub const HYBRID_WRAP_SCHEME: &str = "x25519+mlkem768+hkdf-sha256";
/// PQC-strict signature scheme identifier.
pub const STRICT_SIGNATURE_SCHEME: &str = "mldsa65";
/// PQC-strict KEM identifier.
pub const STRICT_KEM_SCHEME: &str = "mlkem768";
/// PQC-strict key-wrap identifier.
pub const STRICT_WRAP_SCHEME: &str = "mlkem768+hkdf-sha256";

/// Security profile for VES PQC migration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SecurityProfile {
    /// Classical Ed25519 / X25519 only.
    Legacy,
    /// Hybrid classical + PQ: `ed25519+mldsa65` / `x25519+mlkem768`.
    Hybrid,
    /// PQ-only: `mldsa65` / `mlkem768`.
    PqcStrict,
}

/// Public component of a hybrid Ed25519 + ML-DSA-65 keypair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HybridSigningPublicKey {
    /// Ed25519 verifying key (32 bytes).
    #[serde(with = "hex")]
    pub ed25519_public_key: [u8; 32],
    /// Encoded ML-DSA-65 verifying key.
    #[serde(with = "hex")]
    pub ml_dsa_65_public_key: Vec<u8>,
}

/// Private component of a hybrid Ed25519 + ML-DSA-65 keypair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HybridSigningPrivateKey {
    /// Ed25519 signing seed (32 bytes).
    #[serde(with = "hex")]
    pub ed25519_private_key: [u8; 32],
    /// ML-DSA-65 seed (32 bytes).
    #[serde(with = "hex")]
    pub ml_dsa_65_seed: [u8; 32],
}

/// Full hybrid Ed25519 + ML-DSA-65 keypair material.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HybridSigningKeypair {
    /// Public keys used for verification.
    pub public: HybridSigningPublicKey,
    /// Private keys used for signing.
    pub private: HybridSigningPrivateKey,
}

/// Hybrid event signature bundle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HybridSignatureBundle {
    /// Classical Ed25519 signature.
    #[serde(with = "hex")]
    pub ed25519_signature: [u8; 64],
    /// PQ ML-DSA-65 signature.
    #[serde(with = "hex")]
    pub ml_dsa_65_signature: Vec<u8>,
}

/// Hybrid recipient public key material for `X25519 + ML-KEM-768`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HybridRecipientPublicKey {
    /// Recipient key identifier.
    pub kid: u32,
    /// Classical X25519 public key.
    #[serde(with = "hex")]
    pub x25519_public_key: [u8; 32],
    /// PQ ML-KEM-768 public key bytes.
    #[serde(with = "hex")]
    pub ml_kem_768_public_key: Vec<u8>,
}

/// Hybrid recipient private key material for `X25519 + ML-KEM-768`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HybridRecipientPrivateKey {
    /// Classical X25519 private key.
    #[serde(with = "hex")]
    pub x25519_private_key: [u8; 32],
    /// PQ ML-KEM-768 seed used to reconstruct the decapsulation key.
    #[serde(with = "hex")]
    pub ml_kem_768_seed: [u8; 64],
}

/// Full hybrid recipient keypair material.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HybridRecipientKeypair {
    /// Public recipient key material.
    pub public: HybridRecipientPublicKey,
    /// Private recipient key material.
    pub private: HybridRecipientPrivateKey,
}

/// Hybrid recipient-wrapped DEK metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HybridWrappedDek {
    /// Recipient key identifier.
    pub recipient_kid: u32,
    /// Wrap algorithm identifier (always [`HYBRID_WRAP_SCHEME`]).
    pub wrap_alg: Cow<'static, str>,
    /// Ephemeral X25519 public key.
    #[serde(with = "hex")]
    pub x25519_enc: [u8; 32],
    /// ML-KEM-768 ciphertext.
    #[serde(with = "hex")]
    pub ml_kem_ct: Vec<u8>,
    /// AES-GCM nonce for DEK wrapping.
    #[serde(with = "hex")]
    pub wrap_nonce: [u8; NONCE_SIZE],
    /// Wrapped DEK ciphertext + tag.
    #[serde(with = "hex")]
    pub wrapped_key: Vec<u8>,
}

// ---------------------------------------------------------------------------
// PQC-strict types (ML-DSA-65 only, ML-KEM-768 only)
// ---------------------------------------------------------------------------

/// ML-DSA-65-only verifying key for PQC-strict mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StrictSigningPublicKey {
    /// Encoded ML-DSA-65 verifying key.
    #[serde(with = "hex")]
    pub ml_dsa_65_public_key: Vec<u8>,
}

/// ML-DSA-65-only signing key seed for PQC-strict mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StrictSigningPrivateKey {
    /// ML-DSA-65 seed (32 bytes).
    #[serde(with = "hex")]
    pub ml_dsa_65_seed: [u8; 32],
}

/// Full ML-DSA-65-only keypair for PQC-strict mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StrictSigningKeypair {
    /// Public key for verification.
    pub public: StrictSigningPublicKey,
    /// Private key for signing.
    pub private: StrictSigningPrivateKey,
}

/// ML-KEM-768-only recipient public key for PQC-strict mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StrictRecipientPublicKey {
    /// Recipient key identifier.
    pub kid: u32,
    /// ML-KEM-768 public key bytes.
    #[serde(with = "hex")]
    pub ml_kem_768_public_key: Vec<u8>,
}

/// ML-KEM-768-only recipient private key for PQC-strict mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StrictRecipientPrivateKey {
    /// ML-KEM-768 seed (64 bytes).
    #[serde(with = "hex")]
    pub ml_kem_768_seed: [u8; 64],
}

/// Full ML-KEM-768-only recipient keypair for PQC-strict mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StrictRecipientKeypair {
    /// Public key for encapsulation.
    pub public: StrictRecipientPublicKey,
    /// Private key for decapsulation.
    pub private: StrictRecipientPrivateKey,
}

/// ML-KEM-768-only wrapped DEK for PQC-strict mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StrictWrappedDek {
    /// Recipient key identifier.
    pub recipient_kid: u32,
    /// Wrap algorithm identifier (always [`STRICT_WRAP_SCHEME`]).
    pub wrap_alg: Cow<'static, str>,
    /// ML-KEM-768 ciphertext.
    #[serde(with = "hex")]
    pub ml_kem_ct: Vec<u8>,
    /// AES-GCM nonce for DEK wrapping.
    #[serde(with = "hex")]
    pub wrap_nonce: [u8; NONCE_SIZE],
    /// Wrapped DEK ciphertext + tag.
    #[serde(with = "hex")]
    pub wrapped_key: Vec<u8>,
}

/// Result of PQC-strict payload encryption.
#[derive(Debug, Clone)]
pub struct StrictEncryptionResult {
    /// The PQC-strict encrypted payload structure.
    pub payload_encrypted: serde_json::Value,
    /// 16-byte payload salt.
    pub salt: [u8; 16],
    /// 32-byte payload plain hash.
    pub payload_plain_hash: [u8; 32],
    /// 32-byte payload cipher hash.
    pub payload_cipher_hash: [u8; 32],
}

/// Result of hybrid payload encryption.
#[derive(Debug, Clone)]
pub struct HybridEncryptionResult {
    /// The hybrid-encrypted payload structure.
    pub payload_encrypted: serde_json::Value,
    /// 16-byte payload salt.
    pub salt: [u8; 16],
    /// 32-byte payload plain hash.
    pub payload_plain_hash: [u8; 32],
    /// 32-byte payload cipher hash.
    pub payload_cipher_hash: [u8; 32],
}

fn ml_dsa_signing_key_from_seed(seed: &[u8; 32]) -> MlDsaSigningKey<MlDsa65> {
    <MlDsa65 as KeyGen>::from_seed(&(*seed).into())
}

fn ml_dsa_verifying_key_from_bytes(
    public_key: &[u8],
) -> Result<MlDsaVerifyingKey<MlDsa65>, CryptoError> {
    let encoded = MlDsaEncodedVerifyingKey::<MlDsa65>::try_from(public_key)
        .map_err(|_| CryptoError::SignatureError("Invalid ML-DSA-65 public key".to_string()))?;
    Ok(MlDsaVerifyingKey::decode(&encoded))
}

fn ml_dsa_signature_from_bytes(signature: &[u8]) -> Result<MlDsaSignature<MlDsa65>, CryptoError> {
    MlDsaSignature::<MlDsa65>::try_from(signature)
        .map_err(|_| CryptoError::SignatureError("Invalid ML-DSA-65 signature".to_string()))
}

fn ml_kem_decapsulation_key_from_seed(seed: &[u8; 64]) -> DecapsulationKey768 {
    DecapsulationKey768::from_seed(MlKemSeed::from(*seed))
}

fn ml_kem_encapsulation_key_from_bytes(
    public_key: &[u8],
) -> Result<EncapsulationKey768, CryptoError> {
    EncapsulationKey768::new_from_slice(public_key)
        .map_err(|_| CryptoError::KeyWrapError("Invalid ML-KEM-768 public key".to_string()))
}

fn ml_kem_ciphertext_from_bytes(ciphertext: &[u8]) -> Result<MlKemCiphertext768, CryptoError> {
    MlKemCiphertext768::try_from(ciphertext)
        .map_err(|_| CryptoError::KeyWrapError("Invalid ML-KEM-768 ciphertext".to_string()))
}

fn hkdf_wrap_key(
    shared_secret: &[u8],
    salt: &[u8],
    info: &[u8],
) -> Result<Zeroizing<[u8; 32]>, CryptoError> {
    let hk = Hkdf::<Sha256>::new(Some(salt), shared_secret);
    let mut wrapping_key = Zeroizing::new([0u8; KEY_SIZE]);
    hk.expand(info, wrapping_key.as_mut()).map_err(|error| {
        CryptoError::KeyWrapError(format!("HKDF expand failed for wrap key: {error}"))
    })?;
    Ok(wrapping_key)
}

fn hybrid_wrap_info(info: &[u8]) -> Vec<u8> {
    let mut derived = Vec::with_capacity(WRAP_SCHEME_INFO_PREFIX.len() + info.len());
    derived.extend_from_slice(WRAP_SCHEME_INFO_PREFIX);
    derived.extend_from_slice(info);
    derived
}

fn payload_plaintext_from_components(
    salt: &[u8; SALT_SIZE],
    payload: &serde_json::Value,
) -> Result<Vec<u8>, CryptoError> {
    let canonical = canonicalize_json(payload)?;
    let mut plaintext = Vec::with_capacity(SALT_SIZE + canonical.len());
    plaintext.extend_from_slice(salt);
    plaintext.extend_from_slice(canonical.as_bytes());
    Ok(plaintext)
}

/// Generate a hybrid `Ed25519 + ML-DSA-65` keypair.
///
/// # Errors
///
/// Returns [`CryptoError::SignatureError`] if ML-DSA key generation fails.
pub fn generate_hybrid_signing_keypair() -> Result<HybridSigningKeypair, CryptoError> {
    let (ed25519_private_key, ed25519_public_key) = generate_keypair();

    let mut rng = rand::thread_rng();
    let mut ml_dsa_65_seed = [0u8; 32];
    rng.fill_bytes(&mut ml_dsa_65_seed);

    let ml_dsa_signing_key = ml_dsa_signing_key_from_seed(&ml_dsa_65_seed);
    let encoded_public = ml_dsa_signing_key.verifying_key().encode();

    Ok(HybridSigningKeypair {
        public: HybridSigningPublicKey {
            ed25519_public_key,
            ml_dsa_65_public_key: encoded_public.as_slice().to_vec(),
        },
        private: HybridSigningPrivateKey { ed25519_private_key, ml_dsa_65_seed },
    })
}

/// Sign an event-signing hash with the hybrid `Ed25519 + ML-DSA-65` scheme.
///
/// # Errors
///
/// Returns [`CryptoError::SignatureError`] when either signature algorithm fails.
pub fn hybrid_sign_event_hash(
    event_signing_hash: &[u8; 32],
    private_key: &HybridSigningPrivateKey,
) -> Result<HybridSignatureBundle, CryptoError> {
    let ed25519_signature = sign_event_hash(event_signing_hash, &private_key.ed25519_private_key)?;

    let ml_dsa_signing_key = ml_dsa_signing_key_from_seed(&private_key.ml_dsa_65_seed);
    let ml_dsa_signature: MlDsaSignature<MlDsa65> = ml_dsa_signing_key
        .try_sign(event_signing_hash)
        .map_err(|error| CryptoError::SignatureError(error.to_string()))?;

    Ok(HybridSignatureBundle {
        ed25519_signature,
        ml_dsa_65_signature: ml_dsa_signature.encode().as_slice().to_vec(),
    })
}

/// Verify a hybrid `Ed25519 + ML-DSA-65` event signature.
#[must_use]
pub fn hybrid_verify_event_signature(
    event_signing_hash: &[u8; 32],
    signature: &HybridSignatureBundle,
    public_key: &HybridSigningPublicKey,
) -> bool {
    if !verify_event_signature(
        event_signing_hash,
        &signature.ed25519_signature,
        &public_key.ed25519_public_key,
    ) {
        return false;
    }

    let Ok(ml_dsa_verifying_key) = ml_dsa_verifying_key_from_bytes(&public_key.ml_dsa_65_public_key)
    else {
        return false;
    };
    let Ok(ml_dsa_signature) = ml_dsa_signature_from_bytes(&signature.ml_dsa_65_signature) else {
        return false;
    };

    ml_dsa_verifying_key.verify(event_signing_hash, &ml_dsa_signature).is_ok()
}

// ===========================================================================
// Prepared signers (amortize key expansion across multiple signatures)
// ===========================================================================

/// Pre-expanded hybrid signer that amortizes ML-DSA-65 key derivation.
///
/// Use this when signing multiple messages with the same keypair. The
/// expensive `from_seed` derivation happens once at construction time.
#[allow(missing_debug_implementations)]
pub struct PreparedHybridSigner {
    ed25519_private_key: [u8; 32],
    ml_dsa_signing_key: MlDsaSigningKey<MlDsa65>,
}

impl PreparedHybridSigner {
    /// Create a prepared signer from a hybrid private key.
    #[must_use]
    pub fn new(private_key: &HybridSigningPrivateKey) -> Self {
        Self {
            ed25519_private_key: private_key.ed25519_private_key,
            ml_dsa_signing_key: ml_dsa_signing_key_from_seed(&private_key.ml_dsa_65_seed),
        }
    }

    /// Sign an event-signing hash with the pre-expanded keys.
    ///
    /// # Errors
    ///
    /// Returns [`CryptoError::SignatureError`] if signing fails.
    pub fn sign(&self, event_signing_hash: &[u8; 32]) -> Result<HybridSignatureBundle, CryptoError> {
        let ed25519_signature = sign_event_hash(event_signing_hash, &self.ed25519_private_key)?;
        let ml_dsa_signature: MlDsaSignature<MlDsa65> = self
            .ml_dsa_signing_key
            .try_sign(event_signing_hash)
            .map_err(|error| CryptoError::SignatureError(error.to_string()))?;

        Ok(HybridSignatureBundle {
            ed25519_signature,
            ml_dsa_65_signature: ml_dsa_signature.encode().as_slice().to_vec(),
        })
    }
}

/// Pre-expanded strict signer that amortizes ML-DSA-65 key derivation.
#[allow(missing_debug_implementations)]
pub struct PreparedStrictSigner {
    ml_dsa_signing_key: MlDsaSigningKey<MlDsa65>,
}

impl PreparedStrictSigner {
    /// Create a prepared signer from a strict private key.
    #[must_use]
    pub fn new(private_key: &StrictSigningPrivateKey) -> Self {
        Self {
            ml_dsa_signing_key: ml_dsa_signing_key_from_seed(&private_key.ml_dsa_65_seed),
        }
    }

    /// Sign an event-signing hash with the pre-expanded key.
    ///
    /// # Errors
    ///
    /// Returns [`CryptoError::SignatureError`] if signing fails.
    pub fn sign(&self, event_signing_hash: &[u8; 32]) -> Result<Vec<u8>, CryptoError> {
        let sig: MlDsaSignature<MlDsa65> = self
            .ml_dsa_signing_key
            .try_sign(event_signing_hash)
            .map_err(|error| CryptoError::SignatureError(error.to_string()))?;
        Ok(sig.encode().as_slice().to_vec())
    }
}

/// Pre-parsed hybrid verifier that amortizes ML-DSA-65 public key parsing.
#[allow(missing_debug_implementations)]
pub struct PreparedHybridVerifier {
    ed25519_public_key: [u8; 32],
    ml_dsa_verifying_key: MlDsaVerifyingKey<MlDsa65>,
}

impl PreparedHybridVerifier {
    /// Create a prepared verifier from a hybrid public key.
    ///
    /// Returns `None` if the ML-DSA-65 public key is invalid.
    #[must_use]
    pub fn new(public_key: &HybridSigningPublicKey) -> Option<Self> {
        let ml_dsa_verifying_key =
            ml_dsa_verifying_key_from_bytes(&public_key.ml_dsa_65_public_key).ok()?;
        Some(Self {
            ed25519_public_key: public_key.ed25519_public_key,
            ml_dsa_verifying_key,
        })
    }

    /// Verify a hybrid signature with the pre-parsed keys.
    #[must_use]
    pub fn verify(
        &self,
        event_signing_hash: &[u8; 32],
        signature: &HybridSignatureBundle,
    ) -> bool {
        if !verify_event_signature(
            event_signing_hash,
            &signature.ed25519_signature,
            &self.ed25519_public_key,
        ) {
            return false;
        }
        let Ok(ml_dsa_sig) = ml_dsa_signature_from_bytes(&signature.ml_dsa_65_signature) else {
            return false;
        };
        self.ml_dsa_verifying_key.verify(event_signing_hash, &ml_dsa_sig).is_ok()
    }
}

/// Generate a hybrid `X25519 + ML-KEM-768` recipient keypair.
pub fn generate_hybrid_recipient_keypair(kid: u32) -> Result<HybridRecipientKeypair, CryptoError> {
    let mut rng = rand::thread_rng();

    let x25519_private_key = StaticSecret::random_from_rng(&mut rng).to_bytes();
    let x25519_secret = StaticSecret::from(x25519_private_key);
    let x25519_public_key = *X25519PublicKey::from(&x25519_secret).as_bytes();

    let mut ml_kem_768_seed = [0u8; 64];
    rng.fill_bytes(&mut ml_kem_768_seed);
    let ml_kem_key = ml_kem_decapsulation_key_from_seed(&ml_kem_768_seed);
    let ml_kem_768_public_key = ml_kem_key.encapsulation_key().to_bytes().as_slice().to_vec();

    Ok(HybridRecipientKeypair {
        public: HybridRecipientPublicKey { kid, x25519_public_key, ml_kem_768_public_key },
        private: HybridRecipientPrivateKey { x25519_private_key, ml_kem_768_seed },
    })
}

fn validate_unique_recipient_kids<'a>(
    recipient_keys: impl IntoIterator<Item = &'a HybridRecipientPublicKey>,
) -> Result<(), CryptoError> {
    let mut seen = HashSet::new();
    for recipient in recipient_keys {
        if !seen.insert(recipient.kid) {
            return Err(CryptoError::EncryptionError(format!(
                "Duplicate recipient_kid entry: {}",
                recipient.kid
            )));
        }
    }
    Ok(())
}

/// Wrap a DEK using hybrid `X25519 + ML-KEM-768` shared-secret derivation.
///
/// # Errors
///
/// Returns [`CryptoError::KeyWrapError`] if wrapping or key parsing fails.
pub fn wrap_dek_hybrid(
    dek: &[u8; 32],
    recipient_public_key: &HybridRecipientPublicKey,
    info: &[u8],
) -> Result<HybridWrappedDek, CryptoError> {
    let mut rng = rand::thread_rng();

    let mut ml_kem_random = Zeroizing::new([0u8; 32]);
    rng.fill_bytes(ml_kem_random.as_mut());
    let ml_kem_public = ml_kem_encapsulation_key_from_bytes(&recipient_public_key.ml_kem_768_public_key)?;
    let (ml_kem_ciphertext, ml_kem_shared_secret) =
        ml_kem_public.encapsulate_deterministic(&MlKemB32::from(*ml_kem_random));

    let x25519_ephemeral_secret = EphemeralSecret::random_from_rng(&mut rng);
    let x25519_ephemeral_public = X25519PublicKey::from(&x25519_ephemeral_secret);
    let x25519_recipient_public = X25519PublicKey::from(recipient_public_key.x25519_public_key);
    let x25519_shared_secret = x25519_ephemeral_secret.diffie_hellman(&x25519_recipient_public);

    let mut hybrid_shared_secret = Zeroizing::new([0u8; 64]);
    hybrid_shared_secret[..32].copy_from_slice(ml_kem_shared_secret.as_slice());
    hybrid_shared_secret[32..].copy_from_slice(x25519_shared_secret.as_bytes());
    let wrap_info = hybrid_wrap_info(info);
    let wrapping_key = hkdf_wrap_key(&*hybrid_shared_secret, HYBRID_HKDF_SALT, &wrap_info)?;

    let mut wrap_nonce = [0u8; NONCE_SIZE];
    rng.fill_bytes(&mut wrap_nonce);

    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&*wrapping_key));
    let ciphertext = cipher
        .encrypt(
            Nonce::from_slice(&wrap_nonce),
            aes_gcm::aead::Payload { msg: dek.as_ref(), aad: info },
        )
        .map_err(|error| CryptoError::KeyWrapError(error.to_string()))?;

    Ok(HybridWrappedDek {
        recipient_kid: recipient_public_key.kid,
        wrap_alg: Cow::Borrowed(HYBRID_WRAP_SCHEME),
        x25519_enc: *x25519_ephemeral_public.as_bytes(),
        ml_kem_ct: ml_kem_ciphertext.as_slice().to_vec(),
        wrap_nonce,
        wrapped_key: ciphertext,
    })
}

/// Unwrap a DEK using hybrid `X25519 + ML-KEM-768` shared-secret derivation.
///
/// # Errors
///
/// Returns [`CryptoError::KeyWrapError`] if unwrapping or key parsing fails.
pub fn unwrap_dek_hybrid(
    wrapped: &HybridWrappedDek,
    recipient_private_key: &HybridRecipientPrivateKey,
    info: &[u8],
) -> Result<[u8; 32], CryptoError> {
    let ml_kem_key = ml_kem_decapsulation_key_from_seed(&recipient_private_key.ml_kem_768_seed);
    let ml_kem_ciphertext = ml_kem_ciphertext_from_bytes(&wrapped.ml_kem_ct)?;
    let ml_kem_shared_secret = ml_kem_key.decapsulate(&ml_kem_ciphertext);

    let x25519_private = StaticSecret::from(recipient_private_key.x25519_private_key);
    let x25519_ephemeral_public = X25519PublicKey::from(wrapped.x25519_enc);
    let x25519_shared_secret = x25519_private.diffie_hellman(&x25519_ephemeral_public);

    let mut hybrid_shared_secret = Zeroizing::new([0u8; 64]);
    hybrid_shared_secret[..32].copy_from_slice(ml_kem_shared_secret.as_slice());
    hybrid_shared_secret[32..].copy_from_slice(x25519_shared_secret.as_bytes());
    let wrap_info = hybrid_wrap_info(info);
    let wrapping_key = hkdf_wrap_key(&*hybrid_shared_secret, HYBRID_HKDF_SALT, &wrap_info)?;

    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&*wrapping_key));
    let dek_bytes = cipher
        .decrypt(
            Nonce::from_slice(&wrapped.wrap_nonce),
            aes_gcm::aead::Payload { msg: wrapped.wrapped_key.as_slice(), aad: info },
        )
        .map_err(|error| CryptoError::KeyWrapError(error.to_string()))?;
    if dek_bytes.len() != KEY_SIZE {
        return Err(CryptoError::KeyWrapError(format!(
            "Invalid unwrapped DEK length: expected {KEY_SIZE}, got {}",
            dek_bytes.len()
        )));
    }

    let mut dek = Zeroizing::new([0u8; KEY_SIZE]);
    dek.copy_from_slice(&dek_bytes);
    Ok(*dek)
}

fn hybrid_wrapped_dek_to_json(wrapped: &HybridWrappedDek) -> serde_json::Value {
    let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    serde_json::json!({
        "recipient_kid": wrapped.recipient_kid,
        "wrap_alg": wrapped.wrap_alg,
        "x25519_enc_b64u": b64.encode(wrapped.x25519_enc),
        "mlkem_ct_b64u": b64.encode(&wrapped.ml_kem_ct),
        "wrap_nonce_b64u": b64.encode(wrapped.wrap_nonce),
        "ct_b64u": b64.encode(&wrapped.wrapped_key),
    })
}

fn hybrid_wrapped_dek_from_json(
    value: &serde_json::Value,
) -> Result<HybridWrappedDek, CryptoError> {
    let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let recipient_kid = value
        .get("recipient_kid")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| CryptoError::DecryptionError("Missing recipient_kid".to_string()))?;
    let wrap_alg = value
        .get("wrap_alg")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| CryptoError::DecryptionError("Missing wrap_alg".to_string()))?;
    if wrap_alg != HYBRID_WRAP_SCHEME {
        return Err(CryptoError::DecryptionError(format!(
            "Unsupported wrap_alg: expected {HYBRID_WRAP_SCHEME}, got {wrap_alg}"
        )));
    }
    // wrap_alg validated as the expected constant; use the static reference directly
    let _ = wrap_alg;

    let x25519_enc = b64
        .decode(
            value
                .get("x25519_enc_b64u")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| {
                    CryptoError::DecryptionError("Missing x25519_enc_b64u".to_string())
                })?,
        )
        .map_err(|error| CryptoError::DecryptionError(error.to_string()))?;
    if x25519_enc.len() != 32 {
        return Err(CryptoError::DecryptionError(format!(
            "Invalid x25519_enc_b64u length: expected 32, got {}",
            x25519_enc.len()
        )));
    }
    let mut x25519_enc_arr = [0u8; 32];
    x25519_enc_arr.copy_from_slice(&x25519_enc);

    let ml_kem_ct = b64
        .decode(
            value
                .get("mlkem_ct_b64u")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| CryptoError::DecryptionError("Missing mlkem_ct_b64u".to_string()))?,
        )
        .map_err(|error| CryptoError::DecryptionError(error.to_string()))?;

    let wrap_nonce = b64
        .decode(
            value
                .get("wrap_nonce_b64u")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| {
                    CryptoError::DecryptionError("Missing wrap_nonce_b64u".to_string())
                })?,
        )
        .map_err(|error| CryptoError::DecryptionError(error.to_string()))?;
    if wrap_nonce.len() != NONCE_SIZE {
        return Err(CryptoError::DecryptionError(format!(
            "Invalid wrap_nonce_b64u length: expected {NONCE_SIZE}, got {}",
            wrap_nonce.len()
        )));
    }
    let mut wrap_nonce_arr = [0u8; NONCE_SIZE];
    wrap_nonce_arr.copy_from_slice(&wrap_nonce);

    let wrapped_key = b64
        .decode(
            value
                .get("ct_b64u")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| CryptoError::DecryptionError("Missing ct_b64u".to_string()))?,
        )
        .map_err(|error| CryptoError::DecryptionError(error.to_string()))?;

    Ok(HybridWrappedDek {
        recipient_kid: recipient_kid as u32,
        wrap_alg: Cow::Borrowed(HYBRID_WRAP_SCHEME),
        x25519_enc: x25519_enc_arr,
        ml_kem_ct,
        wrap_nonce: wrap_nonce_arr,
        wrapped_key,
    })
}

/// Encrypt a payload using AES-256-GCM and hybrid `X25519 + ML-KEM-768` recipient wrapping.
///
/// # Errors
///
/// Returns an error if canonicalization, encryption, or recipient key wrapping fails.
pub fn encrypt_payload_hybrid(
    payload: &serde_json::Value,
    aad_params: &PayloadAadParams<'_>,
    recipient_keys: &[HybridRecipientPublicKey],
) -> Result<HybridEncryptionResult, CryptoError> {
    if recipient_keys.is_empty() {
        return Err(CryptoError::NoRecipients);
    }
    validate_unique_recipient_kids(recipient_keys.iter())?;

    let mut rng = rand::thread_rng();
    let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;

    let mut salt = [0u8; SALT_SIZE];
    let mut dek = Zeroizing::new([0u8; KEY_SIZE]);
    let mut nonce_bytes = [0u8; NONCE_SIZE];
    rng.fill_bytes(&mut salt);
    rng.fill_bytes(dek.as_mut());
    rng.fill_bytes(&mut nonce_bytes);

    let payload_plain_hash = compute_payload_plain_hash(payload, Some(&salt))?;
    let updated_aad_params =
        PayloadAadParams { payload_plain_hash: &payload_plain_hash, ..*aad_params };
    let payload_aad = compute_payload_aad(&updated_aad_params)?;
    let plaintext = payload_plaintext_from_components(&salt, payload)?;

    let payload_cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&*dek));
    let ciphertext_with_tag = payload_cipher
        .encrypt(
            Nonce::from_slice(&nonce_bytes),
            aes_gcm::aead::Payload { msg: &plaintext, aad: &payload_aad },
        )
        .map_err(|error| CryptoError::EncryptionError(error.to_string()))?;
    if ciphertext_with_tag.len() < TAG_SIZE {
        return Err(CryptoError::EncryptionError(
            "AES-GCM output shorter than authentication tag".to_string(),
        ));
    }

    let ct_len = ciphertext_with_tag.len() - TAG_SIZE;
    let ciphertext = &ciphertext_with_tag[..ct_len];
    let tag = &ciphertext_with_tag[ct_len..];

    let mut recipients = Vec::with_capacity(recipient_keys.len());
    for recipient_key in recipient_keys {
        let wrapped = wrap_dek_hybrid(&dek, recipient_key, &payload_aad)?;
        recipients.push(hybrid_wrapped_dek_to_json(&wrapped));
    }
    recipients.sort_by(|left, right| {
        let left_kid =
            left.get("recipient_kid").and_then(serde_json::Value::as_u64).unwrap_or_default();
        let right_kid =
            right.get("recipient_kid").and_then(serde_json::Value::as_u64).unwrap_or_default();
        left_kid.cmp(&right_kid)
    });

    let recipients_hash = compute_recipients_hash(&recipients)?;
    let cipher_params = PayloadCipherParams {
        nonce: &nonce_bytes,
        payload_aad: &payload_aad,
        ciphertext,
        tag,
        recipients_hash: &recipients_hash,
    };
    let payload_cipher_hash = compute_payload_cipher_hash(Some(&cipher_params));

    let payload_encrypted = serde_json::json!({
        "enc_version": HYBRID_ENC_VERSION,
        "aead": PAYLOAD_AEAD_ALGORITHM,
        "nonce_b64u": b64.encode(nonce_bytes),
        "ciphertext_b64u": b64.encode(ciphertext),
        "tag_b64u": b64.encode(tag),
        "hpke": {
            "mode": "hybrid-base",
            "kem": HYBRID_KEM_SCHEME,
            "kdf": "HKDF-SHA256",
            "aead": WRAP_AEAD_ALGORITHM,
            "sig": HYBRID_SIGNATURE_SCHEME,
        },
        "recipients": recipients,
    });

    Ok(HybridEncryptionResult {
        payload_encrypted,
        salt,
        payload_plain_hash,
        payload_cipher_hash,
    })
}

/// Decrypt a payload previously encrypted by [`encrypt_payload_hybrid`].
///
/// # Errors
///
/// Returns an error if the envelope is malformed, the recipient is missing, or
/// the decrypted payload hash does not match the expected value.
pub fn decrypt_payload_hybrid(
    payload_encrypted: &serde_json::Value,
    payload_aad: &[u8; 32],
    recipient_kid: u32,
    recipient_private_key: &HybridRecipientPrivateKey,
    expected_plain_hash: &[u8; 32],
) -> Result<serde_json::Value, CryptoError> {
    let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;

    let enc_version = payload_encrypted
        .get("enc_version")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| CryptoError::DecryptionError("Missing enc_version".to_string()))?;
    if enc_version != HYBRID_ENC_VERSION {
        return Err(CryptoError::DecryptionError(format!(
            "Unsupported enc_version: expected {HYBRID_ENC_VERSION}, got {enc_version}"
        )));
    }

    let aead = payload_encrypted
        .get("aead")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| CryptoError::DecryptionError("Missing aead".to_string()))?;
    if aead != PAYLOAD_AEAD_ALGORITHM {
        return Err(CryptoError::DecryptionError(format!(
            "Unsupported aead: expected {PAYLOAD_AEAD_ALGORITHM}, got {aead}"
        )));
    }

    let hpke = payload_encrypted
        .get("hpke")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| CryptoError::DecryptionError("Missing hpke".to_string()))?;
    for (field, expected) in [
        ("mode", "hybrid-base"),
        ("kem", HYBRID_KEM_SCHEME),
        ("kdf", "HKDF-SHA256"),
        ("aead", WRAP_AEAD_ALGORITHM),
    ] {
        let value = hpke
            .get(field)
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| CryptoError::DecryptionError(format!("Missing hpke.{field}")))?;
        if value != expected {
            return Err(CryptoError::DecryptionError(format!(
                "Unsupported hpke.{field}: expected {expected}, got {value}"
            )));
        }
    }

    let recipients = payload_encrypted
        .get("recipients")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| CryptoError::DecryptionError("Missing recipients".to_string()))?;
    let wrapped = recipients
        .iter()
        .find(|recipient| {
            recipient.get("recipient_kid").and_then(serde_json::Value::as_u64)
                == Some(u64::from(recipient_kid))
        })
        .ok_or(CryptoError::RecipientNotFound(recipient_kid))
        .and_then(hybrid_wrapped_dek_from_json)?;
    let dek = unwrap_dek_hybrid(&wrapped, recipient_private_key, payload_aad)?;

    let nonce = b64
        .decode(
            payload_encrypted
                .get("nonce_b64u")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| CryptoError::DecryptionError("Missing nonce_b64u".to_string()))?,
        )
        .map_err(|error| CryptoError::DecryptionError(error.to_string()))?;
    if nonce.len() != NONCE_SIZE {
        return Err(CryptoError::DecryptionError(format!(
            "Invalid nonce length: expected {NONCE_SIZE}, got {}",
            nonce.len()
        )));
    }

    let ciphertext = b64
        .decode(
            payload_encrypted
                .get("ciphertext_b64u")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| {
                    CryptoError::DecryptionError("Missing ciphertext_b64u".to_string())
                })?,
        )
        .map_err(|error| CryptoError::DecryptionError(error.to_string()))?;
    let tag = b64
        .decode(
            payload_encrypted
                .get("tag_b64u")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| CryptoError::DecryptionError("Missing tag_b64u".to_string()))?,
        )
        .map_err(|error| CryptoError::DecryptionError(error.to_string()))?;
    if tag.len() != TAG_SIZE {
        return Err(CryptoError::DecryptionError(format!(
            "Invalid tag length: expected {TAG_SIZE}, got {}",
            tag.len()
        )));
    }

    let mut ciphertext_with_tag = Vec::with_capacity(ciphertext.len() + tag.len());
    ciphertext_with_tag.extend_from_slice(&ciphertext);
    ciphertext_with_tag.extend_from_slice(&tag);

    let payload_cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&dek));
    let plaintext = payload_cipher
        .decrypt(
            Nonce::from_slice(&nonce),
            aes_gcm::aead::Payload { msg: &ciphertext_with_tag, aad: payload_aad },
        )
        .map_err(|error| CryptoError::DecryptionError(error.to_string()))?;
    if plaintext.len() < SALT_SIZE {
        return Err(CryptoError::DecryptionError(
            "Payload plaintext shorter than salt".to_string(),
        ));
    }

    let mut salt = [0u8; SALT_SIZE];
    salt.copy_from_slice(&plaintext[..SALT_SIZE]);
    let payload_bytes = &plaintext[SALT_SIZE..];
    let payload: serde_json::Value = serde_json::from_slice(payload_bytes)
        .map_err(|error| CryptoError::DecryptionError(error.to_string()))?;

    let computed_hash = compute_payload_plain_hash(&payload, Some(&salt))?;
    if computed_hash.ct_ne(expected_plain_hash).into() {
        return Err(CryptoError::PayloadHashMismatch);
    }

    Ok(payload)
}

// ===========================================================================
// PQC-strict signing (ML-DSA-65 only)
// ===========================================================================

/// Generate an ML-DSA-65-only signing keypair for PQC-strict mode.
///
/// # Errors
///
/// Returns [`CryptoError::SignatureError`] if key generation fails.
pub fn generate_strict_signing_keypair() -> Result<StrictSigningKeypair, CryptoError> {
    let mut rng = rand::thread_rng();
    let mut seed = [0u8; 32];
    rng.fill_bytes(&mut seed);

    let signing_key = ml_dsa_signing_key_from_seed(&seed);
    let encoded = signing_key.verifying_key().encode();

    Ok(StrictSigningKeypair {
        public: StrictSigningPublicKey {
            ml_dsa_65_public_key: encoded.as_slice().to_vec(),
        },
        private: StrictSigningPrivateKey { ml_dsa_65_seed: seed },
    })
}

/// Sign a 32-byte hash with ML-DSA-65 only (PQC-strict mode).
///
/// # Errors
///
/// Returns [`CryptoError::SignatureError`] if signing fails.
pub fn strict_sign_event_hash(
    event_signing_hash: &[u8; 32],
    private_key: &StrictSigningPrivateKey,
) -> Result<Vec<u8>, CryptoError> {
    let signing_key = ml_dsa_signing_key_from_seed(&private_key.ml_dsa_65_seed);
    let signature: MlDsaSignature<MlDsa65> = signing_key
        .try_sign(event_signing_hash)
        .map_err(|error| CryptoError::SignatureError(error.to_string()))?;
    Ok(signature.encode().as_slice().to_vec())
}

/// Verify an ML-DSA-65-only event signature (PQC-strict mode).
#[must_use]
pub fn strict_verify_event_signature(
    event_signing_hash: &[u8; 32],
    signature: &[u8],
    public_key: &StrictSigningPublicKey,
) -> bool {
    let Ok(verifying_key) = ml_dsa_verifying_key_from_bytes(&public_key.ml_dsa_65_public_key)
    else {
        return false;
    };
    let Ok(sig) = ml_dsa_signature_from_bytes(signature) else {
        return false;
    };
    verifying_key.verify(event_signing_hash, &sig).is_ok()
}

// ===========================================================================
// PQC-strict KEM (ML-KEM-768 only)
// ===========================================================================

/// Generate an ML-KEM-768-only recipient keypair for PQC-strict mode.
pub fn generate_strict_recipient_keypair(kid: u32) -> Result<StrictRecipientKeypair, CryptoError> {
    let mut rng = rand::thread_rng();
    let mut seed = [0u8; 64];
    rng.fill_bytes(&mut seed);

    let dk = ml_kem_decapsulation_key_from_seed(&seed);
    let public_key = dk.encapsulation_key().to_bytes().as_slice().to_vec();

    Ok(StrictRecipientKeypair {
        public: StrictRecipientPublicKey { kid, ml_kem_768_public_key: public_key },
        private: StrictRecipientPrivateKey { ml_kem_768_seed: seed },
    })
}

fn strict_wrap_info(info: &[u8]) -> Vec<u8> {
    let mut derived = Vec::with_capacity(STRICT_WRAP_SCHEME_INFO_PREFIX.len() + info.len());
    derived.extend_from_slice(STRICT_WRAP_SCHEME_INFO_PREFIX);
    derived.extend_from_slice(info);
    derived
}

/// Wrap a DEK using ML-KEM-768 only (PQC-strict mode).
///
/// # Errors
///
/// Returns [`CryptoError::KeyWrapError`] if wrapping fails.
pub fn wrap_dek_strict(
    dek: &[u8; 32],
    recipient_public_key: &StrictRecipientPublicKey,
    info: &[u8],
) -> Result<StrictWrappedDek, CryptoError> {
    let mut rng = rand::thread_rng();

    let mut ml_kem_random = [0u8; 32];
    rng.fill_bytes(&mut ml_kem_random);
    let ek = ml_kem_encapsulation_key_from_bytes(&recipient_public_key.ml_kem_768_public_key)?;
    let (ct, shared_secret) = ek.encapsulate_deterministic(&MlKemB32::from(ml_kem_random));

    let wrap_info = strict_wrap_info(info);
    let wrapping_key = hkdf_wrap_key(shared_secret.as_slice(), STRICT_HKDF_SALT, &wrap_info)?;

    let mut wrap_nonce = [0u8; NONCE_SIZE];
    rng.fill_bytes(&mut wrap_nonce);

    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&*wrapping_key));
    let ciphertext = cipher
        .encrypt(
            Nonce::from_slice(&wrap_nonce),
            aes_gcm::aead::Payload { msg: dek.as_ref(), aad: info },
        )
        .map_err(|error| CryptoError::KeyWrapError(error.to_string()))?;

    Ok(StrictWrappedDek {
        recipient_kid: recipient_public_key.kid,
        wrap_alg: Cow::Borrowed(STRICT_WRAP_SCHEME),
        ml_kem_ct: ct.as_slice().to_vec(),
        wrap_nonce,
        wrapped_key: ciphertext,
    })
}

/// Unwrap a DEK using ML-KEM-768 only (PQC-strict mode).
///
/// # Errors
///
/// Returns [`CryptoError::KeyWrapError`] if unwrapping fails.
pub fn unwrap_dek_strict(
    wrapped: &StrictWrappedDek,
    recipient_private_key: &StrictRecipientPrivateKey,
    info: &[u8],
) -> Result<[u8; 32], CryptoError> {
    let dk = ml_kem_decapsulation_key_from_seed(&recipient_private_key.ml_kem_768_seed);
    let ct = ml_kem_ciphertext_from_bytes(&wrapped.ml_kem_ct)?;
    let shared_secret = dk.decapsulate(&ct);

    let wrap_info = strict_wrap_info(info);
    let wrapping_key = hkdf_wrap_key(shared_secret.as_slice(), STRICT_HKDF_SALT, &wrap_info)?;

    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&*wrapping_key));
    let dek_bytes = cipher
        .decrypt(
            Nonce::from_slice(&wrapped.wrap_nonce),
            aes_gcm::aead::Payload { msg: wrapped.wrapped_key.as_slice(), aad: info },
        )
        .map_err(|error| CryptoError::KeyWrapError(error.to_string()))?;

    if dek_bytes.len() != KEY_SIZE {
        return Err(CryptoError::KeyWrapError(format!(
            "Invalid unwrapped DEK length: expected {KEY_SIZE}, got {}",
            dek_bytes.len()
        )));
    }

    let mut dek = Zeroizing::new([0u8; KEY_SIZE]);
    dek.copy_from_slice(&dek_bytes);
    Ok(*dek)
}

fn strict_wrapped_dek_to_json(wrapped: &StrictWrappedDek) -> serde_json::Value {
    let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    serde_json::json!({
        "recipient_kid": wrapped.recipient_kid,
        "wrap_alg": wrapped.wrap_alg,
        "mlkem_ct_b64u": b64.encode(&wrapped.ml_kem_ct),
        "wrap_nonce_b64u": b64.encode(wrapped.wrap_nonce),
        "ct_b64u": b64.encode(&wrapped.wrapped_key),
    })
}

fn strict_wrapped_dek_from_json(value: &serde_json::Value) -> Result<StrictWrappedDek, CryptoError> {
    let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let recipient_kid = value
        .get("recipient_kid")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| CryptoError::DecryptionError("Missing recipient_kid".to_string()))?;
    let wrap_alg = value
        .get("wrap_alg")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| CryptoError::DecryptionError("Missing wrap_alg".to_string()))?;
    if wrap_alg != STRICT_WRAP_SCHEME {
        return Err(CryptoError::DecryptionError(format!(
            "Unsupported wrap_alg: expected {STRICT_WRAP_SCHEME}, got {wrap_alg}"
        )));
    }

    let ml_kem_ct = b64
        .decode(
            value
                .get("mlkem_ct_b64u")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| CryptoError::DecryptionError("Missing mlkem_ct_b64u".to_string()))?,
        )
        .map_err(|error| CryptoError::DecryptionError(error.to_string()))?;

    let wrap_nonce = b64
        .decode(
            value
                .get("wrap_nonce_b64u")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| CryptoError::DecryptionError("Missing wrap_nonce_b64u".to_string()))?,
        )
        .map_err(|error| CryptoError::DecryptionError(error.to_string()))?;
    if wrap_nonce.len() != NONCE_SIZE {
        return Err(CryptoError::DecryptionError(format!(
            "Invalid wrap_nonce length: expected {NONCE_SIZE}, got {}",
            wrap_nonce.len()
        )));
    }
    let mut wrap_nonce_arr = [0u8; NONCE_SIZE];
    wrap_nonce_arr.copy_from_slice(&wrap_nonce);

    let wrapped_key = b64
        .decode(
            value
                .get("ct_b64u")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| CryptoError::DecryptionError("Missing ct_b64u".to_string()))?,
        )
        .map_err(|error| CryptoError::DecryptionError(error.to_string()))?;

    // wrap_alg validated as the expected constant above
    let _ = wrap_alg;
    Ok(StrictWrappedDek {
        recipient_kid: recipient_kid as u32,
        wrap_alg: Cow::Borrowed(STRICT_WRAP_SCHEME),
        ml_kem_ct,
        wrap_nonce: wrap_nonce_arr,
        wrapped_key,
    })
}

// ===========================================================================
// PQC-strict payload encryption / decryption
// ===========================================================================

/// Encrypt a payload using AES-256-GCM and ML-KEM-768-only recipient wrapping.
///
/// # Errors
///
/// Returns an error if canonicalization, encryption, or wrapping fails.
pub fn encrypt_payload_strict(
    payload: &serde_json::Value,
    aad_params: &PayloadAadParams<'_>,
    recipient_keys: &[StrictRecipientPublicKey],
) -> Result<StrictEncryptionResult, CryptoError> {
    if recipient_keys.is_empty() {
        return Err(CryptoError::NoRecipients);
    }
    // Validate unique kids
    {
        let mut seen = HashSet::new();
        for r in recipient_keys {
            if !seen.insert(r.kid) {
                return Err(CryptoError::EncryptionError(format!(
                    "Duplicate recipient_kid entry: {}",
                    r.kid
                )));
            }
        }
    }

    let mut rng = rand::thread_rng();
    let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;

    let mut salt = [0u8; SALT_SIZE];
    let mut dek = Zeroizing::new([0u8; KEY_SIZE]);
    let mut nonce_bytes = [0u8; NONCE_SIZE];
    rng.fill_bytes(&mut salt);
    rng.fill_bytes(dek.as_mut());
    rng.fill_bytes(&mut nonce_bytes);

    let payload_plain_hash = compute_payload_plain_hash(payload, Some(&salt))?;
    let updated_aad_params =
        PayloadAadParams { payload_plain_hash: &payload_plain_hash, ..*aad_params };
    let payload_aad = compute_payload_aad(&updated_aad_params)?;
    let plaintext = payload_plaintext_from_components(&salt, payload)?;

    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&*dek));
    let ciphertext_with_tag = cipher
        .encrypt(
            Nonce::from_slice(&nonce_bytes),
            aes_gcm::aead::Payload { msg: &plaintext, aad: &payload_aad },
        )
        .map_err(|error| CryptoError::EncryptionError(error.to_string()))?;
    if ciphertext_with_tag.len() < TAG_SIZE {
        return Err(CryptoError::EncryptionError(
            "AES-GCM output shorter than authentication tag".to_string(),
        ));
    }

    let ct_len = ciphertext_with_tag.len() - TAG_SIZE;
    let ciphertext = &ciphertext_with_tag[..ct_len];
    let tag = &ciphertext_with_tag[ct_len..];

    let mut recipients = Vec::with_capacity(recipient_keys.len());
    for rk in recipient_keys {
        let wrapped = wrap_dek_strict(&dek, rk, &payload_aad)?;
        recipients.push(strict_wrapped_dek_to_json(&wrapped));
    }
    recipients.sort_by(|a, b| {
        let ak = a.get("recipient_kid").and_then(serde_json::Value::as_u64).unwrap_or_default();
        let bk = b.get("recipient_kid").and_then(serde_json::Value::as_u64).unwrap_or_default();
        ak.cmp(&bk)
    });

    let recipients_hash = compute_recipients_hash(&recipients)?;
    let cipher_params = PayloadCipherParams {
        nonce: &nonce_bytes,
        payload_aad: &payload_aad,
        ciphertext,
        tag,
        recipients_hash: &recipients_hash,
    };
    let payload_cipher_hash = compute_payload_cipher_hash(Some(&cipher_params));

    let payload_encrypted = serde_json::json!({
        "enc_version": STRICT_ENC_VERSION,
        "aead": PAYLOAD_AEAD_ALGORITHM,
        "nonce_b64u": b64.encode(nonce_bytes),
        "ciphertext_b64u": b64.encode(ciphertext),
        "tag_b64u": b64.encode(tag),
        "hpke": {
            "mode": "pqc-base",
            "kem": STRICT_KEM_SCHEME,
            "kdf": "HKDF-SHA256",
            "aead": WRAP_AEAD_ALGORITHM,
        },
        "recipients": recipients,
    });

    Ok(StrictEncryptionResult {
        payload_encrypted,
        salt,
        payload_plain_hash,
        payload_cipher_hash,
    })
}

/// Decrypt a payload previously encrypted by [`encrypt_payload_strict`].
///
/// # Errors
///
/// Returns an error if the envelope is malformed, the recipient is missing, or
/// the decrypted payload hash does not match.
pub fn decrypt_payload_strict(
    payload_encrypted: &serde_json::Value,
    payload_aad: &[u8; 32],
    recipient_kid: u32,
    recipient_private_key: &StrictRecipientPrivateKey,
    expected_plain_hash: &[u8; 32],
) -> Result<serde_json::Value, CryptoError> {
    let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;

    let enc_version = payload_encrypted
        .get("enc_version")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| CryptoError::DecryptionError("Missing enc_version".to_string()))?;
    if enc_version != STRICT_ENC_VERSION {
        return Err(CryptoError::DecryptionError(format!(
            "Unsupported enc_version: expected {STRICT_ENC_VERSION}, got {enc_version}"
        )));
    }

    let aead = payload_encrypted
        .get("aead")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| CryptoError::DecryptionError("Missing aead".to_string()))?;
    if aead != PAYLOAD_AEAD_ALGORITHM {
        return Err(CryptoError::DecryptionError(format!(
            "Unsupported aead: expected {PAYLOAD_AEAD_ALGORITHM}, got {aead}"
        )));
    }

    let hpke = payload_encrypted
        .get("hpke")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| CryptoError::DecryptionError("Missing hpke".to_string()))?;
    for (field, expected) in [
        ("mode", "pqc-base"),
        ("kem", STRICT_KEM_SCHEME),
        ("kdf", "HKDF-SHA256"),
        ("aead", WRAP_AEAD_ALGORITHM),
    ] {
        let value = hpke
            .get(field)
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| CryptoError::DecryptionError(format!("Missing hpke.{field}")))?;
        if value != expected {
            return Err(CryptoError::DecryptionError(format!(
                "Unsupported hpke.{field}: expected {expected}, got {value}"
            )));
        }
    }

    let recipients = payload_encrypted
        .get("recipients")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| CryptoError::DecryptionError("Missing recipients".to_string()))?;
    let wrapped = recipients
        .iter()
        .find(|r| {
            r.get("recipient_kid").and_then(serde_json::Value::as_u64) == Some(u64::from(recipient_kid))
        })
        .ok_or(CryptoError::RecipientNotFound(recipient_kid))
        .and_then(strict_wrapped_dek_from_json)?;
    let dek = unwrap_dek_strict(&wrapped, recipient_private_key, payload_aad)?;

    let nonce = b64
        .decode(
            payload_encrypted
                .get("nonce_b64u")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| CryptoError::DecryptionError("Missing nonce_b64u".to_string()))?,
        )
        .map_err(|error| CryptoError::DecryptionError(error.to_string()))?;
    if nonce.len() != NONCE_SIZE {
        return Err(CryptoError::DecryptionError(format!(
            "Invalid nonce length: expected {NONCE_SIZE}, got {}",
            nonce.len()
        )));
    }

    let ciphertext = b64
        .decode(
            payload_encrypted
                .get("ciphertext_b64u")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| CryptoError::DecryptionError("Missing ciphertext_b64u".to_string()))?,
        )
        .map_err(|error| CryptoError::DecryptionError(error.to_string()))?;
    let tag = b64
        .decode(
            payload_encrypted
                .get("tag_b64u")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| CryptoError::DecryptionError("Missing tag_b64u".to_string()))?,
        )
        .map_err(|error| CryptoError::DecryptionError(error.to_string()))?;
    if tag.len() != TAG_SIZE {
        return Err(CryptoError::DecryptionError(format!(
            "Invalid tag length: expected {TAG_SIZE}, got {}",
            tag.len()
        )));
    }

    let mut ciphertext_with_tag = Vec::with_capacity(ciphertext.len() + tag.len());
    ciphertext_with_tag.extend_from_slice(&ciphertext);
    ciphertext_with_tag.extend_from_slice(&tag);

    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&dek));
    let plain = cipher
        .decrypt(
            Nonce::from_slice(&nonce),
            aes_gcm::aead::Payload { msg: &ciphertext_with_tag, aad: payload_aad },
        )
        .map_err(|error| CryptoError::DecryptionError(error.to_string()))?;
    if plain.len() < SALT_SIZE {
        return Err(CryptoError::DecryptionError(
            "Payload plaintext shorter than salt".to_string(),
        ));
    }

    let mut salt = [0u8; SALT_SIZE];
    salt.copy_from_slice(&plain[..SALT_SIZE]);
    let payload: serde_json::Value = serde_json::from_slice(&plain[SALT_SIZE..])
        .map_err(|error| CryptoError::DecryptionError(error.to_string()))?;

    let computed = compute_payload_plain_hash(&payload, Some(&salt))?;
    if computed.ct_ne(expected_plain_hash).into() {
        return Err(CryptoError::PayloadHashMismatch);
    }
    Ok(payload)
}

// ===========================================================================
// Proof-of-possession (PoP)
// ===========================================================================

fn pop_challenge(public_key_bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(POP_DOMAIN);
    hasher.update(public_key_bytes);
    hasher.finalize().into()
}

/// Generate a proof-of-possession bundle for a hybrid signing keypair.
///
/// The proof-of-possession proves the registrant holds both the Ed25519 and
/// ML-DSA-65 private keys by signing
/// `SHA-256("VES_POP_V1" || ed25519_pk || ml_dsa_65_pk)`.
///
/// # Errors
///
/// Returns [`CryptoError::SignatureError`] if signing fails.
pub fn generate_hybrid_signing_pop(
    keypair: &HybridSigningKeypair,
) -> Result<HybridSignatureBundle, CryptoError> {
    let mut pk_bytes = Vec::with_capacity(32 + keypair.public.ml_dsa_65_public_key.len());
    pk_bytes.extend_from_slice(&keypair.public.ed25519_public_key);
    pk_bytes.extend_from_slice(&keypair.public.ml_dsa_65_public_key);
    let challenge = pop_challenge(&pk_bytes);
    hybrid_sign_event_hash(&challenge, &keypair.private)
}

/// Verify a hybrid signing proof-of-possession bundle.
#[must_use]
pub fn verify_hybrid_signing_pop(
    pop: &HybridSignatureBundle,
    public_key: &HybridSigningPublicKey,
) -> bool {
    let mut pk_bytes = Vec::with_capacity(32 + public_key.ml_dsa_65_public_key.len());
    pk_bytes.extend_from_slice(&public_key.ed25519_public_key);
    pk_bytes.extend_from_slice(&public_key.ml_dsa_65_public_key);
    let challenge = pop_challenge(&pk_bytes);
    hybrid_verify_event_signature(&challenge, pop, public_key)
}

/// Generate a proof-of-possession for a PQC-strict signing keypair.
///
/// # Errors
///
/// Returns [`CryptoError::SignatureError`] if signing fails.
pub fn generate_strict_signing_pop(
    keypair: &StrictSigningKeypair,
) -> Result<Vec<u8>, CryptoError> {
    let challenge = pop_challenge(&keypair.public.ml_dsa_65_public_key);
    strict_sign_event_hash(&challenge, &keypair.private)
}

/// Verify a PQC-strict signing proof-of-possession.
#[must_use]
pub fn verify_strict_signing_pop(pop: &[u8], public_key: &StrictSigningPublicKey) -> bool {
    let challenge = pop_challenge(&public_key.ml_dsa_65_public_key);
    strict_verify_event_signature(&challenge, pop, public_key)
}

// ===========================================================================
// Receipt signing (delegates to event signing — same algorithm, different role)
// ===========================================================================

/// Sign a receipt hash with hybrid `Ed25519 + ML-DSA-65`.
///
/// Semantically identical to [`hybrid_sign_event_hash`] but named for receipt
/// signing context per VES-RECEIPT-2.
///
/// # Errors
///
/// Returns [`CryptoError::SignatureError`] if signing fails.
pub fn hybrid_sign_receipt_hash(
    receipt_hash: &[u8; 32],
    private_key: &HybridSigningPrivateKey,
) -> Result<HybridSignatureBundle, CryptoError> {
    hybrid_sign_event_hash(receipt_hash, private_key)
}

/// Verify a hybrid receipt signature per VES-RECEIPT-2.
#[must_use]
pub fn hybrid_verify_receipt_signature(
    receipt_hash: &[u8; 32],
    signature: &HybridSignatureBundle,
    public_key: &HybridSigningPublicKey,
) -> bool {
    hybrid_verify_event_signature(receipt_hash, signature, public_key)
}

/// Sign a receipt hash with ML-DSA-65 only (PQC-strict).
///
/// # Errors
///
/// Returns [`CryptoError::SignatureError`] if signing fails.
pub fn strict_sign_receipt_hash(
    receipt_hash: &[u8; 32],
    private_key: &StrictSigningPrivateKey,
) -> Result<Vec<u8>, CryptoError> {
    strict_sign_event_hash(receipt_hash, private_key)
}

/// Verify a PQC-strict receipt signature.
#[must_use]
pub fn strict_verify_receipt_signature(
    receipt_hash: &[u8; 32],
    signature: &[u8],
    public_key: &StrictSigningPublicKey,
) -> bool {
    strict_verify_event_signature(receipt_hash, signature, public_key)
}

// ===========================================================================
// Cross-language test vectors
// ===========================================================================

/// Known-answer seed for cross-language signing test vectors.
pub const TEST_VECTOR_SIGNING_SEED: [u8; 32] = [
    0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
    0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10,
    0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18,
    0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f, 0x20,
];

/// Known-answer message hash for cross-language test vectors.
pub const TEST_VECTOR_MESSAGE_HASH: [u8; 32] = [0x42; 32];

/// Derive the ML-DSA-65 public key from a known seed for test vectors.
///
/// Used in cross-language interop tests to verify both Rust and Node produce
/// identical public keys from the same seed.
#[must_use]
pub fn test_vector_ml_dsa_public_key(seed: &[u8; 32]) -> Vec<u8> {
    let sk = ml_dsa_signing_key_from_seed(seed);
    sk.verifying_key().encode().as_slice().to_vec()
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
    fn hybrid_signature_roundtrip() {
        let keypair = generate_hybrid_signing_keypair().unwrap();
        let event_hash = [42u8; 32];

        let signature = hybrid_sign_event_hash(&event_hash, &keypair.private).unwrap();
        assert!(hybrid_verify_event_signature(&event_hash, &signature, &keypair.public));
    }

    #[test]
    fn hybrid_signature_wrong_public_key_fails() {
        let signer = generate_hybrid_signing_keypair().unwrap();
        let verifier = generate_hybrid_signing_keypair().unwrap();
        let event_hash = [42u8; 32];

        let signature = hybrid_sign_event_hash(&event_hash, &signer.private).unwrap();
        assert!(!hybrid_verify_event_signature(&event_hash, &signature, &verifier.public));
    }

    #[test]
    fn hybrid_dek_wrap_unwrap_roundtrip() {
        let recipient = generate_hybrid_recipient_keypair(7).unwrap();
        let dek = [7u8; 32];
        let info = b"ves-test-wrap";

        let wrapped = wrap_dek_hybrid(&dek, &recipient.public, info).unwrap();
        let recovered = unwrap_dek_hybrid(&wrapped, &recipient.private, info).unwrap();
        assert_eq!(recovered, dek);
    }

    #[test]
    fn hybrid_encrypt_decrypt_roundtrip() {
        let recipient = generate_hybrid_recipient_keypair(1).unwrap();
        let payload = json!({
            "order_id": "ORD-100",
            "total": 1250,
            "currency": "USD"
        });

        let provisional_plain_hash = compute_payload_plain_hash(&payload, None).unwrap();
        let aad_params = test_aad_params(&provisional_plain_hash);
        let encrypted =
            encrypt_payload_hybrid(&payload, &aad_params, &[recipient.public.clone()]).unwrap();

        let decrypt_aad =
            PayloadAadParams { payload_plain_hash: &encrypted.payload_plain_hash, ..aad_params };
        let payload_aad = compute_payload_aad(&decrypt_aad).unwrap();
        let decrypted = decrypt_payload_hybrid(
            &encrypted.payload_encrypted,
            &payload_aad,
            recipient.public.kid,
            &recipient.private,
            &encrypted.payload_plain_hash,
        )
        .unwrap();

        assert_eq!(decrypted, payload);
    }

    // -----------------------------------------------------------------------
    // Serde roundtrip tests
    // -----------------------------------------------------------------------

    #[test]
    fn serde_roundtrip_hybrid_signing_keypair() {
        let keypair = generate_hybrid_signing_keypair().unwrap();
        let json = serde_json::to_string(&keypair).unwrap();
        let deserialized: HybridSigningKeypair = serde_json::from_str(&json).unwrap();
        assert_eq!(keypair, deserialized);
    }

    #[test]
    fn serde_roundtrip_hybrid_signature_bundle() {
        let keypair = generate_hybrid_signing_keypair().unwrap();
        let sig = hybrid_sign_event_hash(&[1u8; 32], &keypair.private).unwrap();
        let json = serde_json::to_string(&sig).unwrap();
        let deserialized: HybridSignatureBundle = serde_json::from_str(&json).unwrap();
        assert_eq!(sig, deserialized);
    }

    #[test]
    fn serde_roundtrip_hybrid_recipient_keypair() {
        let kp = generate_hybrid_recipient_keypair(42).unwrap();
        let json = serde_json::to_string(&kp).unwrap();
        let deserialized: HybridRecipientKeypair = serde_json::from_str(&json).unwrap();
        assert_eq!(kp, deserialized);
    }

    #[test]
    fn serde_roundtrip_hybrid_wrapped_dek() {
        let recipient = generate_hybrid_recipient_keypair(1).unwrap();
        let dek = [9u8; 32];
        let wrapped = wrap_dek_hybrid(&dek, &recipient.public, b"test").unwrap();
        let json = serde_json::to_string(&wrapped).unwrap();
        let deserialized: HybridWrappedDek = serde_json::from_str(&json).unwrap();
        assert_eq!(wrapped, deserialized);
    }

    #[test]
    fn serde_roundtrip_security_profile() {
        for profile in [SecurityProfile::Legacy, SecurityProfile::Hybrid, SecurityProfile::PqcStrict] {
            let json = serde_json::to_string(&profile).unwrap();
            let deserialized: SecurityProfile = serde_json::from_str(&json).unwrap();
            assert_eq!(profile, deserialized);
        }
        // Verify kebab-case serialization
        assert_eq!(serde_json::to_string(&SecurityProfile::PqcStrict).unwrap(), "\"pqc-strict\"");
        assert_eq!(serde_json::to_string(&SecurityProfile::Legacy).unwrap(), "\"legacy\"");
        assert_eq!(serde_json::to_string(&SecurityProfile::Hybrid).unwrap(), "\"hybrid\"");
    }

    #[test]
    fn serde_roundtrip_strict_signing_keypair() {
        let kp = generate_strict_signing_keypair().unwrap();
        let json = serde_json::to_string(&kp).unwrap();
        let deserialized: StrictSigningKeypair = serde_json::from_str(&json).unwrap();
        assert_eq!(kp, deserialized);
    }

    #[test]
    fn serde_roundtrip_strict_recipient_keypair() {
        let kp = generate_strict_recipient_keypair(99).unwrap();
        let json = serde_json::to_string(&kp).unwrap();
        let deserialized: StrictRecipientKeypair = serde_json::from_str(&json).unwrap();
        assert_eq!(kp, deserialized);
    }

    // -----------------------------------------------------------------------
    // PQC-strict signing tests
    // -----------------------------------------------------------------------

    #[test]
    fn strict_signature_roundtrip() {
        let keypair = generate_strict_signing_keypair().unwrap();
        let hash = [77u8; 32];
        let sig = strict_sign_event_hash(&hash, &keypair.private).unwrap();
        assert!(strict_verify_event_signature(&hash, &sig, &keypair.public));
    }

    #[test]
    fn strict_signature_wrong_key_fails() {
        let signer = generate_strict_signing_keypair().unwrap();
        let verifier = generate_strict_signing_keypair().unwrap();
        let hash = [77u8; 32];
        let sig = strict_sign_event_hash(&hash, &signer.private).unwrap();
        assert!(!strict_verify_event_signature(&hash, &sig, &verifier.public));
    }

    #[test]
    fn strict_signature_wrong_hash_fails() {
        let keypair = generate_strict_signing_keypair().unwrap();
        let sig = strict_sign_event_hash(&[1u8; 32], &keypair.private).unwrap();
        assert!(!strict_verify_event_signature(&[2u8; 32], &sig, &keypair.public));
    }

    #[test]
    fn strict_signature_truncated_fails() {
        let keypair = generate_strict_signing_keypair().unwrap();
        let hash = [55u8; 32];
        let sig = strict_sign_event_hash(&hash, &keypair.private).unwrap();
        assert!(!strict_verify_event_signature(&hash, &sig[..sig.len() - 1], &keypair.public));
    }

    #[test]
    fn strict_signature_empty_fails() {
        let keypair = generate_strict_signing_keypair().unwrap();
        assert!(!strict_verify_event_signature(&[0u8; 32], &[], &keypair.public));
    }

    // -----------------------------------------------------------------------
    // PQC-strict KEM tests
    // -----------------------------------------------------------------------

    #[test]
    fn strict_dek_wrap_unwrap_roundtrip() {
        let recipient = generate_strict_recipient_keypair(5).unwrap();
        let dek = [0xAB; 32];
        let info = b"strict-test";
        let wrapped = wrap_dek_strict(&dek, &recipient.public, info).unwrap();
        let recovered = unwrap_dek_strict(&wrapped, &recipient.private, info).unwrap();
        assert_eq!(recovered, dek);
    }

    #[test]
    fn strict_dek_wrong_key_fails() {
        let sender_rk = generate_strict_recipient_keypair(1).unwrap();
        let wrong_rk = generate_strict_recipient_keypair(2).unwrap();
        let dek = [0xCD; 32];
        let wrapped = wrap_dek_strict(&dek, &sender_rk.public, b"test").unwrap();
        assert!(unwrap_dek_strict(&wrapped, &wrong_rk.private, b"test").is_err());
    }

    #[test]
    fn strict_dek_wrong_info_fails() {
        let rk = generate_strict_recipient_keypair(1).unwrap();
        let dek = [0xEF; 32];
        let wrapped = wrap_dek_strict(&dek, &rk.public, b"info-a").unwrap();
        assert!(unwrap_dek_strict(&wrapped, &rk.private, b"info-b").is_err());
    }

    // -----------------------------------------------------------------------
    // PQC-strict encrypt/decrypt tests
    // -----------------------------------------------------------------------

    #[test]
    fn strict_encrypt_decrypt_roundtrip() {
        let recipient = generate_strict_recipient_keypair(1).unwrap();
        let payload = json!({"item": "widget", "qty": 3});

        let provisional = compute_payload_plain_hash(&payload, None).unwrap();
        let aad = test_aad_params(&provisional);
        let encrypted =
            encrypt_payload_strict(&payload, &aad, &[recipient.public.clone()]).unwrap();

        let dec_aad = PayloadAadParams {
            payload_plain_hash: &encrypted.payload_plain_hash,
            ..aad
        };
        let payload_aad = compute_payload_aad(&dec_aad).unwrap();
        let decrypted = decrypt_payload_strict(
            &encrypted.payload_encrypted,
            &payload_aad,
            1,
            &recipient.private,
            &encrypted.payload_plain_hash,
        )
        .unwrap();
        assert_eq!(decrypted, payload);
    }

    #[test]
    fn strict_encrypt_multi_recipient() {
        let r1 = generate_strict_recipient_keypair(1).unwrap();
        let r2 = generate_strict_recipient_keypair(2).unwrap();
        let payload = json!({"multi": true});

        let provisional = compute_payload_plain_hash(&payload, None).unwrap();
        let aad = test_aad_params(&provisional);
        let encrypted = encrypt_payload_strict(
            &payload,
            &aad,
            &[r1.public.clone(), r2.public.clone()],
        )
        .unwrap();

        for (kid, private) in [(1, &r1.private), (2, &r2.private)] {
            let dec_aad = PayloadAadParams {
                payload_plain_hash: &encrypted.payload_plain_hash,
                ..aad
            };
            let payload_aad = compute_payload_aad(&dec_aad).unwrap();
            let decrypted = decrypt_payload_strict(
                &encrypted.payload_encrypted,
                &payload_aad,
                kid,
                private,
                &encrypted.payload_plain_hash,
            )
            .unwrap();
            assert_eq!(decrypted, payload);
        }
    }

    #[test]
    fn strict_encrypt_no_recipients_fails() {
        let payload = json!({"x": 1});
        let provisional = compute_payload_plain_hash(&payload, None).unwrap();
        let aad = test_aad_params(&provisional);
        assert!(encrypt_payload_strict(&payload, &aad, &[]).is_err());
    }

    #[test]
    fn strict_encrypt_duplicate_kids_fails() {
        let r1 = generate_strict_recipient_keypair(1).unwrap();
        let r2_dup = StrictRecipientPublicKey {
            kid: 1,
            ml_kem_768_public_key: r1.public.ml_kem_768_public_key.clone(),
        };
        let payload = json!({"dup": true});
        let provisional = compute_payload_plain_hash(&payload, None).unwrap();
        let aad = test_aad_params(&provisional);
        assert!(encrypt_payload_strict(&payload, &aad, &[r1.public, r2_dup]).is_err());
    }

    #[test]
    fn strict_decrypt_wrong_recipient_fails() {
        let r1 = generate_strict_recipient_keypair(1).unwrap();
        let payload = json!({"secret": "data"});
        let provisional = compute_payload_plain_hash(&payload, None).unwrap();
        let aad = test_aad_params(&provisional);
        let encrypted =
            encrypt_payload_strict(&payload, &aad, &[r1.public.clone()]).unwrap();

        let dec_aad = PayloadAadParams {
            payload_plain_hash: &encrypted.payload_plain_hash,
            ..aad
        };
        let payload_aad = compute_payload_aad(&dec_aad).unwrap();
        // Ask for non-existent kid
        let result = decrypt_payload_strict(
            &encrypted.payload_encrypted,
            &payload_aad,
            99,
            &r1.private,
            &encrypted.payload_plain_hash,
        );
        assert!(matches!(result, Err(CryptoError::RecipientNotFound(99))));
    }

    // -----------------------------------------------------------------------
    // Proof-of-possession tests
    // -----------------------------------------------------------------------

    #[test]
    fn hybrid_pop_roundtrip() {
        let keypair = generate_hybrid_signing_keypair().unwrap();
        let pop = generate_hybrid_signing_pop(&keypair).unwrap();
        assert!(verify_hybrid_signing_pop(&pop, &keypair.public));
    }

    #[test]
    fn hybrid_pop_wrong_key_fails() {
        let signer = generate_hybrid_signing_keypair().unwrap();
        let other = generate_hybrid_signing_keypair().unwrap();
        let pop = generate_hybrid_signing_pop(&signer).unwrap();
        assert!(!verify_hybrid_signing_pop(&pop, &other.public));
    }

    #[test]
    fn strict_pop_roundtrip() {
        let keypair = generate_strict_signing_keypair().unwrap();
        let pop = generate_strict_signing_pop(&keypair).unwrap();
        assert!(verify_strict_signing_pop(&pop, &keypair.public));
    }

    #[test]
    fn strict_pop_wrong_key_fails() {
        let signer = generate_strict_signing_keypair().unwrap();
        let other = generate_strict_signing_keypair().unwrap();
        let pop = generate_strict_signing_pop(&signer).unwrap();
        assert!(!verify_strict_signing_pop(&pop, &other.public));
    }

    // -----------------------------------------------------------------------
    // Receipt signing tests
    // -----------------------------------------------------------------------

    #[test]
    fn hybrid_receipt_sign_verify() {
        let keypair = generate_hybrid_signing_keypair().unwrap();
        let receipt_hash = [0xBB; 32];
        let sig = hybrid_sign_receipt_hash(&receipt_hash, &keypair.private).unwrap();
        assert!(hybrid_verify_receipt_signature(&receipt_hash, &sig, &keypair.public));
    }

    #[test]
    fn strict_receipt_sign_verify() {
        let keypair = generate_strict_signing_keypair().unwrap();
        let receipt_hash = [0xCC; 32];
        let sig = strict_sign_receipt_hash(&receipt_hash, &keypair.private).unwrap();
        assert!(strict_verify_receipt_signature(&receipt_hash, &sig, &keypair.public));
    }

    // -----------------------------------------------------------------------
    // Hybrid negative tests
    // -----------------------------------------------------------------------

    #[test]
    fn hybrid_signature_tampered_ed25519_fails() {
        let keypair = generate_hybrid_signing_keypair().unwrap();
        let hash = [42u8; 32];
        let mut sig = hybrid_sign_event_hash(&hash, &keypair.private).unwrap();
        sig.ed25519_signature[0] ^= 0xFF;
        assert!(!hybrid_verify_event_signature(&hash, &sig, &keypair.public));
    }

    #[test]
    fn hybrid_signature_tampered_mldsa_fails() {
        let keypair = generate_hybrid_signing_keypair().unwrap();
        let hash = [42u8; 32];
        let mut sig = hybrid_sign_event_hash(&hash, &keypair.private).unwrap();
        if let Some(byte) = sig.ml_dsa_65_signature.first_mut() {
            *byte ^= 0xFF;
        }
        assert!(!hybrid_verify_event_signature(&hash, &sig, &keypair.public));
    }

    #[test]
    fn hybrid_verify_empty_mldsa_signature_fails() {
        let keypair = generate_hybrid_signing_keypair().unwrap();
        let hash = [42u8; 32];
        let mut sig = hybrid_sign_event_hash(&hash, &keypair.private).unwrap();
        sig.ml_dsa_65_signature.clear();
        assert!(!hybrid_verify_event_signature(&hash, &sig, &keypair.public));
    }

    #[test]
    fn hybrid_verify_empty_mldsa_public_key_fails() {
        let keypair = generate_hybrid_signing_keypair().unwrap();
        let hash = [42u8; 32];
        let sig = hybrid_sign_event_hash(&hash, &keypair.private).unwrap();
        let bad_pk = HybridSigningPublicKey {
            ed25519_public_key: keypair.public.ed25519_public_key,
            ml_dsa_65_public_key: vec![],
        };
        assert!(!hybrid_verify_event_signature(&hash, &sig, &bad_pk));
    }

    #[test]
    fn hybrid_dek_wrong_recipient_fails() {
        let r1 = generate_hybrid_recipient_keypair(1).unwrap();
        let r2 = generate_hybrid_recipient_keypair(2).unwrap();
        let dek = [0xAA; 32];
        let wrapped = wrap_dek_hybrid(&dek, &r1.public, b"info").unwrap();
        assert!(unwrap_dek_hybrid(&wrapped, &r2.private, b"info").is_err());
    }

    #[test]
    fn hybrid_dek_wrong_info_fails() {
        let r = generate_hybrid_recipient_keypair(1).unwrap();
        let dek = [0xBB; 32];
        let wrapped = wrap_dek_hybrid(&dek, &r.public, b"info-a").unwrap();
        assert!(unwrap_dek_hybrid(&wrapped, &r.private, b"info-b").is_err());
    }

    #[test]
    fn hybrid_encrypt_multi_recipient_roundtrip() {
        let r1 = generate_hybrid_recipient_keypair(1).unwrap();
        let r2 = generate_hybrid_recipient_keypair(2).unwrap();
        let payload = json!({"multi": true});

        let provisional = compute_payload_plain_hash(&payload, None).unwrap();
        let aad = test_aad_params(&provisional);
        let encrypted = encrypt_payload_hybrid(
            &payload,
            &aad,
            &[r1.public.clone(), r2.public.clone()],
        )
        .unwrap();

        for (kid, private) in [(1, &r1.private), (2, &r2.private)] {
            let dec_aad = PayloadAadParams {
                payload_plain_hash: &encrypted.payload_plain_hash,
                ..aad
            };
            let paad = compute_payload_aad(&dec_aad).unwrap();
            let decrypted = decrypt_payload_hybrid(
                &encrypted.payload_encrypted,
                &paad,
                kid,
                private,
                &encrypted.payload_plain_hash,
            )
            .unwrap();
            assert_eq!(decrypted, payload);
        }
    }

    #[test]
    fn hybrid_encrypt_no_recipients_fails() {
        let payload = json!({"x": 1});
        let provisional = compute_payload_plain_hash(&payload, None).unwrap();
        let aad = test_aad_params(&provisional);
        assert!(encrypt_payload_hybrid(&payload, &aad, &[]).is_err());
    }

    #[test]
    fn hybrid_encrypt_duplicate_kids_fails() {
        let r1 = generate_hybrid_recipient_keypair(1).unwrap();
        let r2_dup = HybridRecipientPublicKey {
            kid: 1,
            x25519_public_key: r1.public.x25519_public_key,
            ml_kem_768_public_key: r1.public.ml_kem_768_public_key.clone(),
        };
        let payload = json!({"dup": true});
        let provisional = compute_payload_plain_hash(&payload, None).unwrap();
        let aad = test_aad_params(&provisional);
        assert!(encrypt_payload_hybrid(&payload, &aad, &[r1.public, r2_dup]).is_err());
    }

    // -----------------------------------------------------------------------
    // Cross-language test vector tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_vector_ml_dsa_public_key_deterministic() {
        let pk1 = test_vector_ml_dsa_public_key(&TEST_VECTOR_SIGNING_SEED);
        let pk2 = test_vector_ml_dsa_public_key(&TEST_VECTOR_SIGNING_SEED);
        assert_eq!(pk1, pk2);
        assert!(!pk1.is_empty());
    }

    #[test]
    fn test_vector_strict_sign_deterministic_public_key() {
        let kp = StrictSigningKeypair {
            public: StrictSigningPublicKey {
                ml_dsa_65_public_key: test_vector_ml_dsa_public_key(&TEST_VECTOR_SIGNING_SEED),
            },
            private: StrictSigningPrivateKey {
                ml_dsa_65_seed: TEST_VECTOR_SIGNING_SEED,
            },
        };
        let sig = strict_sign_event_hash(&TEST_VECTOR_MESSAGE_HASH, &kp.private).unwrap();
        assert!(strict_verify_event_signature(&TEST_VECTOR_MESSAGE_HASH, &sig, &kp.public));
    }

    // -----------------------------------------------------------------------
    // Prepared signer/verifier tests
    // -----------------------------------------------------------------------

    #[test]
    fn prepared_hybrid_signer_roundtrip() {
        let keypair = generate_hybrid_signing_keypair().unwrap();
        let signer = PreparedHybridSigner::new(&keypair.private);
        let hash = [0xAA; 32];
        let sig = signer.sign(&hash).unwrap();
        assert!(hybrid_verify_event_signature(&hash, &sig, &keypair.public));
    }

    #[test]
    fn prepared_hybrid_signer_multiple_messages() {
        let keypair = generate_hybrid_signing_keypair().unwrap();
        let signer = PreparedHybridSigner::new(&keypair.private);
        for i in 0..5u8 {
            let hash = [i; 32];
            let sig = signer.sign(&hash).unwrap();
            assert!(hybrid_verify_event_signature(&hash, &sig, &keypair.public));
        }
    }

    #[test]
    fn prepared_hybrid_verifier_roundtrip() {
        let keypair = generate_hybrid_signing_keypair().unwrap();
        let verifier = PreparedHybridVerifier::new(&keypair.public).unwrap();
        let hash = [0xBB; 32];
        let sig = hybrid_sign_event_hash(&hash, &keypair.private).unwrap();
        assert!(verifier.verify(&hash, &sig));
    }

    #[test]
    fn prepared_hybrid_verifier_rejects_wrong_sig() {
        let kp1 = generate_hybrid_signing_keypair().unwrap();
        let kp2 = generate_hybrid_signing_keypair().unwrap();
        let verifier = PreparedHybridVerifier::new(&kp1.public).unwrap();
        let hash = [0xCC; 32];
        let sig = hybrid_sign_event_hash(&hash, &kp2.private).unwrap();
        assert!(!verifier.verify(&hash, &sig));
    }

    #[test]
    fn prepared_strict_signer_roundtrip() {
        let keypair = generate_strict_signing_keypair().unwrap();
        let signer = PreparedStrictSigner::new(&keypair.private);
        let hash = [0xDD; 32];
        let sig = signer.sign(&hash).unwrap();
        assert!(strict_verify_event_signature(&hash, &sig, &keypair.public));
    }

    #[test]
    fn prepared_strict_signer_multiple_messages() {
        let keypair = generate_strict_signing_keypair().unwrap();
        let signer = PreparedStrictSigner::new(&keypair.private);
        for i in 0..5u8 {
            let hash = [i; 32];
            let sig = signer.sign(&hash).unwrap();
            assert!(strict_verify_event_signature(&hash, &sig, &keypair.public));
        }
    }
}
