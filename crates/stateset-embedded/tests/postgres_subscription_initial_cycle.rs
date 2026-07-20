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
    BillingCycleFilter, BillingInterval, CreateCustomer, CreateSubscription, CreateSubscriptionPlan,
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
