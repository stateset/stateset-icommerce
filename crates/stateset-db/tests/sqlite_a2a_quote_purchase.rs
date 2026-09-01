//! Regression tests for the A2A quote double-purchase race on SQLite.
//!
//! `create_purchase` read and validated the quote on a pooled connection and
//! then, in a separate IMMEDIATE transaction, inserted the purchase and flipped
//! the quote to `purchased` with an unconditional `UPDATE ... WHERE id`. Two
//! buyers racing on the same quote both passed the status check and both got a
//! purchase. Consuming a quote is now a conditional transition executed on the
//! same transaction as the purchase insert.

#![cfg(feature = "sqlite")]

use rust_decimal_macros::dec;
use stateset_core::{
    A2ACommerceRepository, A2APurchaseFilter, CommerceError, CreateA2APurchase, CreateA2AQuote,
    ItemAvailability, QuoteStatus, QuotedItem,
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

#[test]
fn sqlite_a2a_second_purchase_of_consumed_quote_is_refused() {
    let db = SqliteDatabase::in_memory().expect("in-memory sqlite");
    let (buyer, seller) = (Uuid::new_v4(), Uuid::new_v4());
    let quote_id = quoted_quote(&db, buyer, seller);
    let repo = db.a2a_purchases();

    let first = repo.create_purchase(purchase_input(quote_id, buyer, seller)).expect("first");
    let quote = db.a2a_quotes().get_quote(quote_id).expect("get").expect("exists");
    assert_eq!(quote.status, QuoteStatus::Purchased);
    assert_eq!(quote.purchase_id, Some(first.id));

    let err = repo
        .create_purchase(purchase_input(quote_id, buyer, seller))
        .expect_err("second purchase of a consumed quote must be refused");
    assert!(
        matches!(err, CommerceError::ValidationError(_) | CommerceError::Conflict(_)),
        "got {err:?}"
    );
    let quote = db.a2a_quotes().get_quote(quote_id).expect("get").expect("exists");
    assert_eq!(quote.purchase_id, Some(first.id), "quote must stay linked to the first purchase");
}

#[test]
fn sqlite_a2a_concurrent_purchases_consume_quote_exactly_once() {
    let db = Arc::new(SqliteDatabase::in_memory().expect("in-memory sqlite"));
    for round in 0..20 {
        let (buyer, seller) = (Uuid::new_v4(), Uuid::new_v4());
        let quote_id = quoted_quote(&db, buyer, seller);

        let contenders = 4;
        let barrier = Arc::new(Barrier::new(contenders));
        let handles: Vec<_> = (0..contenders)
            .map(|_| {
                let db = Arc::clone(&db);
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    let repo = db.a2a_purchases();
                    barrier.wait();
                    repo.create_purchase(purchase_input(quote_id, buyer, seller))
                })
            })
            .collect();
        let results: Vec<_> = handles.into_iter().map(|h| h.join().expect("thread")).collect();
        let successes = results.iter().filter(|r| r.is_ok()).count();
        assert_eq!(successes, 1, "round {round}: exactly one buyer may win: {results:?}");

        let winner = results.into_iter().find_map(Result::ok).expect("winner");
        let quote = db.a2a_quotes().get_quote(quote_id).expect("get").expect("exists");
        assert_eq!(quote.status, QuoteStatus::Purchased);
        assert_eq!(quote.purchase_id, Some(winner.id));

        let linked = db
            .a2a_purchases()
            .list_purchases(A2APurchaseFilter { buyer_agent_id: Some(buyer), ..Default::default() })
            .expect("list")
            .into_iter()
            .filter(|p| p.quote_id == Some(quote_id))
            .count();
        assert_eq!(linked, 1, "round {round}: only one purchase row may reference the quote");
    }
}
