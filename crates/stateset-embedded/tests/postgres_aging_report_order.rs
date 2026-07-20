//! Postgres parity for the AR aging report's deterministic tiebreaker (SQLite
//! covered by `aging_report_orders_ties_by_customer_id`). Customers with equal
//! outstanding balances must be ordered by `customer_id` so the report — and the
//! LIMIT/OFFSET pagination on top of it — is stable and identical across
//! backends.

#![cfg(feature = "postgres")]

use std::collections::HashSet;

use rust_decimal_macros::dec;
use stateset_core::{ArAgingFilter, CreateCustomer, CreateInvoice, CreateInvoiceItem};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test]
async fn postgres_aging_report_orders_ties_by_customer_id() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping aging report order test");
        return;
    };
    let commerce =
        AsyncCommerce::connect(&url).await.expect("connect to postgres and run migrations");

    // Five customers, each with one invoice of the same balance ($100) — all tie
    // on total_outstanding, so only the customer_id tiebreaker orders them.
    let unique = uuid::Uuid::new_v4().to_string();
    let mut ids = Vec::new();
    for i in 0..5 {
        let customer = commerce
            .customers()
            .create(CreateCustomer {
                email: format!("aging-{}-{i}@example.com", &unique[..8]),
                first_name: "Test".into(),
                last_name: "Customer".into(),
                ..Default::default()
            })
            .await
            .expect("create customer");
        commerce
            .invoices()
            .create(CreateInvoice {
                customer_id: customer.id,
                days_until_due: Some(30),
                items: vec![CreateInvoiceItem {
                    description: "Widget".into(),
                    quantity: dec!(1),
                    unit_price: dec!(100),
                    ..Default::default()
                }],
                ..Default::default()
            })
            .await
            .expect("create invoice");
        ids.push(customer.id.into_uuid());
    }

    let report = commerce
        .accounts_receivable()
        .get_aging_report(ArAgingFilter::default())
        .await
        .expect("aging report");

    // Restrict to the customers we created (ignore any others), and assert they
    // come back in ascending customer_id order.
    let mine: HashSet<uuid::Uuid> = ids.iter().copied().collect();
    let returned: Vec<uuid::Uuid> =
        report.iter().map(|r| r.customer_id).filter(|id| mine.contains(id)).collect();
    let mut expected = ids.clone();
    expected.sort();
    assert_eq!(
        returned, expected,
        "equal-balance customers must be ordered by ascending customer_id"
    );
}
