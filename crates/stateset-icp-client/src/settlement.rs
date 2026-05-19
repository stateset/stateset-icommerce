//! `SettlementReceipt` verification — Stripe-style one-call validator
//! for the most load-bearing artifact in ICP-1.0.
//!
//! The receipt is signed by BOTH the merchant AND the Settler over the
//! canonical bytes of the receipt body *minus* the two signature
//! fields themselves. Both signatures cover **identical** canonical
//! input — so verifying both against their respective published
//! Ed25519 pubkeys is the only way to treat settlement as final.
//!
//! Mirrors the `JavaScript` and `Python` SDK helpers byte-for-byte.

use crate::Error;
use crate::canonical::canonical_json;
use crate::identity::verify_ed25519;
use serde_json::{Map, Value};

/// Options for [`verify_settlement_receipt`].
#[derive(Debug, Clone, Copy)]
pub struct VerifySettlementReceiptOptions {
    /// When `false`, the settler-signature check is skipped entirely —
    /// the settler pubkey argument is then unused. Default `true`.
    pub require_settler: bool,
}

impl Default for VerifySettlementReceiptOptions {
    fn default() -> Self {
        Self { require_settler: true }
    }
}

/// Verify a co-signed `SettlementReceipt`. Returns the receipt
/// unchanged on success.
///
/// Algorithm (matches the handler's signing path):
///   1. Strip `merchant_signature` and `settler_signature` fields.
///   2. Re-canonicalize the remainder (RFC 8785 JCS).
///   3. Verify `merchant_signature.sig` against `merchant_pubkey_hex`.
///   4. Verify `settler_signature.sig` against `settler_pubkey_hex`
///      (skipped if `opts.require_settler = false`).
///
/// Returns `Err(Error::Icp { code: "...", ... })`:
///   - `format.missing_field` — receipt missing a required signature field.
///   - `signature.invalid` — merchant signature failed.
///   - `settlement.settler_signature_invalid` — settler signature failed.
pub fn verify_settlement_receipt(
    receipt: &Value,
    merchant_pubkey_hex: &str,
    settler_pubkey_hex: &str,
    opts: VerifySettlementReceiptOptions,
) -> Result<Value, Error> {
    let obj = receipt.as_object().ok_or_else(|| Error::Icp {
        code: "format.missing_field".to_string(),
        message: "receipt must be an object".to_string(),
    })?;

    let merchant_sig = extract_sig(obj, "merchant_signature")?;
    let need_settler = opts.require_settler;
    let settler_sig =
        if need_settler { Some(extract_sig(obj, "settler_signature")?) } else { None };

    // Strip both signature fields and canonicalize. Both signatures
    // cover the same canonical bytes, so we compute them once.
    let mut unsigned: Map<String, Value> = obj.clone();
    unsigned.remove("merchant_signature");
    unsigned.remove("settler_signature");
    let canonical = canonical_json(&Value::Object(unsigned))?;

    if !verify_ed25519(canonical.as_bytes(), &merchant_sig.sig, merchant_pubkey_hex) {
        return Err(Error::Icp {
            code: "signature.invalid".to_string(),
            message: format!("merchant signature verification failed (kid={})", merchant_sig.kid,),
        });
    }

    if let Some(settler) = settler_sig {
        if !verify_ed25519(canonical.as_bytes(), &settler.sig, settler_pubkey_hex) {
            return Err(Error::Icp {
                code: "settlement.settler_signature_invalid".to_string(),
                message: format!("settler signature verification failed (kid={})", settler.kid,),
            });
        }
    }

    Ok(receipt.clone())
}

struct ExtractedSig {
    sig: String,
    kid: String,
}

fn extract_sig(obj: &Map<String, Value>, field: &str) -> Result<ExtractedSig, Error> {
    let sig_obj = obj.get(field).and_then(Value::as_object).ok_or_else(|| Error::Icp {
        code: "format.missing_field".to_string(),
        message: format!("receipt.{field}.sig required"),
    })?;
    let sig = sig_obj
        .get("sig")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::Icp {
            code: "format.missing_field".to_string(),
            message: format!("receipt.{field}.sig required"),
        })?
        .to_string();
    let kid = sig_obj.get("kid").and_then(Value::as_str).unwrap_or("<unknown>").to_string();
    Ok(ExtractedSig { sig, kid })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Identity;
    use serde_json::json;

    fn merchant_id() -> Identity {
        Identity::from_seeds_hex(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        )
        .unwrap()
    }
    fn settler_id() -> Identity {
        Identity::from_seeds_hex(
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        )
        .unwrap()
    }

    fn build_signed_receipt() -> Value {
        let unsigned = json!({
            "type": "icp.settlement.receipt",
            "v": "icp-1.0",
            "settlement_id": "icp_set_TEST",
            "escrow_id": "0xabcdef",
            "intent_id": "icp_int_TEST",
            "final_state": "released",
            "amount": {"amount": "29.99", "currency": "USDC"},
            "rail": "demo-mock",
            "rail_txid": "0xcafe",
            "settled_at": "2026-05-12T18:00:00.000Z",
            "released_to": "0xMerchantPayout",
        });
        let canonical = canonical_json(&unsigned).unwrap();
        let m_sig = merchant_id().sign_hex(canonical.as_bytes());
        let s_sig = settler_id().sign_hex(canonical.as_bytes());

        let mut obj = unsigned.as_object().unwrap().clone();
        obj.insert(
            "merchant_signature".to_string(),
            json!({"alg": "ed25519", "kid": "aid:v1:zMerchant", "sig": m_sig}),
        );
        obj.insert(
            "settler_signature".to_string(),
            json!({"alg": "ed25519", "kid": "aid:v1:zSettler", "sig": s_sig}),
        );
        Value::Object(obj)
    }

    fn merchant_pub() -> String {
        hex::encode(merchant_id().ed_pubkey())
    }
    fn settler_pub() -> String {
        hex::encode(settler_id().ed_pubkey())
    }

    fn opts() -> VerifySettlementReceiptOptions {
        VerifySettlementReceiptOptions::default()
    }

    #[test]
    fn happy_path_returns_receipt() {
        let r = build_signed_receipt();
        let out = verify_settlement_receipt(&r, &merchant_pub(), &settler_pub(), opts())
            .expect("should verify");
        assert_eq!(out["final_state"], "released");
    }

    #[test]
    fn tampered_amount_rejected_with_merchant_signature_invalid() {
        let mut r = build_signed_receipt();
        r["amount"] = json!({"amount": "999.99", "currency": "USDC"});
        let err =
            verify_settlement_receipt(&r, &merchant_pub(), &settler_pub(), opts()).unwrap_err();
        match err {
            Error::Icp { code, .. } => assert_eq!(code, "signature.invalid"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn wrong_settler_pubkey_rejected_with_typed_code() {
        let r = build_signed_receipt();
        // Settler pubkey doesn't match the seed that signed.
        let wrong = "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20";
        let err = verify_settlement_receipt(&r, &merchant_pub(), wrong, opts()).unwrap_err();
        match err {
            Error::Icp { code, .. } => assert_eq!(code, "settlement.settler_signature_invalid"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn missing_merchant_signature_rejected_with_format_error() {
        let mut r = build_signed_receipt();
        r.as_object_mut().unwrap().remove("merchant_signature");
        let err =
            verify_settlement_receipt(&r, &merchant_pub(), &settler_pub(), opts()).unwrap_err();
        match err {
            Error::Icp { code, message } => {
                assert_eq!(code, "format.missing_field");
                assert!(message.contains("merchant_signature"));
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn missing_settler_signature_rejected_with_format_error() {
        let mut r = build_signed_receipt();
        r.as_object_mut().unwrap().remove("settler_signature");
        let err =
            verify_settlement_receipt(&r, &merchant_pub(), &settler_pub(), opts()).unwrap_err();
        match err {
            Error::Icp { code, message } => {
                assert_eq!(code, "format.missing_field");
                assert!(message.contains("settler_signature"));
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn require_settler_false_skips_settler_check() {
        let mut r = build_signed_receipt();
        r.as_object_mut().unwrap().remove("settler_signature");
        let opts = VerifySettlementReceiptOptions { require_settler: false };
        // Should NOT error despite missing settler_signature.
        let out = verify_settlement_receipt(&r, &merchant_pub(), "0".repeat(64).as_str(), opts)
            .expect("should verify without settler");
        assert_eq!(out["final_state"], "released");
    }

    #[test]
    fn both_signatures_cover_same_canonical_bytes() {
        let r = build_signed_receipt();
        let obj = r.as_object().unwrap();
        let mut unsigned = obj.clone();
        unsigned.remove("merchant_signature");
        unsigned.remove("settler_signature");
        let canonical = canonical_json(&Value::Object(unsigned)).unwrap();
        let expected_m = merchant_id().sign_hex(canonical.as_bytes());
        let expected_s = settler_id().sign_hex(canonical.as_bytes());
        assert_eq!(obj["merchant_signature"]["sig"].as_str().unwrap(), expected_m);
        assert_eq!(obj["settler_signature"]["sig"].as_str().unwrap(), expected_s);
    }
}
