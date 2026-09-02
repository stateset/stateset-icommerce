//! Regression test for the Postgres A2A quote double-purchase race.
//!
//! `create_purchase_async` validated the quote with a plain `SELECT` on the
//! pool (no transaction, no `FOR UPDATE`) and then, in a separate transaction,
//! inserted the purchase and flipped the quote to `purchased` with an
//! unconditional `UPDATE ... WHERE id`. Two buyers racing on one quote both
//! passed the check and both got a purchase. The quote is now locked with
//! `FOR UPDATE` inside the purchase transaction and consumed with a conditional
//! UPDATE whose row count is checked.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use rust_decimal_macros::dec;
use stateset_core::{
    A2APurchaseFilter, CommerceError, CreateA2APurchase, CreateA2AQuote, ItemAvailability,
    QuoteStatus, QuotedItem,
};
use stateset_db::PostgresDatabase;
use std::sync::Arc;
use uuid::Uuid;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

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

async fn quoted_quote(db: &PostgresDatabase, buyer: Uuid, seller: Uuid) -> Uuid {
    let repo = db.a2a_quotes();
    let quote = repo
        .create_quote_async(CreateA2AQuote {
            buyer_agent_id: buyer,
            seller_agent_id: seller,
            items: vec![item()],
            subtotal: dec!(10),
            total: dec!(10),
            valid_until: chrono::Utc::now() + chrono::Duration::hours(1),
            ..Default::default()
        })
        .await
        .expect("create quote");
    repo.update_quote_status_async(quote.id, QuoteStatus::Quoted).await.expect("quote -> quoted");
    quote.id
}

fn purchase_input(quote_id: Uuid, buyer: Uuid, seller: Uuid) -> CreateA2APurchase {
    CreateA2APurchase {
        buyer_agent_id: buyer,
        seller_agent_id: seller,
        quote_id: Some(quote_id),
        items: vec![item()],
        total: dec!(10),
        ..Default::default()
    }
}

/// Sequential guard: a consumed quote refuses a second purchase.
#[tokio::test]
async fn postgres_a2a_second_purchase_of_consumed_quote_is_refused() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");
    let (buyer, seller) = (Uuid::new_v4(), Uuid::new_v4());
    let quote_id = quoted_quote(&db, buyer, seller).await;

    let first = db
        .a2a_purchases()
        .create_purchase_async(purchase_input(quote_id, buyer, seller))
        .await
        .expect("first purchase");
    let err = db
        .a2a_purchases()
        .create_purchase_async(purchase_input(quote_id, buyer, seller))
        .await
        .expect_err("second purchase must be refused");
    assert!(
        matches!(err, CommerceError::ValidationError(_) | CommerceError::Conflict(_)),
        "got {err:?}"
    );
    let quote = db.a2a_quotes().get_quote_async(quote_id).await.expect("get").expect("exists");
    assert_eq!(quote.status, QuoteStatus::Purchased);
    assert_eq!(quote.purchase_id, Some(first.id));
}

/// Many buyers racing on one quote: exactly one purchase wins.
#[tokio::test]
async fn postgres_a2a_concurrent_purchases_consume_quote_exactly_once() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.expect("connect + migrate"));
    let (buyer, seller) = (Uuid::new_v4(), Uuid::new_v4());
    let quote_id = quoted_quote(&db, buyer, seller).await;

    let contenders = 16;
    let barrier = Arc::new(tokio::sync::Barrier::new(contenders));
    let mut handles = Vec::with_capacity(contenders);
    for _ in 0..contenders {
        let db = Arc::clone(&db);
        let barrier = Arc::clone(&barrier);
        handles.push(tokio::spawn(async move {
            barrier.wait().await;
            db.a2a_purchases().create_purchase_async(purchase_input(quote_id, buyer, seller)).await
        }));
    }
    let mut results = Vec::with_capacity(contenders);
    for h in handles {
        results.push(h.await.expect("task"));
    }
    let successes = results.iter().filter(|r| r.is_ok()).count();
    assert_eq!(successes, 1, "exactly one buyer may win: {results:?}");

    let winner = results.into_iter().find_map(Result::ok).expect("winner");
    let quote = db.a2a_quotes().get_quote_async(quote_id).await.expect("get").expect("exists");
    assert_eq!(quote.status, QuoteStatus::Purchased);
    assert_eq!(quote.purchase_id, Some(winner.id));

    let linked = db
        .a2a_purchases()
        .list_purchases_async(A2APurchaseFilter {
            buyer_agent_id: Some(buyer),
            ..Default::default()
        })
        .await
        .expect("list")
        .into_iter()
        .filter(|p| p.quote_id == Some(quote_id))
        .count();
    assert_eq!(linked, 1, "only one purchase row may reference the quote");
}

// ---------------------------------------------------------------------------
// Purchase / quote status transitions are atomic (row lock + conditional
// UPDATE); confirm_delivery is one-shot.
// ---------------------------------------------------------------------------

use stateset_core::PurchaseStatus;

/// A purchase (no quote) walked to `Shipped` through the public transitions.
async fn shipped_purchase(db: &PostgresDatabase) -> Uuid {
    let repo = db.a2a_purchases();
    let purchase = repo
        .create_purchase_async(CreateA2APurchase {
            buyer_agent_id: Uuid::new_v4(),
            seller_agent_id: Uuid::new_v4(),
            items: vec![item()],
            total: dec!(10),
            ..Default::default()
        })
        .await
        .expect("create purchase");
    for next in [PurchaseStatus::PaymentPending, PurchaseStatus::Paid, PurchaseStatus::Shipped] {
        repo.update_purchase_status_async(purchase.id, next).await.expect("walk to shipped");
    }
    purchase.id
}

#[tokio::test]
async fn postgres_a2a_completed_vs_cancelled_exactly_one_wins() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.expect("connect + migrate"));
    for round in 0..10 {
        let purchase_id = shipped_purchase(&db).await;
        let barrier = Arc::new(tokio::sync::Barrier::new(2));
        let spawn = |target: PurchaseStatus| {
            let (db, barrier) = (Arc::clone(&db), Arc::clone(&barrier));
            tokio::spawn(async move {
                barrier.wait().await;
                db.a2a_purchases()
                    .update_purchase_status_async(purchase_id, target)
                    .await
                    .map(|p| p.status)
            })
        };
        let complete = spawn(PurchaseStatus::Completed);
        let cancel = spawn(PurchaseStatus::Cancelled);
        let results = [complete.await.expect("task"), cancel.await.expect("task")];
        let winners: Vec<_> = results.iter().filter_map(|r| r.as_ref().ok()).collect();
        assert_eq!(winners.len(), 1, "round {round}: exactly one transition may win: {results:?}");
        for r in &results {
            if let Err(e) = r {
                assert!(
                    matches!(e, CommerceError::ValidationError(_) | CommerceError::Conflict(_)),
                    "loser must fail with a status error, got {e:?}"
                );
            }
        }
        let stored =
            db.a2a_purchases().get_purchase_async(purchase_id).await.expect("get").expect("exists");
        assert_eq!(&stored.status, winners[0], "round {round}: stored status must be the winner");
    }
}

#[tokio::test]
async fn postgres_a2a_quote_accept_vs_reject_exactly_one_wins() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.expect("connect + migrate"));
    for round in 0..10 {
        let quote_id = quoted_quote(&db, Uuid::new_v4(), Uuid::new_v4()).await;
        let barrier = Arc::new(tokio::sync::Barrier::new(2));
        let spawn = |target: QuoteStatus| {
            let (db, barrier) = (Arc::clone(&db), Arc::clone(&barrier));
            tokio::spawn(async move {
                barrier.wait().await;
                db.a2a_quotes().update_quote_status_async(quote_id, target).await.map(|q| q.status)
            })
        };
        let accept = spawn(QuoteStatus::Accepted);
        let reject = spawn(QuoteStatus::Rejected);
        let results = [accept.await.expect("task"), reject.await.expect("task")];
        let winners: Vec<_> = results.iter().filter_map(|r| r.as_ref().ok()).collect();
        assert_eq!(winners.len(), 1, "round {round}: exactly one transition may win: {results:?}");
        let stored = db.a2a_quotes().get_quote_async(quote_id).await.expect("get").expect("exists");
        assert_eq!(&stored.status, winners[0], "round {round}: stored status must be the winner");
    }
}

#[tokio::test]
async fn postgres_a2a_second_confirm_delivery_is_refused() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");
    let purchase_id = shipped_purchase(&db).await;
    let repo = db.a2a_purchases();

    let first = repo
        .confirm_delivery_async(purchase_id, "sig-first", Some(5), Some("great"))
        .await
        .expect("first confirm");
    assert_eq!(first.status, PurchaseStatus::Completed);

    let err = repo
        .confirm_delivery_async(purchase_id, "sig-second", Some(1), Some("changed my mind"))
        .await
        .expect_err("second confirm must be refused");
    assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");

    let after = repo.get_purchase_async(purchase_id).await.expect("get").expect("exists");
    assert_eq!(after.delivery_confirmation_signature.as_deref(), Some("sig-first"));
    assert_eq!(after.buyer_rating, Some(5));
    assert_eq!(after.buyer_feedback.as_deref(), Some("great"));
    assert_eq!(after.delivery_confirmed_at, first.delivery_confirmed_at);
}

#[tokio::test]
async fn postgres_a2a_confirm_delivery_racing_cancel_cannot_resurrect() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.expect("connect + migrate"));
    for round in 0..10 {
        let purchase_id = shipped_purchase(&db).await;
        let barrier = Arc::new(tokio::sync::Barrier::new(2));
        let confirm = {
            let (db, barrier) = (Arc::clone(&db), Arc::clone(&barrier));
            tokio::spawn(async move {
                barrier.wait().await;
                db.a2a_purchases()
                    .confirm_delivery_async(purchase_id, "sig", Some(5), None)
                    .await
                    .map(|p| p.status)
            })
        };
        let cancel = {
            let (db, barrier) = (Arc::clone(&db), Arc::clone(&barrier));
            tokio::spawn(async move {
                barrier.wait().await;
                db.a2a_purchases()
                    .update_purchase_status_async(purchase_id, PurchaseStatus::Cancelled)
                    .await
                    .map(|p| p.status)
            })
        };
        let results = [confirm.await.expect("task"), cancel.await.expect("task")];
        let winners: Vec<_> = results.iter().filter_map(|r| r.as_ref().ok()).collect();
        assert_eq!(winners.len(), 1, "round {round}: exactly one may win: {results:?}");
        let stored =
            db.a2a_purchases().get_purchase_async(purchase_id).await.expect("get").expect("exists");
        assert_eq!(&stored.status, winners[0], "round {round}: stored status must be the winner");
        if stored.status == PurchaseStatus::Cancelled {
            assert!(stored.delivery_confirmation_signature.is_none());
        }
    }
}

#[tokio::test]
async fn postgres_a2a_quoted_quote_may_move_to_purchased() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");
    let quote_id = quoted_quote(&db, Uuid::new_v4(), Uuid::new_v4()).await;
    let purchased = db
        .a2a_quotes()
        .update_quote_status_async(quote_id, QuoteStatus::Purchased)
        .await
        .expect("quoted -> purchased mirrors what create_purchase does");
    assert_eq!(purchased.status, QuoteStatus::Purchased);
}
