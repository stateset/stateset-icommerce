#![cfg(feature = "sqlite")]

//! Regression: SQLite `x402_payment_intents::count` applied only
//! payer/payee/status/network/asset, dropping `order_id`, `batch_id`, `from_date`,
//! and `to_date` — all of which `list` (and Postgres) apply. So a filtered count by
//! order/batch/date disagreed with the corresponding filtered list. `count` now
//! mirrors `list`'s filter set.

use stateset_core::{
    CreateX402PaymentIntent, X402Asset, X402Network, X402PaymentIntentFilter,
    X402PaymentIntentRepository,
};
use stateset_db::SqliteDatabase;

#[test]
fn sqlite_x402_count_matches_list_by_order_id() {
    let db = SqliteDatabase::in_memory().expect("in-memory sqlite");
    let repo = db.x402_payment_intents();

    let order_a = uuid::Uuid::new_v4();
    let order_b = uuid::Uuid::new_v4();
    let make = |order_id: uuid::Uuid| {
        repo.create(CreateX402PaymentIntent {
            payer_address: "0xpayer".into(),
            payee_address: "0xpayee".into(),
            amount: 1000,
            asset: X402Asset::Usdc,
            network: X402Network::SetChain,
            nonce: None,
            validity_seconds: None,
            resource_uri: None,
            resource_method: None,
            description: None,
            cart_id: None,
            order_id: Some(order_id),
            invoice_id: None,
            merchant_id: None,
            idempotency_key: None,
            metadata: None,
            signature_scheme: None,
        })
        .expect("create intent");
    };
    make(order_a);
    make(order_b);

    // Sanity: unfiltered count sees both.
    assert_eq!(repo.count(X402PaymentIntentFilter::default()).unwrap(), 2);

    let filter = X402PaymentIntentFilter { order_id: Some(order_a), ..Default::default() };
    let listed = repo.list(filter.clone()).unwrap();
    assert_eq!(listed.len(), 1, "list filters by order_id");
    assert_eq!(
        repo.count(filter).unwrap(),
        1,
        "count must filter by order_id (and match the filtered list)"
    );
}
