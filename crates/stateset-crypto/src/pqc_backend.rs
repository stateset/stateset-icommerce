use ml_dsa::signature::{Keypair, Signer, Verifier};
use ml_dsa::{
    EncodedVerifyingKey as MlDsaEncodedVerifyingKey, KeyGen, MlDsa65, Signature as MlDsaSignature,
    SigningKey as InnerMlDsaSigningKey, VerifyingKey as InnerMlDsaVerifyingKey,
};
use ml_kem::kem::{Decapsulate, KeyExport, TryKeyInit};
use ml_kem::{
    B32 as MlKemB32, DecapsulationKey768 as InnerMlKemDecapsulationKey768,
    EncapsulationKey768 as InnerMlKemEncapsulationKey768, Seed as MlKemSeed,
    ml_kem_768::Ciphertext as InnerMlKemCiphertext768,
};

use crate::CryptoError;

pub(crate) struct MlDsa65SigningKey(InnerMlDsaSigningKey<MlDsa65>);

impl MlDsa65SigningKey {
    pub(crate) fn from_seed(seed: &[u8; 32]) -> Self {
        Self(<MlDsa65 as KeyGen>::from_seed(&(*seed).into()))
    }

    pub(crate) fn verifying_key_bytes(&self) -> Vec<u8> {
        self.0.verifying_key().encode().as_slice().to_vec()
    }

    pub(crate) fn sign(&self, message: &[u8; 32]) -> Result<Vec<u8>, CryptoError> {
        let signature: MlDsaSignature<MlDsa65> = self
            .0
            .try_sign(message)
            .map_err(|error| CryptoError::SignatureError(error.to_string()))?;
        Ok(signature.encode().as_slice().to_vec())
    }
}

pub(crate) struct MlDsa65VerifyingKey(InnerMlDsaVerifyingKey<MlDsa65>);

impl MlDsa65VerifyingKey {
    pub(crate) fn from_bytes(public_key: &[u8]) -> Result<Self, CryptoError> {
        let encoded = MlDsaEncodedVerifyingKey::<MlDsa65>::try_from(public_key)
            .map_err(|_| CryptoError::SignatureError("Invalid ML-DSA-65 public key".to_string()))?;
        Ok(Self(InnerMlDsaVerifyingKey::decode(&encoded)))
    }

    pub(crate) fn verify(&self, message: &[u8; 32], signature: &[u8]) -> Result<(), CryptoError> {
        let signature = MlDsaSignature::<MlDsa65>::try_from(signature)
            .map_err(|_| CryptoError::SignatureError("Invalid ML-DSA-65 signature".to_string()))?;
        self.0
            .verify(message, &signature)
            .map_err(|_| CryptoError::SignatureError("ML-DSA-65 verification failed".to_string()))
    }
}

pub(crate) struct MlKem768DecapsulationKey(InnerMlKemDecapsulationKey768);

impl MlKem768DecapsulationKey {
    pub(crate) fn from_seed(seed: &[u8; 64]) -> Self {
        Self(InnerMlKemDecapsulationKey768::from_seed(MlKemSeed::from(*seed)))
    }

    pub(crate) fn encapsulation_key_bytes(&self) -> Vec<u8> {
        self.0.encapsulation_key().to_bytes().as_slice().to_vec()
    }

    pub(crate) fn decapsulate(&self, ciphertext: &[u8]) -> Result<[u8; 32], CryptoError> {
        let ciphertext = InnerMlKemCiphertext768::try_from(ciphertext)
            .map_err(|_| CryptoError::KeyWrapError("Invalid ML-KEM-768 ciphertext".to_string()))?;
        let shared_secret = self.0.decapsulate(&ciphertext);
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(shared_secret.as_slice());
        Ok(bytes)
    }
}

pub(crate) struct MlKem768EncapsulationKey(InnerMlKemEncapsulationKey768);

impl MlKem768EncapsulationKey {
    pub(crate) fn from_bytes(public_key: &[u8]) -> Result<Self, CryptoError> {
        InnerMlKemEncapsulationKey768::new_from_slice(public_key)
            .map(Self)
            .map_err(|_| CryptoError::KeyWrapError("Invalid ML-KEM-768 public key".to_string()))
    }

    pub(crate) fn encapsulate_deterministic(&self, randomness: &[u8; 32]) -> (Vec<u8>, [u8; 32]) {
        let (ciphertext, shared_secret) =
            self.0.encapsulate_deterministic(&MlKemB32::from(*randomness));
        let mut shared_secret_bytes = [0u8; 32];
        shared_secret_bytes.copy_from_slice(shared_secret.as_slice());
        (ciphertext.as_slice().to_vec(), shared_secret_bytes)
    }
}
