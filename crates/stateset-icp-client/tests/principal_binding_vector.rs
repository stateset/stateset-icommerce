//! The cross-language `PrincipalBinding` vector.
//!
//! `packages/icp-client/test/fixtures/principal-binding-vector.json` is read
//! by the `JavaScript` suite (`packages/icp-client/test/client.test.mjs`) and
//! the Python suite (`packages/icp-python-client/tests/test_client.py`). This
//! test reads the SAME file: if the Rust SDK's canonical bytes or signature
//! drift from the other two SDKs by so much as a byte, this fails.
//!
//! A binding is only worth anything if the handler that checks it can
//! reproduce the bytes the principal signed, so "byte-identical" is the whole
//! contract — not an optimisation.

use stateset_icp_client::{Money, PrincipalBindingParams, PrincipalIdentity, canonical_json};

fn fixture() -> serde_json::Value {
    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = manifest.parent().unwrap().parent().unwrap();
    let path = root.join("packages/icp-client/test/fixtures/principal-binding-vector.json");
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_str(&text).expect("fixture is JSON")
}

fn params_from(vector: &serde_json::Value) -> PrincipalBindingParams {
    let p = &vector["params"];
    PrincipalBindingParams::new(p["principal"].as_str().unwrap(), p["agent"].as_str().unwrap())
        .expires_at(p["expires_at"].as_str().unwrap())
        .verbs(p["verbs"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()))
        .max_per_intent(Money {
            amount: p["max_per_intent"]["amount"].as_str().unwrap().to_string(),
            currency: p["max_per_intent"]["currency"].as_str().unwrap().to_string(),
        })
        .revocation(p["revocation"].as_str().unwrap())
}

#[test]
fn principal_identity_derives_the_vector_pubkey() {
    let vector = fixture();
    let principal =
        PrincipalIdentity::from_seed_hex(vector["principal_seed_hex"].as_str().unwrap()).unwrap();
    assert_eq!(
        principal.ed_pubkey_hex(),
        vector["expected_principal_pubkey_hex"].as_str().unwrap()
    );
}

#[test]
fn signed_binding_matches_the_cross_language_vector() {
    let vector = fixture();
    let principal =
        PrincipalIdentity::from_seed_hex(vector["principal_seed_hex"].as_str().unwrap()).unwrap();

    let binding = params_from(&vector).sign(&principal).expect("sign");

    // 1. The canonical bytes the principal signed over — the binding minus
    //    its own signature, RFC 8785. This is the string the handler
    //    re-derives in `checkDelegation`.
    let unsigned = serde_json::to_value(&binding)
        .map(|mut v| {
            v.as_object_mut().unwrap().remove("signature");
            v
        })
        .unwrap();
    assert_eq!(
        canonical_json(&unsigned).unwrap(),
        vector["expected_canonical"].as_str().unwrap(),
        "canonical bytes must match the JS/Python vector exactly"
    );

    // 2. The signature hex itself: same seed, same bytes, same Ed25519.
    assert_eq!(binding.signature.sig, vector["expected_signature_hex"].as_str().unwrap());
    assert_eq!(binding.signature.alg, "ed25519");
    assert_eq!(binding.signature.kid, vector["params"]["principal"].as_str().unwrap());

    // 3. The whole serialized binding, field for field.
    assert_eq!(serde_json::to_value(&binding).unwrap(), vector["expected_binding"]);
}

#[test]
fn expiry_is_normalised_to_the_millisecond_form_both_sdks_emit() {
    // JS builds this field as `new Date(expiresAt).toISOString()`. A Rust SDK
    // that passed an RFC 3339 string through untouched would sign different
    // bytes for the same input, and no other SDK could reproduce them.
    let principal = PrincipalIdentity::from_seed_hex(
        "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60",
    )
    .unwrap();
    for input in ["2026-01-01T00:00:00Z", "2026-01-01T00:00:00.000Z", "2026-01-01T02:00:00+02:00"] {
        let binding = PrincipalBindingParams::new("did:web:x.example", "aid:v1:zAgent")
            .expires_at(input)
            .sign(&principal)
            .expect("sign");
        assert_eq!(binding.expiry, "2026-01-01T00:00:00.000Z", "input {input}");
    }
}

#[test]
fn an_unparsable_expiry_is_rejected_at_signing_time() {
    let principal = PrincipalIdentity::generate();
    let err = PrincipalBindingParams::new("did:web:x.example", "aid:v1:zAgent")
        .expires_at("last tuesday")
        .sign(&principal)
        .expect_err("must reject");
    assert!(format!("{err}").contains("RFC 3339"), "got: {err}");
}

#[test]
fn empty_verbs_and_blank_principal_are_rejected() {
    let principal = PrincipalIdentity::generate();
    assert!(
        PrincipalBindingParams::new("did:web:x.example", "aid:v1:zAgent")
            .verbs(Vec::<String>::new())
            .sign(&principal)
            .is_err()
    );
    assert!(PrincipalBindingParams::new("  ", "aid:v1:zAgent").sign(&principal).is_err());
    assert!(PrincipalBindingParams::new("did:web:x.example", "").sign(&principal).is_err());
}

#[test]
fn defaults_mirror_the_reference_sdks() {
    let principal = PrincipalIdentity::generate();
    let binding =
        PrincipalBindingParams::new("did:web:x.example", "aid:v1:zAgent").sign(&principal).unwrap();
    assert_eq!(binding.authority.max_per_intent.amount, "10000");
    assert_eq!(binding.authority.max_per_intent.currency, "USDC");
    assert_eq!(binding.revocation, "https://example.com/icp-revocation/aid:v1:zAgent");
    // Every verb this client can emit — and nothing wider.
    assert_eq!(
        binding.authority.verbs,
        vec![
            "channel.register",
            "inventory.query",
            "payout.request",
            "purchase.create",
            "purchase.return",
            "quote.request",
            "subscription.cancel",
            "subscription.create",
        ]
    );
}
