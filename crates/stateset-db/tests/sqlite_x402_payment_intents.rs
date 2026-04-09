#![cfg(feature = "sqlite")]

use stateset_core::{
    CommerceError, CreateX402PaymentIntent, SignX402PaymentIntent, X402_DEFAULT_SIGNATURE_SCHEME,
    X402Asset, X402IntentStatus, X402Network, X402PaymentIntentRepository, X402SignatureScheme,
};
use stateset_db::SqliteDatabase;

fn create_test_intent(db: &SqliteDatabase) -> stateset_core::X402PaymentIntent {
    db.x402_payment_intents()
        .create(CreateX402PaymentIntent {
            payer_address: "0xpayer-sqlite-test".to_string(),
            payee_address: "0xpayee-sqlite-test".to_string(),
            amount: 1_000_000,
            asset: X402Asset::Usdc,
            network: X402Network::SetChain,
            ..Default::default()
        })
        .expect("create sqlite x402 intent")
}

#[test]
fn sqlite_x402_rejects_ed25519_downgrade_for_new_intents() {
    let db = SqliteDatabase::in_memory().expect("create in-memory sqlite db");
    let repo = db.x402_payment_intents();
    let intent = create_test_intent(&db);

    assert_eq!(intent.payer_signature_scheme, Some(X402_DEFAULT_SIGNATURE_SCHEME));

    let mut locally_signed = intent.clone();
    locally_signed.sign_with_ed25519(&[11u8; 32]).expect("locally sign legacy intent");

    let result = repo.sign(
        intent.id,
        SignX402PaymentIntent {
            intent_id: intent.id,
            signature_scheme: Some(X402SignatureScheme::Ed25519),
            signature: locally_signed.payer_signature.expect("legacy signature"),
            public_key: locally_signed.payer_public_key.expect("legacy public key"),
            signature_bundle: None,
            public_key_bundle: None,
        },
    );

    assert!(matches!(
        result,
        Err(CommerceError::ValidationError(message))
            if message.contains("ed25519_ml_dsa65") && message.contains("refusing ed25519")
    ));
}

#[test]
fn sqlite_x402_allows_ed25519_signing_for_legacy_rows() {
    let db = SqliteDatabase::in_memory().expect("create in-memory sqlite db");
    let intent = create_test_intent(&db);

    let conn = db.conn().expect("get sqlite connection");
    conn.execute(
        "UPDATE x402_payment_intents SET payer_signature_scheme = NULL WHERE id = ?",
        [intent.id.to_string()],
    )
    .expect("downgrade intent to legacy row");
    drop(conn);

    let repo = db.x402_payment_intents();
    let legacy_intent = repo.get(intent.id).expect("get legacy row").expect("legacy row exists");
    assert_eq!(legacy_intent.payer_signature_scheme, None);

    let mut locally_signed = legacy_intent;
    locally_signed.sign_with_ed25519(&[19u8; 32]).expect("locally sign legacy row");

    let signed = repo
        .sign(
            intent.id,
            SignX402PaymentIntent {
                intent_id: intent.id,
                signature_scheme: Some(X402SignatureScheme::Ed25519),
                signature: locally_signed.payer_signature.expect("legacy signature"),
                public_key: locally_signed.payer_public_key.expect("legacy public key"),
                signature_bundle: None,
                public_key_bundle: None,
            },
        )
        .expect("sign legacy row");

    assert_eq!(signed.status, X402IntentStatus::Signed);
    assert_eq!(signed.payer_signature_scheme, Some(X402SignatureScheme::Ed25519));
}

#[test]
fn sqlite_x402_create_respects_configured_signature_scheme() {
    let db = SqliteDatabase::in_memory().expect("create in-memory sqlite db");

    let intent = db
        .x402_payment_intents()
        .create(CreateX402PaymentIntent {
            payer_address: "0xpayer-sqlite-strict".to_string(),
            payee_address: "0xpayee-sqlite-strict".to_string(),
            amount: 1_000_000,
            asset: X402Asset::Usdc,
            network: X402Network::SetChain,
            signature_scheme: Some(X402SignatureScheme::MlDsa65),
            ..Default::default()
        })
        .expect("create strict sqlite x402 intent");

    assert_eq!(intent.payer_signature_scheme, Some(X402SignatureScheme::MlDsa65));
}
