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

// ============================================================================
// Round 4: transactional coupon reads, all promotion types, preview parity,
// totals-through-one-path, tax rescale, money scale, discount recovery
// ============================================================================

/// Every coupon read during a cart mutation or checkout happens on the
/// mutation's own transaction: with a pool of ONE connection, add / update /
/// checkout with a coupon must complete rather than deadlock waiting for a
/// second pooled connection.
#[tokio::test]
async fn postgres_size_one_pool_coupon_paths_do_not_deadlock() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    drop(db); // migrations applied
    let url = postgres_url().expect("url");
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .expect("size-1 pool");
    let carts = PgCartRepository::new(pool.clone());
    let promos = PgPromotionRepository::new(pool.clone());
    let code = unique("ONECONN");
    active_promo_with_coupon(&promos, &code, |c| c).await;

    let run = async {
        let cart = checkoutable_cart(&carts).await;
        let id = cart.id.into_uuid();
        let cart = carts.apply_discount_async(id, &code).await.expect("apply");
        assert_eq!(cart.discount_amount, dec!(1.00));
        let item = carts.add_item_async(id, line("SKU-ONE", 1, dec!(30))).await.expect("add");
        carts
            .update_item_async(item.id, UpdateCartItem { quantity: Some(2), ..Default::default() })
            .await
            .expect("update");
        let cart = carts.get_async(id).await.expect("ok").expect("found");
        assert_eq!(cart.subtotal, dec!(70));
        assert_eq!(cart.discount_amount, dec!(7.00));
        carts.remove_item_async(item.id).await.expect("remove");
        let result = carts.complete_async(id).await.expect("checkout");
        assert_eq!(result.total_charged, dec!(9.00));
    };
    tokio::time::timeout(std::time::Duration::from_secs(20), run)
        .await
        .expect("coupon paths deadlocked on a size-1 pool");
}

/// Mirror of the SQLite `bundle_coupon_loses_discount_when_bundle_item_removed`.
#[tokio::test]
async fn postgres_bundle_coupon_loses_discount_when_bundle_item_removed() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let (carts, promos) = (db.carts(), db.promotions());
    // `cart_items.product_id` is a real foreign key on Postgres.
    let mut product_ids = Vec::new();
    for name in ["Widget", "Gadget"] {
        let product = db
            .products()
            .create_async(stateset_core::CreateProduct {
                name: format!("{name} {}", unique("BNDL")),
                ..Default::default()
            })
            .await
            .expect("create product");
        product_ids.push(product.id);
    }
    let (widget, gadget) = (product_ids[0], product_ids[1]);
    let code = unique("BUNDLE15");
    let promo = promos
        .create_async(CreatePromotion {
            code: Some(format!("{code}-PROMO")),
            name: "Widget + Gadget bundle".into(),
            promotion_type: PromotionType::BundleDiscount,
            trigger: PromotionTrigger::CouponCode,
            target: PromotionTarget::Order,
            stacking: StackingBehavior::Stackable,
            bundle_product_ids: Some(vec![widget, gadget]),
            bundle_discount: Some(dec!(15)),
            ..Default::default()
        })
        .await
        .expect("create promo");
    promos.activate_async(promo.id.into_uuid()).await.expect("activate");
    promos.create_coupon_async(coupon_input(promo.id, &code)).await.expect("coupon");

    let cart = checkoutable_cart(&carts).await;
    let id = cart.id.into_uuid();
    let with_product = |product, sku: &str, price| AddCartItem {
        product_id: Some(product),
        ..line(sku, 1, price)
    };
    carts.add_item_async(id, with_product(widget, "SKU-WIDGET", dec!(40))).await.expect("add");
    let gadget_line =
        carts.add_item_async(id, with_product(gadget, "SKU-GADGET", dec!(60))).await.expect("add");
    let cart = carts.apply_discount_async(id, &code).await.expect("applies");
    assert_eq!(cart.subtotal, dec!(110));
    assert_eq!(cart.discount_amount, dec!(15));
    assert_eq!(cart.grand_total, dec!(95));

    carts.remove_item_async(gadget_line.id).await.expect("remove");
    let cart = carts.get_async(id).await.expect("ok").expect("found");
    assert_eq!(cart.subtotal, dec!(50));
    assert_eq!(cart.discount_amount, dec!(0), "bundle discount must be re-derived");
    assert_eq!(cart.grand_total, dec!(50));
    assert_eq!(cart.coupon_code.as_deref(), Some(code.as_str()));

    carts.add_item_async(id, with_product(gadget, "SKU-GADGET", dec!(60))).await.expect("add");
    let cart = carts.get_async(id).await.expect("ok").expect("found");
    assert_eq!(cart.discount_amount, dec!(15));
    assert_eq!(cart.grand_total, dec!(95));
}

/// The kernel's checkout Preview runs the same coupon re-validation as
/// Apply: it cannot succeed where Apply would refuse, and reports the same
/// error.
#[tokio::test]
async fn postgres_checkout_preview_refuses_coupon_that_stopped_qualifying() {
    use stateset_core::{
        CommandEnvelope, CommitCheckout, ExecutionStatus, KernelCommandPolicy, KernelPolicy,
        KernelPrincipal, PrincipalKind, SetCartPayment,
    };
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let (carts, promos) = (db.carts(), db.promotions());
    let code = unique("TWENTY100");
    pct_promo_with_minimum(&promos, &code, dec!(0.20), dec!(100)).await;
    let cart = checkoutable_cart(&carts).await;
    let id = cart.id.into_uuid();
    carts
        .set_payment_async(
            id,
            SetCartPayment {
                payment_method: "credit_card".into(),
                payment_token: Some("tok_preview".into()),
                ..Default::default()
            },
        )
        .await
        .expect("payment");
    let item = carts.add_item_async(id, line("SKU-BIG", 1, dec!(90))).await.expect("add");
    carts.apply_discount_async(id, &code).await.expect("applies at $100");
    carts
        .update_item_async(
            item.id,
            UpdateCartItem { unit_price: Some(dec!(20)), ..Default::default() },
        )
        .await
        .expect("reprice");

    let policy = KernelPolicy::new("commerce-policy-1")
        .allow("checkout.commit", KernelCommandPolicy::requiring(["checkout.commit"]));
    let mut preview = CommandEnvelope::preview(
        "checkout.commit",
        format!("preview-{}", unique("KEY")),
        KernelPrincipal {
            id: "agent:preview".into(),
            kind: PrincipalKind::Agent,
            tenant_id: Some("tenant-postgres".into()),
            delegated_by: Some("user-postgres".into()),
            capabilities: vec!["checkout.commit".into()],
        },
        CommitCheckout { cart_id: cart.id, stock_policy: None },
    );
    preview.store_id = Some("store-postgres".into());
    preview.policy_version = Some("commerce-policy-1".into());
    let receipt = db
        .kernel_executor(policy)
        .execute_commit_checkout_async(&preview)
        .await
        .expect("preview executes");
    assert_eq!(receipt.status, ExecutionStatus::Rejected, "preview must refuse: {receipt:?}");
    let message = receipt.error_message.clone().unwrap_or_default();
    assert!(
        message.contains(&code) && message.contains("no longer valid"),
        "preview must report the coupon error: {message}"
    );
    let apply_err = carts.complete_async(id).await.expect_err("apply refuses too");
    assert_eq!(apply_err.to_string(), message, "preview and apply must agree");
}

/// Mirror of the SQLite `update_with_discount_amount_recomputes_grand_total`.
#[tokio::test]
async fn postgres_update_with_discount_amount_recomputes_grand_total() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let carts = db.carts();
    let cart = checkoutable_cart(&carts).await;
    let id = cart.id.into_uuid();
    let cart = carts
        .update_async(
            id,
            UpdateCart {
                discount_amount: Some(dec!(3)),
                discount_description: Some("Manual".into()),
                ..Default::default()
            },
        )
        .await
        .expect("update");
    assert_eq!(cart.discount_amount, dec!(3));
    assert_eq!(cart.grand_total, dec!(7));
    let cart = carts
        .update_async(id, UpdateCart { discount_amount: Some(dec!(50)), ..Default::default() })
        .await
        .expect("update");
    assert_eq!(cart.discount_amount, dec!(10), "capped at what the cart can cover");
    assert_eq!(cart.grand_total, dec!(0));
    let err = carts
        .update_async(id, UpdateCart { discount_amount: Some(dec!(-1)), ..Default::default() })
        .await
        .expect_err("negative discount");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
}

/// `create` with initial items prices the cart through the shared totals
/// path (subtotal, discount cap, `grand_total`, rounding) — not a bare
/// `grand_total = subtotal` write — and applies the money-scale rule.
#[tokio::test]
async fn postgres_create_with_items_prices_through_totals_path() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let carts = db.carts();
    let cart = carts
        .create_async(CreateCart {
            items: Some(vec![line("SKU-A", 2, dec!(10.25)), line("SKU-B", 1, dec!(4.50))]),
            ..Default::default()
        })
        .await
        .expect("create");
    assert_eq!(cart.items.len(), 2);
    assert_eq!(cart.subtotal, dec!(25.00));
    assert_eq!(cart.grand_total, dec!(25.00));
    assert_eq!(cart.grand_total, cart.subtotal + cart.tax_amount + cart.shipping_amount);

    let err = carts
        .create_async(CreateCart {
            items: Some(vec![line("SKU-C", 1, dec!(1.234))]),
            ..Default::default()
        })
        .await
        .expect_err("sub-cent price");
    assert!(matches!(err, CommerceError::MoneyScaleExceedsCurrency { .. }), "got {err:?}");
}

/// Mirror of the SQLite `tax_follows_line_changes_proportionally`.
#[tokio::test]
async fn postgres_tax_follows_line_changes_proportionally() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let carts = db.carts();
    let cart = checkoutable_cart(&carts).await;
    let id = cart.id.into_uuid();
    let cart = carts.set_tax_async(id, dec!(0.80)).await.expect("tax");
    assert_eq!(cart.grand_total, dec!(10.80));

    let more = carts.add_item_async(id, line("SKU-MORE", 1, dec!(10))).await.expect("add");
    let cart = carts.get_async(id).await.expect("ok").expect("found");
    assert_eq!(cart.subtotal, dec!(20));
    assert_eq!(cart.tax_amount, dec!(1.60), "tax must be recomputed for the new lines");
    assert_eq!(cart.grand_total, dec!(21.60));

    carts
        .update_item_async(more.id, UpdateCartItem { quantity: Some(3), ..Default::default() })
        .await
        .expect("qty");
    let cart = carts.get_async(id).await.expect("ok").expect("found");
    assert_eq!(cart.subtotal, dec!(40));
    assert_eq!(cart.tax_amount, dec!(3.20));
    assert_eq!(cart.grand_total, dec!(43.20));

    carts.clear_items_async(id).await.expect("clear");
    let cart = carts.get_async(id).await.expect("ok").expect("found");
    assert_eq!(cart.tax_amount, dec!(0), "an empty cart carries no tax");
    assert_eq!(cart.grand_total, dec!(0));
}

/// Mirror of the SQLite `add_and_update_item_reject_sub_minor_unit_money`.
#[tokio::test]
async fn postgres_add_and_update_item_reject_sub_minor_unit_money() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let carts = db.carts();
    let cart = checkoutable_cart(&carts).await;
    let id = cart.id.into_uuid();
    let item = cart.items[0].clone();
    let scale_err = |err: CommerceError| {
        assert!(
            matches!(err, CommerceError::MoneyScaleExceedsCurrency { .. }),
            "expected MoneyScaleExceedsCurrency, got {err:?}"
        );
    };
    scale_err(carts.add_item_async(id, line("SKU-TINY", 1, dec!(10.001))).await.expect_err("add"));
    scale_err(
        carts
            .add_item_async(
                id,
                AddCartItem { original_price: Some(dec!(12.345)), ..line("SKU-ORIG", 1, dec!(10)) },
            )
            .await
            .expect_err("original"),
    );
    scale_err(
        carts
            .update_item_async(
                item.id,
                UpdateCartItem { unit_price: Some(dec!(9.995)), ..Default::default() },
            )
            .await
            .expect_err("unit price"),
    );
    scale_err(
        carts
            .update_item_async(
                item.id,
                UpdateCartItem { discount_amount: Some(dec!(0.001)), ..Default::default() },
            )
            .await
            .expect_err("discount"),
    );
    carts.add_item_async(id, line("SKU-OK", 1, dec!(10.500))).await.expect("trailing zeros ok");
    let cart = carts.get_async(id).await.expect("ok").expect("found");
    assert_eq!(cart.items.len(), 2);
    assert_eq!(cart.subtotal, dec!(20.50));
}

/// Mirror of the embedded `test_cart_remove_discount`: remove after apply.
#[tokio::test]
async fn postgres_remove_discount_after_apply() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let (carts, promos) = (db.carts(), db.promotions());
    let code = unique("SAVE10");
    active_promo_with_coupon(&promos, &code, |c| CreateCouponCode { usage_limit: Some(100), ..c })
        .await;
    let cart = cart_with_subtotal(&carts, dec!(59.98)).await;
    let id = cart.id.into_uuid();
    let cart = carts.apply_discount_async(id, &code).await.expect("apply");
    assert_eq!(cart.discount_amount, dec!(6.00));
    let cart = carts.remove_discount_async(id).await.expect("remove");
    assert!(cart.coupon_code.is_none());
    assert_eq!(cart.discount_amount, dec!(0));
    assert_eq!(cart.discount_description, None);
    assert_eq!(cart.grand_total, dec!(59.98));
}

/// Mirror of the SQLite `remove_discount_recovers_from_not_applied_state`.
#[tokio::test]
async fn postgres_remove_discount_recovers_from_not_applied_state() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let (carts, promos) = (db.carts(), db.promotions());
    let code = unique("TWENTY100");
    pct_promo_with_minimum(&promos, &code, dec!(0.20), dec!(100)).await;
    let cart = checkoutable_cart(&carts).await;
    let id = cart.id.into_uuid();
    let item = carts.add_item_async(id, line("SKU-BIG", 1, dec!(90))).await.expect("add");
    carts.apply_discount_async(id, &code).await.expect("applies at $100");
    carts
        .update_item_async(
            item.id,
            UpdateCartItem { unit_price: Some(dec!(20)), ..Default::default() },
        )
        .await
        .expect("reprice");
    let cart = carts.get_async(id).await.expect("ok").expect("found");
    assert!(cart.discount_description.unwrap_or_default().contains("not applied"));
    carts.complete_async(id).await.expect_err("refused while not applied");

    let cart = carts.remove_discount_async(id).await.expect("remove");
    assert_eq!(cart.coupon_code, None);
    assert_eq!(cart.discount_amount, dec!(0));
    assert_eq!(cart.discount_description, None);
    assert_eq!(cart.grand_total, dec!(30));
    let result = carts.complete_async(id).await.expect("checks out without the coupon");
    assert_eq!(result.total_charged, dec!(30));
}

// ============================================================================
// Round 6: the cart line guard on every path, guarded/atomic cart money,
// preview↔apply consumption parity, and guest-checkout customer identity
// ============================================================================

/// A catalogued, sellable SKU: an `Active` product with one active variant at
/// `price`. `products.create` mints a `Draft` product, so publishing is a
/// separate step.
async fn catalogue_sku(
    db: &PostgresDatabase,
    price: Decimal,
) -> (stateset_core::ProductId, String) {
    let tag = Uuid::new_v4().simple().to_string();
    let sku = format!("CAT-{}", &tag[..12]);
    let product = db
        .products()
        .create_async(stateset_core::CreateProduct {
            name: format!("Product {sku}"),
            slug: Some(format!("product-{}", &tag[..12])),
            variants: Some(vec![stateset_core::CreateProductVariant {
                sku: sku.clone(),
                price,
                ..Default::default()
            }]),
            ..Default::default()
        })
        .await
        .expect("create product");
    db.products()
        .update_async(
            product.id,
            stateset_core::UpdateProduct {
                status: Some(stateset_core::ProductStatus::Active),
                ..Default::default()
            },
        )
        .await
        .expect("publish product");
    (product.id, sku)
}

async fn archive_product(db: &PostgresDatabase, product_id: stateset_core::ProductId) {
    db.products()
        .update_async(
            product_id,
            stateset_core::UpdateProduct {
                status: Some(stateset_core::ProductStatus::Archived),
                ..Default::default()
            },
        )
        .await
        .expect("archive product");
}

fn catalogue_line(sku: &str, quantity: i32, unit_price: Decimal) -> AddCartItem {
    AddCartItem {
        sku: sku.to_string(),
        name: sku.to_string(),
        quantity,
        unit_price,
        ..Default::default()
    }
}

fn assert_not_purchasable(err: &CommerceError, sku: &str) {
    match err {
        CommerceError::ValidationError(msg) => {
            assert!(msg.contains(sku), "expected {sku:?} named in {msg:?}");
            assert!(
                msg.contains("not purchasable") || msg.contains("no longer available"),
                "expected a purchasability reason in {msg:?}"
            );
        }
        other => panic!("expected ValidationError, got {other:?}"),
    }
}

/// `create_async` with `items` reaches `add_item_internal` directly, so it used
/// to skip the purchasability guard `add_item_async` runs.
#[tokio::test]
async fn postgres_create_with_items_refuses_a_sku_withdrawn_from_the_catalogue() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let carts = db.carts();
    let (product_id, sku) = catalogue_sku(&db, dec!(10.00)).await;
    archive_product(&db, product_id).await;

    let err = carts
        .create_async(CreateCart {
            items: Some(vec![catalogue_line(&sku, 1, dec!(10.00))]),
            ..Default::default()
        })
        .await
        .expect_err("a withdrawn SKU must not enter a cart at creation time");
    assert_not_purchasable(&err, &sku);

    // A live catalogue SKU and an ad-hoc SKU both still create fine.
    let (_, live) = catalogue_sku(&db, dec!(4.00)).await;
    let cart = carts
        .create_async(CreateCart {
            items: Some(vec![
                catalogue_line(&live, 1, dec!(4.00)),
                catalogue_line(&unique("SKU-ADHOC"), 2, dec!(3.00)),
            ]),
            ..Default::default()
        })
        .await
        .expect("create with sellable lines");
    let cart = carts.get_async(cart.id.into_uuid()).await.expect("ok").expect("found");
    assert_eq!(cart.items.len(), 2);
    assert_eq!(cart.subtotal, dec!(10.00));
}

/// Raising the quantity of a line whose SKU has since been withdrawn must be
/// refused; shrinking it must not.
#[tokio::test]
async fn postgres_update_item_refuses_raising_quantity_on_a_withdrawn_sku() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let carts = db.carts();
    let (_product_id, sku) = catalogue_sku(&db, dec!(10.00)).await;
    let cart = carts.create_async(CreateCart::default()).await.expect("cart");
    let id = cart.id.into_uuid();
    let item = carts
        .add_item_async(id, catalogue_line(&sku, 2, dec!(10.00)))
        .await
        .expect("add while sellable");

    // Withdraw the variant out from under the live cart line. The repository
    // refuses to soft-delete a variant a cart still holds, so this stands in
    // for the withdrawal happening first, elsewhere.
    sqlx::query("UPDATE product_variants SET is_active = false WHERE sku = $1")
        .bind(&sku)
        .execute(db.pool())
        .await
        .expect("withdraw the variant");

    let err = carts
        .update_item_async(item.id, UpdateCartItem { quantity: Some(5), ..Default::default() })
        .await
        .expect_err("must not grow a line on a withdrawn SKU");
    assert_not_purchasable(&err, &sku);
    let cart = carts.get_async(id).await.expect("ok").expect("found");
    assert_eq!(cart.items[0].quantity, 2, "the refused update must roll back");

    // Shrinking still works: the customer can back out of a dead line.
    carts
        .update_item_async(item.id, UpdateCartItem { quantity: Some(1), ..Default::default() })
        .await
        .expect("shrinking a withdrawn line is allowed");
    let cart = carts.get_async(id).await.expect("ok").expect("found");
    assert_eq!(cart.items[0].quantity, 1);
    carts.remove_item_async(item.id).await.expect("removing is allowed");
}

/// Catalogue lines are priced from the CATALOGUE on every repository path, not
/// from the client. This used to live only in the embedded accessor, so the
/// whole async Postgres API priced from the client.
#[tokio::test]
async fn postgres_cart_lines_are_priced_from_the_catalogue_not_the_client() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let carts = db.carts();
    let (_product_id, sku) = catalogue_sku(&db, dec!(25.00)).await;
    let expect_price_refusal = |err: &CommerceError| match err {
        CommerceError::ValidationError(msg) => {
            assert!(msg.contains(&sku), "{msg}");
            assert!(msg.contains("catalog price"), "{msg}");
        }
        other => panic!("expected ValidationError, got {other:?}"),
    };

    let err = carts
        .create_async(CreateCart {
            items: Some(vec![catalogue_line(&sku, 1, dec!(1.00))]),
            ..Default::default()
        })
        .await
        .expect_err("create must not price a catalogue line from the client");
    expect_price_refusal(&err);

    let cart = carts.create_async(CreateCart::default()).await.expect("cart");
    let id = cart.id.into_uuid();
    let err = carts
        .add_item_async(id, catalogue_line(&sku, 1, dec!(1.00)))
        .await
        .expect_err("add_item must not price a catalogue line from the client");
    expect_price_refusal(&err);

    let item = carts
        .add_item_async(id, catalogue_line(&sku, 1, dec!(25.00)))
        .await
        .expect("catalog price");
    carts.add_item_async(id, line("SKU-ADHOC", 1, dec!(7.77))).await.expect("ad-hoc line");

    let err = carts
        .update_item_async(
            item.id,
            UpdateCartItem { unit_price: Some(dec!(0.01)), ..Default::default() },
        )
        .await
        .expect_err("update_item must not reprice a catalogue line");
    expect_price_refusal(&err);
    let cart = carts.get_async(id).await.expect("ok").expect("found");
    assert_eq!(cart.subtotal, dec!(32.77));
}

/// `set_tax_async` writes money straight onto the cart: it must reject a
/// negative amount, a sub-cent amount, and any cart that is no longer active.
#[tokio::test]
async fn postgres_set_tax_refuses_negative_sub_cent_and_inactive_carts() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let carts = db.carts();
    let cart = cart_with_subtotal(&carts, dec!(100.00)).await;
    let id = cart.id.into_uuid();

    let err = carts.set_tax_async(id, dec!(-5.00)).await.expect_err("negative tax");
    assert!(
        matches!(&err, CommerceError::ValidationError(m) if m.contains("must not be negative")),
        "got {err:?}"
    );
    let err = carts.set_tax_async(id, dec!(0.005)).await.expect_err("sub-cent tax");
    assert!(matches!(err, CommerceError::MoneyScaleExceedsCurrency { .. }), "got {err:?}");

    let stored = carts.get_async(id).await.expect("ok").expect("found");
    assert_eq!(stored.tax_amount, dec!(0), "a refused set_tax must not write");
    assert_eq!(stored.grand_total, dec!(100.00));

    let taxed = carts.set_tax_async(id, dec!(8.25)).await.expect("a real tax amount");
    assert_eq!(taxed.tax_amount, dec!(8.25));
    assert_eq!(taxed.grand_total, dec!(108.25));

    carts.cancel_async(id).await.expect("cancel");
    let err = carts.set_tax_async(id, dec!(1.00)).await.expect_err("cancelled cart");
    assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");
    assert_eq!(carts.get_async(id).await.expect("ok").expect("found").tax_amount, dec!(8.25));
}

/// A completed cart's totals are settled against a minted order.
#[tokio::test]
async fn postgres_set_tax_refuses_a_completed_cart() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let carts = db.carts();
    let cart = checkoutable_cart(&carts).await;
    let id = cart.id.into_uuid();
    let result = carts.complete_async(id).await.expect("checkout");
    let err = carts.set_tax_async(id, dec!(999.00)).await.expect_err("completed cart");
    assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");
    let cart = carts.get_async(id).await.expect("ok").expect("found");
    assert_eq!(cart.tax_amount, dec!(0));
    assert_eq!(cart.grand_total, result.total_charged);
}

/// `set_shipping_async` carries the same money guard as `set_tax_async`.
#[tokio::test]
async fn postgres_set_shipping_refuses_negative_and_sub_cent_amounts() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let carts = db.carts();
    let cart = cart_with_subtotal(&carts, dec!(50.00)).await;
    let id = cart.id.into_uuid();
    let shipping = |amount: Decimal| SetCartShipping {
        shipping_address: test_address(),
        shipping_method: Some("Ground".into()),
        shipping_carrier: Some("USPS".into()),
        shipping_amount: Some(amount),
    };

    let err =
        carts.set_shipping_async(id, shipping(dec!(-1.00))).await.expect_err("negative shipping");
    assert!(
        matches!(&err, CommerceError::ValidationError(m) if m.contains("must not be negative")),
        "got {err:?}"
    );
    let err =
        carts.set_shipping_async(id, shipping(dec!(1.005))).await.expect_err("sub-cent shipping");
    assert!(matches!(err, CommerceError::MoneyScaleExceedsCurrency { .. }), "got {err:?}");

    let stored = carts.get_async(id).await.expect("ok").expect("found");
    assert_eq!(stored.shipping_amount, dec!(0), "a refused set_shipping must not write");
    assert!(stored.shipping_method.is_none(), "nor may it write the method");

    let shipped = carts.set_shipping_async(id, shipping(dec!(6.50))).await.expect("real amount");
    assert_eq!(shipped.shipping_amount, dec!(6.50));
    assert_eq!(shipped.grand_total, dec!(56.50));

    carts.abandon_async(id).await.expect("abandon");
    let err = carts.set_shipping_async(id, shipping(dec!(1.00))).await.expect_err("abandoned cart");
    assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");
}

/// `set_tax` racing concurrent `add_item` calls must land on top of a
/// consistent subtotal, never on a half-computed one: both take the cart row
/// lock, and each writes its amount and reprices in ONE transaction.
#[tokio::test]
async fn postgres_concurrent_set_tax_and_add_item_keep_the_cart_consistent() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let carts = db.carts();
    let cart = carts.create_async(CreateCart::default()).await.expect("create");
    let id = cart.id.into_uuid();
    carts.add_item_async(id, line("SKU-SEED", 1, dec!(10.00))).await.expect("seed line");

    const ADDS: i64 = 8;
    let mut handles = Vec::new();
    for n in 0..ADDS {
        let carts = db.carts();
        handles.push(tokio::spawn(async move {
            carts.add_item_async(id, line("SKU-RACE", 1, Decimal::from(n + 1))).await.expect("add");
        }));
    }
    let taxer = db.carts();
    handles.push(tokio::spawn(async move {
        taxer.set_tax_async(id, dec!(3.00)).await.expect("set tax");
    }));
    for handle in handles {
        handle.await.expect("join");
    }

    let cart = carts.get_async(id).await.expect("ok").expect("found");
    let expected_subtotal = dec!(10.00) + Decimal::from(ADDS * (ADDS + 1) / 2);
    assert_eq!(cart.subtotal, expected_subtotal, "a concurrent add was lost");
    // The tax lands somewhere between the amount set and that amount carried
    // up to the final subtotal (`rescale_tax` follows the lines it was
    // computed on), but it is always present and always priced into the
    // stored grand total.
    assert!(cart.tax_amount >= dec!(3.00), "the tax was lost: {}", cart.tax_amount);
    assert_eq!(
        cart.grand_total,
        cart.subtotal + cart.tax_amount + cart.shipping_amount - cart.discount_amount,
        "grand_total must agree with the parts it is made of"
    );
}

// ---------------------------------------------------------------------------
// Preview ↔ Apply consumption parity (through the kernel, the public path)
// ---------------------------------------------------------------------------

fn checkout_policy() -> stateset_core::KernelPolicy {
    stateset_core::KernelPolicy::new("cart-round6").allow(
        "checkout.commit",
        stateset_core::KernelCommandPolicy::requiring(["checkout.commit"]),
    )
}

fn checkout_command(
    cart_id: stateset_core::CartId,
    mode: stateset_core::ExecutionMode,
) -> stateset_core::CommandEnvelope<stateset_core::CommitCheckout> {
    let mut command = stateset_core::CommandEnvelope::preview(
        "checkout.commit",
        format!("cart-round6-{}", Uuid::new_v4()),
        stateset_core::KernelPrincipal {
            id: "agent:round6".into(),
            kind: stateset_core::PrincipalKind::Agent,
            tenant_id: Some("tenant-1".into()),
            delegated_by: Some("user-1".into()),
            capabilities: vec!["checkout.commit".into()],
        },
        stateset_core::CommitCheckout { cart_id, stock_policy: None },
    );
    command.store_id = Some("store-1".into());
    command.policy_version = Some("cart-round6".into());
    command.mode = mode;
    command
}

/// Preview and Apply must agree. Apply consumes the cart's coupon
/// (`consume_cart_coupon_in_tx`), and the per-customer limits it enforces are
/// checked against the customer Apply RESOLVES from the guest cart's e-mail —
/// not the `customer_id` on the cart, which is `None`. Preview never exercised
/// that consumption at all, so an exhausted coupon sailed through Preview and
/// then failed Apply.
#[tokio::test]
async fn postgres_preview_and_apply_agree_on_a_coupon_exhausted_for_the_customer() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let (carts, promos) = (db.carts(), db.promotions());
    let code = unique("PERCUST");
    let (_promo, coupon) = active_promo_with_coupon(&promos, &code, |c| CreateCouponCode {
        per_customer_limit: Some(1),
        ..c
    })
    .await;

    // The guest cart's e-mail already belongs to a customer who has spent
    // their one redemption of this coupon.
    let email = format!("{}@example.com", unique("percust").to_lowercase());
    let existing = db
        .customers()
        .create_async(CreateCustomer {
            email: email.clone(),
            first_name: "Ada".into(),
            last_name: "Lovelace".into(),
            ..Default::default()
        })
        .await
        .expect("customer");
    promos
        .record_usage_async(
            coupon.promotion_id,
            Some(coupon.id),
            Some(existing.id),
            None,
            None,
            dec!(1),
            "USD",
        )
        .await
        .expect("their earlier redemption");

    let cart = checkoutable_cart(&carts).await;
    let id = cart.id.into_uuid();
    carts
        .update_async(id, UpdateCart { customer_email: Some(email.clone()), ..Default::default() })
        .await
        .expect("guest cart carries the e-mail");
    let cart = carts.get_async(id).await.expect("ok").expect("found");
    assert!(cart.customer_id.is_none(), "guest cart: identity comes from the e-mail");
    carts.apply_discount_async(id, &code).await.expect("applies to the anonymous cart");

    let executor = db.kernel_executor(checkout_policy());
    let previewed = executor
        .execute_commit_checkout_async(&checkout_command(
            cart.id,
            stateset_core::ExecutionMode::Preview,
        ))
        .await
        .expect("preview returns a receipt");
    assert_eq!(
        previewed.status,
        stateset_core::ExecutionStatus::Rejected,
        "preview must refuse what apply refuses: {:?}",
        previewed.error_message
    );
    let preview_message = previewed.error_message.clone().unwrap_or_default();
    assert!(
        preview_message.contains("Per-customer coupon usage limit reached"),
        "preview said {preview_message:?}"
    );

    let applied = executor
        .execute_commit_checkout_async(&checkout_command(
            cart.id,
            stateset_core::ExecutionMode::Apply,
        ))
        .await
        .expect("apply returns a receipt");
    assert_eq!(applied.status, stateset_core::ExecutionStatus::Rejected);
    assert_eq!(
        applied.error_message.unwrap_or_default(),
        preview_message,
        "preview and apply must refuse identically"
    );
    assert_eq!(
        carts.get_async(id).await.expect("ok").expect("found").status,
        CartStatus::Active,
        "neither preview nor the failed apply may advance the cart"
    );
}

/// Preview accepts a cart whose coupon is still redeemable and writes nothing:
/// the coupon is consumed exactly once, by Apply.
#[tokio::test]
async fn postgres_preview_does_not_consume_the_coupon() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let (carts, promos) = (db.carts(), db.promotions());
    let code = unique("PREVIEWOK");
    let (promo, coupon) = active_promo_with_coupon(&promos, &code, |c| CreateCouponCode {
        usage_limit: Some(1),
        ..c
    })
    .await;
    let cart = checkoutable_cart(&carts).await;
    let id = cart.id.into_uuid();
    carts.apply_discount_async(id, &code).await.expect("applies");

    let executor = db.kernel_executor(checkout_policy());
    let previewed = executor
        .execute_commit_checkout_async(&checkout_command(
            cart.id,
            stateset_core::ExecutionMode::Preview,
        ))
        .await
        .expect("preview");
    assert_eq!(
        previewed.status,
        stateset_core::ExecutionStatus::Previewed,
        "{:?}",
        previewed.error_message
    );
    assert_eq!(
        promos.get_coupon_async(coupon.id).await.expect("ok").expect("coupon").usage_count,
        0,
        "preview must not consume the coupon"
    );
    assert_eq!(promos.get_async(promo.id).await.expect("ok").expect("promo").usage_count, 0);

    let applied = executor
        .execute_commit_checkout_async(&checkout_command(
            cart.id,
            stateset_core::ExecutionMode::Apply,
        ))
        .await
        .expect("apply");
    assert_eq!(applied.status, stateset_core::ExecutionStatus::Succeeded);
    assert_eq!(
        promos.get_coupon_async(coupon.id).await.expect("ok").expect("coupon").usage_count,
        1
    );
}

/// Guest checkout mints its customer through the customers repository's
/// get-or-create, so the row carries the normalised `email_key` and is
/// retrievable by e-mail afterwards. It used to open-code an INSERT that never
/// set the key (and an `ON CONFLICT (email)` on the raw column), so the
/// customer was unreachable by e-mail and two guests differing only in case
/// became two customers.
#[tokio::test]
async fn postgres_guest_checkout_customer_is_retrievable_by_email_case_insensitively() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let carts = db.carts();
    let local = unique("guest").to_lowercase();
    let mixed_case = format!("Guest.{local}@Example.COM");
    let normalised = mixed_case.to_lowercase();

    let guest_cart = |email: String| {
        let carts = db.carts();
        async move {
            let cart = checkoutable_cart(&carts).await;
            let id = cart.id.into_uuid();
            carts
                .update_async(id, UpdateCart { customer_email: Some(email), ..Default::default() })
                .await
                .expect("guest e-mail");
            id
        }
    };

    let first = guest_cart(mixed_case.clone()).await;
    carts.complete_async(first).await.expect("guest checkout");

    let created = db
        .customers()
        .get_by_email_async(&normalised)
        .await
        .expect("ok")
        .expect("a guest-checkout customer must be retrievable by e-mail");
    assert_eq!(created.email, normalised, "stored normalised");
    assert_eq!(
        db.customers()
            .get_by_email_async(&mixed_case.to_uppercase())
            .await
            .expect("ok")
            .map(|c| c.id),
        Some(created.id),
        "lookup is case-insensitive"
    );

    // A second guest whose address differs only in case is the SAME customer.
    let second = guest_cart(format!("guest.{local}@EXAMPLE.com")).await;
    carts.complete_async(second).await.expect("second guest checkout");
    let second_cart = carts.get_async(second).await.expect("ok").expect("found");
    assert_eq!(
        second_cart.customer_id,
        Some(created.id),
        "case-differing guests must resolve to one customer"
    );
}

/// `create_batch_atomic_async` is a third path that puts SKUs on cart lines,
/// and it inserted them unguarded too.
#[tokio::test]
async fn postgres_create_batch_atomic_refuses_a_sku_withdrawn_from_the_catalogue() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
        return;
    };
    let carts = db.carts();
    let (_live_id, live) = catalogue_sku(&db, dec!(5.00)).await;
    let (gone_id, gone) = catalogue_sku(&db, dec!(10.00)).await;
    archive_product(&db, gone_id).await;

    let err = carts
        .create_batch_atomic_async(vec![
            CreateCart {
                items: Some(vec![catalogue_line(&live, 1, dec!(5.00))]),
                ..Default::default()
            },
            CreateCart {
                items: Some(vec![catalogue_line(&gone, 1, dec!(10.00))]),
                ..Default::default()
            },
        ])
        .await
        .expect_err("a withdrawn SKU must not enter a cart through the batch path");
    assert_not_purchasable(&err, &gone);

    // A batch line priced away from the catalogue is refused too.
    let err = carts
        .create_batch_atomic_async(vec![CreateCart {
            items: Some(vec![catalogue_line(&live, 1, dec!(1.00))]),
            ..Default::default()
        }])
        .await
        .expect_err("batch lines are priced from the catalogue");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");

    let created = carts
        .create_batch_atomic_async(vec![CreateCart {
            items: Some(vec![catalogue_line(&live, 2, dec!(5.00))]),
            ..Default::default()
        }])
        .await
        .expect("sellable, catalogue-priced lines still batch-create");
    assert_eq!(created.len(), 1);
    assert_eq!(created[0].subtotal, dec!(10.00));
}
