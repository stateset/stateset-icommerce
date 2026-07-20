//! Postgres per-customer promotion usage-limit must be race-safe.
//!
//! `record_usage` enforces `per_customer_limit` with a COUNT-then-INSERT against
//! the `promotion_usage` ledger. Under a plain READ COMMITTED transaction with no
//! row lock, two concurrent redemptions for the same (promotion, customer) both
//! read the ledger before either inserts, both pass the limit check, and both
//! insert — over-redeeming the limit. The SQLite backend already serializes via
//! `BEGIN IMMEDIATE`; Postgres now locks the promotion row `FOR UPDATE`.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use std::sync::Arc;

use rust_decimal_macros::dec;
use stateset_core::{
    CreateCustomer, CreatePromotion, PromotionTarget, PromotionTrigger, PromotionType,
    StackingBehavior,
};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_per_customer_usage_limit_is_race_safe() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping promo usage race test");
        return;
    };
    let commerce = Arc::new(AsyncCommerce::connect(&url).await.expect("connect + migrate"));

    let unique = uuid::Uuid::new_v4().to_string();
    // promotion_usage.customer_id FKs to customers, so use a real customer.
    let customer = commerce
        .customers()
        .create(CreateCustomer {
            email: format!("promo-race-{}@example.com", &unique[..8]),
            first_name: "Race".into(),
            last_name: "Test".into(),
            phone: None,
            accepts_marketing: None,
            tags: None,
            metadata: None,
        })
        .await
        .expect("create customer");
    let customer_id = customer.id.into_uuid();

    // Promotion capped at ONE redemption per customer.
    let promo = commerce
        .promotions()
        .create(CreatePromotion {
            code: Some(format!("ONCE-{}", &unique[..8])),
            name: "One per customer".into(),
            promotion_type: PromotionType::FixedAmountOff,
            trigger: PromotionTrigger::CouponCode,
            target: PromotionTarget::Order,
            stacking: StackingBehavior::Stackable,
            fixed_amount_off: Some(dec!(5)),
            per_customer_limit: Some(1),
            ..Default::default()
        })
        .await
        .expect("create promo");
    commerce.promotions().activate(promo.id.into_uuid()).await.expect("activate");
    let promo_id = promo.id.into_uuid();

    // Fire many concurrent redemptions for the SAME customer.
    let attempts = 10;
    let barrier = Arc::new(tokio::sync::Barrier::new(attempts));
    let mut handles = Vec::new();
    for _ in 0..attempts {
        let c = commerce.clone();
        let b = barrier.clone();
        handles.push(tokio::spawn(async move {
            b.wait().await;
            c.promotions()
                .record_usage(promo_id, None, Some(customer_id), None, None, dec!(5), "USD")
                .await
        }));
    }

    let mut succeeded = 0;
    for h in handles {
        if h.await.expect("join").is_ok() {
            succeeded += 1;
        }
    }

    assert_eq!(
        succeeded, 1,
        "exactly one redemption may succeed under per_customer_limit=1, got {succeeded}"
    );
}
