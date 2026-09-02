//! Regression tests for A2A purchase/quote status transitions on SQLite.
//!
//! `update_purchase_status`, `update_quote_status` and `confirm_delivery` read
//! the row on a pooled connection, validated the transition in memory, then
//! issued an unconditional `UPDATE ... WHERE id = ?` on another connection.
//! A seller's `Completed` racing a buyer's `Cancelled` both validated against
//! the same `Shipped` snapshot and the last writer won; `confirm_delivery` was
//! repeatable (rewriting signature and rating) and, racing a cancel, could
//! resurrect a cancelled purchase to `Completed`. Every transition now runs
//! inside one `BEGIN IMMEDIATE` transaction with a `status = <observed>`
//! predicate whose row count is checked.

#![cfg(feature = "sqlite")]

use rust_decimal_macros::dec;
use stateset_core::{
    A2ACommerceRepository, CommerceError, CreateA2APurchase, CreateA2AQuote, ItemAvailability,
    PurchaseStatus, QuoteStatus, QuotedItem,
};
use stateset_db::SqliteDatabase;
use std::sync::{Arc, Barrier};
use uuid::Uuid;

fn item() -> QuotedItem {
    QuotedItem {
        line_number: 1,
        sku: Some("SKU-1".into()),
        name: "Widget".into(),
        quantity: 1,
        unit_price: dec!(10),
        total: dec!(10),
        availability: ItemAvailability::InStock,
        lead_time_days: None,
    }
}

fn quoted_quote(db: &SqliteDatabase, buyer: Uuid, seller: Uuid) -> Uuid {
    let repo = db.a2a_quotes();
    let quote = repo
        .create_quote(CreateA2AQuote {
            buyer_agent_id: buyer,
            seller_agent_id: seller,
            items: vec![item()],
            subtotal: dec!(10),
            total: dec!(10),
            valid_until: chrono::Utc::now() + chrono::Duration::hours(1),
            ..Default::default()
        })
        .expect("create quote");
    repo.update_quote_status(quote.id, QuoteStatus::Quoted).expect("quote -> quoted");
    quote.id
}

/// A purchase (no quote) walked to `Shipped` through the public transitions.
fn shipped_purchase(db: &SqliteDatabase) -> Uuid {
    let repo = db.a2a_purchases();
    let purchase = repo
        .create_purchase(CreateA2APurchase {
            buyer_agent_id: Uuid::new_v4(),
            seller_agent_id: Uuid::new_v4(),
            items: vec![item()],
            total: dec!(10),
            ..Default::default()
        })
        .expect("create purchase");
    for next in [PurchaseStatus::PaymentPending, PurchaseStatus::Paid, PurchaseStatus::Shipped] {
        repo.update_purchase_status(purchase.id, next).expect("walk to shipped");
    }
    purchase.id
}

const fn is_status_error(err: &CommerceError) -> bool {
    matches!(err, CommerceError::ValidationError(_) | CommerceError::Conflict(_))
}

// ---------------------------------------------------------------------------
// D1: purchase / quote status transitions are atomic
// ---------------------------------------------------------------------------

#[test]
fn sqlite_a2a_completed_vs_cancelled_exactly_one_wins() {
    let db = Arc::new(SqliteDatabase::in_memory().expect("in-memory sqlite"));
    for round in 0..20 {
        let purchase_id = shipped_purchase(&db);
        let barrier = Arc::new(Barrier::new(2));
        let complete = {
            let (db, barrier) = (Arc::clone(&db), Arc::clone(&barrier));
            std::thread::spawn(move || {
                barrier.wait();
                db.a2a_purchases()
                    .update_purchase_status(purchase_id, PurchaseStatus::Completed)
                    .map(|p| p.status)
            })
        };
        let cancel = {
            let (db, barrier) = (Arc::clone(&db), Arc::clone(&barrier));
            std::thread::spawn(move || {
                barrier.wait();
                db.a2a_purchases()
                    .update_purchase_status(purchase_id, PurchaseStatus::Cancelled)
                    .map(|p| p.status)
            })
        };
        let results = [complete.join().expect("thread"), cancel.join().expect("thread")];
        let winners: Vec<_> = results.iter().filter_map(|r| r.as_ref().ok()).collect();
        assert_eq!(winners.len(), 1, "round {round}: exactly one transition may win: {results:?}");
        for r in &results {
            if let Err(e) = r {
                assert!(is_status_error(e), "loser must fail with a status error, got {e:?}");
            }
        }
        let stored = db.a2a_purchases().get_purchase(purchase_id).expect("get").expect("exists");
        assert_eq!(&stored.status, winners[0], "round {round}: stored status must be the winner");
    }
}

#[test]
fn sqlite_a2a_quote_accept_vs_reject_exactly_one_wins() {
    let db = Arc::new(SqliteDatabase::in_memory().expect("in-memory sqlite"));
    for round in 0..20 {
        let quote_id = quoted_quote(&db, Uuid::new_v4(), Uuid::new_v4());
        let barrier = Arc::new(Barrier::new(2));
        let spawn = |target: QuoteStatus| {
            let (db, barrier) = (Arc::clone(&db), Arc::clone(&barrier));
            std::thread::spawn(move || {
                barrier.wait();
                db.a2a_quotes().update_quote_status(quote_id, target).map(|q| q.status)
            })
        };
        let accept = spawn(QuoteStatus::Accepted);
        let reject = spawn(QuoteStatus::Rejected);
        let results = [accept.join().expect("thread"), reject.join().expect("thread")];
        let winners: Vec<_> = results.iter().filter_map(|r| r.as_ref().ok()).collect();
        assert_eq!(winners.len(), 1, "round {round}: exactly one transition may win: {results:?}");
        let stored = db.a2a_quotes().get_quote(quote_id).expect("get").expect("exists");
        assert_eq!(&stored.status, winners[0], "round {round}: stored status must be the winner");
    }
}

// ---------------------------------------------------------------------------
// D2: confirm_delivery is a one-shot Shipped/Delivered -> Completed transition
// ---------------------------------------------------------------------------

#[test]
fn sqlite_a2a_second_confirm_delivery_is_refused() {
    let db = SqliteDatabase::in_memory().expect("in-memory sqlite");
    let purchase_id = shipped_purchase(&db);
    let repo = db.a2a_purchases();

    let first = repo
        .confirm_delivery(purchase_id, "sig-first", Some(5), Some("great"))
        .expect("first confirm");
    assert_eq!(first.status, PurchaseStatus::Completed);
    assert_eq!(first.delivery_confirmation_signature.as_deref(), Some("sig-first"));
    assert_eq!(first.buyer_rating, Some(5));

    let err = repo
        .confirm_delivery(purchase_id, "sig-second", Some(1), Some("changed my mind"))
        .expect_err("second confirm must be refused");
    assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");

    let after = repo.get_purchase(purchase_id).expect("get").expect("exists");
    assert_eq!(after.delivery_confirmation_signature.as_deref(), Some("sig-first"));
    assert_eq!(after.buyer_rating, Some(5));
    assert_eq!(after.buyer_feedback.as_deref(), Some("great"));
    assert_eq!(after.delivery_confirmed_at, first.delivery_confirmed_at);
}

#[test]
fn sqlite_a2a_confirm_delivery_requires_shipped_or_delivered() {
    let db = SqliteDatabase::in_memory().expect("in-memory sqlite");
    let repo = db.a2a_purchases();
    let purchase = repo
        .create_purchase(CreateA2APurchase {
            buyer_agent_id: Uuid::new_v4(),
            seller_agent_id: Uuid::new_v4(),
            items: vec![item()],
            total: dec!(10),
            ..Default::default()
        })
        .expect("create purchase");
    repo.update_purchase_status(purchase.id, PurchaseStatus::PaymentPending).expect("pending");
    repo.update_purchase_status(purchase.id, PurchaseStatus::Paid).expect("paid");
    let err = repo.confirm_delivery(purchase.id, "sig", None, None).expect_err("paid != shipped");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");

    repo.update_purchase_status(purchase.id, PurchaseStatus::Shipped).expect("shipped");
    repo.update_purchase_status(purchase.id, PurchaseStatus::Delivered).expect("delivered");
    let confirmed = repo.confirm_delivery(purchase.id, "sig", Some(4), None).expect("confirm");
    assert_eq!(confirmed.status, PurchaseStatus::Completed);
}

#[test]
fn sqlite_a2a_confirm_delivery_racing_cancel_cannot_resurrect() {
    let db = Arc::new(SqliteDatabase::in_memory().expect("in-memory sqlite"));
    for round in 0..20 {
        let purchase_id = shipped_purchase(&db);
        let barrier = Arc::new(Barrier::new(2));
        let confirm = {
            let (db, barrier) = (Arc::clone(&db), Arc::clone(&barrier));
            std::thread::spawn(move || {
                barrier.wait();
                db.a2a_purchases()
                    .confirm_delivery(purchase_id, "sig", Some(5), None)
                    .map(|p| p.status)
            })
        };
        let cancel = {
            let (db, barrier) = (Arc::clone(&db), Arc::clone(&barrier));
            std::thread::spawn(move || {
                barrier.wait();
                db.a2a_purchases()
                    .update_purchase_status(purchase_id, PurchaseStatus::Cancelled)
                    .map(|p| p.status)
            })
        };
        let results = [confirm.join().expect("thread"), cancel.join().expect("thread")];
        let winners: Vec<_> = results.iter().filter_map(|r| r.as_ref().ok()).collect();
        assert_eq!(winners.len(), 1, "round {round}: exactly one may win: {results:?}");
        let stored = db.a2a_purchases().get_purchase(purchase_id).expect("get").expect("exists");
        assert_eq!(&stored.status, winners[0], "round {round}: stored status must be the winner");
        if stored.status == PurchaseStatus::Cancelled {
            assert!(stored.delivery_confirmation_signature.is_none());
        }
    }
}

// ---------------------------------------------------------------------------
// D8: the quote transition table agrees with create_purchase
// ---------------------------------------------------------------------------

#[test]
fn sqlite_a2a_quoted_quote_may_move_to_purchased() {
    let db = SqliteDatabase::in_memory().expect("in-memory sqlite");
    let quote_id = quoted_quote(&db, Uuid::new_v4(), Uuid::new_v4());
    let purchased = db
        .a2a_quotes()
        .update_quote_status(quote_id, QuoteStatus::Purchased)
        .expect("quoted -> purchased mirrors what create_purchase does");
    assert_eq!(purchased.status, QuoteStatus::Purchased);
}
