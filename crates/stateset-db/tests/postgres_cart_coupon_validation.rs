//! Regression tests for coupon validation at the Postgres cart layer
//! (mirrors the SQLite in-module `coupon_validation` tests).
//!
//! Before this guard, `apply_discount` looked a coupon up by code and priced
//! its promotion with no check of coupon status / window / usage limit,
//! promotion status / window / usage limit, promotion conditions, or
//! per-customer limits — so an expired single-use coupon applied forever —
//! and checkout never consumed the coupon.
//!
//! These tests require a live Postgres instance (`POSTGRES_URL` /
//! `DATABASE_URL`) and are skipped otherwise, so they run only in CI with a
//! provisioned database (the Postgres Parity job).

#![cfg(feature = "postgres")]

use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{
    AddCartItem, Cart, CartAddress, CartStatus, CommerceError, ConditionOperator, ConditionType,
    CouponCode, CouponStatus, CreateCart, CreateCouponCode, CreateCustomer, CreatePromotion,
    CreatePromotionCondition, Promotion, PromotionStatus, PromotionTarget, PromotionTrigger,
    PromotionType, SetCartShipping, StackingBehavior, UpdateCart, UpdateCartItem, UpdatePromotion,
};
use stateset_db::PostgresDatabase;
use stateset_db::postgres::{PgCartRepository, PgPromotionRepository};
use std::env;
use uuid::Uuid;

fn postgres_url() -> Option<String> {
    env::var("POSTGRES_URL").ok().or_else(|| env::var("DATABASE_URL").ok())
}

async fn connect() -> Option<PostgresDatabase> {
    let url = postgres_url()?;
    Some(PostgresDatabase::connect(&url).await.expect("connect to postgres and run migrations"))
}

fn unique(prefix: &str) -> String {
    format!("{prefix}-{}", &Uuid::new_v4().simple().to_string()[..8]).to_uppercase()
}

fn coupon_input(promotion_id: stateset_core::PromotionId, code: &str) -> CreateCouponCode {
    CreateCouponCode {
        promotion_id,
        code: code.into(),
        usage_limit: None,
        per_customer_limit: None,
        starts_at: None,
        ends_at: None,
        metadata: None,
    }
}

async fn active_promo_with_coupon(
    promos: &PgPromotionRepository,
    code: &str,
    coupon: impl FnOnce(CreateCouponCode) -> CreateCouponCode,
) -> (Promotion, CouponCode) {
    let promo = promos
        .create_async(CreatePromotion {
            code: Some(format!("{code}-PROMO")),
            name: format!("{code} promo"),
            promotion_type: PromotionType::PercentageOff,
            trigger: PromotionTrigger::CouponCode,
            target: PromotionTarget::Order,
            stacking: StackingBehavior::Stackable,
            percentage_off: Some(dec!(0.10)),
            ..Default::default()
        })
        .await
        .expect("create promo");
    let promo = promos.activate_async(promo.id.into_uuid()).await.expect("activate");
    let coupon = promos
        .create_coupon_async(coupon(coupon_input(promo.id, code)))
        .await
        .expect("create coupon");
    (promo, coupon)
}

async fn cart_with_subtotal(carts: &PgCartRepository, subtotal: Decimal) -> Cart {
    let cart = carts.create_async(CreateCart::default()).await.expect("create cart");
    carts
        .add_item_async(
            cart.id.into_uuid(),
            AddCartItem {
                sku: unique("SKU"),
                name: "Coupon test item".into(),
                quantity: 1,
                unit_price: subtotal,
                ..Default::default()
            },
        )
        .await
        .expect("add item");
    carts.get_async(cart.id.into_uuid()).await.expect("ok").expect("found")
}

fn assert_refused(result: Result<Cart, CommerceError>, expected_fragment: &str) {
    match result {
        Err(CommerceError::ValidationError(msg)) => assert!(
            msg.to_lowercase().contains(&expected_fragment.to_lowercase()),
            "expected a ValidationError mentioning {expected_fragment:?}, got {msg:?}"
        ),
        Err(other) => panic!("expected ValidationError, got {other:?}"),
        Ok(cart) => panic!(
            "coupon must be refused, but it applied a discount of {} to cart {}",
            cart.discount_amount, cart.id
        ),
    }
}

#[tokio::test]
async fn postgres_apply_discount_valid_coupon_still_applies() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let (carts, promos) = (db.carts(), db.promotions());
    let code = unique("VALID10");
    active_promo_with_coupon(&promos, &code, |c| c).await;
    let cart = cart_with_subtotal(&carts, dec!(33.33)).await;
    let cart = carts.apply_discount_async(cart.id.into_uuid(), &code).await.expect("applies");
    assert_eq!(cart.discount_amount, dec!(3.33), "rounded to currency precision");
    assert_eq!(cart.grand_total, dec!(30.00));
    assert_eq!(cart.coupon_code.as_deref(), Some(code.as_str()));
}

#[tokio::test]
async fn postgres_apply_discount_refuses_inactive_promotions() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let (carts, promos) = (db.carts(), db.promotions());

    // Draft
    let code = unique("DRAFT10");
    let (promo, _) = active_promo_with_coupon(&promos, &code, |c| c).await;
    promos
        .update_async(
            promo.id.into_uuid(),
            UpdatePromotion { status: Some(PromotionStatus::Draft), ..Default::default() },
        )
        .await
        .expect("draft");
    let cart = cart_with_subtotal(&carts, dec!(100)).await;
    assert_refused(carts.apply_discount_async(cart.id.into_uuid(), &code).await, "not active");

    // Paused
    let code = unique("PAUSED10");
    let (promo, _) = active_promo_with_coupon(&promos, &code, |c| c).await;
    promos
        .update_async(
            promo.id.into_uuid(),
            UpdatePromotion { status: Some(PromotionStatus::Paused), ..Default::default() },
        )
        .await
        .expect("pause");
    let cart = cart_with_subtotal(&carts, dec!(100)).await;
    assert_refused(carts.apply_discount_async(cart.id.into_uuid(), &code).await, "not active");

    // Expired window
    let code = unique("EXPIRED10");
    let (promo, _) = active_promo_with_coupon(&promos, &code, |c| c).await;
    promos
        .update_async(
            promo.id.into_uuid(),
            UpdatePromotion {
                starts_at: Some(Utc::now() - Duration::days(30)),
                ends_at: Some(Utc::now() - Duration::days(1)),
                ..Default::default()
            },
        )
        .await
        .expect("expire");
    let cart = cart_with_subtotal(&carts, dec!(100)).await;
    assert_refused(carts.apply_discount_async(cart.id.into_uuid(), &code).await, "expired");

    // Not yet started
    let code = unique("FUTURE10");
    let (promo, _) = active_promo_with_coupon(&promos, &code, |c| c).await;
    promos
        .update_async(
            promo.id.into_uuid(),
            UpdatePromotion {
                starts_at: Some(Utc::now() + Duration::days(1)),
                ..Default::default()
            },
        )
        .await
        .expect("future");
    let cart = cart_with_subtotal(&carts, dec!(100)).await;
    assert_refused(carts.apply_discount_async(cart.id.into_uuid(), &code).await, "not started");

    // Promotion total usage limit reached
    let code = unique("PROMOCAP10");
    let (promo, _) = active_promo_with_coupon(&promos, &code, |c| c).await;
    promos
        .update_async(
            promo.id.into_uuid(),
            UpdatePromotion { total_usage_limit: Some(1), ..Default::default() },
        )
        .await
        .expect("cap");
    promos
        .record_usage_async(promo.id, None, None, None, None, dec!(10), "USD")
        .await
        .expect("use");
    let cart = cart_with_subtotal(&carts, dec!(100)).await;
    assert_refused(carts.apply_discount_async(cart.id.into_uuid(), &code).await, "usage limit");
}

#[tokio::test]
async fn postgres_apply_discount_refuses_unusable_coupons() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let (carts, promos) = (db.carts(), db.promotions());

    // Disabled coupon
    let code = unique("DISABLED10");
    let (_, coupon) = active_promo_with_coupon(&promos, &code, |c| c).await;
    promos.set_coupon_status_async(coupon.id, CouponStatus::Disabled).await.expect("disable");
    let cart = cart_with_subtotal(&carts, dec!(100)).await;
    assert_refused(
        carts.apply_discount_async(cart.id.into_uuid(), &code).await,
        "coupon is not active",
    );

    // Expired coupon window
    let code = unique("OLDCODE10");
    active_promo_with_coupon(&promos, &code, |c| CreateCouponCode {
        ends_at: Some(Utc::now() - Duration::hours(1)),
        ..c
    })
    .await;
    let cart = cart_with_subtotal(&carts, dec!(100)).await;
    assert_refused(carts.apply_discount_async(cart.id.into_uuid(), &code).await, "expired");

    // Coupon usage limit reached
    let code = unique("ONCE10");
    let (promo, coupon) = active_promo_with_coupon(&promos, &code, |c| CreateCouponCode {
        usage_limit: Some(1),
        ..c
    })
    .await;
    promos
        .record_usage_async(promo.id, Some(coupon.id), None, None, None, dec!(10), "USD")
        .await
        .expect("first redemption");
    let cart = cart_with_subtotal(&carts, dec!(100)).await;
    assert_refused(carts.apply_discount_async(cart.id.into_uuid(), &code).await, "usage limit");
}

#[tokio::test]
async fn postgres_apply_discount_enforces_per_customer_and_minimum_subtotal() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let (carts, promos, customers) = (db.carts(), db.promotions(), db.customers());

    let mk = |email: String| async {
        customers
            .create_async(CreateCustomer {
                email,
                first_name: "Test".into(),
                last_name: "Customer".into(),
                ..Default::default()
            })
            .await
            .expect("customer")
            .id
    };
    let alice = mk(format!("{}@example.com", unique("alice").to_lowercase())).await;
    let bob = mk(format!("{}@example.com", unique("bob").to_lowercase())).await;

    let code = unique("PERCUST10");
    let (promo, coupon) = active_promo_with_coupon(&promos, &code, |c| CreateCouponCode {
        per_customer_limit: Some(1),
        ..c
    })
    .await;
    promos
        .record_usage_async(promo.id, Some(coupon.id), Some(alice), None, None, dec!(10), "USD")
        .await
        .expect("alice used it once");

    let alice_cart = carts
        .create_async(CreateCart { customer_id: Some(alice), ..Default::default() })
        .await
        .expect("cart");
    carts
        .add_item_async(
            alice_cart.id.into_uuid(),
            AddCartItem {
                sku: unique("SKU"),
                name: "x".into(),
                quantity: 1,
                unit_price: dec!(100),
                ..Default::default()
            },
        )
        .await
        .expect("add");
    assert_refused(
        carts.apply_discount_async(alice_cart.id.into_uuid(), &code).await,
        "per-customer",
    );

    let bob_cart = carts
        .create_async(CreateCart { customer_id: Some(bob), ..Default::default() })
        .await
        .expect("cart");
    carts
        .add_item_async(
            bob_cart.id.into_uuid(),
            AddCartItem {
                sku: unique("SKU"),
                name: "x".into(),
                quantity: 1,
                unit_price: dec!(100),
                ..Default::default()
            },
        )
        .await
        .expect("add");
    let bob_cart =
        carts.apply_discount_async(bob_cart.id.into_uuid(), &code).await.expect("bob is fine");
    assert_eq!(bob_cart.discount_amount, dec!(10));

    // Minimum subtotal condition (fail-closed evaluation from stateset-core).
    let code = unique("MIN50");
    let promo = promos
        .create_async(CreatePromotion {
            code: Some(format!("{code}-PROMO")),
            name: "min 50".into(),
            promotion_type: PromotionType::PercentageOff,
            trigger: PromotionTrigger::CouponCode,
            target: PromotionTarget::Order,
            stacking: StackingBehavior::Stackable,
            percentage_off: Some(dec!(0.10)),
            conditions: Some(vec![CreatePromotionCondition {
                condition_type: ConditionType::MinimumSubtotal,
                operator: ConditionOperator::GreaterThanOrEqual,
                value: "50".into(),
                is_required: true,
            }]),
            ..Default::default()
        })
        .await
        .expect("create promo");
    promos.activate_async(promo.id.into_uuid()).await.expect("activate");
    promos.create_coupon_async(coupon_input(promo.id, &code)).await.expect("coupon");

    let small = cart_with_subtotal(&carts, dec!(20)).await;
    assert_refused(
        carts.apply_discount_async(small.id.into_uuid(), &code).await,
        "conditions not met",
    );
    let big = cart_with_subtotal(&carts, dec!(80)).await;
    let big = carts.apply_discount_async(big.id.into_uuid(), &code).await.expect("meets minimum");
    assert_eq!(big.discount_amount, dec!(8));
}

fn test_address() -> CartAddress {
    CartAddress {
        first_name: "Ada".into(),
        last_name: "Lovelace".into(),
        company: None,
        line1: "1 Analytical Engine Way".into(),
        line2: None,
        city: "London".into(),
        state: None,
        postal_code: "N1 9GU".into(),
        country: "GB".into(),
        phone: None,
        email: Some("ada@example.com".into()),
    }
}

async fn checkoutable_cart(carts: &PgCartRepository) -> Cart {
    let cart = carts
        .create_async(CreateCart {
            customer_email: Some(format!("{}@example.com", unique("buyer").to_lowercase())),
            customer_name: Some("Ada Lovelace".into()),
            shipping_address: Some(test_address()),
            ..Default::default()
        })
        .await
        .expect("create");
    carts
        .add_item_async(
            cart.id.into_uuid(),
            AddCartItem {
                sku: unique("SKU-CHK"),
                name: "Checkout item".into(),
                quantity: 1,
                unit_price: dec!(10),
                ..Default::default()
            },
        )
        .await
        .expect("add");
    carts.set_shipping_address_async(cart.id.into_uuid(), test_address()).await.expect("ship");
    carts.get_async(cart.id.into_uuid()).await.expect("ok").expect("found")
}

#[tokio::test]
async fn postgres_checkout_consumes_coupon_applied_in_lowercase() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let (carts, promos) = (db.carts(), db.promotions());
    let code = unique("CASE10");
    let (_promo, coupon) = active_promo_with_coupon(&promos, &code, |c| CreateCouponCode {
        usage_limit: Some(1),
        ..c
    })
    .await;
    let cart = checkoutable_cart(&carts).await;
    let applied = carts
        .apply_discount_async(cart.id.into_uuid(), &code.to_lowercase())
        .await
        .expect("applies");
    assert_eq!(applied.coupon_code.as_deref(), Some(code.as_str()), "cart stores canonical code");
    carts.complete_async(cart.id.into_uuid()).await.expect("checkout");

    let coupon_after = promos.get_coupon_async(coupon.id).await.expect("ok").expect("coupon");
    assert_eq!(coupon_after.usage_count, 1, "lowercase entry must still consume the coupon");
}

#[tokio::test]
async fn postgres_checkout_records_coupon_usage_once() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let (carts, promos) = (db.carts(), db.promotions());
    let code = unique("CHECKOUT10");
    let (promo, coupon) = active_promo_with_coupon(&promos, &code, |c| CreateCouponCode {
        usage_limit: Some(1),
        ..c
    })
    .await;
    let cart = checkoutable_cart(&carts).await;
    carts.apply_discount_async(cart.id.into_uuid(), &code).await.expect("applies");
    let result = carts.complete_async(cart.id.into_uuid()).await.expect("checkout");

    let coupon_after = promos.get_coupon_async(coupon.id).await.expect("ok").expect("coupon");
    assert_eq!(coupon_after.usage_count, 1, "coupon usage_count must advance at checkout");
    let promo_after = promos.get_async(promo.id).await.expect("ok").expect("promo");
    assert_eq!(promo_after.usage_count, 1, "promotion usage_count must advance at checkout");

    let ledger = promos.usage_for_cart_async(cart.id).await.expect("ledger");
    assert_eq!(ledger.len(), 1, "exactly one usage row per checkout");
    assert_eq!(ledger[0].coupon_id, Some(coupon.id));
    assert_eq!(ledger[0].order_id, Some(result.order_id));

    // Idempotent re-complete must not double count.
    carts.complete_async(cart.id.into_uuid()).await.expect("idempotent checkout");
    let coupon_after = promos.get_coupon_async(coupon.id).await.expect("ok").expect("coupon");
    assert_eq!(coupon_after.usage_count, 1);

    // The single-use coupon is now spent for everyone else.
    let other = cart_with_subtotal(&carts, dec!(100)).await;
    assert_refused(carts.apply_discount_async(other.id.into_uuid(), &code).await, "usage limit");
}

#[tokio::test]
async fn postgres_checkout_refuses_coupon_exhausted_since_apply() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let (carts, promos) = (db.carts(), db.promotions());
    let code = unique("RACE10");
    let (promo, coupon) = active_promo_with_coupon(&promos, &code, |c| CreateCouponCode {
        usage_limit: Some(1),
        ..c
    })
    .await;
    let cart = checkoutable_cart(&carts).await;
    carts.apply_discount_async(cart.id.into_uuid(), &code).await.expect("applies");
    promos
        .record_usage_async(promo.id, Some(coupon.id), None, None, None, dec!(1), "USD")
        .await
        .expect("other redemption");

    let err = carts
        .complete_async(cart.id.into_uuid())
        .await
        .expect_err("checkout must refuse the spent coupon");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
    let cart = carts.get_async(cart.id.into_uuid()).await.expect("ok").expect("found");
    assert_eq!(cart.status, CartStatus::Active, "failed checkout must roll back");
}

// ---------------------------------------------------------------------------
// Live discount derivation, negative-total cap, expiry and item validation
// (mirrors the SQLite in-module tests of the same names).
// ---------------------------------------------------------------------------

async fn pct_promo_with_minimum(
    promos: &PgPromotionRepository,
    code: &str,
    pct: Decimal,
    minimum: Decimal,
) {
    let promo = promos
        .create_async(CreatePromotion {
            code: Some(format!("{code}-PROMO")),
            name: format!("{code} promo"),
            promotion_type: PromotionType::PercentageOff,
            trigger: PromotionTrigger::CouponCode,
            target: PromotionTarget::Order,
            stacking: StackingBehavior::Stackable,
            percentage_off: Some(pct),
            conditions: Some(vec![CreatePromotionCondition {
                condition_type: ConditionType::MinimumSubtotal,
                operator: ConditionOperator::GreaterThanOrEqual,
                value: minimum.to_string(),
                is_required: true,
            }]),
            ..Default::default()
        })
        .await
        .expect("create promo");
    promos.activate_async(promo.id.into_uuid()).await.expect("activate");
    promos.create_coupon_async(coupon_input(promo.id, code)).await.expect("coupon");
}

fn line(sku: &str, quantity: i32, unit_price: Decimal) -> AddCartItem {
    AddCartItem { sku: unique(sku), name: sku.into(), quantity, unit_price, ..Default::default() }
}

#[tokio::test]
async fn postgres_coupon_discount_is_re_derived_when_cart_contents_change() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let (carts, promos) = (db.carts(), db.promotions());
    let code = unique("TWENTY100");
    pct_promo_with_minimum(&promos, &code, dec!(0.20), dec!(100)).await;
    let cart = checkoutable_cart(&carts).await; // $10 of lines
    let id = cart.id.into_uuid();
    let item = carts.add_item_async(id, line("SKU-BIG", 1, dec!(90))).await.expect("add");
    let cart = carts.apply_discount_async(id, &code).await.expect("applies at $100");
    assert_eq!(cart.discount_amount, dec!(20));
    assert_eq!(cart.grand_total, dec!(80));

    // Drop to $30: the coupon stays on the cart but no longer qualifies.
    carts
        .update_item_async(
            item.id,
            UpdateCartItem { unit_price: Some(dec!(20)), ..Default::default() },
        )
        .await
        .expect("reprice");
    let cart = carts.get_async(id).await.expect("ok").expect("found");
    assert_eq!(cart.subtotal, dec!(30));
    assert_eq!(cart.discount_amount, dec!(0), "frozen discount must be re-derived");
    assert_eq!(cart.grand_total, dec!(30));
    assert_eq!(cart.coupon_code.as_deref(), Some(code.as_str()), "coupon is kept");
    let description = cart.discount_description.clone().unwrap_or_default();
    assert!(
        description.contains("not applied") && description.contains(&code),
        "returned cart must say why the discount is zero: {description:?}"
    );

    // Checkout refuses the non-qualifying coupon instead of minting a stale
    // discount (or silently dropping it).
    let err = carts.complete_async(id).await.expect_err("must refuse");
    match err {
        CommerceError::ValidationError(msg) => assert!(
            msg.contains(&code) && msg.to_lowercase().contains("conditions not met"),
            "reason must name the coupon and the failed check: {msg}"
        ),
        other => panic!("expected ValidationError, got {other:?}"),
    }
    let cart = carts.get_async(id).await.expect("ok").expect("found");
    assert_eq!(cart.status, CartStatus::Active);

    // Grow back past the minimum: the discount comes back on its own.
    carts.add_item_async(id, line("SKU-MORE", 1, dec!(70))).await.expect("add");
    let cart = carts.get_async(id).await.expect("ok").expect("found");
    assert_eq!(cart.subtotal, dec!(100));
    assert_eq!(cart.discount_amount, dec!(20));
    assert_eq!(cart.discount_description.as_deref(), Some(format!("{code} promo").as_str()));
    assert_eq!(cart.grand_total, dec!(80));

    // Removing a line re-derives too.
    carts.remove_item_async(item.id).await.expect("remove");
    let cart = carts.get_async(id).await.expect("ok").expect("found");
    assert_eq!(cart.subtotal, dec!(80));
    assert_eq!(cart.discount_amount, dec!(0));
}

#[tokio::test]
async fn postgres_checkout_refuses_coupon_paused_since_apply() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let (carts, promos) = (db.carts(), db.promotions());
    let code = unique("PAUSED10");
    let (promo, _) = active_promo_with_coupon(&promos, &code, |c| c).await;
    let cart = checkoutable_cart(&carts).await;
    let id = cart.id.into_uuid();
    let cart = carts.apply_discount_async(id, &code).await.expect("applies");
    assert_eq!(cart.discount_amount, dec!(1));
    promos
        .update_async(
            promo.id.into_uuid(),
            UpdatePromotion { status: Some(PromotionStatus::Paused), ..Default::default() },
        )
        .await
        .expect("pause");

    let err = carts.complete_async(id).await.expect_err("must refuse the paused promotion");
    match err {
        CommerceError::ValidationError(msg) => {
            assert!(msg.contains(&code) && msg.to_lowercase().contains("not active"), "got {msg}");
        }
        other => panic!("expected ValidationError, got {other:?}"),
    }
    let cart = carts.get_async(id).await.expect("ok").expect("found");
    assert_eq!(cart.status, CartStatus::Active, "failed checkout must roll back");
    assert!(promos.usage_for_cart_async(cart.id).await.expect("ledger").is_empty());
}

#[tokio::test]
async fn postgres_fixed_amount_coupon_is_capped_at_coverable_amount() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let (carts, promos) = (db.carts(), db.promotions());
    let code = unique("FIFTY");
    let promo = promos
        .create_async(CreatePromotion {
            code: Some(format!("{code}-PROMO")),
            name: "fifty off".into(),
            promotion_type: PromotionType::FixedAmountOff,
            trigger: PromotionTrigger::CouponCode,
            target: PromotionTarget::Order,
            stacking: StackingBehavior::Stackable,
            fixed_amount_off: Some(dec!(50)),
            ..Default::default()
        })
        .await
        .expect("promo");
    promos.activate_async(promo.id.into_uuid()).await.expect("activate");
    promos.create_coupon_async(coupon_input(promo.id, &code)).await.expect("coupon");

    let cart = checkoutable_cart(&carts).await; // $10 of lines
    let id = cart.id.into_uuid();
    let cart = carts.apply_discount_async(id, &code).await.expect("applies");
    assert_eq!(cart.discount_amount, dec!(10));
    assert_eq!(cart.grand_total, dec!(0));

    let result = carts.complete_async(id).await.expect("checkout");
    let order =
        db.orders().get_async(result.order_id.into_uuid()).await.expect("ok").expect("order");
    assert_eq!(order.discount_amount, dec!(10));
    assert_eq!(order.total_amount, dec!(0));
    assert_eq!(result.total_charged, dec!(0));
}

#[tokio::test]
async fn postgres_checkout_order_money_matches_cart_grand_total() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let (carts, promos) = (db.carts(), db.promotions());
    let code = unique("PARITY10");
    active_promo_with_coupon(&promos, &code, |c| c).await;
    let cart = checkoutable_cart(&carts).await;
    let id = cart.id.into_uuid();
    carts.add_item_async(id, line("SKU-2", 3, dec!(19.99))).await.expect("add");
    carts.set_tax_async(id, dec!(5.25)).await.expect("tax");
    carts
        .set_shipping_async(
            id,
            SetCartShipping {
                shipping_address: test_address(),
                shipping_method: Some("ground".into()),
                shipping_carrier: None,
                shipping_amount: Some(dec!(7.50)),
            },
        )
        .await
        .expect("shipping");
    let cart = carts.apply_discount_async(id, &code).await.expect("applies");
    assert_eq!(cart.subtotal, dec!(69.97));
    assert_eq!(cart.discount_amount, dec!(7.00)); // 10% of 69.97, rounded
    let expected_total = dec!(69.97) + dec!(5.25) + dec!(7.50) - dec!(7.00);
    assert_eq!(cart.grand_total, expected_total);

    let result = carts.complete_async(id).await.expect("checkout");
    assert_eq!(result.total_charged, expected_total);
    let order =
        db.orders().get_async(result.order_id.into_uuid()).await.expect("ok").expect("order");
    assert_eq!(order.tax_amount, cart.tax_amount);
    assert_eq!(order.shipping_amount, cart.shipping_amount);
    assert_eq!(order.discount_amount, cart.discount_amount);
    assert_eq!(order.total_amount, cart.grand_total);
    let lines: Decimal = order.items.iter().map(|i| i.total).sum();
    assert_eq!(
        order.total_amount,
        lines + order.tax_amount + order.shipping_amount - order.discount_amount
    );
}

#[tokio::test]
async fn postgres_checkout_caps_manual_discount_at_coverable_amount() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let carts = db.carts();
    let cart = checkoutable_cart(&carts).await; // $10 of lines
    let id = cart.id.into_uuid();
    carts
        .update_async(id, UpdateCart { discount_amount: Some(dec!(100)), ..Default::default() })
        .await
        .expect("oversized manual discount");
    let cart = carts.recalculate_async(id).await.expect("recalc");
    assert_eq!(cart.discount_amount, dec!(10), "stored discount capped");
    assert_eq!(cart.grand_total, dec!(0));

    let result = carts.complete_async(id).await.expect("checkout");
    let order =
        db.orders().get_async(result.order_id.into_uuid()).await.expect("ok").expect("order");
    assert_eq!(order.discount_amount, dec!(10));
    assert_eq!(order.total_amount, dec!(0));
}

#[tokio::test]
async fn postgres_checkout_refuses_cart_past_expires_at() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let carts = db.carts();
    let cart = checkoutable_cart(&carts).await;
    let id = cart.id.into_uuid();
    sqlx::query("UPDATE carts SET expires_at = $1 WHERE id = $2")
        .bind(Utc::now() - Duration::minutes(5))
        .bind(id)
        .execute(db.pool())
        .await
        .expect("backdate expiry");
    let cart = carts.get_async(id).await.expect("ok").expect("found");
    assert_eq!(cart.status, CartStatus::Active);
    assert!(cart.is_expired());
    assert!(!cart.is_ready_for_checkout());

    let err = carts.complete_async(id).await.expect_err("expired cart must not check out");
    match err {
        CommerceError::ValidationError(msg) => {
            assert!(msg.to_lowercase().contains("expired"), "got {msg}");
        }
        other => panic!("expected ValidationError, got {other:?}"),
    }
    let cart = carts.get_async(id).await.expect("ok").expect("found");
    assert!(cart.order_id.is_none());
}

#[tokio::test]
async fn postgres_update_item_rejects_non_positive_quantity() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let carts = db.carts();
    let cart = checkoutable_cart(&carts).await;
    let id = cart.id.into_uuid();
    let item = cart.items[0].clone();
    for qty in [0, -1] {
        let err = carts
            .update_item_async(
                item.id,
                UpdateCartItem { quantity: Some(qty), ..Default::default() },
            )
            .await
            .expect_err("non-positive quantity must be refused");
        assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
    }
    let cart = carts.get_async(id).await.expect("ok").expect("found");
    assert_eq!(cart.items.len(), 1);
    assert_eq!(cart.items[0].quantity, 1);
    assert_eq!(cart.subtotal, dec!(10));
}

/// Concurrent `add_item` calls on one cart must not lose an update in the
/// stored subtotal: each add locks the cart row before summing its lines.
#[tokio::test]
async fn postgres_concurrent_add_item_keeps_subtotal_consistent() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let carts = db.carts();
    let cart = carts.create_async(CreateCart::default()).await.expect("create");
    let id = cart.id.into_uuid();

    const ADDS: i64 = 12;
    let mut handles = Vec::new();
    for n in 0..ADDS {
        let carts = db.carts();
        handles.push(tokio::spawn(async move {
            carts.add_item_async(id, line("SKU-PAR", 1, Decimal::from(n + 1))).await.expect("add")
        }));
    }
    for handle in handles {
        handle.await.expect("join");
    }

    let cart = carts.get_async(id).await.expect("ok").expect("found");
    assert_eq!(cart.items.len(), ADDS as usize);
    let expected = Decimal::from(ADDS * (ADDS + 1) / 2);
    assert_eq!(cart.subtotal, expected, "stored subtotal lost a concurrent add");
    assert_eq!(cart.grand_total, expected);
}
