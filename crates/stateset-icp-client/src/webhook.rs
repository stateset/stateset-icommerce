//! ICPIP-0005 receiver helpers — `verify_webhook` is the Stripe-style
//! one-call validator for inbound webhook events.
//!
//! Hand it the raw HTTP body bytes, request headers, HTTP method/path,
//! and the merchant's raw 32-byte Ed25519 pubkey from `.well-known/icp`.
//! Get back the parsed `EventEnvelope` JSON value, OR an
//! [`Error::Icp`] with a `channel.*` code that maps directly to HTTP
//! status.
//!
//! Performs every check ICPIP-0005 §6 mandates:
//!   1. HTTP timestamp within ±`tolerance_seconds` (default 300) →
//!      `channel.replay` on miss.
//!   2. HTTP-layer `X-ICP-Signature: ed25519=<hex>` verifies
//!      cryptographically against `<ts>.<method>.<path>.<body>`.
//!   3. Body parses as `{envelope, signature}`.
//!   4. Envelope signature verifies against the merchant pubkey over
//!      the envelope's canonical JSON bytes.
//!
//! Mirrors the `JavaScript` and `Python` SDK helpers byte-for-byte.

use crate::Error;
use crate::canonical::canonical_json;
use crate::identity::verify_ed25519;
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};

/// Options controlling [`verify_webhook`] behavior.
#[derive(Debug, Clone, Copy)]
pub struct VerifyWebhookOptions {
    /// Allowed clock skew between sender and receiver, in seconds.
    /// Defaults to 300 (ICPIP-0005 §6 mandate).
    pub tolerance_seconds: u64,
    /// Override "now" for testing. `None` uses `SystemTime::now()`.
    pub now_seconds: Option<u64>,
}

impl Default for VerifyWebhookOptions {
    fn default() -> Self {
        Self { tolerance_seconds: 300, now_seconds: None }
    }
}

/// Verify an inbound webhook and return its parsed `EventEnvelope`.
///
/// `headers` is any iterable of `(name, value)` pairs (typically your
/// HTTP library's header collection). Lookup is case-insensitive.
///
/// Returns `Err(Error::Icp { code: "channel.*", … })` on any failure.
pub fn verify_webhook<H>(
    body: &str,
    headers: H,
    method: &str,
    path: &str,
    merchant_pubkey_hex: &str,
    opts: VerifyWebhookOptions,
) -> Result<Value, Error>
where
    H: IntoIterator,
    H::Item: HeaderPair,
{
    // Collect headers once with normalized lowercase keys.
    let mut header_map: Vec<(String, String)> = Vec::with_capacity(8);
    for pair in headers {
        let (k, v) = pair.into_pair();
        header_map.push((k.to_ascii_lowercase(), v));
    }
    let lookup = |name: &str| -> Option<&str> {
        let lower = name.to_ascii_lowercase();
        header_map.iter().find_map(|(k, v)| (*k == lower).then_some(v.as_str()))
    };

    // 1. Timestamp window.
    let ts_header = lookup("x-icp-timestamp").ok_or_else(|| Error::Icp {
        code: "channel.signature_invalid".to_string(),
        message: "missing X-ICP-Timestamp header".to_string(),
    })?;
    let ts: u64 = ts_header.parse().map_err(|_| Error::Icp {
        code: "channel.signature_invalid".to_string(),
        message: format!("invalid X-ICP-Timestamp: {ts_header}"),
    })?;
    let now = opts
        .now_seconds
        .unwrap_or_else(|| SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs()));
    let drift = now.abs_diff(ts);
    if drift > opts.tolerance_seconds {
        return Err(Error::Icp {
            code: "channel.replay".to_string(),
            message: format!("timestamp {ts} outside ±{}s of {now}", opts.tolerance_seconds,),
        });
    }

    // 2. HTTP-layer signature.
    let sig_header = lookup("x-icp-signature").ok_or_else(|| Error::Icp {
        code: "channel.signature_invalid".to_string(),
        message: "missing X-ICP-Signature header".to_string(),
    })?;
    let http_sig_hex = sig_header.strip_prefix("ed25519=").ok_or_else(|| Error::Icp {
        code: "channel.signature_invalid".to_string(),
        message: "X-ICP-Signature must be ed25519=<hex>".to_string(),
    })?;
    let http_material = format!("{ts_header}.{method}.{path}.{body}");
    if !verify_ed25519(http_material.as_bytes(), http_sig_hex, merchant_pubkey_hex) {
        return Err(Error::Icp {
            code: "channel.signature_invalid".to_string(),
            message: "HTTP-layer signature verification failed".to_string(),
        });
    }

    // 3. Body shape.
    let parsed: Value = serde_json::from_str(body).map_err(|e| Error::Icp {
        code: "channel.signature_invalid".to_string(),
        message: format!("body is not JSON: {e}"),
    })?;
    let envelope = parsed.get("envelope").cloned().ok_or_else(|| Error::Icp {
        code: "channel.signature_invalid".to_string(),
        message: "body missing 'envelope'".to_string(),
    })?;
    let envelope_sig = parsed
        .get("signature")
        .and_then(|s| s.get("sig"))
        .and_then(Value::as_str)
        .ok_or_else(|| Error::Icp {
            code: "channel.signature_invalid".to_string(),
            message: "body missing signature.sig".to_string(),
        })?
        .to_string();

    // 4. Envelope signature over canonical bytes.
    let envelope_canonical = canonical_json(&envelope)?;
    if !verify_ed25519(envelope_canonical.as_bytes(), &envelope_sig, merchant_pubkey_hex) {
        return Err(Error::Icp {
            code: "channel.signature_invalid".to_string(),
            message: "envelope signature verification failed".to_string(),
        });
    }

    Ok(envelope)
}

/// Trait implemented by everything that can be coerced into a header
/// (name, value) string pair. Lets `verify_webhook` accept `Vec<(String,
/// String)>`, `HashMap<String, String>`, `&[(&str, &str)]`, and the
/// header types of popular HTTP crates without taking a dependency on
/// any of them.
pub trait HeaderPair {
    /// Convert into an owned `(name, value)` pair.
    fn into_pair(self) -> (String, String);
}

impl HeaderPair for (String, String) {
    fn into_pair(self) -> (String, String) {
        self
    }
}

impl HeaderPair for (&str, &str) {
    fn into_pair(self) -> (String, String) {
        (self.0.to_string(), self.1.to_string())
    }
}

impl HeaderPair for (&String, &String) {
    fn into_pair(self) -> (String, String) {
        (self.0.clone(), self.1.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Identity;
    use serde_json::json;

    // Fixed merchant keypair so tests are deterministic.
    const MERCHANT_SEED: &str = "5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a";

    fn merchant() -> Identity {
        Identity::from_seeds_hex(MERCHANT_SEED, MERCHANT_SEED).unwrap()
    }

    fn merchant_pubkey_hex() -> String {
        hex::encode(merchant().ed_pubkey())
    }

    fn sample_envelope() -> Value {
        json!({
            "v": "icp-1.0",
            "event_id": "icp_evt_test001",
            "event_type": "settlement.released",
            "channel_id": "icp_ch_test001",
            "sequence": 1,
            "originated_at": "2026-05-12T15:22:09.000Z",
            "source": "aid:v1:zMerchantTest",
            "target": "aid:v1:zAgentTest",
            "payload": {
                "settlement_id": "icp_set_abc",
                "escrow_id": "0xabc",
                "amount": {"amount": "29.99", "currency": "USDC"},
                "final_state": "released"
            },
            "previous_event_id": null,
            "delivery_attempt": 1
        })
    }

    fn forge(
        envelope: Value,
        now: u64,
        method: &str,
        path: &str,
    ) -> (String, Vec<(String, String)>) {
        let m = merchant();
        let env_canonical = canonical_json(&envelope).unwrap();
        let env_sig = m.sign_hex(env_canonical.as_bytes());
        let body = format!(
            "{{\"envelope\":{env_canonical},\"signature\":{{\"alg\":\"ed25519\",\"kid\":\"{}\",\"sig\":\"{env_sig}\"}}}}",
            envelope["source"].as_str().unwrap(),
        );
        let ts = now.to_string();
        let material = format!("{ts}.{method}.{path}.{body}");
        let http_sig = m.sign_hex(material.as_bytes());
        let headers = vec![
            ("content-type".to_string(), "application/json".to_string()),
            ("x-icp-timestamp".to_string(), ts),
            ("x-icp-signature".to_string(), format!("ed25519={http_sig}")),
            ("x-icp-channel-id".to_string(), envelope["channel_id"].as_str().unwrap().to_string()),
        ];
        (body, headers)
    }

    fn opts_at(now: u64) -> VerifyWebhookOptions {
        VerifyWebhookOptions { now_seconds: Some(now), tolerance_seconds: 300 }
    }

    #[test]
    fn happy_path_returns_parsed_envelope() {
        let now = 1_900_000_000;
        let (body, headers) = forge(sample_envelope(), now, "POST", "/icp/events");
        let env = verify_webhook(
            &body,
            headers,
            "POST",
            "/icp/events",
            &merchant_pubkey_hex(),
            opts_at(now),
        )
        .expect("should verify");
        assert_eq!(env["event_id"], "icp_evt_test001");
        assert_eq!(env["payload"]["final_state"], "released");
    }

    #[test]
    fn tampered_body_rejected() {
        let now = 1_900_000_000;
        let (body, headers) = forge(sample_envelope(), now, "POST", "/icp/events");
        let tampered = body.replace("29.99", "99.99");
        let err = verify_webhook(
            &tampered,
            headers,
            "POST",
            "/icp/events",
            &merchant_pubkey_hex(),
            opts_at(now),
        )
        .unwrap_err();
        match err {
            Error::Icp { code, .. } => assert_eq!(code, "channel.signature_invalid"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn stale_timestamp_rejected_with_channel_replay() {
        let stale = 1_900_000_000;
        let (body, headers) = forge(sample_envelope(), stale, "POST", "/icp/events");
        let now = stale + 600; // outside ±300s
        let err = verify_webhook(
            &body,
            headers,
            "POST",
            "/icp/events",
            &merchant_pubkey_hex(),
            opts_at(now),
        )
        .unwrap_err();
        match err {
            Error::Icp { code, .. } => assert_eq!(code, "channel.replay"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn missing_timestamp_rejected() {
        let now = 1_900_000_000;
        let (body, headers) = forge(sample_envelope(), now, "POST", "/icp/events");
        let without_ts: Vec<_> =
            headers.into_iter().filter(|(k, _)| k != "x-icp-timestamp").collect();
        let err = verify_webhook(
            &body,
            without_ts,
            "POST",
            "/icp/events",
            &merchant_pubkey_hex(),
            opts_at(now),
        )
        .unwrap_err();
        match err {
            Error::Icp { code, message } => {
                assert_eq!(code, "channel.signature_invalid");
                assert!(message.contains("X-ICP-Timestamp"));
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn missing_signature_header_rejected() {
        let now = 1_900_000_000;
        let (body, headers) = forge(sample_envelope(), now, "POST", "/icp/events");
        let without_sig: Vec<_> =
            headers.into_iter().filter(|(k, _)| k != "x-icp-signature").collect();
        let err = verify_webhook(
            &body,
            without_sig,
            "POST",
            "/icp/events",
            &merchant_pubkey_hex(),
            opts_at(now),
        )
        .unwrap_err();
        match err {
            Error::Icp { code, .. } => assert_eq!(code, "channel.signature_invalid"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn malformed_signature_algorithm_rejected() {
        let now = 1_900_000_000;
        let (body, mut headers) = forge(sample_envelope(), now, "POST", "/icp/events");
        for (k, v) in &mut headers {
            if k == "x-icp-signature" {
                *v = "hmac-sha256=deadbeef".to_string();
            }
        }
        let err = verify_webhook(
            &body,
            headers,
            "POST",
            "/icp/events",
            &merchant_pubkey_hex(),
            opts_at(now),
        )
        .unwrap_err();
        match err {
            Error::Icp { code, .. } => assert_eq!(code, "channel.signature_invalid"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn wrong_pubkey_rejected() {
        let now = 1_900_000_000;
        let (body, headers) = forge(sample_envelope(), now, "POST", "/icp/events");
        let other_pubkey = "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20";
        let err = verify_webhook(&body, headers, "POST", "/icp/events", other_pubkey, opts_at(now))
            .unwrap_err();
        match err {
            Error::Icp { code, .. } => assert_eq!(code, "channel.signature_invalid"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn case_insensitive_header_lookup() {
        let now = 1_900_000_000;
        let (body, headers) = forge(sample_envelope(), now, "POST", "/icp/events");
        // Upper-case the names.
        let upper: Vec<_> = headers.into_iter().map(|(k, v)| (k.to_uppercase(), v)).collect();
        let env = verify_webhook(
            &body,
            upper,
            "POST",
            "/icp/events",
            &merchant_pubkey_hex(),
            opts_at(now),
        )
        .expect("should verify with upper-case headers");
        assert_eq!(env["event_id"], "icp_evt_test001");
    }

    #[test]
    fn slice_of_str_pairs_supported() {
        let now = 1_900_000_000;
        let (body, headers) = forge(sample_envelope(), now, "POST", "/icp/events");
        // Borrow as (&str, &str) pairs to exercise the HeaderPair impl.
        let refs: Vec<(&str, &str)> =
            headers.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        let env = verify_webhook(
            &body,
            refs,
            "POST",
            "/icp/events",
            &merchant_pubkey_hex(),
            opts_at(now),
        )
        .expect("should verify");
        assert_eq!(env["event_id"], "icp_evt_test001");
    }
}
