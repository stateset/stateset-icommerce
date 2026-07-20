//! Postgres parity for `list_billing_cycles` date filters + ordering.
//!
//! SQLite dropped `from_date`/`to_date` and ordered by `cycle_number DESC`;
//! Postgres filters `period_start >= from_date` / `period_end <= to_date` and orders
//! by `period_start DESC`. This locks in that behavior so the two backends agree.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use rust_decimal_macros::dec;
use stateset_core::{
    BillingCycleFilter, BillingInterval, CreateBillingCycle, CreateCustomer, CreateSubscription,
    CreateSubscriptionPlan,
};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test]
async fn postgres_list_billing_cycles_filters_by_date_and_orders_by_period_start() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping billing-cycle filter test");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");
    let subs = commerce.subscriptions();

    let unique = uuid::Uuid::new_v4().to_string();
    let customer = commerce
        .customers()
        .create(CreateCustomer {
            email: format!("bc-{}@example.com", &unique[..8]),
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

    // create_subscription seeds cycle 1 with period_start = now.
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

    let dt = |s: &str| s.parse::<chrono::DateTime<chrono::Utc>>().unwrap();
    subs.create_billing_cycle(CreateBillingCycle {
        subscription_id: sub.id,
        cycle_number: 2,
        period_start: dt("2020-01-15T00:00:00Z"),
        period_end: dt("2020-01-31T00:00:00Z"),
    })
    .await
    .expect("cycle 2");
    subs.create_billing_cycle(CreateBillingCycle {
        subscription_id: sub.id,
        cycle_number: 3,
        period_start: dt("2020-02-15T00:00:00Z"),
        period_end: dt("2020-02-28T00:00:00Z"),
    })
    .await
    .expect("cycle 3");

    let base = || BillingCycleFilter { subscription_id: Some(sub.id), ..Default::default() };

    let jan = subs
        .list_billing_cycles(BillingCycleFilter {
            from_date: Some(dt("2020-01-01T00:00:00Z")),
            to_date: Some(dt("2020-01-31T00:00:00Z")),
            ..base()
        })
        .await
        .expect("list jan");
    assert_eq!(jan.len(), 1, "date window should select only cycle 2");
    assert_eq!(jan[0].cycle_number, 2);

    let janfeb = subs
        .list_billing_cycles(BillingCycleFilter {
            from_date: Some(dt("2020-01-01T00:00:00Z")),
            to_date: Some(dt("2020-02-28T00:00:00Z")),
            ..base()
        })
        .await
        .expect("list jan-feb");
    assert_eq!(janfeb.len(), 2, "date window should select cycles 2 and 3");

    // period_start DESC: the now-dated cycle 1 sorts ahead of the 2020 cycles.
    let all = subs.list_billing_cycles(base()).await.expect("list all");
    assert_eq!(all.len(), 3);
    assert_eq!(all[0].cycle_number, 1, "newest period_start (cycle 1) must sort first");
}
