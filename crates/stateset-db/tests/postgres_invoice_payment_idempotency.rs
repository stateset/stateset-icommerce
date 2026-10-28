#![cfg(feature = "postgres")]

use rust_decimal_macros::dec;
use stateset_core::{CreateCustomer, CreateInvoice, CreateInvoiceItem, RecordInvoicePayment};
use stateset_db::PostgresDatabase;
use uuid::Uuid;

#[tokio::test]
async fn postgres_invoice_payment_replay_and_conflict() {
    let Ok(url) = std::env::var("POSTGRES_URL") else {
        eprintln!("POSTGRES_URL not set; skipping live Postgres test");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.unwrap();
    let customer = db
        .customers()
        .create_async(CreateCustomer {
            email: format!("idempotency-{}@example.invalid", Uuid::new_v4()),
            first_name: "Set".into(),
            last_name: "Test".into(),
            phone: None,
            accepts_marketing: None,
            tags: None,
            metadata: None,
        })
        .await
        .unwrap();
    let invoice = db
        .invoices()
        .create_async(CreateInvoice {
            customer_id: customer.id,
            items: vec![CreateInvoiceItem {
                description: "Settlement".into(),
                quantity: dec!(1),
                unit_price: dec!(100),
                ..Default::default()
            }],
            ..Default::default()
        })
        .await
        .unwrap();
    let payment = RecordInvoicePayment {
        amount: dec!(40),
        payment_id: Some(Uuid::new_v4()),
        ..Default::default()
    };
    assert_eq!(
        db.invoices()
            .record_payment_async(invoice.id.into_uuid(), payment.clone())
            .await
            .unwrap()
            .amount_paid,
        dec!(40)
    );
    let invoices = db.invoices();
    let (first, second) = tokio::join!(
        invoices.record_payment_async(invoice.id.into_uuid(), payment.clone()),
        invoices.record_payment_async(invoice.id.into_uuid(), payment.clone())
    );
    assert_eq!(first.unwrap().amount_paid, dec!(40));
    assert_eq!(second.unwrap().amount_paid, dec!(40));
    assert!(
        db.invoices()
            .record_payment_async(
                invoice.id.into_uuid(),
                RecordInvoicePayment { amount: dec!(41), ..payment.clone() }
            )
            .await
            .is_err()
    );
    assert!(
        db.invoices()
            .record_payment_async(
                invoice.id.into_uuid(),
                RecordInvoicePayment {
                    amount: dec!(1),
                    payment_id: Some(Uuid::nil()),
                    ..Default::default()
                }
            )
            .await
            .is_err()
    );
    db.invoices()
        .record_payment_async(
            invoice.id.into_uuid(),
            RecordInvoicePayment {
                amount: dec!(60),
                payment_id: Some(Uuid::new_v4()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(
        db.invoices()
            .record_payment_async(invoice.id.into_uuid(), payment)
            .await
            .unwrap()
            .amount_paid,
        dec!(100)
    );
}
