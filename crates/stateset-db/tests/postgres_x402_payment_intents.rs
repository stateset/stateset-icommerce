#[cfg(feature = "postgres")]
use stateset_core::{
    CommerceError, CreateX402PaymentIntent, SignX402PaymentIntent, X402_DEFAULT_SIGNATURE_SCHEME,
    X402Asset, X402IntentStatus, X402Network, X402PaymentIntentFilter, X402SignatureScheme,
};
#[cfg(feature = "postgres")]
use stateset_crypto::pqc::generate_hybrid_signing_keypair;
#[cfg(feature = "postgres")]
use stateset_db::PostgresDatabase;
#[cfg(feature = "postgres")]
use std::env;
#[cfg(feature = "postgres")]
use uuid::Uuid;

#[cfg(feature = "postgres")]
fn postgres_url() -> Option<String> {
    env::var("POSTGRES_URL").ok().or_else(|| env::var("DATABASE_URL").ok())
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_x402_payment_intent_smoke() {
    let url = match postgres_url() {
        Some(url) => url,
        None => {
            eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping postgres x402 intent test");
            return;
        }
    };

    let db = PostgresDatabase::connect(&url).await.expect("connect to postgres and run migrations");

    let repo = db.x402_payment_intents();
    let payer = format!("0xtest{}", Uuid::new_v4().to_string().replace('-', ""));
    let payee = format!("0xpayee{}", Uuid::new_v4().to_string().replace('-', ""));
    let cart_id = Uuid::new_v4();
    let idempotency = format!("idem-{}", Uuid::new_v4());

    let intent = repo
        .create_async(CreateX402PaymentIntent {
            payer_address: payer.clone(),
            payee_address: payee.clone(),
            amount: 1_000_000,
            asset: X402Asset::Usdc,
            network: X402Network::SetChain,
            nonce: None,
            validity_seconds: Some(3600),
            resource_uri: Some("/x402/test".to_string()),
            resource_method: Some("POST".to_string()),
            description: Some("test intent".to_string()),
            cart_id: Some(cart_id),
            order_id: None,
            invoice_id: None,
            merchant_id: Some("merchant-test".to_string()),
            idempotency_key: Some(idempotency.clone()),
            metadata: Some("{\"source\":\"test\"}".to_string()),
            signature_scheme: None,
        })
        .await
        .expect("create intent");

    assert_eq!(intent.status, X402IntentStatus::Created);
    assert_eq!(intent.cart_id, Some(cart_id));
    assert_eq!(intent.payer_signature_scheme, Some(X402_DEFAULT_SIGNATURE_SCHEME));

    let by_key = repo
        .get_by_idempotency_key_async(&idempotency)
        .await
        .expect("get by idempotency")
        .expect("intent for idempotency");
    assert_eq!(by_key.id, intent.id);

    let mut local_signed = intent.clone();
    let keypair = generate_hybrid_signing_keypair().expect("generate hybrid signing keypair");
    local_signed.sign_with_hybrid(&keypair).expect("sign intent locally");

    let signed = repo
        .sign_async(
            intent.id,
            SignX402PaymentIntent {
                intent_id: intent.id,
                signature_scheme: None,
                signature: local_signed.payer_signature.clone().expect("generated signature"),
                public_key: local_signed.payer_public_key.clone().expect("generated public key"),
                signature_bundle: local_signed.payer_signature_bundle.clone(),
                public_key_bundle: local_signed.payer_public_key_bundle.clone(),
            },
        )
        .await
        .expect("sign intent");
    assert_eq!(signed.status, X402IntentStatus::Signed);
    assert_eq!(signed.payer_signature_scheme, Some(X402_DEFAULT_SIGNATURE_SCHEME));

    let batch_id = Uuid::new_v4();
    let sequenced =
        repo.mark_sequenced_async(intent.id, 42, batch_id).await.expect("sequence intent");
    assert_eq!(sequenced.status, X402IntentStatus::Sequenced);

    let settled = repo
        .mark_settled_async(intent.id, &format!("0xtxhash-{}", Uuid::new_v4().as_simple()), 123)
        .await
        .expect("settle intent");
    assert_eq!(settled.status, X402IntentStatus::Settled);

    let for_cart = repo.for_cart_async(cart_id).await.expect("for_cart");
    assert!(for_cart.iter().any(|i| i.id == intent.id));

    let list = repo
        .list_async(X402PaymentIntentFilter {
            payer_address: Some(payer),
            payee_address: None,
            status: None,
            network: Some(X402Network::SetChain),
            asset: Some(X402Asset::Usdc),
            order_id: None,
            batch_id: None,
            from_date: None,
            to_date: None,
            limit: Some(10),
            offset: Some(0),
        })
        .await
        .expect("list intents");

    assert!(!list.is_empty(), "expected at least one intent in list");
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_x402_rejects_ed25519_downgrade_for_new_intents() {
    let url = match postgres_url() {
        Some(url) => url,
        None => {
            eprintln!(
                "POSTGRES_URL or DATABASE_URL not set; skipping postgres x402 downgrade test"
            );
            return;
        }
    };

    let db = PostgresDatabase::connect(&url).await.expect("connect to postgres and run migrations");
    let repo = db.x402_payment_intents();

    let intent = repo
        .create_async(CreateX402PaymentIntent {
            payer_address: format!("0xtest{}", Uuid::new_v4().to_string().replace('-', "")),
            payee_address: format!("0xpayee{}", Uuid::new_v4().to_string().replace('-', "")),
            amount: 1_000_000,
            asset: X402Asset::Usdc,
            network: X402Network::SetChain,
            ..Default::default()
        })
        .await
        .expect("create intent");

    let mut local_signed = intent.clone();
    local_signed.sign_with_ed25519(&[13u8; 32]).expect("locally sign legacy intent");

    let result = repo
        .sign_async(
            intent.id,
            SignX402PaymentIntent {
                intent_id: intent.id,
                signature_scheme: Some(X402SignatureScheme::Ed25519),
                signature: local_signed.payer_signature.expect("legacy signature"),
                public_key: local_signed.payer_public_key.expect("legacy public key"),
                signature_bundle: None,
                public_key_bundle: None,
            },
        )
        .await;

    assert!(matches!(
        result,
        Err(CommerceError::ValidationError(message))
            if message.contains("ed25519_ml_dsa65") && message.contains("refusing ed25519")
    ));
}

// ---------------------------------------------------------------------------
// Expiry enforced at sequence/settle, tx_hash uniqueness, batch-create parity.
// ---------------------------------------------------------------------------

#[cfg(feature = "postgres")]
fn fresh_input() -> CreateX402PaymentIntent {
    CreateX402PaymentIntent {
        payer_address: format!("0xtest{}", Uuid::new_v4().to_string().replace('-', "")),
        payee_address: format!("0xpayee{}", Uuid::new_v4().to_string().replace('-', "")),
        amount: 1_000_000,
        asset: X402Asset::Usdc,
        network: X402Network::SetChain,
        ..Default::default()
    }
}

#[cfg(feature = "postgres")]
async fn force_status(db: &PostgresDatabase, id: Uuid, status: X402IntentStatus) {
    sqlx::query("UPDATE x402_payment_intents SET status = $1 WHERE id = $2")
        .bind(status.to_string())
        .bind(id)
        .execute(db.pool())
        .await
        .expect("force status");
}

#[cfg(feature = "postgres")]
async fn force_valid_until(db: &PostgresDatabase, id: Uuid, valid_until: i64) {
    sqlx::query("UPDATE x402_payment_intents SET valid_until = $1 WHERE id = $2")
        .bind(valid_until)
        .bind(id)
        .execute(db.pool())
        .await
        .expect("force valid_until");
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_x402_settle_after_valid_until_is_refused_and_swept() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");
    let repo = db.x402_payment_intents();
    let intent = repo.create_async(fresh_input()).await.expect("create");
    force_status(&db, intent.id, X402IntentStatus::Signed).await;
    let sequenced =
        repo.mark_sequenced_async(intent.id, 1, Uuid::new_v4()).await.expect("sequence");
    assert_eq!(sequenced.status, X402IntentStatus::Sequenced);

    force_valid_until(&db, intent.id, chrono::Utc::now().timestamp() - 30).await;

    let err = repo.mark_settled_async(intent.id, "0xlate", 5).await.expect_err("expired");
    assert!(
        matches!(&err, CommerceError::ValidationError(m) if m.contains("expired")),
        "got {err:?}"
    );
    let stored = repo.get_async(intent.id).await.expect("get").expect("exists");
    assert_eq!(stored.status, X402IntentStatus::Sequenced);
    assert!(stored.tx_hash.is_none());

    // The sweeper is global; invoke it and verify the effect on this intent.
    // Races in CI across neighboring tests can make the affected-row count flaky;
    // assert on the final stored status instead of the count.
    let _ = repo.expire_stale_intents_async().await.expect("sweep");
    let stored = repo.get_async(intent.id).await.expect("get").expect("exists");
    assert_eq!(stored.status, X402IntentStatus::Expired);
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_x402_sequence_after_valid_until_is_refused() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");
    let repo = db.x402_payment_intents();
    let intent = repo.create_async(fresh_input()).await.expect("create");
    force_status(&db, intent.id, X402IntentStatus::Signed).await;
    force_valid_until(&db, intent.id, chrono::Utc::now().timestamp() - 30).await;

    let err = repo.mark_sequenced_async(intent.id, 1, Uuid::new_v4()).await.expect_err("expired");
    assert!(
        matches!(&err, CommerceError::ValidationError(m) if m.contains("expired")),
        "got {err:?}"
    );
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_x402_sweeper_leaves_batched_intents_alone() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");
    let repo = db.x402_payment_intents();
    let intent = repo.create_async(fresh_input()).await.expect("create");
    force_status(&db, intent.id, X402IntentStatus::Batched).await;
    force_valid_until(&db, intent.id, chrono::Utc::now().timestamp() - 30).await;

    repo.expire_stale_intents_async().await.expect("sweep");
    assert_eq!(
        repo.get_async(intent.id).await.expect("get").expect("exists").status,
        X402IntentStatus::Batched
    );
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_x402_tx_hash_settles_at_most_one_intent() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");
    let repo = db.x402_payment_intents();
    let first = repo.create_async(fresh_input()).await.expect("create");
    let second = repo.create_async(fresh_input()).await.expect("create");
    force_status(&db, first.id, X402IntentStatus::Sequenced).await;
    force_status(&db, second.id, X402IntentStatus::Sequenced).await;
    let tx_hash = format!("0xshared-{}", Uuid::new_v4().as_simple());

    repo.mark_settled_async(first.id, &tx_hash, 10).await.expect("first settle");
    let err = repo.mark_settled_async(second.id, &tx_hash, 11).await.expect_err("reused tx_hash");
    assert!(
        matches!(&err, CommerceError::Conflict(m) if m.contains(&tx_hash) && m.contains(&first.id.to_string())),
        "got {err:?}"
    );
    let stored = repo.get_async(second.id).await.expect("get").expect("exists");
    assert_eq!(stored.status, X402IntentStatus::Sequenced);
    assert!(stored.tx_hash.is_none());

    // The database enforces it too, for writers that bypass the repository.
    let raw = sqlx::query(
        "UPDATE x402_payment_intents SET tx_hash = $1, tx_hash_key = $1, status = 'settled' WHERE id = $2",
    )
    .bind(&tx_hash)
    .bind(second.id)
    .execute(db.pool())
    .await;
    assert!(raw.is_err(), "unique index on tx_hash_key must reject the duplicate");
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_x402_batch_created_intent_rejects_ed25519_downgrade() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");
    let repo = db.x402_payment_intents();
    let created = repo.create_batch_atomic_async(vec![fresh_input()]).await.expect("batch");
    let intent = created.into_iter().next().expect("one intent");
    assert_eq!(intent.payer_signature_scheme, Some(X402_DEFAULT_SIGNATURE_SCHEME));
    assert!(intent.signing_hash.is_some(), "batch create must persist the signing hash");

    let mut local_signed = intent.clone();
    local_signed.sign_with_ed25519(&[13u8; 32]).expect("locally sign");
    let result = repo
        .sign_async(
            intent.id,
            SignX402PaymentIntent {
                intent_id: intent.id,
                signature_scheme: Some(X402SignatureScheme::Ed25519),
                signature: local_signed.payer_signature.expect("signature"),
                public_key: local_signed.payer_public_key.expect("public key"),
                signature_bundle: None,
                public_key_bundle: None,
            },
        )
        .await;
    assert!(matches!(
        result,
        Err(CommerceError::ValidationError(message))
            if message.contains("ed25519_ml_dsa65") && message.contains("refusing ed25519")
    ));
}
