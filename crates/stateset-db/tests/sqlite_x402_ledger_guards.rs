//! SQLite guards for the x402 ledger: the `Batched` transition and the credit
//! ledger under concurrent debits.
#![cfg(feature = "sqlite")]

use stateset_core::{
    CommerceError, CreateX402PaymentIntent, X402Asset, X402CreditAdjustment, X402CreditDirection,
    X402CreditRepository, X402IntentStatus, X402Network, X402PaymentIntentRepository,
};
use stateset_db::SqliteDatabase;
use std::sync::{Arc, Barrier};

fn create_intent(db: &SqliteDatabase) -> stateset_core::X402PaymentIntent {
    db.x402_payment_intents()
        .create(CreateX402PaymentIntent {
            payer_address: "0xpayer-batched".to_string(),
            payee_address: "0xpayee-batched".to_string(),
            amount: 1_000_000,
            asset: X402Asset::Usdc,
            network: X402Network::SetChain,
            ..Default::default()
        })
        .expect("create intent")
}

type Transition =
    Box<dyn FnOnce(&SqliteDatabase) -> Result<X402IntentStatus, CommerceError> + Send>;

fn force_status(db: &SqliteDatabase, id: uuid::Uuid, status: X402IntentStatus) {
    db.conn()
        .expect("conn")
        .execute(
            "UPDATE x402_payment_intents SET status = ? WHERE id = ?",
            rusqlite::params![status.to_string(), id.to_string()],
        )
        .expect("force status");
}

// ---------------------------------------------------------------------------
// Batched: Sequenced -> Batched -> Settled, guarded and sweeper-exempt
// ---------------------------------------------------------------------------

#[test]
fn sqlite_x402_mark_batched_is_guarded_and_records_commitment() {
    let db = SqliteDatabase::in_memory().expect("db");
    let intent = create_intent(&db);
    let repo = db.x402_payment_intents();

    // Only Sequenced intents may be batched.
    let err = repo.mark_batched(intent.id, "0xroot", vec![]).expect_err("created cannot batch");
    assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");
    force_status(&db, intent.id, X402IntentStatus::Signed);
    let err = repo.mark_batched(intent.id, "0xroot", vec![]).expect_err("signed cannot batch");
    assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");

    force_status(&db, intent.id, X402IntentStatus::Sequenced);
    let err = repo.mark_batched(intent.id, "   ", vec![]).expect_err("root required");
    assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");

    let batched =
        repo.mark_batched(intent.id, "0xroot", vec!["0xaa".into(), "0xbb".into()]).expect("batch");
    assert_eq!(batched.status, X402IntentStatus::Batched);
    assert_eq!(batched.batch_merkle_root.as_deref(), Some("0xroot"));
    assert_eq!(batched.inclusion_proof, Some(vec!["0xaa".to_string(), "0xbb".to_string()]));

    // Batched is one-shot and cancel is refused, but settle/fail proceed.
    let err = repo.mark_batched(intent.id, "0xroot", vec![]).expect_err("already batched");
    assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");
    let err = repo.cancel(intent.id).expect_err("batched cannot be cancelled by payer");
    assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");

    // The sweeper leaves it alone even past valid_until.
    db.conn()
        .expect("conn")
        .execute(
            "UPDATE x402_payment_intents SET valid_until = 1 WHERE id = ?",
            [intent.id.to_string()],
        )
        .expect("force valid_until");
    assert_eq!(repo.expire_stale_intents().expect("sweep"), 0);
    assert_eq!(repo.get(intent.id).unwrap().unwrap().status, X402IntentStatus::Batched);
    // ...but a late settlement is still refused, like for Sequenced.
    let err = repo.mark_settled(intent.id, "0xlate", 1).expect_err("late settle");
    assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");
    db.conn()
        .expect("conn")
        .execute(
            "UPDATE x402_payment_intents SET valid_until = 9999999999 WHERE id = ?",
            [intent.id.to_string()],
        )
        .expect("restore valid_until");
    let settled = repo.mark_settled(intent.id, "0xtx-batched", 42).expect("settle from batched");
    assert_eq!(settled.status, X402IntentStatus::Settled);
}

#[test]
fn sqlite_x402_batch_racing_expire_and_fail_has_one_winner() {
    let db = Arc::new(SqliteDatabase::in_memory().expect("db"));
    for round in 0..15 {
        let intent = create_intent(&db);
        force_status(&db, intent.id, X402IntentStatus::Sequenced);
        let barrier = Arc::new(Barrier::new(3));
        let spawn = |f: Transition| {
            let (db, barrier) = (Arc::clone(&db), Arc::clone(&barrier));
            std::thread::spawn(move || {
                barrier.wait();
                f(&db)
            })
        };
        let id = intent.id;
        let handles = [
            spawn(Box::new(move |db| {
                db.x402_payment_intents().mark_batched(id, "0xroot", vec![]).map(|i| i.status)
            })),
            spawn(Box::new(move |db| db.x402_payment_intents().mark_expired(id).map(|i| i.status))),
            spawn(Box::new(move |db| {
                db.x402_payment_intents().mark_failed(id, "boom").map(|i| i.status)
            })),
        ];
        let results: Vec<_> = handles.into_iter().map(|h| h.join().expect("thread")).collect();
        let winners: Vec<_> = results.iter().filter_map(|r| r.as_ref().ok()).collect();
        assert_eq!(winners.len(), 1, "round {round}: exactly one transition wins: {results:?}");
        let stored = db.x402_payment_intents().get(id).unwrap().unwrap();
        assert_eq!(&stored.status, winners[0]);
    }
}

// ---------------------------------------------------------------------------
// Credit ledger: concurrent debits never overdraw
// ---------------------------------------------------------------------------

#[test]
fn sqlite_x402_credit_concurrent_debits_never_go_negative() {
    let db = Arc::new(SqliteDatabase::in_memory().expect("db"));
    let payer = "0xcredit-race";
    let (asset, network) = (X402Asset::Usdc, X402Network::SetChain);
    for round in 0..10 {
        // Balance covers exactly 3 debits of 100; 8 contenders race.
        let start = db.x402_credits().get_balance(payer, asset, network).expect("balance");
        db.x402_credits()
            .adjust_balance(X402CreditAdjustment {
                payer_address: payer.into(),
                asset,
                network,
                direction: X402CreditDirection::Credit,
                amount: 300 - start,
                reason: None,
                reference_id: None,
                metadata: None,
            })
            .expect("top up");
        let contenders = 8;
        let barrier = Arc::new(Barrier::new(contenders));
        let handles: Vec<_> = (0..contenders)
            .map(|_| {
                let (db, barrier) = (Arc::clone(&db), Arc::clone(&barrier));
                std::thread::spawn(move || {
                    barrier.wait();
                    db.x402_credits().adjust_balance(X402CreditAdjustment {
                        payer_address: payer.into(),
                        asset,
                        network,
                        direction: X402CreditDirection::Debit,
                        amount: 100,
                        reason: None,
                        reference_id: None,
                        metadata: None,
                    })
                })
            })
            .collect();
        let results: Vec<_> = handles.into_iter().map(|h| h.join().expect("thread")).collect();
        let ok = results.iter().filter(|r| r.is_ok()).count();
        assert_eq!(ok, 3, "round {round}: exactly the affordable debits succeed: {results:?}");
        for r in &results {
            if let Err(e) = r {
                assert!(
                    matches!(e, CommerceError::NotPermitted(_) | CommerceError::Conflict(_)),
                    "losers fail with insufficient balance/conflict, got {e:?}"
                );
            }
        }
        let balance = db.x402_credits().get_balance(payer, asset, network).expect("balance");
        assert_eq!(balance, 0, "round {round}");
        let raw: i64 = db
            .conn()
            .unwrap()
            .query_row(
                "SELECT balance FROM x402_credit_accounts WHERE payer_address = ?",
                [payer],
                |row| row.get(0),
            )
            .unwrap();
        assert!(raw >= 0, "balance must never be negative, got {raw}");
    }
}

#[test]
fn sqlite_x402_credit_debit_is_conditional_on_balance_row() {
    // A writer that mutates the balance underneath the repository between
    // its read and its UPDATE cannot make the ledger go negative: the UPDATE
    // carries `balance = <read> AND balance >= <amount>`.
    let db = SqliteDatabase::in_memory().expect("db");
    let payer = "0xcredit-cond";
    let (asset, network) = (X402Asset::Usdc, X402Network::SetChain);
    db.x402_credits()
        .adjust_balance(X402CreditAdjustment {
            payer_address: payer.into(),
            asset,
            network,
            direction: X402CreditDirection::Credit,
            amount: 50,
            reason: None,
            reference_id: None,
            metadata: None,
        })
        .expect("credit");
    let err = db
        .x402_credits()
        .adjust_balance(X402CreditAdjustment {
            payer_address: payer.into(),
            asset,
            network,
            direction: X402CreditDirection::Debit,
            amount: 51,
            reason: None,
            reference_id: None,
            metadata: None,
        })
        .expect_err("overdraw");
    assert!(matches!(err, CommerceError::NotPermitted(_)), "{err:?}");
    assert_eq!(db.x402_credits().get_balance(payer, asset, network).unwrap(), 50);
}
