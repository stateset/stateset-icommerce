//! Postgres side of the `get_customer_aging` empty-set parity guard.
//!
//! For a customer that exists but has no open invoices, both backends must
//! return `Ok(Some(..))` with zero balances — NOT `NotFound`. SQLite previously
//! returned `Err(NotFound)` here (also breaking `get_customer_summary`); this
//! asserts the Postgres behavior the SQLite unit test now matches
//! (`sqlite/accounts_receivable.rs::
//! get_customer_aging_returns_zeros_for_existing_customer_without_invoices`).
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use rust_decimal_macros::dec;
use stateset_core::CreateCustomer;
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test]
async fn postgres_customer_aging_is_zeros_for_existing_customer_without_invoices() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping customer-aging test");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");

    let unique = uuid::Uuid::new_v4().to_string();
    let customer = commerce
        .customers()
        .create(CreateCustomer {
            email: format!("paidup-{}@example.com", &unique[..8]),
            first_name: "Paid".into(),
            last_name: "Up".into(),
            phone: None,
            accepts_marketing: None,
            tags: None,
            metadata: None,
        })
        .await
        .expect("create customer");

    let aging = commerce
        .accounts_receivable()
        .get_customer_aging(customer.id.into_uuid())
        .await
        .expect("existing customer must not error")
        .expect("existing customer must return Some, even with no open invoices");
    assert_eq!(aging.invoice_count, 0);
    assert_eq!(aging.total_outstanding, dec!(0));
    assert_eq!(aging.current, dec!(0));

    // A truly-unknown customer is still None (not an error).
    let missing = commerce
        .accounts_receivable()
        .get_customer_aging(uuid::Uuid::new_v4())
        .await
        .expect("unknown customer is not an error");
    assert!(missing.is_none(), "unknown customer must be None");
}
