//! Postgres side of the subscription initial-billing-cycle parity guard.
//!
//! Creating a subscription must seed an initial billing cycle (cycle 1) for the
//! current period — SQLite did this, Postgres created none. This asserts the
//! Postgres behavior now matches (see
//! `sqlite/subscriptions.rs::create_subscription_seeds_an_initial_billing_cycle`).
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use rust_decimal_macros::dec;
use stateset_core::{
    BillingCycleFilter, BillingCycleStatus, BillingInterval, CreateCustomer, CreateSubscription,
    CreateSubscriptionPlan, SkipBillingCycle,
};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test]
async fn postgres_create_subscription_seeds_an_initial_billing_cycle() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping subscription cycle test");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");
    let subs = commerce.subscriptions();

    let unique = uuid::Uuid::new_v4().to_string();
    let customer = commerce
        .customers()
        .create(CreateCustomer {
            email: format!("sub-{}@example.com", &unique[..8]),
            first_name: "Sub".into(),
            last_name: "Scriber".into(),
            phone: None,
            accepts_marketing: None,
            tags: None,
            metadata: None,
        })
        .await
        .expect("create customer");

    let plan = subs
        .create_plan(CreateSubscriptionPlan {
            code: None,
            name: "Test Plan".into(),
            description: None,
            billing_interval: BillingInterval::Monthly,
            custom_interval_days: None,
            price: dec!(10.00),
            setup_fee: None,
            currency: None,
            trial_days: None,
            trial_requires_payment_method: None,
            min_cycles: None,
            max_cycles: None,
            items: None,
            discount_percent: None,
            discount_amount: None,
            metadata: None,
        })
        .await
        .expect("create plan");
    subs.activate_plan(plan.id).await.expect("activate plan");

    let sub = subs
        .create_subscription(CreateSubscription {
            customer_id: customer.id,
            plan_id: plan.id,
            items: None,
            price: None,
            payment_method_id: None,
            shipping_address: None,
            billing_address: None,
            skip_trial: None,
            start_date: None,
            coupon_code: None,
            metadata: None,
        })
        .await
        .expect("create subscription");

    let cycles = subs
        .list_billing_cycles(BillingCycleFilter {
            subscription_id: Some(sub.id),
            ..Default::default()
        })
        .await
        .expect("list billing cycles");
    assert_eq!(cycles.len(), 1, "a new subscription must have an initial billing cycle");
    assert_eq!(cycles[0].cycle_number, 1);
}

/// Postgres side of `sqlite/subscriptions.rs::skip_billing_cycle_marks_the_due_cycle_skipped_and_seeds_the_next_period`:
/// skipping settles the due cycle as `skipped` and seeds the next scheduled
/// cycle for the new period, keeping the one-scheduled-cycle-per-current-period
/// invariant that `create_subscription` establishes.
#[tokio::test]
async fn postgres_skip_billing_cycle_marks_the_due_cycle_skipped_and_seeds_the_next_period() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping subscription skip-cycle test");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");
    let subs = commerce.subscriptions();

    let unique = uuid::Uuid::new_v4().to_string();
    let customer = commerce
        .customers()
        .create(CreateCustomer {
            email: format!("skip-{}@example.com", &unique[..8]),
            first_name: "Sub".into(),
            last_name: "Skipper".into(),
            phone: None,
            accepts_marketing: None,
            tags: None,
            metadata: None,
        })
        .await
        .expect("create customer");

    let plan = subs
        .create_plan(CreateSubscriptionPlan {
            code: None,
            name: "Skip Plan".into(),
            description: None,
            billing_interval: BillingInterval::Monthly,
            custom_interval_days: None,
            price: dec!(10.00),
            setup_fee: None,
            currency: None,
            trial_days: None,
            trial_requires_payment_method: None,
            min_cycles: None,
            max_cycles: None,
            items: None,
            discount_percent: None,
            discount_amount: None,
            metadata: None,
        })
        .await
        .expect("create plan");
    subs.activate_plan(plan.id).await.expect("activate plan");

    let sub = subs
        .create_subscription(CreateSubscription {
            customer_id: customer.id,
            plan_id: plan.id,
            items: None,
            price: None,
            payment_method_id: None,
            shipping_address: None,
            billing_address: None,
            skip_trial: Some(true),
            start_date: Some(chrono::Utc::now() - chrono::Duration::days(20)),
            coupon_code: None,
            metadata: None,
        })
        .await
        .expect("create subscription");
    let due_at = sub.next_billing_date.expect("active subscription has a billing date");
    assert_eq!(due_at, sub.current_period_end);

    let skipped = subs
        .skip_billing_cycle(
            sub.id.into_uuid(),
            SkipBillingCycle { reason: Some("Traveling".into()) },
        )
        .await
        .expect("skip");
    let new_due_at = skipped.next_billing_date.expect("still scheduled");
    assert!(new_due_at > due_at);
    assert_eq!(skipped.current_period_end, new_due_at);
    assert_eq!(skipped.current_period_start, due_at);
    assert_eq!(skipped.billing_cycle_count, 0);

    let mut cycles = subs
        .list_billing_cycles(BillingCycleFilter {
            subscription_id: Some(sub.id),
            ..Default::default()
        })
        .await
        .expect("list billing cycles");
    cycles.sort_by_key(|c| c.cycle_number);
    assert_eq!(cycles.len(), 2, "{cycles:?}");

    assert_eq!(cycles[0].cycle_number, 1);
    assert_eq!(cycles[0].status, BillingCycleStatus::Skipped);
    assert_eq!(cycles[0].period_end, due_at);
    assert_eq!(cycles[0].failure_reason.as_deref(), Some("Traveling"));

    assert_eq!(cycles[1].cycle_number, 2);
    assert_eq!(cycles[1].status, BillingCycleStatus::Scheduled);
    assert_eq!(cycles[1].period_start, due_at);
    assert_eq!(cycles[1].period_end, new_due_at);

    let err = subs
        .update_billing_cycle_status(cycles[0].id, BillingCycleStatus::Paid)
        .await
        .expect_err("paying a skipped cycle must be refused");
    assert!(matches!(err, stateset_embedded::CommerceError::ValidationError(_)), "{err:?}");
}
