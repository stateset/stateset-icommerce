#![cfg(feature = "sqlite")]

//! First-order promotions are decided by the customer's order history.
//!
//! Every engine path that builds an [`ApplyPromotionsRequest`] from a cart set
//! `is_first_order: false`, and nothing ever set it back. A `first_order`
//! condition could therefore never be met: the canonical welcome coupon was
//! refused when applied to the cart and again at checkout, for every customer,
//! including one who had never ordered. These tests pin the rule that replaced
//! it — an identified customer's first order is the one with no other order
//! on record — on every path that evaluates it.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{
    ApplyPromotionsRequest, CartId, ConditionOperator, ConditionType, CreatePromotionCondition,
    CurrencyCode, CustomerId, PromotionId, PromotionLineItem, PromotionTrigger,
};
use stateset_embedded::{
    AddCartItem, CartAddress, Commerce, CreateCart, CreateCouponCode, CreateCustomer,
    CreatePromotion, PromotionType, SetCartPayment,
};
use uuid::Uuid;

fn first_order_only() -> Vec<CreatePromotionCondition> {
    vec![CreatePromotionCondition {
        condition_type: ConditionType::FirstOrder,
        operator: ConditionOperator::Equals,
        value: "true".to_string(),
        is_required: true,
    }]
}

fn welcome_coupon(commerce: &Commerce) {
    let promotion = commerce
        .promotions()
        .create(CreatePromotion {
            name: "Welcome 10%".into(),
            promotion_type: PromotionType::PercentageOff,
            trigger: PromotionTrigger::CouponCode,
            percentage_off: Some(dec!(0.10)),
            conditions: Some(first_order_only()),
            ..Default::default()
        })
        .expect("create welcome promotion");
    commerce.promotions().activate(promotion.id).expect("activate welcome promotion");
    commerce
        .promotions()
        .create_coupon(CreateCouponCode {
            code: "WELCOME10".into(),
            promotion_id: promotion.id,
            usage_limit: None,
            per_customer_limit: None,
            starts_at: None,
            ends_at: None,
            metadata: None,
        })
        .expect("create welcome coupon");
}

fn automatic_welcome(commerce: &Commerce) -> PromotionId {
    let promotion = commerce
        .promotions()
        .create(CreatePromotion {
            name: "Automatic welcome".into(),
            promotion_type: PromotionType::PercentageOff,
            trigger: PromotionTrigger::Automatic,
            percentage_off: Some(dec!(0.10)),
            conditions: Some(first_order_only()),
            ..Default::default()
        })
        .expect("create automatic welcome promotion");
    commerce.promotions().activate(promotion.id).expect("activate automatic welcome");
    promotion.id
}

fn customer(commerce: &Commerce) -> CustomerId {
    commerce
        .customers()
        .create(CreateCustomer {
            email: format!("first-order-{}@example.com", Uuid::new_v4()),
            first_name: "First".into(),
            last_name: "Order".into(),
            ..Default::default()
        })
        .expect("create customer")
        .id
}

fn ready_cart(commerce: &Commerce, customer_id: CustomerId) -> CartId {
    let cart = commerce
        .carts()
        .create(CreateCart { customer_id: Some(customer_id), ..Default::default() })
        .expect("create cart");
    commerce
        .carts()
        .add_item(
            cart.id,
            AddCartItem {
                sku: "WELCOME-SKU".into(),
                name: "Welcome widget".into(),
                quantity: 2,
                unit_price: dec!(50.00),
                ..Default::default()
            },
        )
        .expect("add item");
    commerce
        .carts()
        .set_shipping_address(
            cart.id,
            CartAddress {
                first_name: "Ada".into(),
                last_name: "Lovelace".into(),
                line1: "1 Analytical Way".into(),
                city: "London".into(),
                postal_code: "94102".into(),
                country: "US".into(),
                state: Some("CA".into()),
                ..Default::default()
            },
        )
        .expect("set shipping address");
    commerce
        .carts()
        .set_payment(
            cart.id,
            SetCartPayment {
                payment_method: "credit_card".into(),
                payment_token: Some("tok_test".into()),
                ..Default::default()
            },
        )
        .expect("set payment");
    cart.id
}

/// Give `customer_id` an order on record by checking a plain cart out.
fn place_prior_order(commerce: &Commerce, customer_id: CustomerId) {
    let cart = ready_cart(commerce, customer_id);
    commerce.carts().complete(cart).expect("prior order checks out");
}

fn quote(customer_id: Option<CustomerId>, claims_first_order: bool) -> ApplyPromotionsRequest {
    ApplyPromotionsRequest {
        customer_id,
        line_items: vec![PromotionLineItem {
            id: "line-1".into(),
            product_id: None,
            variant_id: None,
            sku: Some("WELCOME-SKU".into()),
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

#[test]
fn a_new_customer_redeems_a_welcome_coupon_through_checkout() {
    let commerce = Commerce::new(":memory:").expect("commerce");
    welcome_coupon(&commerce);
    let shopper = customer(&commerce);
    let cart = ready_cart(&commerce, shopper);

    let discounted =
        commerce.carts().apply_discount(cart, "WELCOME10").expect("a first order qualifies");
    assert_eq!(discounted.discount_amount, dec!(10.00));

    // Checkout inserts the order before it consumes the coupon; the order
    // being placed must not count as the customer's history.
    let result = commerce.carts().complete(cart).expect("the welcome coupon survives checkout");
    let order = commerce.orders().get(result.order_id).expect("get order").expect("order exists");
    assert_eq!(order.discount_amount, dec!(10.00));

    let coupon = commerce
        .promotions()
        .get_coupon_by_code("WELCOME10")
        .expect("get coupon")
        .expect("coupon exists");
    assert_eq!(coupon.usage_count, 1, "checkout consumed the coupon exactly once");
}

#[test]
fn a_returning_customer_cannot_apply_a_welcome_coupon() {
    let commerce = Commerce::new(":memory:").expect("commerce");
    welcome_coupon(&commerce);
    let shopper = customer(&commerce);
    place_prior_order(&commerce, shopper);

    let cart = ready_cart(&commerce, shopper);
    let err = commerce
        .carts()
        .apply_discount(cart, "WELCOME10")
        .expect_err("a second order is not a first order");
    assert!(err.to_string().to_lowercase().contains("condition"), "unexpected refusal: {err}");
}

#[test]
fn a_welcome_coupon_applied_before_the_first_order_cannot_ride_a_second() {
    // The coupon qualified when applied, but by checkout the customer has
    // ordered elsewhere: checkout re-decides from history and refuses.
    let commerce = Commerce::new(":memory:").expect("commerce");
    welcome_coupon(&commerce);
    let shopper = customer(&commerce);

    let cart = ready_cart(&commerce, shopper);
    commerce.carts().apply_discount(cart, "WELCOME10").expect("qualifies while first");
    place_prior_order(&commerce, shopper);

    commerce.carts().complete(cart).expect_err("no longer a first order at checkout");
}

#[test]
fn an_automatic_welcome_promotion_follows_order_history() {
    let commerce = Commerce::new(":memory:").expect("commerce");
    let _ = automatic_welcome(&commerce);
    let shopper = customer(&commerce);

    let first = commerce.promotions().apply(quote(Some(shopper), false)).expect("apply");
    assert_eq!(first.total_discount, dec!(10.00), "an identified customer with no orders");

    place_prior_order(&commerce, shopper);
    let second = commerce.promotions().apply(quote(Some(shopper), true)).expect("apply");
    assert_eq!(
        second.total_discount,
        Decimal::ZERO,
        "order history overrules a caller who claims a returning customer is new"
    );
}

#[test]
fn an_anonymous_quote_keeps_the_callers_claim() {
    // With no customer there is no history to consult; the quote is a
    // preview, and checkout (which always has a customer) decides for real.
    let commerce = Commerce::new(":memory:").expect("commerce");
    let _ = automatic_welcome(&commerce);

    let claimed = commerce.promotions().apply(quote(None, true)).expect("apply");
    assert_eq!(claimed.total_discount, dec!(10.00));
    let unclaimed = commerce.promotions().apply(quote(None, false)).expect("apply");
    assert_eq!(unclaimed.total_discount, Decimal::ZERO);
}

#[test]
fn cart_promotions_apply_the_automatic_welcome_to_a_new_customer() {
    let commerce = Commerce::new(":memory:").expect("commerce");
    let welcome = automatic_welcome(&commerce);
    let shopper = customer(&commerce);
    let cart = ready_cart(&commerce, shopper);

    let priced = commerce.apply_cart_promotions(cart.into_uuid()).expect("price cart");
    assert_eq!(priced.total_discount, dec!(10.00));

    let result = commerce.carts().complete(cart).expect("checkout");
    let order = commerce.orders().get(result.order_id).expect("get order").expect("order exists");
    assert_eq!(order.discount_amount, dec!(10.00));

    // Checkout re-evaluates after inserting the order. Counted as history,
    // that order made the promotion ineligible, so the discount went out on
    // the order but its usage was never recorded and no limit advanced.
    let promotion =
        commerce.promotions().get(welcome).expect("get promotion").expect("promotion exists");
    assert_eq!(promotion.usage_count, 1, "checkout recorded the automatic promotion's usage");
}
