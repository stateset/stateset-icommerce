//! Postgres parity for the shared promotion candidate selection (SQLite
//! covered by the unit tests in `sqlite/promotions.rs`).
//!
//! Pricing (`apply_promotions`) and checkout (`consume_cart_promotions_in_tx`)
//! now build their candidate set with ONE function: a coupon outside its
//! validity window, exhausted, or over its per-customer limit is dropped
//! before evaluation, so a dead Exclusive coupon cannot suppress the
//! automatic promotions the customer was quoted. Per-customer limits apply
//! to automatic promotions too.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`);
//! skipped otherwise.

#![cfg(feature = "postgres")]

use chrono::{Duration, Utc};
use rust_decimal_macros::dec;
use stateset_core::{
    ApplyPromotionsRequest, CreateCouponCode, CreateCustomer, CreatePromotion, CurrencyCode,
    Promotion, PromotionLineItem, PromotionTarget, PromotionTrigger, PromotionType,
    RejectionReason, StackingBehavior,
};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

fn line_item(sku: &str, quantity: i32, unit_price: rust_decimal::Decimal) -> PromotionLineItem {
    PromotionLineItem {
        id: sku.to_string(),
        product_id: None,
        variant_id: None,
        sku: Some(sku.to_string()),
        category_ids: vec![],
        quantity,
        unit_price,
        line_total: unit_price * rust_decimal::Decimal::from(quantity),
    }
}

/// An automatic promotion scoped to `sku` (so other tests' automatic
/// promotions in the shared database never interfere with the assertions).
async fn automatic_promo(
    commerce: &AsyncCommerce,
    sku: &str,
    pct: rust_decimal::Decimal,
    per_customer_limit: Option<i32>,
) -> Promotion {
    let promo = commerce
        .promotions()
        .create(CreatePromotion {
            name: format!("auto {sku}"),
            promotion_type: PromotionType::PercentageOff,
            trigger: PromotionTrigger::Automatic,
            target: PromotionTarget::Order,
            stacking: StackingBehavior::Stackable,
            percentage_off: Some(pct),
            applicable_skus: Some(vec![sku.to_string()]),
            per_customer_limit,
            ..Default::default()
        })
        .await
        .expect("create automatic promo");
    commerce.promotions().activate(promo.id.into_uuid()).await.expect("activate")
}

async fn exclusive_coupon(
    commerce: &AsyncCommerce,
    code: &str,
    ends_at: Option<chrono::DateTime<Utc>>,
) -> Promotion {
    let promo = commerce
        .promotions()
        .create(CreatePromotion {
            code: Some(code.to_string()),
            name: format!("exclusive {code}"),
            promotion_type: PromotionType::PercentageOff,
            trigger: PromotionTrigger::CouponCode,
            target: PromotionTarget::Order,
            stacking: StackingBehavior::Exclusive,
            percentage_off: Some(dec!(0.20)),
            ..Default::default()
        })
        .await
        .expect("create exclusive promo");
    commerce.promotions().activate(promo.id.into_uuid()).await.expect("activate");
    commerce
        .promotions()
        .create_coupon(CreateCouponCode {
            promotion_id: promo.id,
            code: code.to_string(),
            usage_limit: None,
            per_customer_limit: None,
            starts_at: None,
            ends_at,
            metadata: None,
        })
        .await
        .expect("create coupon");
    promo
}

#[tokio::test]
async fn postgres_dead_exclusive_coupon_does_not_suppress_automatic_promotions() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping promotion candidates test");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");
    let unique = uuid::Uuid::new_v4().to_string();
    let sku = format!("CAND-{}", &unique[..8]);
    let auto = automatic_promo(&commerce, &sku, dec!(0.10), None).await;
    let live_code = format!("LIVE-{}", &unique[..8]);
    let live = exclusive_coupon(&commerce, &live_code, None).await;
    let dead_code = format!("DEAD-{}", &unique[..8]);
    let dead = exclusive_coupon(&commerce, &dead_code, Some(Utc::now() - Duration::days(1))).await;

    let request = |code: &str| ApplyPromotionsRequest {
        coupon_codes: vec![code.to_string()],
        line_items: vec![line_item(&sku, 1, dec!(100.00))],
        subtotal: dec!(100.00),
        currency: CurrencyCode::USD,
        ..Default::default()
    };

    // Live Exclusive coupon: it applies alone.
    let priced = commerce.promotions().apply_promotions(request(&live_code)).await.expect("price");
    let applied: Vec<_> = priced.applied_promotions.iter().map(|a| a.promotion_id).collect();
    assert_eq!(applied, vec![live.id], "{priced:?}");
    assert_eq!(priced.total_discount, dec!(20.00));

    // Expired Exclusive coupon: dropped before evaluation, so the automatic
    // promotion is granted — the same set checkout consumes.
    let priced = commerce.promotions().apply_promotions(request(&dead_code)).await.expect("price");
    let applied: Vec<_> = priced.applied_promotions.iter().map(|a| a.promotion_id).collect();
    assert_eq!(applied, vec![auto.id], "{priced:?}");
    assert_eq!(priced.total_discount, dec!(10.00));
    assert!(priced.rejected_promotions.iter().any(|r| {
        r.coupon_code.as_deref() == Some(dead_code.as_str())
            && r.reason_code == RejectionReason::Expired
    }));
    assert!(!applied.contains(&dead.id));
}

#[tokio::test]
async fn postgres_per_customer_limit_applies_to_automatic_promotions() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping promotion candidates test");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");
    let unique = uuid::Uuid::new_v4().to_string();
    let sku = format!("ONCE-{}", &unique[..8]);
    let promo = automatic_promo(&commerce, &sku, dec!(0.10), Some(1)).await;
    let customer = commerce
        .customers()
        .create(CreateCustomer {
            email: format!("once-{}@example.com", &unique[..8]),
            first_name: "Once".into(),
            last_name: "Only".into(),
            phone: None,
            accepts_marketing: None,
            tags: None,
            metadata: None,
        })
        .await
        .expect("create customer");

    let request = |customer_id| ApplyPromotionsRequest {
        customer_id,
        line_items: vec![line_item(&sku, 1, dec!(100.00))],
        subtotal: dec!(100.00),
        currency: CurrencyCode::USD,
        ..Default::default()
    };
    let applies = |priced: &stateset_core::ApplyPromotionsResult| {
        priced.applied_promotions.iter().any(|a| a.promotion_id == promo.id)
    };

    let priced =
        commerce.promotions().apply_promotions(request(Some(customer.id))).await.expect("price");
    assert!(applies(&priced), "first use applies: {priced:?}");
    commerce
        .promotions()
        .record_usage(
            promo.id.into_uuid(),
            None,
            Some(customer.id.into_uuid()),
            None,
            None,
            dec!(10.00),
            "USD",
        )
        .await
        .expect("record usage");

    let priced =
        commerce.promotions().apply_promotions(request(Some(customer.id))).await.expect("price");
    assert!(!applies(&priced), "at the limit: {priced:?}");
    assert!(priced.rejected_promotions.iter().any(|r| {
        r.promotion_id == Some(promo.id) && r.reason_code == RejectionReason::UsageLimitReached
    }));
    // Anonymous carts are not limited (they cannot be attributed).
    let priced = commerce.promotions().apply_promotions(request(None)).await.expect("price");
    assert!(applies(&priced), "{priced:?}");
}
