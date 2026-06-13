//! Intent envelope builders.

use crate::types::{Authority, IntentBase, PrincipalBinding, Signature};
use crate::{Error, Identity, Money, canonical_json};
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
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_secs() as i64;
    (format_rfc3339(now), format_rfc3339(now + window_secs))
}

/// Minimal RFC 3339 formatter for UTC `Z` timestamps. Mirrors what
/// `new Date(...).toISOString()` produces in the JS SDK (millisecond
/// precision with `.000Z` suffix).
fn format_rfc3339(epoch_secs: i64) -> String {
    // Algorithm from Howard Hinnant's date library (civil_from_days).
    let days = epoch_secs.div_euclid(86_400);
    let secs_of_day = epoch_secs.rem_euclid(86_400);

    let hh = secs_of_day / 3600;
    let mm = (secs_of_day % 3600) / 60;
    let ss = secs_of_day % 60;

    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };

    format!("{year:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}.000Z")
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
    max_per_intent: Money,
    authorized_verbs: Vec<String>,
    max_per_payout: Option<Money>,
) -> IntentBase {
    let (iat, exp) = rfc3339_window(300);
    let principal_binding = PrincipalBinding {
        principal: identity.aid().to_string(),
        agent: identity.aid().to_string(),
        authority: Authority { max_per_intent, verbs: authorized_verbs, max_per_payout },
        expiry: rfc3339_window(86_400).1,
        revocation: format!("https://{merchant}/.well-known/icp/revocation"),
        // Demo: principal is self-binding. A real Principal would
        // issue this signature via a separate key-management flow.
        signature: Signature {
            alg: "ed25519".to_string(),
            kid: "self".to_string(),
            sig: "deadbeef".to_string(),
        },
    };
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
    fn rfc3339_format_is_z_suffixed_millis() {
        let s = format_rfc3339(0);
        assert_eq!(s, "1970-01-01T00:00:00.000Z");
        let s = format_rfc3339(1_700_000_000);
        // Sanity: starts with a 4-digit year + dashes + T + colons + Z
        assert!(s.starts_with("2023-"));
        assert!(s.ends_with("Z"));
    }
}
