//! Postgres parity for the scoped-discount bleed fix (SQLite covered by the
//! unit tests in sqlite/promotions.rs). A scoped item-value discount must not
//! exceed the eligible items' worth, even for a misconfigured (>100%)
//! percentage.

#![cfg(feature = "postgres")]

use rust_decimal_macros::dec;
use stateset_core::{
    ApplyPromotionsRequest, CreateCouponCode, CreatePromotion, CurrencyCode, DiscountTier,
    PromotionLineItem, PromotionTarget, PromotionTrigger, PromotionType, StackingBehavior,
};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

fn line_item(sku: &str, line_total: rust_decimal::Decimal) -> PromotionLineItem {
    PromotionLineItem {
        id: sku.to_string(),
        product_id: None,
        variant_id: None,
        sku: Some(sku.to_string()),
        category_ids: vec![],
        quantity: 1,
        unit_price: line_total,
        line_total,
    }
}

#[tokio::test]
async fn postgres_scoped_percentage_cannot_bleed_past_scoped_items() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping promotion scoping test");
        return;
    };
    let commerce =
        AsyncCommerce::connect(&url).await.expect("connect to postgres and run migrations");

    let unique = uuid::Uuid::new_v4().to_string();
    let code = format!("WIDGET-150-{}", &unique[..8]);

    let promo = commerce
        .promotions()
        .create(CreatePromotion {
            code: Some(code.clone()),
            name: "150% off widgets (misconfigured)".into(),
            promotion_type: PromotionType::PercentageOff,
            trigger: PromotionTrigger::CouponCode,
            target: PromotionTarget::Order,
            stacking: StackingBehavior::Stackable,
            percentage_off: Some(dec!(1.5)),
            applicable_skus: Some(vec!["WIDGET".into()]),
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
        line_items: vec![line_item("WIDGET", dec!(40.00)), line_item("GADGET", dec!(60.00))],
        subtotal: dec!(100.00),
        shipping_amount: dec!(10.00),
        shipping_country: None,
        shipping_state: None,
        currency: CurrencyCode::USD,
        is_first_order: false,
    };

    let result = commerce.promotions().apply_promotions(request).await.expect("apply");
    assert_eq!(
        result.total_discount,
        dec!(40.00),
        "discount must cap at the 40.00 of eligible WIDGET items, not bleed into the GADGET: {result:?}"
    );
}

#[tokio::test]
async fn postgres_honors_max_discount_amount() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping max-discount test");
        return;
    };
    let commerce =
        AsyncCommerce::connect(&url).await.expect("connect to postgres and run migrations");

    let unique = uuid::Uuid::new_v4().to_string();
    let code = format!("HALF-MAX10-{}", &unique[..8]);

    // 50% off, but capped at $10.
    let promo = commerce
        .promotions()
        .create(CreatePromotion {
            code: Some(code.clone()),
            name: "50% off, max $10".into(),
            promotion_type: PromotionType::PercentageOff,
            trigger: PromotionTrigger::CouponCode,
            target: PromotionTarget::Order,
            stacking: StackingBehavior::Stackable,
            percentage_off: Some(dec!(0.50)),
            max_discount_amount: Some(dec!(10.00)),
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
        line_items: vec![line_item("WIDGET", dec!(100.00))],
        subtotal: dec!(100.00),
        shipping_amount: dec!(0.00),
        shipping_country: None,
        shipping_state: None,
        currency: CurrencyCode::USD,
        is_first_order: false,
    };

    let result = commerce.promotions().apply_promotions(request).await.expect("apply");
    assert_eq!(
        result.total_discount,
        dec!(10.00),
        "50% of 100 is 50, but the promotion's max_discount_amount caps it at 10: {result:?}"
    );
}

#[tokio::test]
async fn postgres_tiered_picks_highest_applicable_tier_regardless_of_order() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping tiered-order test");
        return;
    };
    let commerce =
        AsyncCommerce::connect(&url).await.expect("connect to postgres and run migrations");

    let unique = uuid::Uuid::new_v4().to_string();
    let code = format!("TIER-{}", &unique[..8]);

    // Tiers listed high-to-low: the $100+ tier must win for a $100 order even
    // though the $0 tier appears last (regression for the Postgres tiered
    // selection that used to keep the last matching open-ended tier).
    let promo = commerce
        .promotions()
        .create(CreatePromotion {
            code: Some(code.clone()),
            name: "Spend more, save more".into(),
            promotion_type: PromotionType::TieredDiscount,
            trigger: PromotionTrigger::CouponCode,
            target: PromotionTarget::Order,
            stacking: StackingBehavior::Stackable,
            tiers: Some(vec![
                DiscountTier {
                    min_value: dec!(100),
                    max_value: None,
                    percentage_off: Some(dec!(0.20)),
                    fixed_amount_off: None,
                },
                DiscountTier {
                    min_value: dec!(0),
                    max_value: None,
                    percentage_off: Some(dec!(0.05)),
                    fixed_amount_off: None,
                },
            ]),
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
        line_items: vec![line_item("WIDGET", dec!(100.00))],
        subtotal: dec!(100.00),
        shipping_amount: dec!(0.00),
        shipping_country: None,
        shipping_state: None,
        currency: CurrencyCode::USD,
        is_first_order: false,
    };

    let result = commerce.promotions().apply_promotions(request).await.expect("apply");
    assert_eq!(
        result.total_discount,
        dec!(20.00),
        "$100 order must hit the $100+ tier (20% = 20.00), not the $0 tier (5%): {result:?}"
    );
}
