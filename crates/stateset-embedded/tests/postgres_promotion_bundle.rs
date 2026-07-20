//! Postgres side of the `BundleDiscount` parity guard.
//!
//! A `BundleDiscount` promotion applies its fixed `bundle_discount` amount.
//! SQLite previously produced $0 for this type (missing match arm) while
//! Postgres applied the full amount; this asserts the Postgres behavior the
//! SQLite unit test (`sqlite/promotions.rs::apply_promotions_applies_bundle_discount`)
//! now matches, guarding the two backends against drifting apart.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use rust_decimal_macros::dec;
use stateset_core::{
    ApplyPromotionsRequest, CreateCouponCode, CreatePromotion, CurrencyCode, PromotionTarget,
    PromotionTrigger, PromotionType, StackingBehavior,
};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test]
async fn postgres_applies_bundle_discount() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping bundle-discount test");
        return;
    };
    let commerce =
        AsyncCommerce::connect(&url).await.expect("connect to postgres and run migrations");

    let unique = uuid::Uuid::new_v4().to_string();
    let code = format!("BUNDLE-15-{}", &unique[..8]);

    let promo = commerce
        .promotions()
        .create(CreatePromotion {
            code: Some(code.clone()),
            name: "$15 off bundle".into(),
            promotion_type: PromotionType::BundleDiscount,
            trigger: PromotionTrigger::CouponCode,
            target: PromotionTarget::Order,
            stacking: StackingBehavior::Stackable,
            bundle_discount: Some(dec!(15.00)),
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

    let request = ApplyPromotionsRequest {
        cart_id: None,
        customer_id: None,
        coupon_codes: vec![code],
        line_items: vec![],
        subtotal: dec!(100.00),
        shipping_amount: dec!(0.00),
        shipping_country: None,
        shipping_state: None,
        currency: CurrencyCode::USD,
        is_first_order: false,
    };

    let result = commerce.promotions().apply_promotions(request).await.expect("apply");
    assert_eq!(result.total_discount, dec!(15.00), "bundle discount must apply $15: {result:?}");
}
