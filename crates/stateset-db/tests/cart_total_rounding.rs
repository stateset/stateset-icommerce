//! Cart subtotal / grand-total rounding parity.
//!
//! A cart stores a `subtotal` and `grand_total`. Both must be real money
//! amounts (rounded to the currency minor unit, 2 dp) and must agree across
//! backends — a stored `grand_total` of `9.999` is not something a buyer can be
//! charged.
//!
//! Historically these diverged: SQLite computed the subtotal in `Decimal` and
//! stored it (and the grand total) as full-precision TEXT with no rounding, so a
//! sub-cent line (e.g. `3.333 × 3 = 9.999`) persisted `grand_total = 9.999`;
//! Postgres stored the same values in `DECIMAL(12,2)` columns, so the column
//! silently rounded them to `10.00`. Same cart → `9.999` (SQLite) vs `10.00`
//! (Postgres), and SQLite's value wasn't chargeable. The fix rounds the subtotal
//! and grand total to 2 dp in Rust on both backends (identical rounding strategy,
//! not relying on Postgres column coercion).
//!
//! The SQLite case always runs; the Postgres case needs `POSTGRES_URL` /
//! `DATABASE_URL` and is skipped otherwise.

use rust_decimal_macros::dec;
use stateset_core::{AddCartItem, CreateCart};

/// A sub-cent line (`3.333 × 3 = 9.999`) must leave the cart with a `subtotal`
/// and `grand_total` rounded to `10.00`.
#[cfg(feature = "sqlite")]
#[test]
fn sqlite_cart_total_rounds_to_cents() {
    use stateset_core::CartRepository;
    use stateset_db::SqliteDatabase;

    let db = SqliteDatabase::in_memory().expect("in-memory sqlite");
    let cart = db.carts().create(CreateCart::default()).expect("create cart");
    db.carts()
        .add_item(
            cart.id,
            AddCartItem {
                sku: "SUBCENT".into(),
                name: "Sub-cent line".into(),
                quantity: 3,
                unit_price: dec!(3.333), // 3 × 3.333 = 9.999
                ..Default::default()
            },
        )
        .expect("add item");

    let cart = db.carts().get(cart.id).expect("get").expect("exists");
    assert_eq!(cart.items[0].total, dec!(10.00), "line total must be rounded to 2dp");
    assert_eq!(cart.subtotal, dec!(10.00), "subtotal must be rounded to 2dp");
    assert_eq!(cart.grand_total, dec!(10.00), "grand total must be rounded to 2dp");
}

/// Same invariant on Postgres, pinning cross-backend parity (its `DECIMAL(12,2)`
/// columns already coerce to 2 dp; the explicit rounding keeps the two backends
/// on the same rounding strategy).
#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_cart_total_rounds_to_cents() {
    use stateset_db::PostgresDatabase;
    use std::env;

    let Some(url) = env::var("POSTGRES_URL").ok().or_else(|| env::var("DATABASE_URL").ok()) else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");

    let cart = db.carts().create_async(CreateCart::default()).await.expect("create cart");
    db.carts()
        .add_item_async(
            cart.id.into(),
            AddCartItem {
                sku: "SUBCENT".into(),
                name: "Sub-cent line".into(),
                quantity: 3,
                unit_price: dec!(3.333),
                ..Default::default()
            },
        )
        .await
        .expect("add item");

    let cart = db.carts().get_async(cart.id.into()).await.expect("get").expect("exists");
    assert_eq!(cart.items[0].total, dec!(10.00), "line total must be rounded to 2dp");
    assert_eq!(cart.subtotal, dec!(10.00), "subtotal must be rounded to 2dp");
    assert_eq!(cart.grand_total, dec!(10.00), "grand total must be rounded to 2dp");
}
