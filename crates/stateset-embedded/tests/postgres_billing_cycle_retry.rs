//! Postgres parity for billing-cycle failure bookkeeping.
//!
//! Marking a billing cycle `failed` must increment its `retry_count` (dunning)
//! and stamp `billed_at` (as SQLite does). The Postgres
//! `update_billing_cycle_status` path updated only `status` + `updated_at`, so a
//! failed cycle never advanced `retry_count` — retry-cap / dunning logic behaved
//! differently between backends. Both now keep the same bookkeeping.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use rust_decimal_macros::dec;
use stateset_core::{
    BillingCycleStatus, BillingInterval, CreateBillingCycle, CreateCustomer, CreateSubscription,
    CreateSubscriptionPlan,
};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test]
async fn postgres_failed_cycle_increments_retry_and_stamps_billed_at() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");
    let unique = uuid::Uuid::new_v4().to_string();

    let customer = commerce
        .customers()
        .create(CreateCustomer {
            email: format!("bc-{unique}@example.com"),
            first_name: "BC".into(),
            last_name: "Retry".into(),
            ..Default::default()
        })
        .await
        .expect("create customer");
    let plan = commerce
        .subscriptions()
        .create_plan(CreateSubscriptionPlan {
            name: format!("Retry Plan {unique}"),
            billing_interval: BillingInterval::Monthly,
            price: dec!(9.99),
            ..Default::default()
        })
        .await
        .expect("create plan");
    commerce.subscriptions().activate_plan(plan.id).await.expect("activate plan");
    let sub = commerce
        .subscriptions()
        .create_subscription(CreateSubscription {
            customer_id: customer.id,
            plan_id: plan.id,
            ..Default::default()
        })
        .await
        .expect("create subscription");

    // Creating the subscription already seeds cycle 1, so the next cycle is
    // number 2. (Re-creating cycle 1 is refused by the unique index from
    // migration 084_billing_cycle_uniqueness.)
    let period_start = sub.current_period_end;
    let cycle = commerce
        .subscriptions()
        .create_billing_cycle(CreateBillingCycle {
            subscription_id: sub.id,
            cycle_number: 2,
            period_start,
            period_end: period_start + chrono::Duration::days(30),
            claimed_by: None,
        })
        .await
        .expect("create billing cycle");
    assert_eq!(cycle.retry_count, 0);
    assert!(cycle.billed_at.is_none());

    // First failure: retry_count -> 1, billed_at stamped.
    let c1 = commerce
        .subscriptions()
        .update_billing_cycle_status(cycle.id, BillingCycleStatus::Failed)
        .await
        .expect("mark failed 1");
    assert_eq!(c1.retry_count, 1, "a failed cycle must increment retry_count");
    assert!(c1.billed_at.is_some(), "a failed cycle must stamp billed_at");

    // Second failure: retry_count -> 2 (dunning attempt count advances).
    let c2 = commerce
        .subscriptions()
        .update_billing_cycle_status(cycle.id, BillingCycleStatus::Failed)
        .await
        .expect("mark failed 2");
    assert_eq!(c2.retry_count, 2, "each failure must increment retry_count");
}
