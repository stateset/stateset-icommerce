//! ICP-1.0 conformance IUT adapter — Rust reference.
//!
//! Reads one JSON object from stdin, dispatches on the test name passed in argv[1],
//! writes one JSON object to stdout. Protocol: see
//! `icp-conformance/iut-adapters/iut.protocol.md`.
//!
//! This binary deliberately does NOT depend on `stateset-icommerce` business
//! logic — only on the canonical crypto + serialization primitives. That keeps
//! the adapter focused on the *protocol* surface, not the implementation
//! surface.

use std::io::{Read, Write};

use anyhow::{Context, Result};
use ed25519_dalek::{Signer, SigningKey, Verifier, VerifyingKey};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use x25519_dalek::{PublicKey as XPublicKey, StaticSecret as XStaticSecret};

fn main() {
    if let Err(e) = run() {
        eprintln!("FATAL: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let test_name = std::env::args().nth(1).context("missing test name argument")?;

    let mut input_str = String::new();
    std::io::stdin().read_to_string(&mut input_str).context("read stdin")?;
    let input: Value = serde_json::from_str(&input_str).context("parse stdin JSON")?;

    let output = match test_name.as_str() {
        "01-aid-derivation" => run_01_aid_derivation(&input)?,
        "02-canonical-json" => run_02_canonical_json(&input)?,
        "03-signature-verification" => run_03_signature_verification(&input)?,
        other => {
            // Per iut.protocol.md: exit 2 + JSON on stderr signals SKIP.
            eprintln!(
                "{}",
                json!({"error": "unsupported", "reason": format!("no handler for {other}")})
            );
            std::process::exit(2);
        }
    };

    let mut stdout = std::io::stdout().lock();
    writeln!(stdout, "{}", serde_json::to_string_pretty(&output).context("serialize output")?)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Test 01: AID derivation and Intent signing
// ---------------------------------------------------------------------------

fn run_01_aid_derivation(input: &Value) -> Result<Value> {
    let agent = input.get("agent").context("missing 'agent' in input")?;
    let ed_seed_hex = agent
        .get("ed25519_seed_hex")
        .and_then(Value::as_str)
        .context("missing agent.ed25519_seed_hex")?;
    let x_seed_hex = agent
        .get("x25519_seed_hex")
        .and_then(Value::as_str)
        .context("missing agent.x25519_seed_hex")?;

    let ed_seed: [u8; 32] = hex::decode(ed_seed_hex)?
        .try_into()
        .map_err(|_| anyhow::anyhow!("ed25519_seed must be 32 bytes"))?;
    let x_seed: [u8; 32] = hex::decode(x_seed_hex)?
        .try_into()
        .map_err(|_| anyhow::anyhow!("x25519_seed must be 32 bytes"))?;

    // --- Keypairs --------------------------------------------------------
    let ed_signing = SigningKey::from_bytes(&ed_seed);
    let ed_verifying: VerifyingKey = ed_signing.verifying_key();
    let ed_pub_raw: [u8; 32] = ed_verifying.to_bytes();

    let x_secret = XStaticSecret::from(x_seed);
    let x_pub: XPublicKey = (&x_secret).into();
    let x_pub_raw: [u8; 32] = x_pub.to_bytes();

    // --- AID per ICP-1.0 §4.2 -------------------------------------------
    let mut hasher = Sha256::new();
    hasher.update(ed_pub_raw);
    hasher.update([0x00u8]);
    hasher.update(x_pub_raw);
    let aid_digest = hasher.finalize();
    let aid = format!("aid:v1:z{}", base58btc_encode(&aid_digest));

    // --- Build Intent: fill buyer + principal_binding.agent --------------
    let intent_input = input.get("intent").context("missing 'intent' in input")?;
    let mut intent = intent_input.clone();
    intent["buyer"] = Value::String(aid.clone());
    if let Some(pb) = intent.get_mut("principal_binding") {
        pb["agent"] = Value::String(aid.clone());
    }

    // --- Canonicalize and sign -------------------------------------------
    let canonical = serde_jcs::to_string(&intent).context("canonicalize JSON")?;
    let sig = ed_signing.sign(canonical.as_bytes());
    let sig_bytes = sig.to_bytes();

    let mut out = serde_json::Map::new();
    out.insert("ed25519_pubkey_hex".into(), json!(hex::encode(ed_pub_raw)));
    out.insert("x25519_pubkey_hex".into(), json!(hex::encode(x_pub_raw)));
    out.insert("aid".into(), json!(aid));
    out.insert("intent_canonical_string".into(), json!(canonical));
    out.insert("intent_canonical_bytes_hex".into(), json!(hex::encode(canonical.as_bytes())));
    out.insert("intent_signature_hex".into(), json!(hex::encode(sig_bytes)));

    // --- Optional negative-case verification -----------------------------
    if input
        .get("params")
        .and_then(|p| p.get("verify_tamper_rejected"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        let tampered = canonical.replacen("29.99", "0.01", 1);
        let parsed_sig = ed25519_dalek::Signature::from_bytes(&sig_bytes);
        let ok = ed_verifying.verify(tampered.as_bytes(), &parsed_sig).is_ok();
        out.insert("tamper_rejected".into(), json!(!ok));
    }

    Ok(Value::Object(out))
}

// ---------------------------------------------------------------------------
// Test 02: Canonical JSON
// ---------------------------------------------------------------------------

fn run_02_canonical_json(input: &Value) -> Result<Value> {
    let cases =
        input.get("cases").and_then(Value::as_array).context("input.cases must be an array")?;

    let mut canonical_strings = Vec::with_capacity(cases.len());
    let mut names = Vec::with_capacity(cases.len());
    for case in cases {
        let value = case.get("value").context("case missing 'value'")?;
        let name =
            case.get("name").and_then(Value::as_str).context("case missing 'name'")?.to_string();
        let canonical = serde_jcs::to_string(value).context("canonicalize JSON")?;
        canonical_strings.push(Value::String(canonical));
        names.push(Value::String(name));
    }

    Ok(json!({
        "canonical_strings": canonical_strings,
        "names": names,
    }))
}

// ---------------------------------------------------------------------------
// Test 03: Signature Verification
// ---------------------------------------------------------------------------

fn run_03_signature_verification(input: &Value) -> Result<Value> {
    let cases =
        input.get("cases").and_then(Value::as_array).context("input.cases must be an array")?;

    let mut verifications = Vec::with_capacity(cases.len());
    let mut names = Vec::with_capacity(cases.len());
    for case in cases {
        let name =
            case.get("name").and_then(Value::as_str).context("case missing 'name'")?.to_string();
        let canonical = case.get("canonical").and_then(Value::as_str).unwrap_or("");
        let signature_hex = case.get("signature_hex").and_then(Value::as_str).unwrap_or("");
        let pubkey_hex = case.get("pubkey_hex").and_then(Value::as_str).unwrap_or("");
        verifications.push(Value::Bool(verify_one(canonical, signature_hex, pubkey_hex)));
        names.push(Value::String(name));
    }

    Ok(json!({
        "verifications": verifications,
        "names": names,
    }))
}

fn verify_one(canonical: &str, signature_hex: &str, pubkey_hex: &str) -> bool {
    use ed25519_dalek::Signature;
    let Ok(sig_bytes) = hex::decode(signature_hex) else { return false };
    if sig_bytes.len() != 64 {
        return false;
    }
    let Ok(pub_bytes) = hex::decode(pubkey_hex) else { return false };
    let Ok(pub_arr): Result<[u8; 32], _> = pub_bytes.try_into() else { return false };
    let Ok(verifying) = VerifyingKey::from_bytes(&pub_arr) else { return false };
    let sig_arr: [u8; 64] = match sig_bytes.try_into() {
        Ok(a) => a,
        Err(_) => return false,
    };
    let sig = Signature::from_bytes(&sig_arr);
    verifying.verify(canonical.as_bytes(), &sig).is_ok()
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Base58btc per Bitcoin / draft-msporny-base58, with leading-zero preservation.
fn base58btc_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

    // Arbitrary-precision base conversion via bigint-by-byte long division.
    let mut digits: Vec<u8> = Vec::with_capacity(bytes.len() * 2);
    let mut input: Vec<u8> = bytes.to_vec();

    // Skip leading zero bytes from input; they become leading '1' chars.
    let mut leading_zeros = 0;
    for b in &input {
        if *b == 0 {
            leading_zeros += 1;
        } else {
            break;
        }
    }
    let mut start = leading_zeros;
    while start < input.len() {
        let mut carry: u32 = 0;
        for byte in input.iter_mut().skip(start) {
            let v = u32::from(*byte) + carry * 256;
            *byte = (v / 58) as u8;
            carry = v % 58;
        }
        digits.push(carry as u8);
        // Advance past any new leading zeros produced by the division.
        while start < input.len() && input[start] == 0 {
            start += 1;
        }
    }

    let mut out = String::with_capacity(leading_zeros + digits.len());
    for _ in 0..leading_zeros {
        out.push('1');
    }
    for d in digits.iter().rev() {
        out.push(ALPHABET[*d as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base58btc_known_vectors() {
        // Bitcoin reference vectors.
        assert_eq!(base58btc_encode(&[0u8]), "1");
        assert_eq!(base58btc_encode(&[0u8, 0u8]), "11");
        assert_eq!(base58btc_encode(b"Hello World!"), "2NEpo7TZRRrLZSi2U");
        // 32-byte zero buffer → 32 leading '1's
        assert_eq!(base58btc_encode(&[0u8; 32]), "1".repeat(32));
    }

    #[test]
    fn rfc8032_canonical_aid() {
        // Joint RFC 8032 + RFC 7748 seeds. Expected AID matches what the
        // JS adapter produces and what's locked into the conformance vector.
        let ed_seed: [u8; 32] =
            hex::decode("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60")
                .unwrap()
                .try_into()
                .unwrap();
        let x_seed: [u8; 32] =
            hex::decode("77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a")
                .unwrap()
                .try_into()
                .unwrap();

        let ed_signing = SigningKey::from_bytes(&ed_seed);
        let ed_pub: [u8; 32] = ed_signing.verifying_key().to_bytes();
        assert_eq!(
            hex::encode(ed_pub),
            "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"
        );

        let x_secret = XStaticSecret::from(x_seed);
        let x_pub: [u8; 32] = XPublicKey::from(&x_secret).to_bytes();
        assert_eq!(
            hex::encode(x_pub),
            "8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a"
        );

        let mut hasher = Sha256::new();
        hasher.update(ed_pub);
        hasher.update([0x00u8]);
        hasher.update(x_pub);
        let digest = hasher.finalize();
        let aid = format!("aid:v1:z{}", base58btc_encode(&digest));
        assert_eq!(aid, "aid:v1:z8aiPxVDKT12yzrWon2VrLRE9VDWiR82NqPaUDJv6Mz6b");
    }
}
