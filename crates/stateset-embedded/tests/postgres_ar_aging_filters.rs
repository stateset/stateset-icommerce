//! Postgres side of the AR aging-report filter parity guard.
//!
//! SQLite's `get_aging_report` ignored the `min_balance` and `aging_bucket`
//! filters (only `customer_id` / `overdue_only` / offset / limit were honored),
//! while Postgres applies both via `HAVING`. This asserts the Postgres behavior
//! the SQLite unit tests now match (`sqlite/accounts_receivable.rs::
//! get_aging_report_honors_min_balance` / `_honors_aging_bucket`), guarding the
//! two backends against drifting apart.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use std::collections::HashSet;

use chrono::{Duration, Utc};
use rust_decimal_macros::dec;
use stateset_core::{AgingBucket, ArAgingFilter, CreateCustomer, CreateInvoice, CreateInvoiceItem};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

async fn customer_with_invoice(
    commerce: &AsyncCommerce,
    tag: &str,
    unit_price: rust_decimal::Decimal,
    due: chrono::DateTime<Utc>,
) -> uuid::Uuid {
    let unique = uuid::Uuid::new_v4().to_string();
    let customer = commerce
        .customers()
        .create(CreateCustomer {
            email: format!("aging-{tag}-{}@example.com", &unique[..8]),
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
            due_date: Some(due),
            items: vec![CreateInvoiceItem {
                description: "Widget".into(),
                quantity: dec!(1),
                unit_price,
                ..Default::default()
            }],
            ..Default::default()
        })
        .await
        .expect("create invoice");
    customer.id.into_uuid()
}

#[tokio::test]
async fn postgres_aging_report_honors_min_balance() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping min_balance test");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");
    let due = Utc::now() + Duration::days(30);

    let small = customer_with_invoice(&commerce, "small", dec!(100), due).await;
    let big = customer_with_invoice(&commerce, "big", dec!(5000), due).await;
    let mine: HashSet<uuid::Uuid> = [small, big].into_iter().collect();

    let filtered = commerce
        .accounts_receivable()
        .get_aging_report(ArAgingFilter { min_balance: Some(dec!(1000)), ..Default::default() })
        .await
        .expect("aging report");
    let returned: HashSet<uuid::Uuid> =
        filtered.iter().map(|r| r.customer_id).filter(|id| mine.contains(id)).collect();

    assert!(returned.contains(&big), "the $5000 customer must pass min_balance 1000");
    assert!(!returned.contains(&small), "the $100 customer must be filtered out by min_balance");
}

#[tokio::test]
async fn postgres_aging_report_honors_aging_bucket() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping aging_bucket test");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");

    let current =
        customer_with_invoice(&commerce, "cur", dec!(200), Utc::now() + Duration::days(10)).await;
    let over90 =
        customer_with_invoice(&commerce, "old", dec!(300), Utc::now() - Duration::days(120)).await;
    let mine: HashSet<uuid::Uuid> = [current, over90].into_iter().collect();

    let ar = commerce.accounts_receivable();

    let over90_report = ar
        .get_aging_report(ArAgingFilter {
            aging_bucket: Some(AgingBucket::DaysOver90),
            ..Default::default()
        })
        .await
        .expect("over90 report");
    let over90_ids: HashSet<uuid::Uuid> =
        over90_report.iter().map(|r| r.customer_id).filter(|id| mine.contains(id)).collect();
    assert!(over90_ids.contains(&over90), "the 120-day customer must match DaysOver90");
    assert!(!over90_ids.contains(&current), "the not-yet-due customer must not match DaysOver90");

    let current_report = ar
        .get_aging_report(ArAgingFilter {
            aging_bucket: Some(AgingBucket::Current),
            ..Default::default()
        })
        .await
        .expect("current report");
    let current_ids: HashSet<uuid::Uuid> =
        current_report.iter().map(|r| r.customer_id).filter(|id| mine.contains(id)).collect();
    assert!(current_ids.contains(&current), "the not-yet-due customer must match Current");
    assert!(!current_ids.contains(&over90), "the 120-day customer must not match Current");
}
