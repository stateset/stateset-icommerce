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
//! Note on inputs: since invariant M1 landed, *order creation* refuses a line
//! whose `unit_price`/`discount`/`tax_amount` exceeds the currency scale, so the
//! sub-cent inputs below go in through `orders().add_item`, which is still
//! unguarded and is exactly the path where an unrounded line total used to leak
//! into `update_order_total`. Creation itself is covered by a rejection
//! assertion.
//!
//! The SQLite case always runs; the Postgres case needs `POSTGRES_URL` /
//! `DATABASE_URL` and is skipped otherwise.

use rust_decimal_macros::dec;
use stateset_core::{CreateCustomer, CreateOrder, CreateOrderItem, ProductId};

/// Creation refuses an over-scaled `unit_price` (M1). A line added afterwards
/// whose raw total carries sub-cent precision (`3.333 × 3 = 9.999`) must still
/// be stored rounded to `10.00`, and the order total must equal the summed line
/// totals.
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

    // M1: an over-scaled unit_price is refused at creation and writes nothing.
    let err = db
        .orders()
        .create(CreateOrder {
            customer_id: customer.id,
            items: vec![CreateOrderItem {
                product_id: ProductId::new(),
                sku: "SUBCENT".into(),
                name: "Sub-cent line".into(),
                quantity: 3,
                unit_price: dec!(3.333),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect_err("3.333 is three-scale and USD allows two");
    assert_eq!(err.invariant_code(), Some("commerce.money.scale_exceeds_currency"), "{err:?}");
    assert!(
        db.orders().list(Default::default()).expect("list").is_empty(),
        "rejection wrote a row"
    );

    // A scale-valid order is the base for the add_item rounding checks below.
    let order = db
        .orders()
        .create(CreateOrder {
            customer_id: customer.id,
            items: vec![CreateOrderItem {
                product_id: ProductId::new(),
                sku: "BASE".into(),
                name: "Base line".into(),
                quantity: 4,
                unit_price: dec!(2.50),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("create order");
    assert_eq!(order.items[0].total, dec!(10.00));
    assert_eq!(order.total_amount, dec!(10.00));
    assert_eq!(
        order.total_amount,
        order.calculate_total(),
        "order total must foot to the summed line totals"
    );

    // A per-line-rounding order must also foot across multiple sub-cent lines:
    // 3 × (0.334 → 0.33) = 0.99, NOT round(3 × 0.334 = 1.002) = 1.00. These go
    // in via add_item, which M1 does not guard.
    let pennies = db
        .orders()
        .create(CreateOrder {
            customer_id: customer.id,
            items: vec![CreateOrderItem {
                product_id: ProductId::new(),
                sku: "P0".into(),
                name: "Penny base".into(),
                quantity: 1,
                unit_price: dec!(0.00),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("create pennies order");
    for sku in ["P1", "P2", "P3"] {
        db.orders().add_item(pennies.id, penny_item(sku)).expect("add penny item");
    }
    let pennies = db.orders().get(pennies.id).expect("get").expect("exists");
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

/// Same invariants on Postgres: creation refuses the over-scaled price, and a
/// sub-cent line added afterwards is rounded to 2 dp with the order total
/// footing to the stored line totals (previously it stored the raw sum).
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

    let err = db
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
        .expect_err("3.333 is three-scale and USD allows two");
    assert_eq!(err.invariant_code(), Some("commerce.money.scale_exceeds_currency"), "{err:?}");

    let order = db
        .orders()
        .create_async(CreateOrder {
            customer_id: customer.id,
            items: vec![CreateOrderItem {
                product_id: ProductId::new(),
                sku: format!("BASE-{unique}"),
                name: "Base line".into(),
                quantity: 4,
                unit_price: dec!(2.50),
                ..Default::default()
            }],
            ..Default::default()
        })
        .await
        .expect("create order");

    let added = db
        .orders()
        .add_item_async(
            order.id.into_uuid(),
            CreateOrderItem {
                product_id: ProductId::new(),
                sku: format!("SUBCENT2-{unique}"),
                name: "Sub-cent line".into(),
                quantity: 3,
                unit_price: dec!(3.333), // 9.999
                ..Default::default()
            },
        )
        .await
        .expect("add item");
    assert_eq!(added.total, dec!(10.00), "line total must be rounded to 2dp");

    let order = db.orders().get_async(order.id.into_uuid()).await.expect("get").expect("exists");
    assert_eq!(order.total_amount, dec!(20.00), "order total must be rounded to 2dp");
    assert_eq!(
        order.total_amount,
        order.calculate_total(),
        "order total must foot to the summed line totals"
    );
}
