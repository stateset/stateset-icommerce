//! Intent envelope builders.

use crate::time::{format_rfc3339_millis, now_millis};
use crate::types::{IntentBase, PrincipalBinding, Signature};
use crate::{Error, Identity, canonical_json};
use rand_core::{OsRng, RngCore};
use serde::Serialize;
use serde_json::Value;

/// A signed `IntentEnvelope` ready for POST to `/icp/v1/intents`.
///
/// Wire shape: `{ intent, signature: {alg, kid, sig}, _pubkey_hex, _x_pubkey_hex }`.
/// This matches the `JavaScript` reference SDK byte-for-byte.
#[derive(Debug, Clone, Serialize)]
pub struct IntentEnvelope {
    /// Verb-specific intent object (already canonicalized by the wire).
    pub intent: Value,
    /// Outer signature envelope.
    pub signature: Signature,
    /// Convenience copy of the Agent's Ed25519 public key (hex) so the
    /// handler can verify without resolving the AID via DID document.
    pub _pubkey_hex: String,
    /// Convenience copy of the Agent's X25519 public key (hex). Required by
    /// the handler to re-derive and bind the AID per ICP-1.0 §4.2.
    pub _x_pubkey_hex: String,
}

/// Build a signed `IntentEnvelope`:
///   1. Re-marshal `intent_value` as canonical JSON (RFC 8785 JCS).
///   2. Sign the canonical bytes with `identity`'s Ed25519 key.
///   3. Wrap in the JS-SDK-compatible envelope.
pub fn build_intent_envelope(
    identity: &Identity,
    intent_value: Value,
) -> Result<IntentEnvelope, Error> {
    let canonical = canonical_json(&intent_value)?;
    let sig = identity.sign_hex(canonical.as_bytes());
    Ok(IntentEnvelope {
        intent: intent_value,
        signature: Signature { alg: "ed25519".to_string(), kid: identity.aid().to_string(), sig },
        _pubkey_hex: hex::encode(identity.ed_pubkey()),
        _x_pubkey_hex: hex::encode(identity.x_pubkey()),
    })
}

/// Generate a fresh 16-byte random nonce as lowercase hex.
pub(crate) fn fresh_nonce_16() -> String {
    let mut bytes = [0u8; 16];
    OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// Generate a fresh `intent_id` with the `icp_int_` prefix.
pub(crate) fn fresh_intent_id() -> String {
    let mut bytes = [0u8; 12];
    OsRng.fill_bytes(&mut bytes);
    format!("icp_int_{}", hex::encode(bytes))
}

/// Format `now` and `now+window_secs` as RFC 3339 in UTC.
pub(crate) fn rfc3339_window(window_secs: i64) -> (String, String) {
    let now = now_millis();
    (format_rfc3339_millis(now), format_rfc3339_millis(now + window_secs * 1000))
}

/// Build the verb-agnostic base portion of an Intent.
///
/// `merchant` and `settler` are wire-required fields; the handler
/// rejects intents missing either.
pub(crate) fn intent_base(
    identity: &Identity,
    verb: &str,
    merchant: &str,
    settler: &str,
    principal_binding: Option<PrincipalBinding>,
) -> IntentBase {
    let (iat, exp) = rfc3339_window(300);
    IntentBase {
        v: "icp-1.0".to_string(),
        verb: verb.to_string(),
        intent_id: fresh_intent_id(),
        buyer: identity.aid().to_string(),
        merchant: merchant.to_string(),
        settler: settler.to_string(),
        expiry: exp.clone(),
        principal_binding,
        nonce: fresh_nonce_16(),
        iat,
        exp,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signature as Ed25519Signature, Verifier};
    use serde_json::json;

    #[test]
    fn envelope_signature_matches_canonical_bytes() {
        let identity = Identity::from_seeds_hex(
            "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60",
            "77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a",
        )
        .unwrap();

        let intent = json!({
            "v": "icp-1.0",
            "verb": "inventory.query",
            "buyer": identity.aid(),
            "merchant": "aid:v1:zMerchantTest",
            "settler": "settler:test",
        });

        let env = build_intent_envelope(&identity, intent).unwrap();

        let canonical = canonical_json(&env.intent).unwrap();
        let sig_bytes: [u8; 64] = hex::decode(&env.signature.sig).unwrap().try_into().unwrap();
        let sig = Ed25519Signature::from_bytes(&sig_bytes);
        assert!(identity.verifying_key().verify(canonical.as_bytes(), &sig).is_ok());
        assert_eq!(env.signature.alg, "ed25519");
        assert_eq!(env.signature.kid, identity.aid());
    }

    #[test]
    fn fresh_nonce_is_32_hex_chars() {
        let n = fresh_nonce_16();
        assert_eq!(n.len(), 32);
        assert!(n.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn intent_window_is_five_minutes_and_omits_an_absent_binding() {
        let identity = Identity::generate();
        let base = intent_base(&identity, "inventory.query", "aid:v1:zM", "settler:test", None);
        let iat = crate::time::parse_rfc3339_millis(&base.iat).unwrap();
        let exp = crate::time::parse_rfc3339_millis(&base.exp).unwrap();
        assert_eq!(exp - iat, 300_000, "handler caps the Intent window at 600s");
        assert_eq!(base.expiry, base.exp);

        // No delegation configured => the key is absent from the wire bytes,
        // not present-and-null: a `null` would be signed over and read by the
        // handler as a malformed binding.
        let wire = serde_json::to_value(&base).unwrap();
        assert!(wire.get("principal_binding").is_none(), "{wire}");
    }
}
