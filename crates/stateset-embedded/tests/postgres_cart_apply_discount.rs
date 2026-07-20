//! Postgres parity for cart coupon application (`carts().apply_discount`).
//!
//! The Postgres `apply_discount_async` used to be a no-op: it stored the coupon
//! *string* on the cart but never looked the coupon up, never resolved its
//! promotion, never computed a `discount_amount`, and never recalculated the
//! grand total — so a valid coupon was silently ignored (buyer charged full
//! price) and an *invalid* coupon was accepted without error. The SQLite
//! `apply_discount` did all of this (look up coupon → resolve promotion →
//! compute discount → recalculate), so the two backends diverged by real
//! dollars. These tests pin the Postgres backend to the SQLite behaviour.
//!
//! They require a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`) and
//! are skipped otherwise.

#![cfg(feature = "postgres")]

use rust_decimal_macros::dec;
use stateset_core::{
    AddCartItem, CommerceError, CreateCart, CreateCouponCode, CreatePromotion, PromotionTarget,
    PromotionTrigger, PromotionType, StackingBehavior,
};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

/// A valid fixed-amount coupon must reduce the cart's `discount_amount` and
/// `grand_total`, not merely stamp the coupon string on the cart.
#[tokio::test]
async fn postgres_apply_fixed_amount_coupon_reduces_total() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");

    let unique = uuid::Uuid::new_v4().to_string();
    let code = format!("SAVE15-{}", &unique[..8]);

    let promo = commerce
        .promotions()
        .create(CreatePromotion {
            code: Some(code.clone()),
            name: "$15 off".into(),
            promotion_type: PromotionType::FixedAmountOff,
            trigger: PromotionTrigger::CouponCode,
            target: PromotionTarget::Order,
            stacking: StackingBehavior::Stackable,
            fixed_amount_off: Some(dec!(15.00)),
            ..Default::default()
        })
        .await
        .expect("create promo");
    commerce.promotions().activate(promo.id.into_uuid()).await.expect("activate");
    commerce
        .promotions()
        .create_coupon(CreateCouponCode {
            promotion_id: promo.id,
            code: code.clone(),
            usage_limit: None,
            per_customer_limit: None,
            starts_at: None,
            ends_at: None,
            metadata: None,
        })
        .await
        .expect("create coupon");

    let cart = commerce.carts().create(CreateCart::default()).await.expect("create cart");
    commerce
        .carts()
        .add_item(
            cart.id.into(),
            AddCartItem {
                sku: "ITEM-1".into(),
                name: "Widget".into(),
                quantity: 1,
                unit_price: dec!(80.00),
                ..Default::default()
            },
        )
        .await
        .expect("add item");

    let updated =
        commerce.carts().apply_discount(cart.id.into(), &code).await.expect("apply valid coupon");

    assert_eq!(updated.discount_amount, dec!(15.00), "discount must be computed, not ignored");
    assert_eq!(updated.grand_total, dec!(65.00), "grand total must reflect the $15 discount");
}

/// A valid percentage coupon (25% off) must discount proportionally.
#[tokio::test]
async fn postgres_apply_percentage_coupon_reduces_total() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");

    let unique = uuid::Uuid::new_v4().to_string();
    let code = format!("QUARTER-{}", &unique[..8]);

    let promo = commerce
        .promotions()
        .create(CreatePromotion {
            code: Some(code.clone()),
            name: "25% off".into(),
            promotion_type: PromotionType::PercentageOff,
            trigger: PromotionTrigger::CouponCode,
            target: PromotionTarget::Order,
            stacking: StackingBehavior::Stackable,
            percentage_off: Some(dec!(0.25)),
            ..Default::default()
        })
        .await
        .expect("create promo");
    commerce.promotions().activate(promo.id.into_uuid()).await.expect("activate");
    commerce
        .promotions()
        .create_coupon(CreateCouponCode {
            promotion_id: promo.id,
            code: code.clone(),
            usage_limit: None,
            per_customer_limit: None,
            starts_at: None,
            ends_at: None,
            metadata: None,
        })
        .await
        .expect("create coupon");

    let cart = commerce.carts().create(CreateCart::default()).await.expect("create cart");
    commerce
        .carts()
        .add_item(
            cart.id.into(),
            AddCartItem {
                sku: "ITEM-2".into(),
                name: "Gadget".into(),
                quantity: 2,
                unit_price: dec!(50.00),
                ..Default::default()
            },
        )
        .await
        .expect("add item");

    let updated =
        commerce.carts().apply_discount(cart.id.into(), &code).await.expect("apply valid coupon");

    // 25% of $100 = $25.
    assert_eq!(updated.discount_amount, dec!(25.00));
    assert_eq!(updated.grand_total, dec!(75.00));
}

/// An unknown coupon code must be rejected with a validation error, not silently
/// stamped onto the cart.
#[tokio::test]
async fn postgres_apply_invalid_coupon_is_rejected() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");

    let cart = commerce.carts().create(CreateCart::default()).await.expect("create cart");
    commerce
        .carts()
        .add_item(
            cart.id.into(),
            AddCartItem {
                sku: "ITEM-3".into(),
                name: "Thing".into(),
                quantity: 1,
                unit_price: dec!(10.00),
                ..Default::default()
            },
        )
        .await
        .expect("add item");

    let err = commerce
        .carts()
        .apply_discount(cart.id.into(), "DOES-NOT-EXIST")
        .await
        .expect_err("invalid coupon must be rejected");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
}
