//! Cart subtotal / grand-total money-scale parity.
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
//! silently rounded them to `10.00`. The first fix rounded both to 2 dp in
//! Rust on both backends. Since round 4 the cart applies invariant M1
//! (`commerce.money.scale_exceeds_currency`) at the input instead — the same
//! rule orders enforce — so a sub-cent unit price is REFUSED rather than
//! silently rounded, on both backends, and cent-scale lines are stored exactly.
//!
//! The SQLite case always runs; the Postgres case needs `POSTGRES_URL` /
//! `DATABASE_URL` and is skipped otherwise.

use rust_decimal_macros::dec;
use stateset_core::{AddCartItem, CommerceError, CreateCart};

fn sub_cent_line() -> AddCartItem {
    AddCartItem {
        sku: "SUBCENT".into(),
        name: "Sub-cent line".into(),
        quantity: 3,
        unit_price: dec!(3.333), // 3 × 3.333 = 9.999
        ..Default::default()
    }
}

fn cent_line() -> AddCartItem {
    AddCartItem {
        sku: "CENTS".into(),
        name: "Cent-scale line".into(),
        quantity: 3,
        unit_price: dec!(3.33),
        ..Default::default()
    }
}

/// A sub-cent line (`3.333 × 3 = 9.999`) is refused with
/// `MoneyScaleExceedsCurrency` (invariant M1) rather than rounded; a
/// cent-scale line is stored exactly and totals stay real money.
#[cfg(feature = "sqlite")]
#[test]
fn sqlite_cart_total_rounds_to_cents() {
    use stateset_core::CartRepository;
    use stateset_db::SqliteDatabase;

    let db = SqliteDatabase::in_memory().expect("in-memory sqlite");
    let cart = db.carts().create(CreateCart::default()).expect("create cart");
    let err = db.carts().add_item(cart.id, sub_cent_line()).expect_err("sub-cent price");
    assert!(matches!(err, CommerceError::MoneyScaleExceedsCurrency { .. }), "got {err:?}");
    let cart = db.carts().get(cart.id).expect("get").expect("exists");
    assert!(cart.items.is_empty(), "refused line must not be stored");

    db.carts().add_item(cart.id, cent_line()).expect("add item");
    let cart = db.carts().get(cart.id).expect("get").expect("exists");
    assert_eq!(cart.items[0].total, dec!(9.99));
    assert_eq!(cart.subtotal, dec!(9.99));
    assert_eq!(cart.grand_total, dec!(9.99));
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
    let err = db
        .carts()
        .add_item_async(cart.id.into(), sub_cent_line())
        .await
        .expect_err("sub-cent price");
    assert!(matches!(err, CommerceError::MoneyScaleExceedsCurrency { .. }), "got {err:?}");
    let cart = db.carts().get_async(cart.id.into()).await.expect("get").expect("exists");
    assert!(cart.items.is_empty(), "refused line must not be stored");

    db.carts().add_item_async(cart.id.into(), cent_line()).await.expect("add item");
    let cart = db.carts().get_async(cart.id.into()).await.expect("get").expect("exists");
    assert_eq!(cart.items[0].total, dec!(9.99));
    assert_eq!(cart.subtotal, dec!(9.99));
    assert_eq!(cart.grand_total, dec!(9.99));
}
