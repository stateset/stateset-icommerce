//! Postgres side of the invoice-`list` filter parity guard.
//!
//! Postgres applies the `min_total` / `max_total` / `min_balance` /
//! `invoice_number` (and date/type) filters via SQL; this asserts the behavior
//! the SQLite backend now matches (see
//! `crates/stateset-db/tests/invoice_list_filters.rs`), guarding the two backends
//! against drifting apart.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{CreateCustomer, CreateInvoice, CreateInvoiceItem, InvoiceFilter};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

fn item(price: Decimal) -> CreateInvoiceItem {
    CreateInvoiceItem {
        description: "Widget".into(),
        quantity: dec!(1),
        unit_price: price,
        ..Default::default()
    }
}

#[tokio::test]
async fn postgres_invoice_list_honors_money_and_number_filters() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping invoice list filter test");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");

    let unique = uuid::Uuid::new_v4().to_string();
    let customer = commerce
        .customers()
        .create(CreateCustomer {
            email: format!("inv-filter-{}@example.com", &unique[..8]),
            first_name: "Inv".into(),
            last_name: "Filter".into(),
            phone: None,
            accepts_marketing: None,
            tags: None,
            metadata: None,
        })
        .await
        .expect("create customer");
    let cust = customer.id;

    let small = commerce
        .invoices()
        .create(CreateInvoice {
            customer_id: cust,
            items: vec![item(dec!(20))],
            ..Default::default()
        })
        .await
        .expect("small invoice");
    let big = commerce
        .invoices()
        .create(CreateInvoice {
            customer_id: cust,
            items: vec![item(dec!(200))],
            ..Default::default()
        })
        .await
        .expect("big invoice");

    // Scope to this customer so a shared DB's other rows don't interfere.
    let base = InvoiceFilter { customer_id: Some(cust), ..Default::default() };

    let all = commerce.invoices().list(base.clone()).await.expect("all");
    assert_eq!(all.len(), 2);

    let over_100 = commerce
        .invoices()
        .list(InvoiceFilter { min_total: Some(dec!(100)), ..base.clone() })
        .await
        .expect("min_total");
    assert_eq!(over_100.len(), 1);
    assert_eq!(over_100[0].id, big.id);

    let under_100 = commerce
        .invoices()
        .list(InvoiceFilter { max_total: Some(dec!(100)), ..base.clone() })
        .await
        .expect("max_total");
    assert_eq!(under_100.len(), 1);
    assert_eq!(under_100[0].id, small.id);

    let by_number = commerce
        .invoices()
        .list(InvoiceFilter { invoice_number: Some(big.invoice_number.clone()), ..base.clone() })
        .await
        .expect("by number");
    assert_eq!(by_number.len(), 1);
    assert_eq!(by_number[0].id, big.id);
}
