//! Postgres mirrors of two promotion behaviours that only had SQLite / pure
//! engine coverage: exclusive-vs-stackable ordering, and Buy X Get Y.
//!
//! Both backends resolve their candidates and then delegate to the shared
//! `evaluate_promotions`, so these assert that the Postgres candidate
//! resolution actually feeds it what the engine expects — stacking decided by
//! priority order, and a BOGO promotion's quantities surviving the round trip
//! through the database.
//!
//! Every promotion here is coupon-triggered and carries a very low priority,
//! so it is evaluated before any automatic promotion another test may have
//! left active on the shared database, and every assertion is scoped to this
//! test's own promotion ids.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`);
//! skipped otherwise.

#![cfg(feature = "postgres")]

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{
    ApplyPromotionsRequest, ApplyPromotionsResult, CreateCouponCode, CreatePromotion, CurrencyCode,
    Promotion, PromotionLineItem, PromotionTarget, PromotionTrigger, PromotionType,
    RejectionReason, StackingBehavior,
};
use stateset_embedded::AsyncCommerce;
use uuid::Uuid;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

async fn connect() -> Option<AsyncCommerce> {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping promotion lifecycle test");
        return None;
    };
    Some(AsyncCommerce::connect(&url).await.expect("connect to postgres and run migrations"))
}

fn line_item(sku: &str, quantity: i32, unit_price: Decimal) -> PromotionLineItem {
    PromotionLineItem {
        id: sku.to_string(),
        product_id: None,
        variant_id: None,
        sku: Some(sku.to_string()),
        category_ids: vec![],
        quantity,
        unit_price,
        line_total: unit_price * Decimal::from(quantity),
    }
}

/// Create an activated, coupon-triggered promotion and its coupon code.
async fn coupon_promotion(commerce: &AsyncCommerce, input: CreatePromotion) -> (Promotion, String) {
    let code = input.code.clone().expect("caller supplies a code");
    let promo = commerce.promotions().create(input).await.expect("create promotion");
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
    (promo, code)
}

fn request(codes: Vec<String>, items: Vec<PromotionLineItem>) -> ApplyPromotionsRequest {
    let subtotal = items.iter().map(|i| i.line_total).sum();
    ApplyPromotionsRequest {
        cart_id: None,
        customer_id: None,
        coupon_codes: codes,
        line_items: items,
        subtotal,
        shipping_amount: dec!(10.00),
        shipping_country: None,
        shipping_state: None,
        currency: CurrencyCode::USD,
        is_first_order: false,
    }
}

fn applied(result: &ApplyPromotionsResult, promo: &Promotion) -> Option<Decimal> {
    result.applied_promotions.iter().find(|a| a.promotion_id == promo.id).map(|a| a.discount_amount)
}

fn rejected_because(result: &ApplyPromotionsResult, promo: &Promotion) -> Option<RejectionReason> {
    result
        .rejected_promotions
        .iter()
        .find(|r| r.promotion_id == Some(promo.id))
        .map(|r| r.reason_code)
}

/// An Exclusive promotion stands alone in BOTH directions, and priority — not
/// the order Postgres returned the candidates in — decides which one that is.
#[tokio::test]
async fn postgres_exclusive_promotion_blocks_stackable_by_priority() {
    let Some(commerce) = connect().await else { return };
    let unique = Uuid::new_v4().to_string();
    let stackable_code = format!("STACK-{}", &unique[..8]);
    let exclusive_code = format!("EXCL-{}", &unique[..8]);

    // The stackable one sorts first, so it applies and the exclusive one
    // cannot join it.
    let (stackable, stackable_code) = coupon_promotion(
        &commerce,
        CreatePromotion {
            code: Some(stackable_code),
            name: "10% off (stackable)".into(),
            promotion_type: PromotionType::PercentageOff,
            trigger: PromotionTrigger::CouponCode,
            target: PromotionTarget::Order,
            stacking: StackingBehavior::Stackable,
            percentage_off: Some(dec!(0.10)),
            priority: Some(-1000),
            ..Default::default()
        },
    )
    .await;
    let (exclusive, exclusive_code) = coupon_promotion(
        &commerce,
        CreatePromotion {
            code: Some(exclusive_code),
            name: "20% off (exclusive)".into(),
            promotion_type: PromotionType::PercentageOff,
            trigger: PromotionTrigger::CouponCode,
            target: PromotionTarget::Order,
            stacking: StackingBehavior::Exclusive,
            percentage_off: Some(dec!(0.20)),
            priority: Some(-999),
            ..Default::default()
        },
    )
    .await;

    let items = vec![line_item("WIDGET", 1, dec!(100.00))];
    let result = commerce
        .promotions()
        .apply_promotions(request(
            vec![stackable_code.clone(), exclusive_code.clone()],
            items.clone(),
        ))
        .await
        .expect("apply");
    assert_eq!(applied(&result, &stackable), Some(dec!(10.00)), "{result:?}");
    assert_eq!(applied(&result, &exclusive), None, "an exclusive cannot join an applied promotion");
    assert_eq!(rejected_because(&result, &exclusive), Some(RejectionReason::NotStackable));

    // Give the exclusive one the lower priority and the outcome flips: it
    // applies alone and blocks the stackable one.
    commerce
        .promotions()
        .update(
            exclusive.id.into_uuid(),
            stateset_core::UpdatePromotion { priority: Some(-1001), ..Default::default() },
        )
        .await
        .expect("reprioritise");

    let result = commerce
        .promotions()
        .apply_promotions(request(vec![stackable_code, exclusive_code], items))
        .await
        .expect("apply");
    assert_eq!(applied(&result, &exclusive), Some(dec!(20.00)), "{result:?}");
    assert_eq!(applied(&result, &stackable), None);
    assert_eq!(rejected_because(&result, &stackable), Some(RejectionReason::NotStackable));
}

/// Buy X Get Y over the Postgres backend: the buy/get quantities and the
/// "get" discount survive the round trip, and every full set of `buy + get`
/// in-scope units earns `get` of them at the configured discount.
#[tokio::test]
async fn postgres_buy_x_get_y_grants_one_free_item_per_full_set() {
    let Some(commerce) = connect().await else { return };
    let unique = Uuid::new_v4().to_string();

    let (promo, code) = coupon_promotion(
        &commerce,
        CreatePromotion {
            code: Some(format!("BOGO-{}", &unique[..8])),
            name: "Buy 2 get 1 free".into(),
            promotion_type: PromotionType::BuyXGetY,
            trigger: PromotionTrigger::CouponCode,
            target: PromotionTarget::Order,
            stacking: StackingBehavior::Stackable,
            buy_quantity: Some(2),
            get_quantity: Some(1),
            get_discount_percent: Some(Decimal::ONE),
            applicable_skus: Some(vec!["BOGO-WIDGET".into()]),
            priority: Some(-1000),
            ..Default::default()
        },
    )
    .await;
    let reloaded =
        commerce.promotions().get(promo.id.into_uuid()).await.expect("get").expect("exists");
    assert_eq!(reloaded.buy_quantity, Some(2));
    assert_eq!(reloaded.get_quantity, Some(1));
    assert_eq!(reloaded.get_discount_percent, Some(Decimal::ONE));

    // 5 in-scope units at $10: one full set of 3 -> one free unit.
    let result = commerce
        .promotions()
        .apply_promotions(request(
            vec![code.clone()],
            vec![line_item("BOGO-WIDGET", 5, dec!(10.00))],
        ))
        .await
        .expect("apply");
    assert_eq!(applied(&result, &promo), Some(dec!(10.00)), "{result:?}");

    // 6 units: two sets -> two free units.
    let result = commerce
        .promotions()
        .apply_promotions(request(
            vec![code.clone()],
            vec![line_item("BOGO-WIDGET", 6, dec!(10.00))],
        ))
        .await
        .expect("apply");
    assert_eq!(applied(&result, &promo), Some(dec!(20.00)), "{result:?}");

    // 2 units: no full set -> the promotion contributes nothing.
    let result = commerce
        .promotions()
        .apply_promotions(request(
            vec![code.clone()],
            vec![line_item("BOGO-WIDGET", 2, dec!(10.00))],
        ))
        .await
        .expect("apply");
    assert_eq!(applied(&result, &promo), None, "{result:?}");

    // Out-of-scope items earn nothing, however many there are.
    let result = commerce
        .promotions()
        .apply_promotions(request(vec![code], vec![line_item("OTHER", 9, dec!(10.00))]))
        .await
        .expect("apply");
    assert_eq!(applied(&result, &promo), None, "{result:?}");
}
