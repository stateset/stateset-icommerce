//! Postgres parity for first-order promotions (SQLite: `first_order_promotions_test.rs`).
//!
//! Every path that evaluates a cart's promotions built its request with
//! `is_first_order: false`, so a `first_order` condition could never be met
//! and the welcome coupon was refused for every customer. The repositories
//! now settle it from the customer's order history — excluding the order
//! checkout has just inserted — and these tests pin the Postgres backend.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`);
//! skipped otherwise. Promotions are scoped to a per-test SKU so automatic
//! promotions left in the shared database by other tests cannot interfere.

#![cfg(feature = "postgres")]

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{
    AddCartItem, ApplyPromotionsRequest, CartAddress, ConditionOperator, ConditionType, CreateCart,
    CreateCouponCode, CreateCustomer, CreatePromotion, CreatePromotionCondition, CurrencyCode,
    CustomerId, PromotionId, PromotionLineItem, PromotionTrigger, PromotionType, SetCartPayment,
};
use stateset_embedded::AsyncCommerce;
use uuid::Uuid;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

fn unique() -> String {
    Uuid::new_v4().simple().to_string()[..10].to_uppercase()
}

fn first_order_only() -> Vec<CreatePromotionCondition> {
    vec![CreatePromotionCondition {
        condition_type: ConditionType::FirstOrder,
        operator: ConditionOperator::Equals,
        value: "true".to_string(),
        is_required: true,
    }]
}

async fn welcome_coupon(commerce: &AsyncCommerce, sku: &str) -> String {
    let code = format!("WELCOME-{}", unique());
    let promotion = commerce
        .promotions()
        .create(CreatePromotion {
            name: "Welcome 10%".into(),
            promotion_type: PromotionType::PercentageOff,
            trigger: PromotionTrigger::CouponCode,
            percentage_off: Some(dec!(0.10)),
            applicable_skus: Some(vec![sku.to_string()]),
            conditions: Some(first_order_only()),
            ..Default::default()
        })
        .await
        .expect("create welcome promotion");
    commerce.promotions().activate(promotion.id.into_uuid()).await.expect("activate");
    commerce
        .promotions()
        .create_coupon(CreateCouponCode {
            code: code.clone(),
            promotion_id: promotion.id,
            usage_limit: None,
            per_customer_limit: None,
            starts_at: None,
            ends_at: None,
            metadata: None,
        })
        .await
        .expect("create welcome coupon");
    code
}

async fn automatic_welcome(commerce: &AsyncCommerce, sku: &str) -> PromotionId {
    let promotion = commerce
        .promotions()
        .create(CreatePromotion {
            name: "Automatic welcome".into(),
            promotion_type: PromotionType::PercentageOff,
            trigger: PromotionTrigger::Automatic,
            percentage_off: Some(dec!(0.10)),
            applicable_skus: Some(vec![sku.to_string()]),
            conditions: Some(first_order_only()),
            ..Default::default()
        })
        .await
        .expect("create automatic welcome");
    commerce.promotions().activate(promotion.id.into_uuid()).await.expect("activate");
    promotion.id
}

async fn customer(commerce: &AsyncCommerce) -> CustomerId {
    commerce
        .customers()
        .create(CreateCustomer {
            email: format!("first-order-{}@example.com", Uuid::new_v4()),
            first_name: "First".into(),
            last_name: "Order".into(),
            ..Default::default()
        })
        .await
        .expect("create customer")
        .id
}

async fn ready_cart(commerce: &AsyncCommerce, customer_id: CustomerId, sku: &str) -> Uuid {
    let cart = commerce
        .carts()
        .create(CreateCart { customer_id: Some(customer_id), ..Default::default() })
        .await
        .expect("create cart");
    let id = cart.id.into_uuid();
    commerce
        .carts()
        .add_item(
            id,
            AddCartItem {
                sku: sku.into(),
                name: "Welcome widget".into(),
                quantity: 2,
                unit_price: dec!(50.00),
                ..Default::default()
            },
        )
        .await
        .expect("add item");
    commerce
        .carts()
        .set_shipping_address(
            id,
            CartAddress {
                first_name: "Ada".into(),
                last_name: "Lovelace".into(),
                line1: "1 Analytical Way".into(),
                city: "San Francisco".into(),
                postal_code: "94102".into(),
                country: "US".into(),
                state: Some("CA".into()),
                ..Default::default()
            },
        )
        .await
        .expect("set shipping address");
    commerce
        .carts()
        .set_payment(
            id,
            SetCartPayment {
                payment_method: "credit_card".into(),
                payment_token: Some("tok_test".into()),
                ..Default::default()
            },
        )
        .await
        .expect("set payment");
    id
}

async fn place_prior_order(commerce: &AsyncCommerce, customer_id: CustomerId) {
    let cart = ready_cart(commerce, customer_id, &format!("PRIOR-{}", unique())).await;
    commerce.carts().complete(cart).await.expect("prior order checks out");
}

fn quote(customer_id: CustomerId, sku: &str, claims_first_order: bool) -> ApplyPromotionsRequest {
    ApplyPromotionsRequest {
        customer_id: Some(customer_id),
        line_items: vec![PromotionLineItem {
            id: "line-1".into(),
            product_id: None,
            variant_id: None,
            sku: Some(sku.into()),
            category_ids: vec![],
            quantity: 2,
            unit_price: dec!(50.00),
            line_total: dec!(100.00),
        }],
        subtotal: dec!(100.00),
        shipping_amount: Decimal::ZERO,
        currency: CurrencyCode::USD,
        is_first_order: claims_first_order,
        ..Default::default()
    }
}

#[tokio::test]
async fn postgres_new_customer_redeems_welcome_coupon_through_checkout() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");
    let sku = format!("WELCOME-SKU-{}", unique());
    let code = welcome_coupon(&commerce, &sku).await;
    let shopper = customer(&commerce).await;
    let cart = ready_cart(&commerce, shopper, &sku).await;

    let discounted =
        commerce.carts().apply_discount(cart, &code).await.expect("a first order qualifies");
    assert_eq!(discounted.discount_amount, dec!(10.00));

    let result = commerce.carts().complete(cart).await.expect("welcome coupon survives checkout");
    let order = commerce
        .orders()
        .get(result.order_id.into_uuid())
        .await
        .expect("get order")
        .expect("order exists");
    assert_eq!(order.discount_amount, dec!(10.00));
}

#[tokio::test]
async fn postgres_returning_customer_cannot_apply_welcome_coupon() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");
    let sku = format!("WELCOME-SKU-{}", unique());
    let code = welcome_coupon(&commerce, &sku).await;
    let shopper = customer(&commerce).await;
    place_prior_order(&commerce, shopper).await;

    let cart = ready_cart(&commerce, shopper, &sku).await;
    commerce
        .carts()
        .apply_discount(cart, &code)
        .await
        .expect_err("a second order is not a first order");
}

#[tokio::test]
async fn postgres_automatic_welcome_follows_order_history_through_checkout() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");
    let sku = format!("WELCOME-SKU-{}", unique());
    let welcome = automatic_welcome(&commerce, &sku).await;
    let shopper = customer(&commerce).await;

    let first =
        commerce.promotions().apply_promotions(quote(shopper, &sku, false)).await.expect("apply");
    assert_eq!(first.total_discount, dec!(10.00), "an identified customer with no orders");

    // Checkout consumes the automatic promotion after inserting the order;
    // that order must not count as history, or usage is never recorded.
    let cart = ready_cart(&commerce, shopper, &sku).await;
    commerce.carts().complete(cart).await.expect("first order checks out");
    let promotion = commerce
        .promotions()
        .get(welcome.into_uuid())
        .await
        .expect("get promotion")
        .expect("promotion exists");
    assert_eq!(promotion.usage_count, 1, "checkout recorded the automatic promotion's usage");

    let second =
        commerce.promotions().apply_promotions(quote(shopper, &sku, true)).await.expect("apply");
    assert_eq!(
        second.total_discount,
        Decimal::ZERO,
        "order history overrules a caller who claims a returning customer is new"
    );
}
