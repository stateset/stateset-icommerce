//! `mark_batched` verifies the batch inclusion proof before recording it.
//!
//! It used to store any Merkle root and proof it was handed. The leaf is now
//! rebuilt from the stored intent and the proof checked against the root at
//! the given leaf index and tree size, inside the write transaction; a proof
//! that does not prove *this* intent is refused and the intent stays
//! `Sequenced` with no commitment recorded.
//!
//! Postgres twin of `sqlite_x402_batch_inclusion.rs`. Requires a live Postgres
//! instance (`POSTGRES_URL` / `DATABASE_URL`); skipped otherwise.
#![cfg(feature = "postgres")]

use stateset_core::{
    CommerceError, CreateX402PaymentIntent, SignX402PaymentIntent, X402Asset, X402BatchInclusion,
    X402IntentStatus, X402Network, X402PaymentIntent,
};
use stateset_crypto::pqc::generate_hybrid_signing_keypair;
use stateset_db::PostgresDatabase;
use uuid::Uuid;

async fn connect() -> Option<PostgresDatabase> {
    let Some(url) =
        std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
    else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return None;
    };
    Some(PostgresDatabase::connect(&url).await.expect("connect + migrate"))
}

/// Create, sign (hybrid), and sequence an intent through the real repository
/// path, so the leaf covers real signature material.
async fn sequenced_intent(db: &PostgresDatabase, sequence_number: u64) -> X402PaymentIntent {
    let repo = db.x402_payment_intents();
    let intent = repo
        .create_async(CreateX402PaymentIntent {
            payer_address: format!("0xpayer{}", Uuid::new_v4().as_simple()),
            payee_address: format!("0xpayee{}", Uuid::new_v4().as_simple()),
            amount: 1_000_000,
            asset: X402Asset::Usdc,
            network: X402Network::SetChain,
            ..Default::default()
        })
        .await
        .expect("create intent");
    let mut local = intent.clone();
    local.sign_with_hybrid(&generate_hybrid_signing_keypair().expect("keypair")).expect("sign");
    repo.sign_async(
        intent.id,
        SignX402PaymentIntent {
            intent_id: intent.id,
            signature_scheme: None,
            signature: local.payer_signature.clone().expect("signature"),
            public_key: local.payer_public_key.clone().expect("public key"),
            signature_bundle: local.payer_signature_bundle.clone(),
            public_key_bundle: local.payer_public_key_bundle.clone(),
        },
    )
    .await
    .expect("sign intent");
    repo.mark_sequenced_async(intent.id, sequence_number, Uuid::new_v4())
        .await
        .expect("sequence intent")
}

/// A three-intent batch; returns the intents in leaf order.
async fn batch(db: &PostgresDatabase) -> Vec<X402PaymentIntent> {
    let mut intents = Vec::new();
    for i in 0..3 {
        intents.push(sequenced_intent(db, 100 + i).await);
    }
    intents
}

fn leaves(intents: &[X402PaymentIntent]) -> Vec<[u8; 32]> {
    intents.iter().map(|i| i.batch_leaf_hash().expect("leaf")).collect()
}

async fn assert_refused_and_unbatched(
    db: &PostgresDatabase,
    id: Uuid,
    inclusion: &X402BatchInclusion,
    why: &str,
) {
    let repo = db.x402_payment_intents();
    let err = repo.mark_batched_async(id, inclusion).await.expect_err(why);
    assert!(matches!(err, CommerceError::ValidationError(_)), "{why}: {err:?}");
    let stored = repo.get_async(id).await.expect("get").expect("exists");
    assert_eq!(stored.status, X402IntentStatus::Sequenced, "{why}: intent must stay sequenced");
    assert_eq!(stored.batch_merkle_root, None, "{why}: no root recorded");
    assert_eq!(stored.inclusion_proof, None, "{why}: no proof recorded");
}

#[tokio::test]
async fn postgres_x402_mark_batched_accepts_a_valid_inclusion_proof() {
    let Some(db) = connect().await else { return };
    let intents = batch(&db).await;
    let inclusion = X402BatchInclusion::from_leaves(&leaves(&intents), 1).expect("proof");

    let batched = db
        .x402_payment_intents()
        .mark_batched_async(intents[1].id, &inclusion)
        .await
        .expect("valid proof");
    assert_eq!(batched.status, X402IntentStatus::Batched);
    assert_eq!(batched.batch_merkle_root.as_deref(), Some(inclusion.merkle_root.as_str()));
    assert_eq!(batched.inclusion_proof.as_ref(), Some(&inclusion.inclusion_proof));
}

#[tokio::test]
async fn postgres_x402_mark_batched_refuses_a_wrong_root() {
    let Some(db) = connect().await else { return };
    let intents = batch(&db).await;
    let mut inclusion = X402BatchInclusion::from_leaves(&leaves(&intents), 1).expect("proof");
    inclusion.merkle_root = format!("0x{}", "ab".repeat(32));
    assert_refused_and_unbatched(&db, intents[1].id, &inclusion, "wrong root").await;

    // A root that is not even a 32-byte hash is refused the same way.
    inclusion.merkle_root = "0xroot".into();
    assert_refused_and_unbatched(&db, intents[1].id, &inclusion, "malformed root").await;
}

#[tokio::test]
async fn postgres_x402_mark_batched_refuses_a_tampered_proof() {
    let Some(db) = connect().await else { return };
    let intents = batch(&db).await;
    let mut inclusion = X402BatchInclusion::from_leaves(&leaves(&intents), 1).expect("proof");
    let first = inclusion.inclusion_proof[0].clone();
    let flipped = if first.ends_with('0') { '1' } else { '0' };
    inclusion.inclusion_proof[0] = format!("{}{flipped}", &first[..first.len() - 1]);
    assert_refused_and_unbatched(&db, intents[1].id, &inclusion, "tampered proof").await;

    let mut truncated = X402BatchInclusion::from_leaves(&leaves(&intents), 1).expect("proof");
    truncated.inclusion_proof.pop();
    assert_refused_and_unbatched(&db, intents[1].id, &truncated, "truncated proof").await;
}

#[tokio::test]
async fn postgres_x402_mark_batched_refuses_a_wrong_leaf_index() {
    let Some(db) = connect().await else { return };
    let intents = batch(&db).await;
    let mut inclusion = X402BatchInclusion::from_leaves(&leaves(&intents), 1).expect("proof");
    inclusion.leaf_index = 0;
    assert_refused_and_unbatched(&db, intents[1].id, &inclusion, "wrong leaf index").await;

    let mut out_of_range = X402BatchInclusion::from_leaves(&leaves(&intents), 1).expect("proof");
    out_of_range.leaf_index = 7;
    assert_refused_and_unbatched(&db, intents[1].id, &out_of_range, "leaf index out of range")
        .await;
}

#[tokio::test]
async fn postgres_x402_mark_batched_refuses_a_proof_for_a_different_intent() {
    let Some(db) = connect().await else { return };
    let intents = batch(&db).await;
    // A genuine proof — but for intents[0]'s leaf — presented for intents[2].
    let other = X402BatchInclusion::from_leaves(&leaves(&intents), 0).expect("proof");
    assert_refused_and_unbatched(&db, intents[2].id, &other, "proof for a different intent").await;

    // A batch that does not contain the intent at all.
    let outsiders = batch(&db).await;
    let foreign = X402BatchInclusion::from_leaves(&leaves(&outsiders), 2).expect("proof");
    assert_refused_and_unbatched(&db, intents[2].id, &foreign, "batch without this intent").await;
}

#[tokio::test]
async fn postgres_x402_mark_settled_refuses_intents_that_were_never_sequenced_or_batched() {
    let Some(db) = connect().await else { return };
    let repo = db.x402_payment_intents();
    let created = repo
        .create_async(CreateX402PaymentIntent {
            payer_address: format!("0xpayer{}", Uuid::new_v4().as_simple()),
            payee_address: "0xpayee".into(),
            amount: 1,
            asset: X402Asset::Usdc,
            network: X402Network::SetChain,
            ..Default::default()
        })
        .await
        .expect("create");
    let err = repo
        .mark_settled_async(created.id, &format!("0xtx-{}", Uuid::new_v4().as_simple()), 1)
        .await
        .expect_err("created");
    assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");
    assert_eq!(
        repo.get_async(created.id).await.unwrap().unwrap().status,
        X402IntentStatus::Created
    );

    // Settling a batched intent still works end to end.
    let intents = batch(&db).await;
    let inclusion = X402BatchInclusion::from_leaves(&leaves(&intents), 0).expect("proof");
    repo.mark_batched_async(intents[0].id, &inclusion).await.expect("batch");
    let settled = repo
        .mark_settled_async(intents[0].id, &format!("0xtx-{}", Uuid::new_v4().as_simple()), 9)
        .await
        .expect("settle batched");
    assert_eq!(settled.status, X402IntentStatus::Settled);
}
