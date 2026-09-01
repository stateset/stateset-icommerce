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

// ---------------------------------------------------------------------------
// Status transitions must be atomic (regression for the SQLite settle TOCTOU:
// `mark_settled` and siblings read the status on one pooled connection and
// then issued an unconditional UPDATE on another, so two concurrent settles —
// or a settle racing a cancel/expire — both passed the check and both wrote).
// ---------------------------------------------------------------------------

fn force_status(db: &SqliteDatabase, id: uuid::Uuid, status: X402IntentStatus) {
    let conn = db.conn().expect("get sqlite connection");
    conn.execute(
        "UPDATE x402_payment_intents SET status = ? WHERE id = ?",
        rusqlite::params![status.to_string(), id.to_string()],
    )
    .expect("force intent status");
}

#[test]
fn sqlite_x402_second_settle_of_settled_intent_is_refused() {
    let db = SqliteDatabase::in_memory().expect("create in-memory sqlite db");
    let repo = db.x402_payment_intents();
    let intent = create_test_intent(&db);
    force_status(&db, intent.id, X402IntentStatus::Sequenced);

    let settled = repo.mark_settled(intent.id, "0xfirst", 10).expect("first settle");
    assert_eq!(settled.status, X402IntentStatus::Settled);
    assert_eq!(settled.tx_hash.as_deref(), Some("0xfirst"));

    let err = repo.mark_settled(intent.id, "0xsecond", 11).expect_err("second settle refused");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");

    let after = repo.get(intent.id).expect("get").expect("exists");
    assert_eq!(after.tx_hash.as_deref(), Some("0xfirst"), "second settle must not overwrite");
    assert_eq!(after.block_number, Some(10));

    // Terminal states can no longer be silently flipped by expire/fail/cancel.
    assert!(repo.mark_expired(intent.id).is_err(), "expire of settled intent must fail");
    assert!(repo.mark_failed(intent.id, "late").is_err(), "fail of settled intent must fail");
    assert!(repo.cancel(intent.id).is_err(), "cancel of settled intent must fail");
    let after = repo.get(intent.id).expect("get").expect("exists");
    assert_eq!(after.status, X402IntentStatus::Settled);
}

#[test]
fn sqlite_x402_concurrent_settles_exactly_one_wins() {
    use std::sync::{Arc, Barrier};

    let db = Arc::new(SqliteDatabase::in_memory().expect("create in-memory sqlite db"));
    // Several rounds with fresh intents so a scheduling fluke cannot mask the race.
    for round in 0..20 {
        let intent = create_test_intent(&db);
        force_status(&db, intent.id, X402IntentStatus::Sequenced);

        let contenders = 4;
        let barrier = Arc::new(Barrier::new(contenders));
        let handles: Vec<_> = (0..contenders)
            .map(|i| {
                let db = Arc::clone(&db);
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    let repo = db.x402_payment_intents();
                    barrier.wait();
                    repo.mark_settled(intent.id, &format!("0xtx-{round}-{i}"), 100 + i as u64)
                })
            })
            .collect();

        let results: Vec<_> = handles.into_iter().map(|h| h.join().expect("thread")).collect();
        let successes = results.iter().filter(|r| r.is_ok()).count();
        assert_eq!(successes, 1, "round {round}: exactly one settle must win, got {results:?}");
        for r in &results {
            if let Err(e) = r {
                assert!(
                    matches!(e, CommerceError::ValidationError(_) | CommerceError::Conflict(_)),
                    "losers must fail with a status error, got {e:?}"
                );
            }
        }
        let winner = results.into_iter().find_map(Result::ok).expect("winner");
        let stored = db.x402_payment_intents().get(intent.id).expect("get").expect("exists");
        assert_eq!(stored.status, X402IntentStatus::Settled);
        assert_eq!(stored.tx_hash, winner.tx_hash, "stored tx_hash must be the winner's");
    }
}

#[test]
fn sqlite_x402_settle_racing_cancel_or_expire_is_serialized() {
    use std::sync::{Arc, Barrier};

    let db = Arc::new(SqliteDatabase::in_memory().expect("create in-memory sqlite db"));
    for round in 0..20 {
        let intent = create_test_intent(&db);
        force_status(&db, intent.id, X402IntentStatus::Sequenced);

        let barrier = Arc::new(Barrier::new(3));
        let settle = {
            let (db, barrier) = (Arc::clone(&db), Arc::clone(&barrier));
            std::thread::spawn(move || {
                barrier.wait();
                db.x402_payment_intents().mark_settled(intent.id, "0xsettle", 1).map(|i| i.status)
            })
        };
        let expire = {
            let (db, barrier) = (Arc::clone(&db), Arc::clone(&barrier));
            std::thread::spawn(move || {
                barrier.wait();
                db.x402_payment_intents().mark_expired(intent.id).map(|i| i.status)
            })
        };
        let fail = {
            let (db, barrier) = (Arc::clone(&db), Arc::clone(&barrier));
            std::thread::spawn(move || {
                barrier.wait();
                db.x402_payment_intents().mark_failed(intent.id, "boom").map(|i| i.status)
            })
        };

        let results = [
            settle.join().expect("settle thread"),
            expire.join().expect("expire thread"),
            fail.join().expect("fail thread"),
        ];
        let winners: Vec<_> = results.iter().filter_map(|r| r.as_ref().ok()).collect();
        assert_eq!(winners.len(), 1, "round {round}: exactly one transition wins: {results:?}");
        let stored = db.x402_payment_intents().get(intent.id).expect("get").expect("exists");
        assert_eq!(&stored.status, winners[0], "stored status must match the sole winner");
    }
}
