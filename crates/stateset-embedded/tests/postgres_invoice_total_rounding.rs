//! Postgres side of the invoice money-rounding parity guard.
//!
//! Postgres stores invoice money in `DECIMAL(12, 2)` columns (rounds on write);
//! this asserts the exact cent-rounded totals the SQLite backend now matches
//! (see `crates/stateset-db/tests/invoice_total_rounding.rs`), guarding the two
//! backends against drifting apart.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use rust_decimal_macros::dec;
use stateset_core::{CreateCustomer, CreateInvoice, CreateInvoiceItem};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test]
async fn postgres_invoice_totals_round_to_cents() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping invoice rounding test");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");

    let unique = uuid::Uuid::new_v4().to_string();
    let customer = commerce
        .customers()
        .create(CreateCustomer {
            email: format!("inv-round-{}@example.com", &unique[..8]),
            first_name: "Inv".into(),
            last_name: "Round".into(),
            phone: None,
            accepts_marketing: None,
            tags: None,
            metadata: None,
        })
        .await
        .expect("create customer");

    // 3 × 3.333 = 9.999 → 10.00.
    let invoice = commerce
        .invoices()
        .create(CreateInvoice {
            customer_id: customer.id,
            items: vec![CreateInvoiceItem {
                description: "Widget".into(),
                quantity: dec!(3),
                unit_price: dec!(3.333),
                ..Default::default()
            }],
            ..Default::default()
        })
        .await
        .expect("create invoice");
    assert_eq!(invoice.items[0].line_total, dec!(10.00));
    assert_eq!(invoice.subtotal, dec!(10.00));
    assert_eq!(invoice.total, dec!(10.00));
    assert_eq!(invoice.balance_due, dec!(10.00));

    // 10.005 at the cent boundary → 10.01 (half away from zero).
    let midpoint = commerce
        .invoices()
        .create(CreateInvoice {
            customer_id: customer.id,
            items: vec![CreateInvoiceItem {
                description: "Widget".into(),
                quantity: dec!(1),
                unit_price: dec!(10.005),
                ..Default::default()
            }],
            ..Default::default()
        })
        .await
        .expect("create invoice");
    assert_eq!(midpoint.total, dec!(10.01));
}
