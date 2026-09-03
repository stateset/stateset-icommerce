//! Regression tests for the Postgres credit ledger (`credit_transactions`).
//!
//! `record_transaction_async` used to read `credit_accounts.current_balance` on
//! the pool (autocommit, no row lock), then INSERT the ledger row on a second
//! pooled connection — with no `begin()` anywhere in the method. Two problems
//! followed:
//!
//! * **Torn running balance.** A concurrent payment or charge could commit
//!   between the read and the insert, so the row was stamped with a balance the
//!   account no longer held. The ledger then overstated the receivable with no
//!   reconciliation path.
//! * **Phantom rows for a missing account.** The read used
//!   `.unwrap_or(Decimal::ZERO)`, so recording against a customer that has no
//!   `credit_accounts` row computed a running balance from an invented zero
//!   instead of failing. SQLite's `record_transaction` uses `query_row` inside
//!   `with_immediate_transaction` and errors `NotFound`, so the two backends
//!   disagreed on the same call.
//!
//! Both are now closed: one transaction, `SELECT ... FOR UPDATE` on the account
//! row, and `NotFound` when there is no account.
//!
//! `list_holds_async`'s `hold_type` filter is pinned here too — the SQLite
//! backend silently dropped it, so this is the parity anchor.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{
    CommerceError, CreateCreditAccount, CreditHoldFilter, CreditHoldType, CreditTransactionFilter,
    CreditTransactionType, CustomerId, PlaceCreditHold, RecordCreditTransaction,
};
use stateset_db::PostgresDatabase;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

async fn connect() -> Option<Arc<PostgresDatabase>> {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return None;
    };
    Some(Arc::new(PostgresDatabase::connect(&url).await.expect("connect + migrate")))
}

/// A fresh credit account with a $1,000,000 limit and `balance` already drawn
/// against it.
async fn seed_account(db: &PostgresDatabase, balance: Decimal) -> CustomerId {
    let customer = CustomerId::new();
    db.credit()
        .create_credit_account_async(CreateCreditAccount {
            customer_id: customer,
            credit_limit: dec!(1000000),
            ..Default::default()
        })
        .await
        .expect("create credit account");

    if balance > Decimal::ZERO {
        db.credit()
            .charge_credit_async(Uuid::from(customer), Uuid::new_v4(), balance)
            .await
            .expect("draw the opening balance");
    }
    customer
}

fn payment(customer: CustomerId, amount: Decimal) -> RecordCreditTransaction {
    RecordCreditTransaction {
        customer_id: customer,
        transaction_type: CreditTransactionType::Payment,
        amount,
        reference_type: Some("test".to_string()),
        reference_id: None,
        notes: None,
    }
}

// ------------------------------------------------------------------ P1

/// The audit's exact scenario, made deterministic by holding the lock from the
/// test's own transaction: a $4,000 payment is mid-flight against a $10,000
/// balance — the account row is locked and the new balance written, but not yet
/// committed. A standalone `record_transaction(Payment, $4,000)` that reads
/// without the lock sees the stale $10,000 and stamps `running_balance = 6,000`
/// even though the true post-payment state is $2,000, overstating the
/// receivable by $4,000.
///
/// With the account row taken `FOR UPDATE` the ledger write waits for the
/// payment to commit and then stamps the balance that actually holds.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_record_transaction_waits_for_a_concurrent_balance_writer() {
    let Some(db) = connect().await else { return };
    let customer = seed_account(&db, dec!(10000)).await;

    // Stand in for `apply_payment_async` mid-transaction: row locked, balance
    // written, not yet committed.
    let mut payer = db.pool().begin().await.expect("begin");
    sqlx::query("SELECT current_balance FROM credit_accounts WHERE customer_id = $1 FOR UPDATE")
        .bind(Uuid::from(customer))
        .fetch_one(payer.as_mut())
        .await
        .expect("lock the account");
    sqlx::query(
        "UPDATE credit_accounts SET current_balance = current_balance - $1, updated_at = NOW()
         WHERE customer_id = $2",
    )
    .bind(dec!(4000))
    .bind(Uuid::from(customer))
    .execute(payer.as_mut())
    .await
    .expect("apply the in-flight payment");

    let ledger = Arc::clone(&db);
    let recording = tokio::spawn(async move {
        ledger.credit().record_transaction_async(payment(customer, dec!(4000))).await
    });

    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(
        !recording.is_finished(),
        "record_transaction must take the account row FOR UPDATE and wait for the in-flight \
         payment; it completed against the stale balance"
    );

    payer.commit().await.expect("commit the payment");

    let recorded = recording.await.expect("join").expect("record the ledger row");
    assert_eq!(
        recorded.running_balance,
        dec!(2000),
        "the ledger must be stamped with the balance that survived the concurrent payment"
    );

    let rows = db
        .credit()
        .list_transactions_async(CreditTransactionFilter {
            customer_id: Some(customer),
            transaction_type: Some(CreditTransactionType::Payment),
            ..Default::default()
        })
        .await
        .expect("list transactions");
    assert_eq!(rows.len(), 1, "exactly one standalone payment row: {rows:?}");
    assert_eq!(rows[0].running_balance, dec!(2000), "the persisted row must agree: {rows:?}");
}

/// Recording against a customer that has no credit account must be refused, not
/// silently booked at a zero running balance. SQLite already errors `NotFound`;
/// Postgres masked the missing account with `unwrap_or(Decimal::ZERO)`.
#[tokio::test]
async fn postgres_record_transaction_rejects_a_missing_account() {
    let Some(db) = connect().await else { return };
    let customer = CustomerId::new();

    let err = db
        .credit()
        .record_transaction_async(payment(customer, dec!(4000)))
        .await
        .expect_err("a customer with no credit account cannot have a ledger row");
    assert!(matches!(err, CommerceError::NotFound), "got {err:?}");

    let (rows,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM credit_transactions WHERE customer_id = $1")
            .bind(Uuid::from(customer))
            .fetch_one(db.pool())
            .await
            .expect("count");
    assert_eq!(rows, 0, "no phantom ledger entry may be written");
}

/// Eight real $500 payments race eight standalone $250 ledger rows against a
/// $10,000 balance. The payments must not lose an update, and every standalone
/// row must be stamped from a balance the account genuinely held — never from a
/// value torn between the read and the insert.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_concurrent_ledger_writes_never_stamp_a_torn_balance() {
    let Some(db) = connect().await else { return };
    let customer = seed_account(&db, dec!(10000)).await;

    let payments = 8u32;
    let mut handles = Vec::new();
    for _ in 0..payments {
        let payer = Arc::clone(&db);
        handles.push(tokio::spawn(async move {
            payer
                .credit()
                .apply_payment_async(Uuid::from(customer), dec!(500), None)
                .await
                .map(|_| ())
        }));
        let recorder = Arc::clone(&db);
        handles.push(tokio::spawn(async move {
            recorder
                .credit()
                .record_transaction_async(payment(customer, dec!(250)))
                .await
                .map(|_| ())
        }));
    }
    for handle in handles {
        handle.await.expect("join").expect("concurrent credit operation");
    }

    let account = db
        .credit()
        .get_credit_account_by_customer_async(Uuid::from(customer))
        .await
        .expect("get account")
        .expect("account exists");
    assert_eq!(
        account.current_balance,
        dec!(10000) - dec!(500) * Decimal::from(payments),
        "no payment may be lost"
    );

    // Every legal balance the account could have held when a standalone row was
    // written, less that row's $250.
    let legal: Vec<Decimal> =
        (0..=payments).map(|k| dec!(10000) - dec!(500) * Decimal::from(k) - dec!(250)).collect();
    let standalone = db
        .credit()
        .list_transactions_async(CreditTransactionFilter {
            customer_id: Some(customer),
            transaction_type: Some(CreditTransactionType::Payment),
            ..Default::default()
        })
        .await
        .expect("list transactions");
    let standalone: Vec<_> = standalone.into_iter().filter(|t| t.amount == dec!(250)).collect();
    assert_eq!(standalone.len(), payments as usize, "one row per standalone call: {standalone:?}");
    for row in &standalone {
        assert!(
            legal.contains(&row.running_balance),
            "running balance {} was never a state of the account (legal: {legal:?})",
            row.running_balance
        );
    }
}

// ------------------------------------------------------------------ P5

/// Listing holds by `hold_type` must return only that type. The Postgres
/// backend already applies the predicate; this is the parity anchor for the
/// SQLite backend, which dropped it and showed an operator filtering the
/// high-risk queue every hold of every type.
#[tokio::test]
async fn postgres_list_holds_filters_by_hold_type() {
    let Some(db) = connect().await else { return };
    let customer = seed_account(&db, Decimal::ZERO).await;

    for hold_type in [CreditHoldType::HighRisk, CreditHoldType::OverLimit, CreditHoldType::Manual] {
        db.credit()
            .place_hold_async(PlaceCreditHold {
                customer_id: customer,
                order_id: None,
                hold_type,
                hold_amount: dec!(10),
                reason: format!("{hold_type}"),
                placed_by: None,
            })
            .await
            .expect("place hold");
    }

    let high_risk = db
        .credit()
        .list_holds_async(CreditHoldFilter {
            customer_id: Some(customer),
            hold_type: Some(CreditHoldType::HighRisk),
            ..Default::default()
        })
        .await
        .expect("list holds");
    assert_eq!(high_risk.len(), 1, "only the high-risk hold may match: {high_risk:?}");
    assert_eq!(high_risk[0].hold_type, CreditHoldType::HighRisk);

    let all = db
        .credit()
        .list_holds_async(CreditHoldFilter { customer_id: Some(customer), ..Default::default() })
        .await
        .expect("list holds");
    assert_eq!(all.len(), 3, "an unfiltered listing still sees every hold: {all:?}");
}

/// `offset` must skip rows on the Postgres backend — the parity anchor for the
/// SQLite fix, which ignored the field entirely.
#[tokio::test]
async fn postgres_list_holds_applies_offset() {
    let Some(db) = connect().await else { return };
    let customer = seed_account(&db, Decimal::ZERO).await;

    for i in 0..3 {
        db.credit()
            .place_hold_async(PlaceCreditHold {
                customer_id: customer,
                order_id: None,
                hold_type: CreditHoldType::Manual,
                hold_amount: Decimal::from(i + 1),
                reason: format!("hold {i}"),
                placed_by: None,
            })
            .await
            .expect("place hold");
    }

    let tail = db
        .credit()
        .list_holds_async(CreditHoldFilter {
            customer_id: Some(customer),
            offset: Some(2),
            ..Default::default()
        })
        .await
        .expect("list holds");
    assert_eq!(tail.len(), 1, "offset 2 of 3 holds leaves one row: {tail:?}");
}
