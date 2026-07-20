//! Postgres parity for the cart grand-total clamp (SQLite covered by
//! `grand_total_never_goes_negative_from_oversized_discount`). A cart-level
//! discount larger than subtotal + tax + shipping must not drive the grand
//! total negative on either backend.

#![cfg(feature = "postgres")]

use rust_decimal_macros::dec;
use stateset_core::{AddCartItem, CreateCart, UpdateCart};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

fn item(sku: &str, qty: i32, price: rust_decimal::Decimal) -> AddCartItem {
    AddCartItem {
        sku: sku.into(),
        name: format!("Item {sku}"),
        quantity: qty,
        unit_price: price,
        ..Default::default()
    }
}

#[tokio::test]
async fn postgres_grand_total_never_goes_negative_from_oversized_discount() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping cart total clamp test");
        return;
    };
    let commerce =
        AsyncCommerce::connect(&url).await.expect("connect to postgres and run migrations");

    let cart = commerce
        .carts()
        .create(CreateCart {
            items: Some(vec![item("SKU-A", 2, dec!(10)), item("SKU-B", 1, dec!(5))]),
            ..Default::default()
        })
        .await
        .expect("create cart"); // subtotal $25

    commerce
        .carts()
        .update(
            cart.id.into_uuid(),
            UpdateCart { discount_amount: Some(dec!(100)), ..Default::default() },
        )
        .await
        .expect("set oversized discount");

    let recalculated = commerce.carts().recalculate(cart.id.into_uuid()).await.expect("recalc");
    assert_eq!(recalculated.subtotal, dec!(25));
    assert_eq!(
        recalculated.grand_total,
        dec!(0),
        "grand total must clamp at zero, not go negative: {recalculated:?}"
    );
}
