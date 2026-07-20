//! Order-total rounding parity + internal consistency.
//!
//! An order's line items store a money `total` and the order stores a
//! `total_amount`. These must satisfy two properties on BOTH backends:
//!  1. Each line `total` is rounded to the currency's minor unit (2 dp) — a
//!     stored line total of `9.999` is not a real money amount.
//!  2. `total_amount == SUM(line totals)` — the order footer must foot to the
//!     line items the customer sees.
//!
//! Historically these diverged: SQLite's single-create rounded the *order* total
//! but stored *unrounded* line totals (so the order didn't foot, and the total
//! silently changed the first time `update_order_total` re-summed the unrounded
//! lines); Postgres rounded nothing at all; and SQLite's batch path also didn't
//! round. Same order → three different persisted totals. The fix rounds each
//! line in the shared `OrderItem::calculate_total`, and every create path sums
//! those rounded line totals — so all paths on both backends agree and foot.
//!
//! The SQLite case always runs; the Postgres case needs `POSTGRES_URL` /
//! `DATABASE_URL` and is skipped otherwise.

use rust_decimal_macros::dec;
use stateset_core::{CreateCustomer, CreateOrder, CreateOrderItem, ProductId};

/// A line whose raw total carries sub-cent precision (`3.333 × 3 = 9.999`) must
/// be stored rounded to `10.00`, and the order total must equal the summed line
/// totals — both at creation and after a later item mutation.
#[cfg(feature = "sqlite")]
#[test]
fn sqlite_order_total_rounds_and_foots() {
    use stateset_core::{CustomerRepository, OrderRepository};
    use stateset_db::SqliteDatabase;

    let db = SqliteDatabase::in_memory().expect("in-memory sqlite");
    let customer = db
        .customers()
        .create(CreateCustomer {
            email: "round@example.com".into(),
            first_name: "Round".into(),
            last_name: "Ing".into(),
            ..Default::default()
        })
        .expect("create customer");

    let order = db
        .orders()
        .create(CreateOrder {
            customer_id: customer.id,
            items: vec![CreateOrderItem {
                product_id: ProductId::new(),
                sku: "SUBCENT".into(),
                name: "Sub-cent line".into(),
                quantity: 3,
                unit_price: dec!(3.333), // 3 × 3.333 = 9.999
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("create order");

    assert_eq!(order.items[0].total, dec!(10.00), "line total must be rounded to 2dp");
    assert_eq!(order.total_amount, dec!(10.00), "order total must be rounded to 2dp");
    assert_eq!(
        order.total_amount,
        order.calculate_total(),
        "order total must foot to the summed line totals"
    );

    // A per-line-rounding order must also foot across multiple sub-cent lines:
    // 3 × (0.334 → 0.33) = 0.99, NOT round(3 × 0.334 = 1.002) = 1.00.
    let pennies = db
        .orders()
        .create(CreateOrder {
            customer_id: customer.id,
            items: vec![penny_item("P1"), penny_item("P2"), penny_item("P3")],
            ..Default::default()
        })
        .expect("create pennies order");
    assert_eq!(
        pennies.total_amount,
        dec!(0.99),
        "must sum rounded line totals, not round the raw sum"
    );
    assert_eq!(pennies.total_amount, pennies.calculate_total());

    // The invariant must survive an item mutation (which re-derives the total
    // from the stored line totals via update_order_total).
    let added = db
        .orders()
        .add_item(
            order.id,
            CreateOrderItem {
                product_id: ProductId::new(),
                sku: "SUBCENT-2".into(),
                name: "Another sub-cent line".into(),
                quantity: 3,
                unit_price: dec!(3.333),
                ..Default::default()
            },
        )
        .expect("add item");
    assert_eq!(added.total, dec!(10.00));

    let after = db.orders().get(order.id).expect("get").expect("exists");
    assert_eq!(after.total_amount, dec!(20.00), "total stays rounded after mutation (no flip)");
    assert_eq!(after.total_amount, after.calculate_total(), "still foots after mutation");
}

#[cfg(feature = "sqlite")]
fn penny_item(sku: &str) -> CreateOrderItem {
    CreateOrderItem {
        product_id: ProductId::new(),
        sku: sku.into(),
        name: "Penny line".into(),
        quantity: 1,
        unit_price: dec!(0.334), // rounds to 0.33
        ..Default::default()
    }
}

/// Same invariant on Postgres: the order total is rounded to 2 dp and foots to
/// the stored line totals (previously it stored the raw, unrounded sum).
#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_order_total_rounds_and_foots() {
    use stateset_db::PostgresDatabase;
    use std::env;

    let Some(url) = env::var("POSTGRES_URL").ok().or_else(|| env::var("DATABASE_URL").ok()) else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");

    let unique = uuid::Uuid::new_v4().to_string();
    let customer = db
        .customers()
        .create_async(CreateCustomer {
            email: format!("round-{unique}@example.com"),
            first_name: "Round".into(),
            last_name: "Ing".into(),
            ..Default::default()
        })
        .await
        .expect("create customer");

    let order = db
        .orders()
        .create_async(CreateOrder {
            customer_id: customer.id,
            items: vec![CreateOrderItem {
                product_id: ProductId::new(),
                sku: format!("SUBCENT-{unique}"),
                name: "Sub-cent line".into(),
                quantity: 3,
                unit_price: dec!(3.333), // 9.999
                ..Default::default()
            }],
            ..Default::default()
        })
        .await
        .expect("create order");

    assert_eq!(order.items[0].total, dec!(10.00), "line total must be rounded to 2dp");
    assert_eq!(order.total_amount, dec!(10.00), "order total must be rounded to 2dp");
    assert_eq!(
        order.total_amount,
        order.calculate_total(),
        "order total must foot to the summed line totals"
    );
}
