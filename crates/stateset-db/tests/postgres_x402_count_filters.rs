//! Postgres parity for `x402_payment_intents::count` filters.
//!
//! SQLite `count` dropped `order_id`/`batch_id`/`from_date`/`to_date`; Postgres
//! applies all of them. This locks in that a filtered count matches the filtered
//! list on Postgres too.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use stateset_core::{CreateX402PaymentIntent, X402Asset, X402Network, X402PaymentIntentFilter};
use stateset_db::PostgresDatabase;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test]
async fn postgres_x402_count_matches_list_by_order_id() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping x402 count filter test");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");
    let repo = db.x402_payment_intents();

    // Distinct order ids isolate this test on a shared database.
    let order_a = uuid::Uuid::new_v4();
    let order_b = uuid::Uuid::new_v4();
    for order_id in [order_a, order_b] {
        repo.create_async(CreateX402PaymentIntent {
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
        .await
        .expect("create intent");
    }

    let filter = X402PaymentIntentFilter { order_id: Some(order_a), ..Default::default() };
    let listed = repo.list_async(filter.clone()).await.expect("list");
    assert_eq!(listed.len(), 1, "list filters by order_id");
    assert_eq!(
        repo.count_async(filter).await.expect("count"),
        1,
        "count must filter by order_id (and match the filtered list)"
    );
}
