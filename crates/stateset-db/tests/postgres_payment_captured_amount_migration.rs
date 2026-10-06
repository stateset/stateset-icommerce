#![cfg(feature = "postgres")]
//! Migration `106_payment_captured_amount` backfills `captured_amount = amount`
//! for every payment that already captured money, leaves in-flight payments
//! uncaptured, and is safe to re-run. One test per binary on purpose: it
//! rewinds a column on the shared database.

use rust_decimal_macros::dec;
use stateset_core::{
    CreateCustomer, CreateOrder, CreateOrderItem, CreatePayment, PaymentMethodType, ProductId,
};
use stateset_db::PostgresDatabase;
use std::env;
use uuid::Uuid;

#[tokio::test]
async fn pg_captured_amount_migration_backfills_settled_payments_and_reruns() {
    let Some(url) = env::var("POSTGRES_URL").ok().or_else(|| env::var("DATABASE_URL").ok()) else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect");
    let customer = db
        .customers()
        .create_async(CreateCustomer {
            email: format!("pg-backfill-{}@example.com", Uuid::new_v4()),
            first_name: "Back".into(),
            last_name: "Fill".into(),
            ..Default::default()
        })
        .await
        .expect("customer");
    let order = db
        .orders()
        .create_async(CreateOrder {
            customer_id: customer.id,
            items: vec![CreateOrderItem {
                product_id: ProductId::new(),
                sku: format!("PG-BF-{}", Uuid::new_v4()),
                name: "Widget".into(),
                quantity: 1,
                unit_price: dec!(90.00),
                ..Default::default()
            }],
            ..Default::default()
        })
        .await
        .expect("order");
    let pay = |amount| CreatePayment {
        order_id: Some(order.id),
        payment_method: PaymentMethodType::CreditCard,
        amount,
        ..Default::default()
    };
    let settled = db.payments().create_async(pay(dec!(40.00))).await.expect("payment");
    db.payments().mark_completed_async(settled.id.into_uuid()).await.expect("complete");
    let in_flight = db.payments().create_async(pay(dec!(50.00))).await.expect("payment");

    sqlx::query("ALTER TABLE payments DROP COLUMN captured_amount")
        .execute(db.pool())
        .await
        .expect("rewind column");
    sqlx::query("DELETE FROM _migrations WHERE name = '106_payment_captured_amount'")
        .execute(db.pool())
        .await
        .expect("rewind migration");
    drop(db);
    let db = PostgresDatabase::connect(&url).await.expect("re-migrate");
    drop(db);
    let db = PostgresDatabase::connect(&url).await.expect("rerun is a no-op");

    let settled = db.payments().get_async(settled.id.into_uuid()).await.expect("get").expect("p");
    assert_eq!(settled.captured_amount, Some(dec!(40.00)));
    let in_flight =
        db.payments().get_async(in_flight.id.into_uuid()).await.expect("get").expect("p");
    assert_eq!(in_flight.captured_amount, None);
}
