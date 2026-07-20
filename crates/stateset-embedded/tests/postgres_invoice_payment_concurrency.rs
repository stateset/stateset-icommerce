//! Postgres parity for the invoice payment concurrency fix (SQLite covered by
//! the `concurrent_payments_are_not_lost` unit test in sqlite/invoices.rs).
//! Recording a payment is a read-modify-write of `amount_paid`; concurrent
//! payments on the same invoice must serialize (via `FOR UPDATE`) so none are
//! lost.

#![cfg(feature = "postgres")]

use std::sync::Arc;

use rust_decimal_macros::dec;
use stateset_core::{
    CreateCustomer, CreateInvoice, CreateInvoiceItem, InvoiceStatus, RecordInvoicePayment,
};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_concurrent_invoice_payments_are_not_lost() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping invoice concurrency test");
        return;
    };
    let commerce = Arc::new(
        AsyncCommerce::connect(&url).await.expect("connect to postgres and run migrations"),
    );

    let unique = uuid::Uuid::new_v4().to_string();
    let customer = commerce
        .customers()
        .create(CreateCustomer {
            email: format!("payer-{}@example.com", &unique[..8]),
            first_name: "Pay".into(),
            last_name: "Er".into(),
            ..Default::default()
        })
        .await
        .expect("create customer");

    // One line of 2 x $50 = $100, no tax/discount/shipping.
    let invoice = commerce
        .invoices()
        .create(CreateInvoice {
            customer_id: customer.id,
            days_until_due: Some(30),
            items: vec![CreateInvoiceItem {
                description: "Widget".into(),
                quantity: dec!(2),
                unit_price: dec!(50),
                ..Default::default()
            }],
            ..Default::default()
        })
        .await
        .expect("create invoice");
    assert_eq!(invoice.balance_due, dec!(100.00));

    // Ten $10 payments land simultaneously.
    let task_count = 10;
    let barrier = Arc::new(tokio::sync::Barrier::new(task_count));
    let invoice_id = invoice.id.into_uuid();
    let mut handles = Vec::new();
    for _ in 0..task_count {
        let commerce = Arc::clone(&commerce);
        let barrier = Arc::clone(&barrier);
        handles.push(tokio::spawn(async move {
            barrier.wait().await;
            commerce
                .invoices()
                .record_payment(
                    invoice_id,
                    RecordInvoicePayment {
                        amount: dec!(10.00),
                        payment_id: None,
                        payment_method: None,
                        reference: None,
                        notes: None,
                    },
                )
                .await
        }));
    }

    let mut successes = 0;
    for h in handles {
        if h.await.expect("join").is_ok() {
            successes += 1;
        }
    }
    assert_eq!(successes, task_count, "every concurrent payment must succeed");

    let fetched = commerce.invoices().get(invoice_id).await.expect("get").expect("found");
    assert_eq!(fetched.amount_paid, dec!(100.00), "payments were lost under concurrency");
    assert_eq!(fetched.balance_due, dec!(0.00));
    assert_eq!(fetched.status, InvoiceStatus::Paid);
}
