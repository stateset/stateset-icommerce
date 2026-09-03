//! Postgres races for the x402 ledger: concurrent settles, settle vs
//! cancel/expire/fail, the `Batched` transition, and concurrent credit
//! debits. Mirrors the SQLite proofs in `sqlite_x402_payment_intents.rs` and
//! `sqlite_x402_ledger_guards.rs`.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.
#![cfg(feature = "postgres")]

use stateset_core::{
    CommerceError, CreateX402PaymentIntent, X402Asset, X402CreditAdjustment, X402CreditDirection,
    X402IntentStatus, X402Network,
};
use stateset_db::PostgresDatabase;
use std::sync::Arc;
use uuid::Uuid;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

async fn create_intent(db: &PostgresDatabase) -> stateset_core::X402PaymentIntent {
    db.x402_payment_intents()
        .create_async(CreateX402PaymentIntent {
            payer_address: format!("0xpayer-{}", Uuid::new_v4().as_simple()),
            payee_address: "0xpayee-race".to_string(),
            amount: 1_000_000,
            asset: X402Asset::Usdc,
            network: X402Network::SetChain,
            ..Default::default()
        })
        .await
        .expect("create intent")
}

async fn force_status(db: &PostgresDatabase, id: Uuid, status: X402IntentStatus) {
    sqlx::query("UPDATE x402_payment_intents SET status = $1 WHERE id = $2")
        .bind(status.to_string())
        .bind(id)
        .execute(db.pool())
        .await
        .expect("force status");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_x402_concurrent_settles_exactly_one_wins() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.expect("connect + migrate"));
    for round in 0..10 {
        let intent = create_intent(&db).await;
        force_status(&db, intent.id, X402IntentStatus::Sequenced).await;
        let tag = Uuid::new_v4().as_simple().to_string();
        let handles: Vec<_> = (0..4)
            .map(|i| {
                let db = Arc::clone(&db);
                let tag = tag.clone();
                tokio::spawn(async move {
                    db.x402_payment_intents()
                        .mark_settled_async(intent.id, &format!("0xtx-{tag}-{i}"), 100 + i)
                        .await
                })
            })
            .collect();
        let mut results = Vec::new();
        for h in handles {
            results.push(h.await.expect("task"));
        }
        let successes = results.iter().filter(|r| r.is_ok()).count();
        assert_eq!(successes, 1, "round {round}: exactly one settle must win: {results:?}");
        for r in &results {
            if let Err(e) = r {
                assert!(
                    matches!(e, CommerceError::ValidationError(_) | CommerceError::Conflict(_)),
                    "losers must fail with a status error, got {e:?}"
                );
            }
        }
        let winner = results.into_iter().find_map(Result::ok).expect("winner");
        let stored =
            db.x402_payment_intents().get_async(intent.id).await.expect("get").expect("exists");
        assert_eq!(stored.status, X402IntentStatus::Settled);
        assert_eq!(stored.tx_hash, winner.tx_hash);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_x402_settle_racing_cancel_expire_fail_has_one_winner() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.expect("connect + migrate"));
    for round in 0..10 {
        let intent = create_intent(&db).await;
        // Signed: settle is refused by status, cancel/expire/fail race.
        // Sequenced: settle/expire/fail race, cancel is refused.
        let start =
            if round % 2 == 0 { X402IntentStatus::Signed } else { X402IntentStatus::Sequenced };
        force_status(&db, intent.id, start).await;
        let id = intent.id;
        let tag = Uuid::new_v4().as_simple().to_string();
        let settle = {
            let db = Arc::clone(&db);
            tokio::spawn(async move {
                db.x402_payment_intents()
                    .mark_settled_async(id, &format!("0xsettle-{tag}"), 1)
                    .await
                    .map(|i| i.status)
            })
        };
        let cancel = {
            let db = Arc::clone(&db);
            tokio::spawn(async move {
                db.x402_payment_intents().cancel_async(id).await.map(|i| i.status)
            })
        };
        let expire = {
            let db = Arc::clone(&db);
            tokio::spawn(async move {
                db.x402_payment_intents().mark_expired_async(id).await.map(|i| i.status)
            })
        };
        let fail = {
            let db = Arc::clone(&db);
            tokio::spawn(async move {
                db.x402_payment_intents().mark_failed_async(id, "boom").await.map(|i| i.status)
            })
        };
        let results = [
            settle.await.expect("task"),
            cancel.await.expect("task"),
            expire.await.expect("task"),
            fail.await.expect("task"),
        ];
        let winners: Vec<_> = results.iter().filter_map(|r| r.as_ref().ok()).collect();
        assert_eq!(winners.len(), 1, "round {round}: exactly one transition wins: {results:?}");
        let stored = db.x402_payment_intents().get_async(id).await.expect("get").expect("exists");
        assert_eq!(&stored.status, winners[0], "stored status must match the sole winner");
    }
}

#[tokio::test]
async fn postgres_x402_mark_batched_is_guarded_and_settles_from_batched() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");
    let repo = db.x402_payment_intents();
    let intent = create_intent(&db).await;

    let err = repo.mark_batched_async(intent.id, "0xroot", vec![]).await.expect_err("created");
    assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");
    force_status(&db, intent.id, X402IntentStatus::Sequenced).await;
    let err = repo.mark_batched_async(intent.id, " ", vec![]).await.expect_err("root required");
    assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");

    let batched = repo
        .mark_batched_async(intent.id, "0xroot", vec!["0xaa".into(), "0xbb".into()])
        .await
        .expect("batch");
    assert_eq!(batched.status, X402IntentStatus::Batched);
    assert_eq!(batched.batch_merkle_root.as_deref(), Some("0xroot"));
    assert_eq!(batched.inclusion_proof, Some(vec!["0xaa".to_string(), "0xbb".to_string()]));

    let err = repo.mark_batched_async(intent.id, "0xroot", vec![]).await.expect_err("twice");
    assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");
    let err = repo.cancel_async(intent.id).await.expect_err("batched cannot be cancelled");
    assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");

    // Sweeper-exempt even past valid_until.
    sqlx::query("UPDATE x402_payment_intents SET valid_until = 1 WHERE id = $1")
        .bind(intent.id)
        .execute(db.pool())
        .await
        .unwrap();
    repo.expire_stale_intents_async().await.expect("sweep");
    assert_eq!(repo.get_async(intent.id).await.unwrap().unwrap().status, X402IntentStatus::Batched);
    sqlx::query("UPDATE x402_payment_intents SET valid_until = 9999999999 WHERE id = $1")
        .bind(intent.id)
        .execute(db.pool())
        .await
        .unwrap();

    let settled = repo
        .mark_settled_async(intent.id, &format!("0xtx-batched-{}", Uuid::new_v4().as_simple()), 7)
        .await
        .expect("settle from batched");
    assert_eq!(settled.status, X402IntentStatus::Settled);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_x402_credit_concurrent_debits_never_go_negative() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.expect("connect + migrate"));
    let payer = format!("0xcredit-race-{}", Uuid::new_v4().as_simple());
    let (asset, network) = (X402Asset::Usdc, X402Network::SetChain);
    for round in 0..8 {
        db.x402_credits()
            .adjust_balance_async(X402CreditAdjustment {
                payer_address: payer.clone(),
                asset,
                network,
                direction: X402CreditDirection::Credit,
                amount: 300,
                reason: None,
                reference_id: None,
                metadata: None,
            })
            .await
            .expect("top up");
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let db = Arc::clone(&db);
                let payer = payer.clone();
                tokio::spawn(async move {
                    db.x402_credits()
                        .adjust_balance_async(X402CreditAdjustment {
                            payer_address: payer,
                            asset,
                            network,
                            direction: X402CreditDirection::Debit,
                            amount: 100,
                            reason: None,
                            reference_id: None,
                            metadata: None,
                        })
                        .await
                })
            })
            .collect();
        let mut results = Vec::new();
        for h in handles {
            results.push(h.await.expect("task"));
        }
        let ok = results.iter().filter(|r| r.is_ok()).count();
        assert_eq!(ok, 3, "round {round}: exactly the affordable debits succeed: {results:?}");
        for r in &results {
            if let Err(e) = r {
                assert!(
                    matches!(e, CommerceError::NotPermitted(_) | CommerceError::Conflict(_)),
                    "got {e:?}"
                );
            }
        }
        let balance = db.x402_credits().get_balance_async(&payer, asset, network).await.unwrap();
        assert_eq!(balance, 0, "round {round}");
        let raw: i64 =
            sqlx::query_scalar("SELECT balance FROM x402_credit_accounts WHERE payer_address = $1")
                .bind(&payer)
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert!(raw >= 0, "balance must never be negative, got {raw}");
    }
}

/// Postgres twin of `sqlite_x402_concurrent_creates_for_one_cart_leave_exactly_one_claim`.
///
/// Two `create_intent` calls for one cart used to both pass the accessor's
/// read-then-create duplicate check and both insert, double-charging the cart.
/// The repository now re-checks inside its write transaction, and the
/// `cart_claim_key` unique index (migration 101) catches the interleaving that
/// beats the re-check.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_x402_concurrent_creates_for_one_cart_leave_exactly_one_claim() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.expect("connect + migrate"));
    for _ in 0..5 {
        let cart_id = Uuid::new_v4();
        let handles: Vec<_> = (0..4)
            .map(|index| {
                let db = Arc::clone(&db);
                tokio::spawn(async move {
                    db.x402_payment_intents()
                        .create_async(CreateX402PaymentIntent {
                            payer_address: format!(
                                "0xpayer-claim-{index}-{}",
                                Uuid::new_v4().as_simple()
                            ),
                            payee_address: "0xpayee-claim".to_string(),
                            amount: 1_000_000,
                            asset: X402Asset::Usdc,
                            network: X402Network::SetChain,
                            cart_id: Some(cart_id),
                            ..Default::default()
                        })
                        .await
                })
            })
            .collect();
        let mut winners = 0;
        for handle in handles {
            match handle.await.expect("join") {
                Ok(_) => winners += 1,
                Err(error) => assert!(
                    matches!(error, CommerceError::Conflict(_)),
                    "losing create must be a conflict, got {error:?}"
                ),
            }
        }
        assert_eq!(winners, 1, "exactly one create may claim a cart");
        let claiming = db.x402_payment_intents().for_cart_async(cart_id).await.expect("for_cart");
        assert_eq!(claiming.len(), 1, "one row per claimed cart");
    }
}

/// Leaving the claiming set releases the cart for a replacement intent.
#[tokio::test]
async fn postgres_x402_cancelling_a_claim_frees_the_cart_for_a_new_intent() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");
    let cart_id = Uuid::new_v4();
    let make = |payer: String| CreateX402PaymentIntent {
        payer_address: payer,
        payee_address: "0xpayee-claim".to_string(),
        amount: 1_000_000,
        asset: X402Asset::Usdc,
        network: X402Network::SetChain,
        cart_id: Some(cart_id),
        ..Default::default()
    };
    let intent = db
        .x402_payment_intents()
        .create_async(make(format!("0xpayer-a-{}", Uuid::new_v4().as_simple())))
        .await
        .expect("first intent claims the cart");
    let blocked = db
        .x402_payment_intents()
        .create_async(make(format!("0xpayer-b-{}", Uuid::new_v4().as_simple())))
        .await;
    match blocked {
        Err(CommerceError::Conflict(message)) => assert!(
            message.contains(&intent.id.to_string()),
            "conflict names the winner: {message}"
        ),
        other => panic!("second claim must conflict, got {other:?}"),
    }
    db.x402_payment_intents().cancel_async(intent.id).await.expect("cancel releases the claim");
    db.x402_payment_intents()
        .create_async(make(format!("0xpayer-c-{}", Uuid::new_v4().as_simple())))
        .await
        .expect("a released cart accepts a new intent");
}
