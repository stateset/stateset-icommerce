//! Postgres mirrors of the SQLite billing-claim lease guards
//! (`tests/billing_claim_test.rs`).
//!
//! The Postgres claim path (`SELECT ... FOR UPDATE SKIP LOCKED`) had no
//! live-database lifecycle test at all, so three behaviours that decide
//! whether a customer can be charged twice were unverified on this backend:
//!
//! - a settled cycle RELEASES the lease, so the subscription is immediately
//!   re-claimable rather than pinned until the lease expires;
//! - `release_billing_claim` by a non-holder is refused (and is a no-op, not
//!   an error);
//! - a live lease hides the subscription from every other worker, and only
//!   the holder may bill it.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`);
//! skipped otherwise.

#![cfg(feature = "postgres")]

use chrono::{DateTime, Duration, Utc};
use rust_decimal_macros::dec;
use stateset_core::{
    BillingCycleStatus, BillingInterval, CommerceError, CreateBillingCycle, CreateCustomer,
    CreateSubscription, CreateSubscriptionPlan, Subscription,
};
use stateset_embedded::AsyncCommerce;
use uuid::Uuid;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

async fn connect() -> Option<AsyncCommerce> {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping billing claim test");
        return None;
    };
    Some(AsyncCommerce::connect(&url).await.expect("connect + migrate"))
}

/// A monthly subscription that started `start`, so it is due whenever the
/// clock has passed one interval.
async fn subscribe(commerce: &AsyncCommerce, start: DateTime<Utc>) -> Subscription {
    let customer = commerce
        .customers()
        .create(CreateCustomer {
            email: format!("claim-{}@example.com", Uuid::new_v4()),
            first_name: "Claim".into(),
            last_name: "Test".into(),
            ..Default::default()
        })
        .await
        .expect("create customer");

    let subs = commerce.subscriptions();
    let plan = subs
        .create_plan(CreateSubscriptionPlan {
            name: format!("Claim Plan {}", Uuid::new_v4()),
            billing_interval: BillingInterval::Monthly,
            price: dec!(29.99),
            ..Default::default()
        })
        .await
        .expect("create plan");
    subs.activate_plan(plan.id).await.expect("activate plan");

    subs.create_subscription(CreateSubscription {
        customer_id: customer.id,
        plan_id: plan.id,
        start_date: Some(start),
        ..Default::default()
    })
    .await
    .expect("create subscription")
}

async fn reload(commerce: &AsyncCommerce, id: stateset_core::SubscriptionId) -> Subscription {
    commerce
        .subscriptions()
        .get_subscription(id.into_uuid())
        .await
        .expect("get subscription")
        .expect("subscription exists")
}

/// A settled cycle ends the claim. Leaving the lease to expire pinned a
/// subscription that had finished billing for the rest of the lease.
#[tokio::test]
async fn postgres_paying_a_cycle_releases_the_lease_immediately() {
    let Some(commerce) = connect().await else { return };
    let now = Utc::now();
    let sub = subscribe(&commerce, now - Duration::days(40)).await;
    let subs = commerce.subscriptions();
    let worker = format!("w1-{}", Uuid::new_v4());

    // A day-long lease, so nothing below can be explained by expiry.
    let claimed = subs.claim_due_for_billing(50, &worker, 86_400, now).await.expect("claim");
    let claimed: Vec<_> = claimed.into_iter().filter(|s| s.id == sub.id).collect();
    assert_eq!(claimed.len(), 1, "our subscription must be claimed");
    assert_eq!(claimed[0].billing_lease_owner.as_deref(), Some(worker.as_str()));

    let start = sub.current_period_end;
    let end = start + Duration::days(30);
    let cycle = subs
        .create_billing_cycle(CreateBillingCycle {
            subscription_id: sub.id,
            cycle_number: 2,
            period_start: start,
            period_end: end,
            claimed_by: Some(worker.clone()),
        })
        .await
        .expect("lease holder bills");

    // Still leased while the cycle is unpaid: another worker is refused.
    let err = subs
        .create_billing_cycle(CreateBillingCycle {
            subscription_id: sub.id,
            cycle_number: 3,
            period_start: end,
            period_end: end + Duration::days(30),
            claimed_by: Some("other-worker".into()),
        })
        .await
        .expect_err("another worker must not bill a leased subscription");
    assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");

    subs.update_billing_cycle_status(cycle.id, BillingCycleStatus::Paid).await.expect("mark paid");

    let after = reload(&commerce, sub.id).await;
    assert_eq!(after.billing_lease_owner, None, "a settled cycle releases the lease");
    assert_eq!(after.billing_lease_until, None);
    assert_eq!(after.billing_cycle_count, 1);

    // Immediately re-claimable by anyone once the clock next comes due,
    // rather than waiting the old lease out.
    let next_due = after.next_billing_date.expect("clock advanced") + Duration::seconds(1);
    let other = format!("w2-{}", Uuid::new_v4());
    let reclaimed = subs.claim_due_for_billing(50, &other, 60, next_due).await.expect("claim");
    let mine: Vec<_> = reclaimed.into_iter().filter(|s| s.id == sub.id).collect();
    assert_eq!(mine.len(), 1, "immediately re-claimable");
    assert_eq!(mine[0].billing_lease_owner.as_deref(), Some(other.as_str()));
}

/// `release_billing_claim` is owner-scoped: a worker that does not hold the
/// lease releases nothing (and gets `false`, not an error), so one worker can
/// never hand another's in-flight subscription back to the pool.
#[tokio::test]
async fn postgres_release_billing_claim_by_a_non_holder_is_refused() {
    let Some(commerce) = connect().await else { return };
    let now = Utc::now();
    let sub = subscribe(&commerce, now - Duration::days(40)).await;
    let subs = commerce.subscriptions();
    let holder = format!("holder-{}", Uuid::new_v4());

    let claimed = subs.claim_due_for_billing(50, &holder, 300, now).await.expect("claim");
    assert!(claimed.iter().any(|s| s.id == sub.id));

    assert!(
        !subs
            .release_billing_claim(sub.id.into_uuid(), "someone-else")
            .await
            .expect("release by non-holder is not an error"),
        "a non-holder must not release the lease"
    );
    let still = reload(&commerce, sub.id).await;
    assert_eq!(still.billing_lease_owner.as_deref(), Some(holder.as_str()), "lease untouched");
    assert!(still.billing_lease_until.is_some());

    // An unclaimed caller still cannot bill it.
    let err = subs
        .create_billing_cycle(CreateBillingCycle {
            subscription_id: sub.id,
            cycle_number: 2,
            period_start: sub.current_period_end,
            period_end: sub.current_period_end + Duration::days(30),
            claimed_by: None,
        })
        .await
        .expect_err("unclaimed caller");
    assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");

    // The owner releases it, and then anyone may bill.
    assert!(subs.release_billing_claim(sub.id.into_uuid(), &holder).await.expect("release"));
    assert_eq!(reload(&commerce, sub.id).await.billing_lease_owner, None);
    subs.create_billing_cycle(CreateBillingCycle {
        subscription_id: sub.id,
        cycle_number: 2,
        period_start: sub.current_period_end,
        period_end: sub.current_period_end + Duration::days(30),
        claimed_by: None,
    })
    .await
    .expect("unleased subscription is billable");
}
