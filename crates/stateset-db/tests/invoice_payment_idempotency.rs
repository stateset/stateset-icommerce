#![cfg(feature = "sqlite")]

use rust_decimal_macros::dec;
use stateset_core::{
    CreateInvoice, CreateInvoiceItem, CustomerId, InvoiceRepository, RecordInvoicePayment,
};
use stateset_db::SqliteDatabase;
use uuid::Uuid;

fn invoice(db: &SqliteDatabase) -> stateset_core::Invoice {
    db.invoices()
        .create(CreateInvoice {
            customer_id: CustomerId::new(),
            items: vec![CreateInvoiceItem {
                description: "Settlement".into(),
                quantity: dec!(1),
                unit_price: dec!(100),
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap()
}

#[test]
fn replay_is_durable_and_amount_conflicts_fail() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("commerce.sqlite");
    let db =
        SqliteDatabase::new(&stateset_db::DatabaseConfig::sqlite(path.to_str().unwrap())).unwrap();
    let inv = invoice(&db);
    let payment = RecordInvoicePayment {
        amount: dec!(40),
        payment_id: Some(Uuid::new_v4()),
        ..Default::default()
    };
    assert_eq!(
        db.invoices().record_payment(inv.id, payment.clone()).unwrap().amount_paid,
        dec!(40)
    );
    drop(db);
    let db =
        SqliteDatabase::new(&stateset_db::DatabaseConfig::sqlite(path.to_str().unwrap())).unwrap();
    assert_eq!(
        db.invoices().record_payment(inv.id, payment.clone()).unwrap().amount_paid,
        dec!(40)
    );
    let conflict = RecordInvoicePayment { amount: dec!(41), ..payment.clone() };
    assert!(db.invoices().record_payment(inv.id, conflict).is_err());
    assert_eq!(db.invoices().get(inv.id).unwrap().unwrap().amount_paid, dec!(40));
    let rest = RecordInvoicePayment {
        amount: dec!(60),
        payment_id: Some(Uuid::new_v4()),
        ..Default::default()
    };
    db.invoices().record_payment(inv.id, rest).unwrap();
    assert_eq!(db.invoices().record_payment(inv.id, payment).unwrap().amount_paid, dec!(100));
}

#[test]
fn failed_payment_does_not_consume_the_id_and_nil_is_rejected() {
    let db = SqliteDatabase::in_memory().unwrap();
    let inv = invoice(&db);
    let payment = RecordInvoicePayment {
        amount: dec!(101),
        payment_id: Some(Uuid::new_v4()),
        ..Default::default()
    };
    assert!(db.invoices().record_payment(inv.id, payment.clone()).is_err());
    let corrected = RecordInvoicePayment { amount: dec!(40), ..payment };
    assert_eq!(db.invoices().record_payment(inv.id, corrected).unwrap().amount_paid, dec!(40));
    let nil = RecordInvoicePayment {
        amount: dec!(10),
        payment_id: Some(Uuid::nil()),
        ..Default::default()
    };
    assert!(db.invoices().record_payment(inv.id, nil).is_err());
}

#[test]
fn concurrent_retries_apply_once() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("commerce.sqlite");
    let db =
        SqliteDatabase::new(&stateset_db::DatabaseConfig::sqlite(path.to_str().unwrap())).unwrap();
    let inv = invoice(&db);
    let payment = RecordInvoicePayment {
        amount: dec!(40),
        payment_id: Some(Uuid::new_v4()),
        ..Default::default()
    };
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    std::thread::scope(|scope| {
        let mut workers = Vec::new();
        for _ in 0..2 {
            let path = path.clone();
            let payment = payment.clone();
            let barrier = barrier.clone();
            workers.push(scope.spawn(move || {
                let db = SqliteDatabase::new(&stateset_db::DatabaseConfig::sqlite(
                    path.to_str().unwrap(),
                ))
                .unwrap();
                barrier.wait();
                db.invoices().record_payment(inv.id, payment).unwrap()
            }));
        }
        for worker in workers {
            assert_eq!(worker.join().unwrap().amount_paid, dec!(40));
        }
    });
    assert_eq!(db.invoices().get(inv.id).unwrap().unwrap().amount_paid, dec!(40));
}
