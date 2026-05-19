//! Agent identity: keypair generation, AID derivation, signing.

use crate::Error;
use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use rand_core::OsRng;
use sha2::{Digest, Sha256};
use x25519_dalek::{PublicKey as XPublicKey, StaticSecret as XStaticSecret};

const BASE58BTC_ALPHABET: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

/// An ICP-1.0 Agent identity: Ed25519 signing key + X25519 transport key + derived AID.
///
/// AID derivation follows ICP-1.0 §4.2:
/// `aid:v1:z` + `Base58btc(SHA-256(ed_pubkey || 0x00 || x_pubkey))`
///
/// The X25519 secret is retained for future confidential-transport
/// support (see ICPIP-0005 draft); ICP-1.0 wire signing only uses the
/// Ed25519 key.
#[derive(Clone)]
pub struct Identity {
    ed_signing: SigningKey,
    #[allow(dead_code)]
    x_secret: XStaticSecret,
    ed_pub_raw: [u8; 32],
    x_pub_raw: [u8; 32],
    aid: String,
}

impl std::fmt::Debug for Identity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Identity")
            .field("aid", &self.aid)
            .field("ed_pubkey", &hex::encode(self.ed_pub_raw))
            .field("x_pubkey", &hex::encode(self.x_pub_raw))
            .finish_non_exhaustive()
    }
}

impl Identity {
    /// Generate a fresh random identity using the OS RNG.
    pub fn generate() -> Self {
        let mut ed_seed = [0u8; 32];
        let mut x_seed = [0u8; 32];
        use rand_core::RngCore;
        OsRng.fill_bytes(&mut ed_seed);
        OsRng.fill_bytes(&mut x_seed);
        Self::from_seeds(&ed_seed, &x_seed).expect("32-byte seeds")
    }

    /// Construct an Identity from explicit 32-byte seeds. Used for tests,
    /// persistence, and conformance vectors.
    pub fn from_seeds(ed_seed: &[u8; 32], x_seed: &[u8; 32]) -> Result<Self, Error> {
        let ed_signing = SigningKey::from_bytes(ed_seed);
        let ed_pub_raw: [u8; 32] = ed_signing.verifying_key().to_bytes();

        let x_secret = XStaticSecret::from(*x_seed);
        let x_pub_raw: [u8; 32] = XPublicKey::from(&x_secret).to_bytes();

        let mut hasher = Sha256::new();
        hasher.update(ed_pub_raw);
        hasher.update([0x00u8]);
        hasher.update(x_pub_raw);
        let digest = hasher.finalize();
        let aid = format!("aid:v1:z{}", base58btc_encode(&digest));

        Ok(Self { ed_signing, x_secret, ed_pub_raw, x_pub_raw, aid })
    }

    /// Construct from explicit hex-encoded 32-byte seeds.
    pub fn from_seeds_hex(ed_seed_hex: &str, x_seed_hex: &str) -> Result<Self, Error> {
        let ed_seed = decode_seed(ed_seed_hex)?;
        let x_seed = decode_seed(x_seed_hex)?;
        Self::from_seeds(&ed_seed, &x_seed)
    }

    /// Returns the derived `aid:v1:z…` identifier.
    pub fn aid(&self) -> &str {
        &self.aid
    }

    /// Raw 32-byte Ed25519 public key.
    pub const fn ed_pubkey(&self) -> [u8; 32] {
        self.ed_pub_raw
    }

    /// Raw 32-byte X25519 public key.
    pub const fn x_pubkey(&self) -> [u8; 32] {
        self.x_pub_raw
    }

    /// Sign a message with the Ed25519 key. Returns the 64-byte signature.
    pub fn sign(&self, message: &[u8]) -> [u8; 64] {
        self.ed_signing.sign(message).to_bytes()
    }

    /// Sign a message and hex-encode the signature.
    pub fn sign_hex(&self, message: &[u8]) -> String {
        hex::encode(self.sign(message))
    }

    /// Verifying key for signature verification by counterparties.
    pub fn verifying_key(&self) -> VerifyingKey {
        self.ed_signing.verifying_key()
    }
}

/// Verify a signature against a raw 32-byte Ed25519 public key (provided as hex).
///
/// Returns `true` if the signature is valid for the given message under the
/// supplied public key, `false` otherwise (including on any malformed input).
/// Never panics; safe to call with attacker-controlled input.
pub fn verify_ed25519(message: &[u8], signature_hex: &str, pubkey_hex: &str) -> bool {
    use ed25519_dalek::{Signature, Verifier};
    let Ok(sig_bytes) = hex::decode(signature_hex) else { return false };
    let Ok(sig_arr): Result<[u8; 64], _> = sig_bytes.try_into() else { return false };
    let Ok(pub_bytes) = hex::decode(pubkey_hex) else { return false };
    let Ok(pub_arr): Result<[u8; 32], _> = pub_bytes.try_into() else { return false };
    let Ok(verifying) = VerifyingKey::from_bytes(&pub_arr) else { return false };
    let sig = Signature::from_bytes(&sig_arr);
    verifying.verify(message, &sig).is_ok()
}

fn decode_seed(hex_str: &str) -> Result<[u8; 32], Error> {
    let bytes = hex::decode(hex_str).map_err(|e| Error::InvalidInput(format!("hex: {e}")))?;
    bytes.try_into().map_err(|_| Error::InvalidInput("seed must be 32 bytes".to_string()))
}

/// Base58btc per Bitcoin / draft-msporny-base58, with leading-zero preservation.
/// Identical algorithm to the JS, Go, and Python IUTs.
fn base58btc_encode(bytes: &[u8]) -> String {
    let mut leading_zeros = 0;
    for b in bytes {
        if *b == 0 {
            leading_zeros += 1;
        } else {
            break;
        }
    }

    let mut input: Vec<u8> = bytes.to_vec();
    let mut digits: Vec<u8> = Vec::with_capacity(bytes.len() * 2);
    let mut start = leading_zeros;
    while start < input.len() {
        let mut carry: u32 = 0;
        for byte in input.iter_mut().skip(start) {
            let v = u32::from(*byte) + carry * 256;
            *byte = (v / 58) as u8;
            carry = v % 58;
        }
        digits.push(carry as u8);
        while start < input.len() && input[start] == 0 {
            start += 1;
        }
    }

    let mut out = String::with_capacity(leading_zeros + digits.len());
    for _ in 0..leading_zeros {
        out.push('1');
    }
    for d in digits.iter().rev() {
        out.push(BASE58BTC_ALPHABET[*d as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc8032_canonical_aid() {
        // Joint RFC 8032 + RFC 7748 test seeds. Locked into conformance vector 01.
        let id = Identity::from_seeds_hex(
            "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60",
            "77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a",
        )
        .unwrap();
        assert_eq!(
            hex::encode(id.ed_pubkey()),
            "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"
        );
        assert_eq!(
            hex::encode(id.x_pubkey()),
            "8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a"
        );
        assert_eq!(id.aid(), "aid:v1:z8aiPxVDKT12yzrWon2VrLRE9VDWiR82NqPaUDJv6Mz6b");
    }

    #[test]
    fn base58btc_leading_zeros() {
        assert_eq!(base58btc_encode(&[0u8]), "1");
        assert_eq!(base58btc_encode(&[0u8, 0u8]), "11");
        assert_eq!(base58btc_encode(b"Hello World!"), "2NEpo7TZRRrLZSi2U");
    }

    #[test]
    fn generated_identity_is_well_formed() {
        let id = Identity::generate();
        assert!(id.aid().starts_with("aid:v1:z"));
        assert_eq!(id.ed_pubkey().len(), 32);
        assert_eq!(id.x_pubkey().len(), 32);
    }

    #[test]
    fn signs_and_verifies() {
        use ed25519_dalek::{Signature, Verifier};
        let id = Identity::generate();
        let msg = b"hello icp";
        let sig_bytes = id.sign(msg);
        let sig = Signature::from_bytes(&sig_bytes);
        assert!(id.verifying_key().verify(msg, &sig).is_ok());
    }
}
