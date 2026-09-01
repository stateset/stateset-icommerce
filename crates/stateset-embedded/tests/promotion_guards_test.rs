#![cfg(feature = "sqlite")]

//! Regression tests for promotion condition evaluation and discount math
//! (SQLite backend, sync `Commerce` engine).
//!
//! The headline defect: promotion conditions used to fail **OPEN**. Every
//! `ConditionType` the evaluator did not have an explicit arm for fell through
//! to `_ => Ok(true)`, so a promotion gated on `customer_group = VIP` (or on a
//! specific product being in the cart) applied to *every* cart. That is a
//! money leak that scales with traffic.
//!
//! Covers:
//! - conditions the evaluator has no data for (`customer_group`,
//!   `customer_email_domain`, `payment_method`) refuse the promotion instead of
//!   applying it;
//! - every condition variant that *is* evaluated, both satisfied and not;
//! - an unsupported operator for a condition type refuses rather than applies;
//! - a promotion with no conditions still applies;
//! - the `total_usage_limit` / `per_customer_limit` guards in `record_usage`
//!   still hold;
//! - discount math stays clamped to the eligible item value and the subtotal.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_embedded::{
    ApplyPromotionsRequest, ApplyPromotionsResult, Commerce, CommerceError, ConditionOperator,
    ConditionType, CreateCouponCode, CreateCustomer, CreatePromotion, CreatePromotionCondition,
    CurrencyCode, CustomerId, ProductId, Promotion, PromotionLineItem, PromotionTarget,
    PromotionTrigger, PromotionType, StackingBehavior, UpdatePromotion,
};
use uuid::Uuid;

// ============================================================================
// Fixtures
// ============================================================================

fn new_commerce() -> Commerce {
    Commerce::new(":memory:").expect("Failed to create in-memory Commerce")
}

fn required(
    condition_type: ConditionType,
    operator: ConditionOperator,
    value: &str,
) -> CreatePromotionCondition {
    CreatePromotionCondition {
        condition_type,
        operator,
        value: value.to_string(),
        is_required: true,
    }
}

fn optional(
    condition_type: ConditionType,
    operator: ConditionOperator,
    value: &str,
) -> CreatePromotionCondition {
    CreatePromotionCondition {
        condition_type,
        operator,
        value: value.to_string(),
        is_required: false,
    }
}

/// An active, automatic 10%-off-order promotion carrying `conditions`.
fn pct_promo(
    commerce: &Commerce,
    name: &str,
    conditions: Vec<CreatePromotionCondition>,
) -> Promotion {
    pct_promo_scoped(commerce, name, conditions, None)
}

fn pct_promo_scoped(
    commerce: &Commerce,
    name: &str,
    conditions: Vec<CreatePromotionCondition>,
    applicable_skus: Option<Vec<String>>,
) -> Promotion {
    let promo = commerce
        .promotions()
        .create(CreatePromotion {
            name: name.to_string(),
            promotion_type: PromotionType::PercentageOff,
            trigger: PromotionTrigger::Automatic,
            target: PromotionTarget::Order,
            stacking: StackingBehavior::Stackable,
            percentage_off: Some(dec!(0.10)),
            conditions: if conditions.is_empty() { None } else { Some(conditions) },
            applicable_skus,
            ..Default::default()
        })
        .expect("Failed to create promotion");

    commerce.promotions().activate(promo.id).expect("Failed to activate promotion")
}

fn line_item(
    sku: &str,
    product_id: Option<ProductId>,
    quantity: i32,
    line_total: Decimal,
) -> PromotionLineItem {
    PromotionLineItem {
        id: sku.to_string(),
        product_id,
        variant_id: None,
        sku: Some(sku.to_string()),
        category_ids: vec![],
        quantity,
        unit_price: line_total / Decimal::from(quantity),
        line_total,
    }
}

/// A $100 cart: 2x WIDGET at $50.
fn cart() -> ApplyPromotionsRequest {
    ApplyPromotionsRequest {
        line_items: vec![line_item("WIDGET", None, 2, dec!(100.00))],
        subtotal: dec!(100.00),
        shipping_amount: dec!(10.00),
        currency: CurrencyCode::USD,
        ..Default::default()
    }
}

fn make_customer(commerce: &Commerce) -> CustomerId {
    commerce
        .customers()
        .create(CreateCustomer {
            email: format!("promo-{}@example.com", Uuid::new_v4()),
            first_name: "Promo".into(),
            last_name: "Tester".into(),
            phone: None,
            accepts_marketing: Some(false),
            tags: None,
            metadata: None,
        })
        .expect("Failed to create customer")
        .id
}

fn apply(commerce: &Commerce, request: ApplyPromotionsRequest) -> ApplyPromotionsResult {
    commerce.promotions().apply(request).expect("Failed to apply promotions")
}

#[track_caller]
fn assert_refused(result: &ApplyPromotionsResult, why: &str) {
    assert!(
        result.applied_promotions.is_empty(),
        "{why}: promotion must not apply, got {:?}",
        result.applied_promotions
    );
    assert_eq!(result.total_discount, Decimal::ZERO, "{why}: cart total must be unchanged");
    assert_eq!(
        result.discounted_subtotal, result.original_subtotal,
        "{why}: subtotal must be unchanged"
    );
    assert_eq!(result.grand_total, dec!(110.00), "{why}: grand total must be unchanged");
    assert!(
        !result.rejected_promotions.is_empty(),
        "{why}: the refusal must be reported, not swallowed"
    );
}

#[track_caller]
fn assert_applied(result: &ApplyPromotionsResult, expected_discount: Decimal, why: &str) {
    assert_eq!(
        result.applied_promotions.len(),
        1,
        "{why}: expected exactly one applied promotion, got {result:?}"
    );
    assert_eq!(result.total_discount, expected_discount, "{why}: {result:?}");
}

// ============================================================================
// Headline: conditions must fail CLOSED
// ============================================================================

#[test]
fn customer_group_condition_cannot_be_evaluated_and_refuses_promotion() {
    // The evaluator has no customer-group data on an ApplyPromotionsRequest.
    // Before the fix this fell through to `_ => Ok(true)` and the VIP-only
    // discount leaked to every cart in the store.
    let commerce = new_commerce();
    pct_promo(
        &commerce,
        "VIP members only",
        vec![required(ConditionType::CustomerGroup, ConditionOperator::Equals, "VIP")],
    );

    let result = apply(&commerce, cart());

    assert_refused(&result, "customer_group is not evaluatable");
}

#[test]
fn customer_email_domain_condition_fails_closed() {
    let commerce = new_commerce();
    pct_promo(
        &commerce,
        "Staff only",
        vec![required(
            ConditionType::CustomerEmailDomain,
            ConditionOperator::Equals,
            "stateset.com",
        )],
    );

    let result = apply(&commerce, cart());

    assert_refused(&result, "customer_email_domain is not evaluatable");
}

#[test]
fn payment_method_condition_fails_closed() {
    let commerce = new_commerce();
    pct_promo(
        &commerce,
        "Pay by ACH and save",
        vec![required(ConditionType::PaymentMethod, ConditionOperator::Equals, "ach")],
    );

    let result = apply(&commerce, cart());

    assert_refused(&result, "payment_method is not known at pricing time");
}

#[test]
fn unsupported_operator_for_a_condition_fails_closed() {
    // `product_in_cart > <uuid>` is meaningless. It must refuse, not apply.
    let commerce = new_commerce();
    pct_promo(
        &commerce,
        "Nonsense operator",
        vec![required(
            ConditionType::ProductInCart,
            ConditionOperator::GreaterThan,
            &Uuid::new_v4().to_string(),
        )],
    );

    let result = apply(&commerce, cart());

    assert_refused(&result, "an inapplicable operator");
}

#[test]
fn promotion_without_conditions_still_applies() {
    let commerce = new_commerce();
    pct_promo(&commerce, "Sitewide 10%", vec![]);

    let result = apply(&commerce, cart());

    assert_applied(&result, dec!(10.00), "an unconditional promotion must still apply");
    assert_eq!(result.grand_total, dec!(100.00));
}

// ============================================================================
// Implemented condition variants
// ============================================================================

#[test]
fn minimum_subtotal_condition_applies_when_met() {
    let commerce = new_commerce();
    pct_promo(
        &commerce,
        "Spend $50",
        vec![required(
            ConditionType::MinimumSubtotal,
            ConditionOperator::GreaterThanOrEqual,
            "50.00",
        )],
    );

    assert_applied(&apply(&commerce, cart()), dec!(10.00), "subtotal 100 >= 50");
}

#[test]
fn minimum_subtotal_condition_refuses_when_not_met() {
    let commerce = new_commerce();
    pct_promo(
        &commerce,
        "Spend $500",
        vec![required(
            ConditionType::MinimumSubtotal,
            ConditionOperator::GreaterThanOrEqual,
            "500.00",
        )],
    );

    assert_refused(&apply(&commerce, cart()), "subtotal 100 < 500");
}

#[test]
fn minimum_quantity_condition_gates_on_total_units() {
    let commerce = new_commerce();
    pct_promo(
        &commerce,
        "Buy 2+",
        vec![required(ConditionType::MinimumQuantity, ConditionOperator::GreaterThanOrEqual, "2")],
    );
    assert_applied(&apply(&commerce, cart()), dec!(10.00), "2 units >= 2");

    let strict = new_commerce();
    pct_promo(
        &strict,
        "Buy 5+",
        vec![required(ConditionType::MinimumQuantity, ConditionOperator::GreaterThanOrEqual, "5")],
    );
    assert_refused(&apply(&strict, cart()), "2 units < 5");
}

#[test]
fn cart_item_count_condition_gates_on_distinct_lines() {
    let commerce = new_commerce();
    pct_promo(
        &commerce,
        "One line only",
        vec![required(ConditionType::CartItemCount, ConditionOperator::Equals, "1")],
    );
    assert_applied(&apply(&commerce, cart()), dec!(10.00), "1 line == 1");

    let strict = new_commerce();
    pct_promo(
        &strict,
        "Three lines",
        vec![required(ConditionType::CartItemCount, ConditionOperator::Equals, "3")],
    );
    assert_refused(&apply(&strict, cart()), "1 line != 3");
}

#[test]
fn product_in_cart_condition_matches_line_item_product() {
    let wanted = ProductId::new();
    let commerce = new_commerce();
    pct_promo(
        &commerce,
        "Only with the featured product",
        vec![required(ConditionType::ProductInCart, ConditionOperator::In, &wanted.to_string())],
    );

    let mut request = cart();
    request.line_items = vec![line_item("WIDGET", Some(wanted), 2, dec!(100.00))];
    assert_applied(&apply(&commerce, request), dec!(10.00), "the wanted product is in the cart");
}

#[test]
fn product_in_cart_condition_refuses_when_product_absent() {
    let commerce = new_commerce();
    pct_promo(
        &commerce,
        "Only with the featured product",
        vec![required(
            ConditionType::ProductInCart,
            ConditionOperator::In,
            &ProductId::new().to_string(),
        )],
    );

    let mut request = cart();
    request.line_items = vec![line_item("WIDGET", Some(ProductId::new()), 2, dec!(100.00))];
    assert_refused(&apply(&commerce, request), "the wanted product is not in the cart");
}

#[test]
fn category_in_cart_condition_matches_line_item_category() {
    let category = Uuid::new_v4();
    let commerce = new_commerce();
    pct_promo(
        &commerce,
        "Outerwear sale",
        vec![required(ConditionType::CategoryInCart, ConditionOperator::In, &category.to_string())],
    );

    let mut request = cart();
    let mut item = line_item("WIDGET", None, 2, dec!(100.00));
    item.category_ids = vec![category];
    request.line_items = vec![item];
    assert_applied(&apply(&commerce, request), dec!(10.00), "the category is in the cart");
}

#[test]
fn category_in_cart_condition_refuses_when_category_absent() {
    let commerce = new_commerce();
    pct_promo(
        &commerce,
        "Outerwear sale",
        vec![required(
            ConditionType::CategoryInCart,
            ConditionOperator::In,
            &Uuid::new_v4().to_string(),
        )],
    );

    // The default cart line carries no categories at all.
    assert_refused(&apply(&commerce, cart()), "no line item is in the category");
}

#[test]
fn sku_in_cart_condition_matches_case_insensitively() {
    let commerce = new_commerce();
    pct_promo(
        &commerce,
        "Widgets only",
        vec![required(ConditionType::SkuInCart, ConditionOperator::In, "gadget,widget")],
    );

    assert_applied(&apply(&commerce, cart()), dec!(10.00), "WIDGET is in the cart");
}

#[test]
fn sku_in_cart_condition_refuses_when_sku_absent() {
    let commerce = new_commerce();
    pct_promo(
        &commerce,
        "Gadgets only",
        vec![required(ConditionType::SkuInCart, ConditionOperator::In, "GADGET")],
    );

    assert_refused(&apply(&commerce, cart()), "GADGET is not in the cart");
}

#[test]
fn sku_not_in_cart_condition_negates() {
    let commerce = new_commerce();
    pct_promo(
        &commerce,
        "Not on clearance items",
        vec![required(ConditionType::SkuInCart, ConditionOperator::NotIn, "CLEARANCE")],
    );
    assert_applied(&apply(&commerce, cart()), dec!(10.00), "no CLEARANCE item in the cart");

    let blocked = new_commerce();
    pct_promo(
        &blocked,
        "Not on clearance items",
        vec![required(ConditionType::SkuInCart, ConditionOperator::NotIn, "WIDGET")],
    );
    assert_refused(&apply(&blocked, cart()), "a WIDGET is in the cart");
}

#[test]
fn customer_id_condition_matches_the_requesting_customer() {
    let customer = CustomerId::new();
    let commerce = new_commerce();
    pct_promo(
        &commerce,
        "For one customer",
        vec![required(ConditionType::CustomerId, ConditionOperator::In, &customer.to_string())],
    );

    let mut request = cart();
    request.customer_id = Some(customer);
    assert_applied(&apply(&commerce, request), dec!(10.00), "the customer matches");
}

#[test]
fn customer_id_condition_refuses_other_customers() {
    let commerce = new_commerce();
    pct_promo(
        &commerce,
        "For one customer",
        vec![required(
            ConditionType::CustomerId,
            ConditionOperator::In,
            &CustomerId::new().to_string(),
        )],
    );

    let mut request = cart();
    request.customer_id = Some(CustomerId::new());
    assert_refused(&apply(&commerce, request), "a different customer");
}

#[test]
fn customer_id_condition_fails_closed_for_anonymous_carts() {
    // An anonymous cart cannot be proven to be (or not to be) the targeted
    // customer, so the promotion must not apply.
    let commerce = new_commerce();
    pct_promo(
        &commerce,
        "For one customer",
        vec![required(
            ConditionType::CustomerId,
            ConditionOperator::NotIn,
            &CustomerId::new().to_string(),
        )],
    );

    assert_refused(&apply(&commerce, cart()), "the shopper is not identified");
}

#[test]
fn first_order_condition_honours_operator_and_value() {
    let commerce = new_commerce();
    pct_promo(
        &commerce,
        "Welcome discount",
        vec![required(ConditionType::FirstOrder, ConditionOperator::Equals, "true")],
    );

    let mut first = cart();
    first.is_first_order = true;
    assert_applied(&apply(&commerce, first), dec!(10.00), "this is a first order");

    // A returning customer must not get the welcome discount.
    assert_refused(&apply(&commerce, cart()), "this is not a first order");
}

#[test]
fn first_order_condition_can_target_returning_customers() {
    // `first_order != true` means "returning customers only". Before the fix
    // the operator and value were ignored entirely, so this leaked to first
    // orders too.
    let commerce = new_commerce();
    pct_promo(
        &commerce,
        "Loyalty discount",
        vec![required(ConditionType::FirstOrder, ConditionOperator::NotEquals, "true")],
    );

    assert_applied(&apply(&commerce, cart()), dec!(10.00), "a returning customer");

    let mut first = cart();
    first.is_first_order = true;
    assert_refused(&apply(&commerce, first), "a first order must not get the loyalty discount");
}

#[test]
fn shipping_country_condition_gates_on_destination() {
    let commerce = new_commerce();
    pct_promo(
        &commerce,
        "US only",
        vec![required(ConditionType::ShippingCountry, ConditionOperator::Equals, "US")],
    );

    let mut us = cart();
    us.shipping_country = Some("us".into());
    assert_applied(&apply(&commerce, us), dec!(10.00), "shipping to the US");

    let mut ca = cart();
    ca.shipping_country = Some("CA".into());
    assert_refused(&apply(&commerce, ca), "shipping to Canada");
}

#[test]
fn shipping_country_condition_fails_closed_when_destination_unknown() {
    let commerce = new_commerce();
    pct_promo(
        &commerce,
        "US only",
        vec![required(ConditionType::ShippingCountry, ConditionOperator::Equals, "US")],
    );

    assert_refused(&apply(&commerce, cart()), "no shipping destination on the request");
}

#[test]
fn shipping_state_condition_gates_on_destination() {
    let commerce = new_commerce();
    pct_promo(
        &commerce,
        "California only",
        vec![required(ConditionType::ShippingState, ConditionOperator::Equals, "CA")],
    );

    let mut ca = cart();
    ca.shipping_state = Some("CA".into());
    assert_applied(&apply(&commerce, ca), dec!(10.00), "shipping to CA");

    let mut ny = cart();
    ny.shipping_state = Some("NY".into());
    assert_refused(&apply(&commerce, ny), "shipping to NY");
}

// ============================================================================
// Required / optional condition grouping
// ============================================================================

#[test]
fn optional_conditions_need_at_least_one_met() {
    let commerce = new_commerce();
    pct_promo(
        &commerce,
        "Either/or",
        vec![
            optional(ConditionType::ShippingCountry, ConditionOperator::Equals, "CA"),
            optional(ConditionType::MinimumSubtotal, ConditionOperator::GreaterThanOrEqual, "50"),
        ],
    );

    assert_applied(&apply(&commerce, cart()), dec!(10.00), "the subtotal branch is met");
}

#[test]
fn optional_conditions_that_all_fail_refuse_the_promotion() {
    let commerce = new_commerce();
    pct_promo(
        &commerce,
        "Either/or",
        vec![
            // Neither branch can be satisfied: one is unevaluatable, the other
            // is simply not met.
            optional(ConditionType::CustomerGroup, ConditionOperator::Equals, "VIP"),
            optional(ConditionType::MinimumSubtotal, ConditionOperator::GreaterThanOrEqual, "500"),
        ],
    );

    assert_refused(&apply(&commerce, cart()), "no optional branch is met");
}

#[test]
fn a_failing_required_condition_refuses_even_when_an_optional_one_passes() {
    let commerce = new_commerce();
    pct_promo(
        &commerce,
        "Required plus optional",
        vec![
            required(ConditionType::CustomerGroup, ConditionOperator::Equals, "VIP"),
            optional(ConditionType::MinimumSubtotal, ConditionOperator::GreaterThanOrEqual, "50"),
        ],
    );

    assert_refused(&apply(&commerce, cart()), "the required condition is unevaluatable");
}

// ============================================================================
// Discount math clamps
// ============================================================================

#[test]
fn discount_is_clamped_to_the_eligible_item_value() {
    let commerce = new_commerce();
    // 150% off (a misconfiguration) scoped to WIDGET must never take more than
    // the $40 of WIDGET value, and must not bleed into the GADGET line.
    let promo = commerce
        .promotions()
        .create(CreatePromotion {
            name: "150% off widgets".into(),
            promotion_type: PromotionType::PercentageOff,
            trigger: PromotionTrigger::Automatic,
            target: PromotionTarget::Order,
            stacking: StackingBehavior::Stackable,
            percentage_off: Some(dec!(1.50)),
            applicable_skus: Some(vec!["WIDGET".into()]),
            ..Default::default()
        })
        .expect("Failed to create promotion");
    commerce.promotions().activate(promo.id).expect("activate");

    let mut request = cart();
    request.line_items =
        vec![line_item("WIDGET", None, 1, dec!(40.00)), line_item("GADGET", None, 1, dec!(60.00))];

    let result = apply(&commerce, request);

    assert_eq!(
        result.total_discount,
        dec!(40.00),
        "discount must cap at the eligible WIDGET value: {result:?}"
    );
    assert_eq!(result.discounted_subtotal, dec!(60.00));
}

#[test]
fn condition_gated_scoped_discount_stays_clamped_when_the_condition_is_met() {
    let commerce = new_commerce();
    pct_promo_scoped(
        &commerce,
        "10% off widgets over $50",
        vec![required(ConditionType::MinimumSubtotal, ConditionOperator::GreaterThanOrEqual, "50")],
        Some(vec!["WIDGET".into()]),
    );

    let mut request = cart();
    request.line_items =
        vec![line_item("WIDGET", None, 1, dec!(40.00)), line_item("GADGET", None, 1, dec!(60.00))];

    let result = apply(&commerce, request);

    assert_applied(&result, dec!(4.00), "10% of the $40 of eligible items only");
}

#[test]
fn total_discount_never_exceeds_the_subtotal() {
    let commerce = new_commerce();
    let promo = commerce
        .promotions()
        .create(CreatePromotion {
            name: "$500 off".into(),
            promotion_type: PromotionType::FixedAmountOff,
            trigger: PromotionTrigger::Automatic,
            target: PromotionTarget::Order,
            stacking: StackingBehavior::Stackable,
            fixed_amount_off: Some(dec!(500.00)),
            ..Default::default()
        })
        .expect("Failed to create promotion");
    commerce.promotions().activate(promo.id).expect("activate");

    let result = apply(&commerce, cart());

    assert_eq!(result.total_discount, dec!(100.00), "clamped to the subtotal: {result:?}");
    assert_eq!(result.discounted_subtotal, Decimal::ZERO);
    assert_eq!(result.grand_total, dec!(10.00), "shipping is still owed");
}

// ============================================================================
// Usage accounting (must not regress)
// ============================================================================

#[test]
fn total_usage_limit_guard_still_holds() {
    let commerce = new_commerce();
    let promo = pct_promo(&commerce, "One redemption only", vec![]);
    commerce
        .promotions()
        .update(promo.id, UpdatePromotion { total_usage_limit: Some(1), ..Default::default() })
        .expect("set usage limit");

    commerce
        .promotions()
        .record_usage(promo.id, None, None, None, None, dec!(10.00), "USD")
        .expect("first redemption");

    let err = commerce
        .promotions()
        .record_usage(promo.id, None, None, None, None, dec!(10.00), "USD")
        .expect_err("second redemption must be rejected");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");

    let fetched = commerce.promotions().get(promo.id).expect("get").expect("found");
    assert_eq!(fetched.usage_count, 1, "the guarded UPDATE must not over-count");
}

#[test]
fn per_customer_usage_limit_guard_still_holds() {
    let commerce = new_commerce();
    let promo = pct_promo(&commerce, "Once per customer", vec![]);
    commerce
        .promotions()
        .update(promo.id, UpdatePromotion { per_customer_limit: Some(1), ..Default::default() })
        .expect("set per-customer limit");

    let alice = make_customer(&commerce);
    commerce
        .promotions()
        .record_usage(promo.id, None, Some(alice), None, None, dec!(10.00), "USD")
        .expect("alice first redemption");

    let err = commerce
        .promotions()
        .record_usage(promo.id, None, Some(alice), None, None, dec!(10.00), "USD")
        .expect_err("alice second redemption must be rejected");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");

    // Another customer is unaffected.
    let bob = make_customer(&commerce);
    commerce
        .promotions()
        .record_usage(promo.id, None, Some(bob), None, None, dec!(10.00), "USD")
        .expect("bob first redemption");
}

#[test]
fn an_exhausted_promotion_is_rejected_at_evaluation_time() {
    let commerce = new_commerce();
    let promo = pct_promo(&commerce, "One redemption only", vec![]);
    commerce
        .promotions()
        .update(promo.id, UpdatePromotion { total_usage_limit: Some(1), ..Default::default() })
        .expect("set usage limit");
    commerce
        .promotions()
        .record_usage(promo.id, None, None, None, None, dec!(10.00), "USD")
        .expect("first redemption");

    assert_refused(&apply(&commerce, cart()), "the promotion is exhausted");
}

// ============================================================================
// Promotion-level customer targeting (same fail-open class as conditions)
// ============================================================================

#[test]
fn customer_group_targeting_fails_closed() {
    // `eligible_customer_groups` is the other way an admin expresses
    // "VIP only", and it was skipped entirely by the eligibility check —
    // so the promotion applied to every cart.
    let commerce = new_commerce();
    let promo = commerce
        .promotions()
        .create(CreatePromotion {
            name: "VIP group only".into(),
            promotion_type: PromotionType::PercentageOff,
            trigger: PromotionTrigger::Automatic,
            target: PromotionTarget::Order,
            stacking: StackingBehavior::Stackable,
            percentage_off: Some(dec!(0.10)),
            eligible_customer_groups: Some(vec!["vip".into()]),
            ..Default::default()
        })
        .expect("Failed to create promotion");
    commerce.promotions().activate(promo.id).expect("activate");

    let mut request = cart();
    request.customer_id = Some(CustomerId::new());

    assert_refused(&apply(&commerce, request), "customer group cannot be verified here");
}

#[test]
fn an_explicitly_eligible_customer_still_gets_a_group_restricted_promotion() {
    let vip = CustomerId::new();
    let commerce = new_commerce();
    let promo = commerce
        .promotions()
        .create(CreatePromotion {
            name: "VIPs and Alice".into(),
            promotion_type: PromotionType::PercentageOff,
            trigger: PromotionTrigger::Automatic,
            target: PromotionTarget::Order,
            stacking: StackingBehavior::Stackable,
            percentage_off: Some(dec!(0.10)),
            eligible_customer_ids: Some(vec![vip]),
            eligible_customer_groups: Some(vec!["vip".into()]),
            ..Default::default()
        })
        .expect("Failed to create promotion");
    commerce.promotions().activate(promo.id).expect("activate");

    let mut request = cart();
    request.customer_id = Some(vip);

    assert_applied(&apply(&commerce, request), dec!(10.00), "the customer is listed explicitly");
}

// ============================================================================
// A promotion is granted at most once per cart
// ============================================================================

fn coupon_promo(commerce: &Commerce, code: &str, trigger: PromotionTrigger) -> Promotion {
    let promo = commerce
        .promotions()
        .create(CreatePromotion {
            name: format!("10% off via {code}"),
            promotion_type: PromotionType::PercentageOff,
            trigger,
            target: PromotionTarget::Order,
            stacking: StackingBehavior::Stackable,
            percentage_off: Some(dec!(0.10)),
            ..Default::default()
        })
        .expect("Failed to create promotion");
    let promo = commerce.promotions().activate(promo.id).expect("activate");
    commerce
        .promotions()
        .create_coupon(CreateCouponCode {
            promotion_id: promo.id,
            code: code.to_string(),
            usage_limit: None,
            per_customer_limit: None,
            starts_at: None,
            ends_at: None,
            metadata: None,
        })
        .expect("Failed to create coupon");
    promo
}

#[test]
fn the_same_coupon_code_twice_is_granted_once() {
    let commerce = new_commerce();
    coupon_promo(&commerce, "SAVE10", PromotionTrigger::CouponCode);

    let mut request = cart();
    request.coupon_codes = vec!["SAVE10".into(), "SAVE10".into()];

    let result = apply(&commerce, request);

    assert_applied(&result, dec!(10.00), "repeating a coupon code must not double the discount");
}

#[test]
fn a_both_trigger_promotion_is_not_granted_automatically_and_by_coupon() {
    let commerce = new_commerce();
    coupon_promo(&commerce, "SAVE10", PromotionTrigger::Both);

    let mut request = cart();
    request.coupon_codes = vec!["SAVE10".into()];

    let result = apply(&commerce, request);

    assert_applied(&result, dec!(10.00), "a Both-trigger promotion must be granted once");
    assert_eq!(
        result.applied_promotions[0].coupon_code.as_deref(),
        Some("SAVE10"),
        "the redemption must keep its coupon attribution: {result:?}"
    );
}
