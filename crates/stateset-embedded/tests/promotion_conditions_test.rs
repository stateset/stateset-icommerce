#![cfg(feature = "sqlite")]

//! Promotion conditions are stored when added and validated when written.
//!
//! `Promotions::add_condition` pushed the condition onto a local copy of the
//! promotion and returned it unchanged: success, with nothing stored. And no
//! write path validated a condition's value, while evaluation reports a
//! malformed value as an error that pricing propagates, so a single
//! misconfigured automatic promotion failed the pricing of every cart.

use rust_decimal_macros::dec;
use stateset_core::{
    ApplyPromotionsRequest, CommerceError, ConditionOperator, ConditionType,
    CreatePromotionCondition, CurrencyCode, PromotionId, PromotionLineItem, PromotionTrigger,
};
use stateset_embedded::{Commerce, CreatePromotion, PromotionType};

fn condition(
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

fn automatic(
    commerce: &Commerce,
    conditions: Option<Vec<CreatePromotionCondition>>,
) -> PromotionId {
    let promotion = commerce
        .promotions()
        .create(CreatePromotion {
            name: "Ten percent".into(),
            promotion_type: PromotionType::PercentageOff,
            trigger: PromotionTrigger::Automatic,
            percentage_off: Some(dec!(0.10)),
            conditions,
            ..Default::default()
        })
        .expect("create promotion");
    commerce.promotions().activate(promotion.id).expect("activate");
    promotion.id
}

fn cart(subtotal: rust_decimal::Decimal) -> ApplyPromotionsRequest {
    ApplyPromotionsRequest {
        line_items: vec![PromotionLineItem {
            id: "line-1".into(),
            product_id: None,
            variant_id: None,
            sku: Some("SKU-1".into()),
            category_ids: vec![],
            quantity: 1,
            unit_price: subtotal,
            line_total: subtotal,
        }],
        subtotal,
        currency: CurrencyCode::USD,
        ..Default::default()
    }
}

#[track_caller]
fn assert_validation(err: CommerceError, mentions: &str) {
    match err {
        CommerceError::ValidationError(message) => {
            assert!(message.contains(mentions), "expected '{mentions}' in: {message}");
        }
        other => panic!("expected a validation error, got {other:?}"),
    }
}

#[test]
fn add_condition_is_stored_and_takes_part_in_pricing() {
    let commerce = Commerce::new(":memory:").expect("commerce");
    let promotion = automatic(&commerce, None);
    assert_eq!(
        commerce.promotions().apply(cart(dec!(40.00))).expect("apply").total_discount,
        dec!(4.00)
    );

    let returned = commerce
        .promotions()
        .add_condition(
            promotion,
            condition(ConditionType::MinimumSubtotal, ConditionOperator::GreaterThanOrEqual, "50"),
        )
        .expect("add condition");
    assert_eq!(returned.conditions.len(), 1, "the returned promotion carries the condition");

    let stored = commerce.promotions().get(promotion).expect("get").expect("exists");
    assert_eq!(stored.conditions.len(), 1, "the condition was persisted, not just returned");

    let below = commerce.promotions().apply(cart(dec!(40.00))).expect("apply");
    assert_eq!(below.total_discount, dec!(0), "a $40 cart is below the new $50 minimum");
    let above = commerce.promotions().apply(cart(dec!(60.00))).expect("apply");
    assert_eq!(above.total_discount, dec!(6.00));
}

#[test]
fn add_condition_to_a_missing_promotion_is_not_found() {
    let commerce = Commerce::new(":memory:").expect("commerce");
    let err = commerce
        .promotions()
        .add_condition(
            PromotionId::new(),
            condition(ConditionType::MinimumSubtotal, ConditionOperator::GreaterThan, "10"),
        )
        .expect_err("no such promotion");
    assert!(matches!(err, CommerceError::NotFound), "got {err:?}");
}

#[test]
fn a_malformed_condition_value_is_refused_when_added() {
    let commerce = Commerce::new(":memory:").expect("commerce");
    let promotion = automatic(&commerce, None);

    for (condition_type, operator, value) in [
        (ConditionType::MinimumSubtotal, ConditionOperator::GreaterThan, "ten dollars"),
        (ConditionType::MinimumQuantity, ConditionOperator::GreaterThan, "2.5"),
        (ConditionType::FirstOrder, ConditionOperator::Equals, "banana"),
        (ConditionType::ProductInCart, ConditionOperator::In, "not-a-uuid"),
        (ConditionType::CustomerId, ConditionOperator::In, "7"),
    ] {
        let err = commerce
            .promotions()
            .add_condition(promotion, condition(condition_type, operator, value))
            .expect_err("malformed value");
        assert_validation(err, value);
    }

    let stored = commerce.promotions().get(promotion).expect("get").expect("exists");
    assert!(stored.conditions.is_empty(), "nothing was stored");
    // Pricing still works for everyone.
    commerce.promotions().apply(cart(dec!(40.00))).expect("pricing is unaffected");
}

#[test]
fn an_operator_the_condition_cannot_use_is_refused() {
    let commerce = Commerce::new(":memory:").expect("commerce");
    let promotion = automatic(&commerce, None);

    for (condition_type, operator, value) in [
        (ConditionType::FirstOrder, ConditionOperator::GreaterThan, "true"),
        (ConditionType::MinimumSubtotal, ConditionOperator::Contains, "10"),
        (ConditionType::SkuInCart, ConditionOperator::LessThan, "SKU-1"),
    ] {
        let err = commerce
            .promotions()
            .add_condition(promotion, condition(condition_type, operator, value))
            .expect_err("operator does not apply");
        assert_validation(err, "does not apply");
    }
}

#[test]
fn a_malformed_condition_is_refused_at_create() {
    // Before, this promotion was stored, and once active every cart's
    // pricing failed with a condition parse error.
    let commerce = Commerce::new(":memory:").expect("commerce");
    let err = commerce
        .promotions()
        .create(CreatePromotion {
            name: "Broken".into(),
            promotion_type: PromotionType::PercentageOff,
            trigger: PromotionTrigger::Automatic,
            percentage_off: Some(dec!(0.10)),
            conditions: Some(vec![condition(
                ConditionType::MinimumSubtotal,
                ConditionOperator::GreaterThan,
                "fifty",
            )]),
            ..Default::default()
        })
        .expect_err("malformed condition");
    assert_validation(err, "fifty");
    assert!(commerce.promotions().get_active().expect("list").is_empty());
}

#[test]
fn conditions_the_cart_cannot_evaluate_yet_are_still_accepted() {
    // They refuse the promotion (fail-closed) until the request carries the
    // context; refusing to store them would break configurations written
    // ahead of that.
    let commerce = Commerce::new(":memory:").expect("commerce");
    let promotion = automatic(&commerce, None);
    commerce
        .promotions()
        .add_condition(
            promotion,
            condition(ConditionType::CustomerGroup, ConditionOperator::Equals, "vip"),
        )
        .expect("accepted");
    let priced = commerce.promotions().apply(cart(dec!(40.00))).expect("apply");
    assert_eq!(priced.total_discount, dec!(0), "fails closed");
}
