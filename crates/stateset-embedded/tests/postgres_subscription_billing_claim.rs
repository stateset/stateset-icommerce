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

use chrono::{DateTime, Duration, TimeZone, Utc};
use rust_decimal_macros::dec;
use stateset_core::{
    BillingCycleStatus, BillingInterval, CancelSubscription, CommerceError, CreateBillingCycle,
    CreateCustomer, CreateSubscription, CreateSubscriptionPlan, Subscription, SubscriptionId,
};
use stateset_embedded::AsyncCommerce;
use uuid::Uuid;

/// The claim API has no tenant filter and the database is shared, so the two
/// tests in this binary run one at a time: each subscribes with a back-dated
/// start date, which is due the instant it exists, so a sibling's
/// `claim_due_for_billing(50, ..)` would otherwise sweep it up before its own
/// claim reaches it. (Mirrors `postgres_billing_claim.rs`.)
static SERIAL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

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

/// An ancient start date, so the subscription under test sorts FIRST in the
/// shared database's due set: `claim_due_for_billing` takes the oldest
/// `limit` rows, and a backlog of other due subscriptions would otherwise
/// crowd this one out of the batch. (Mirrors `postgres_billing_claim.rs`.)
fn epoch() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2001, 6, 1, 12, 0, 0).single().expect("epoch")
}

/// Claim at `at`, hand back every row that is not `mine` — the claim API has
/// no tenant filter, so a batch can contain other tests' subscriptions and
/// must not leave them leased — and return what was claimed of ours.
async fn claim_mine(
    commerce: &AsyncCommerce,
    worker: &str,
    lease_secs: i64,
    at: DateTime<Utc>,
    mine: SubscriptionId,
) -> Vec<Subscription> {
    let subs = commerce.subscriptions();
    let claimed = subs.claim_due_for_billing(50, worker, lease_secs, at).await.expect("claim");
    let mut kept = Vec::new();
    for sub in claimed {
        if sub.id == mine {
            kept.push(sub);
        } else {
            let _ = subs.release_billing_claim(sub.id.into_uuid(), worker).await;
        }
    }
    kept
}

/// Expire the subscription under test, so this binary leaves nothing in the
/// shared database's due set for the next test (or the next run) to trip on.
async fn expire(commerce: &AsyncCommerce, id: SubscriptionId) {
    let _ = commerce
        .subscriptions()
        .cancel_subscription(
            id.into_uuid(),
            CancelSubscription { immediate: Some(true), ..Default::default() },
        )
        .await;
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
    let _serial = SERIAL.lock().await;
    let Some(commerce) = connect().await else { return };
    let now = Utc::now();
    let sub = subscribe(&commerce, epoch()).await;
    let subs = commerce.subscriptions();
    let worker = format!("w1-{}", Uuid::new_v4());

    // A day-long lease, so nothing below can be explained by expiry.
    let claimed = claim_mine(&commerce, &worker, 86_400, now, sub.id).await;
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
    let mine = claim_mine(&commerce, &other, 60, next_due, sub.id).await;
    assert_eq!(mine.len(), 1, "immediately re-claimable");
    assert_eq!(mine[0].billing_lease_owner.as_deref(), Some(other.as_str()));

    expire(&commerce, sub.id).await;
}

/// `release_billing_claim` is owner-scoped: a worker that does not hold the
/// lease releases nothing (and gets `false`, not an error), so one worker can
/// never hand another's in-flight subscription back to the pool.
#[tokio::test]
async fn postgres_release_billing_claim_by_a_non_holder_is_refused() {
    let _serial = SERIAL.lock().await;
    let Some(commerce) = connect().await else { return };
    let now = Utc::now();
    let sub = subscribe(&commerce, epoch()).await;
    let subs = commerce.subscriptions();
    let holder = format!("holder-{}", Uuid::new_v4());

    let claimed = claim_mine(&commerce, &holder, 300, now, sub.id).await;
    assert_eq!(claimed.len(), 1, "our subscription must be claimed");

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

    expire(&commerce, sub.id).await;
}
