//! Creating a subscription must never lose a race with a billing worker.
//!
//! `create_subscription` seeds the subscription's first billing cycle. That
//! seed used to run in its OWN transaction, after the subscription row had
//! already been committed — so a concurrent `claim_due_for_billing` could
//! lease the brand-new row in the gap, and the seed (an unclaimed caller)
//! was then refused by the billing-lease guard. The create failed with
//! `Conflict("... is leased for billing by another worker ...")` and left a
//! committed subscription with no billing cycle behind.
//!
//! A subscription whose start date is in the past is due the instant it
//! exists, so any biller sweeping the due set can hit this window. This test
//! hammers that window: several workers claim in a tight loop while
//! subscriptions are created.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`);
//! skipped otherwise.

#![cfg(feature = "postgres")]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::{DateTime, TimeZone, Utc};
use rust_decimal_macros::dec;
use stateset_core::{
    BillingCycleFilter, BillingInterval, CancelSubscription, CreateCustomer, CreateSubscription,
    CreateSubscriptionPlan,
};
use stateset_embedded::AsyncCommerce;
use uuid::Uuid;

/// How many subscriptions to create while the billers hammer.
const CREATES: usize = 40;
/// How many workers claim concurrently.
const CLAIMERS: usize = 3;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

async fn connect() -> Option<Arc<AsyncCommerce>> {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping create/claim race test");
        return None;
    };
    Some(Arc::new(AsyncCommerce::connect(&url).await.expect("connect + migrate")))
}

/// An ancient start date: these rows sort first in the due set, so the
/// claimers below reliably reach them.
fn epoch() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2001, 6, 1, 12, 0, 0).single().expect("epoch")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_creating_a_subscription_survives_a_concurrent_biller() {
    let Some(commerce) = connect().await else { return };

    let customer = commerce
        .customers()
        .create(CreateCustomer {
            email: format!("race-{}@example.com", Uuid::new_v4()),
            first_name: "Race".into(),
            last_name: "Test".into(),
            ..Default::default()
        })
        .await
        .expect("create customer");

    let plan = commerce
        .subscriptions()
        .create_plan(CreateSubscriptionPlan {
            name: format!("Race Plan {}", Uuid::new_v4()),
            billing_interval: BillingInterval::Monthly,
            price: dec!(19.99),
            ..Default::default()
        })
        .await
        .expect("create plan");
    commerce.subscriptions().activate_plan(plan.id).await.expect("activate plan");

    // Billing workers sweeping the due set, holding each lease just long
    // enough to overlap another task's create, then handing every row back
    // (including anything of someone else's the sweep caught).
    let stop = Arc::new(AtomicBool::new(false));
    let mut claimers = Vec::with_capacity(CLAIMERS);
    for n in 0..CLAIMERS {
        let commerce = Arc::clone(&commerce);
        let stop = Arc::clone(&stop);
        let worker = format!("racer-{n}-{}", Uuid::new_v4());
        claimers.push(tokio::spawn(async move {
            while !stop.load(Ordering::Relaxed) {
                let subs = commerce.subscriptions();
                let claimed = subs
                    .claim_due_for_billing(50, &worker, 60, Utc::now())
                    .await
                    .unwrap_or_default();
                if claimed.is_empty() {
                    tokio::task::yield_now().await;
                    continue;
                }
                tokio::time::sleep(std::time::Duration::from_millis(2)).await;
                for sub in claimed {
                    let _ = subs.release_billing_claim(sub.id.into_uuid(), &worker).await;
                }
            }
        }));
    }

    let mut created = Vec::with_capacity(CREATES);
    let mut failures = Vec::new();
    for _ in 0..CREATES {
        match commerce
            .subscriptions()
            .create_subscription(CreateSubscription {
                customer_id: customer.id,
                plan_id: plan.id,
                start_date: Some(epoch()),
                ..Default::default()
            })
            .await
        {
            Ok(sub) => created.push(sub),
            Err(err) => failures.push(err.to_string()),
        }
    }

    stop.store(true, Ordering::Relaxed);
    for claimer in claimers {
        let _ = claimer.await;
    }

    // Every subscription that was created must own its seeded cycle 1 — the
    // create is all-or-nothing.
    let mut missing_cycle = Vec::new();
    for sub in &created {
        let cycles = commerce
            .subscriptions()
            .list_billing_cycles(BillingCycleFilter {
                subscription_id: Some(sub.id),
                ..Default::default()
            })
            .await
            .expect("list cycles");
        if !cycles.iter().any(|c| c.cycle_number == 1) {
            missing_cycle.push(sub.id.to_string());
        }
    }

    // Clean up before asserting: leave nothing of ours in anyone's due set.
    for sub in &created {
        let _ = commerce
            .subscriptions()
            .cancel_subscription(
                sub.id.into_uuid(),
                CancelSubscription { immediate: Some(true), ..Default::default() },
            )
            .await;
    }

    assert!(
        failures.is_empty(),
        "creating a subscription must not fail because a biller claimed it: {} of {CREATES} failed, e.g. {}",
        failures.len(),
        failures.first().map(String::as_str).unwrap_or_default()
    );
    assert!(missing_cycle.is_empty(), "subscriptions created without cycle 1: {missing_cycle:?}");
}
