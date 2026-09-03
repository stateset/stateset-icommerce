//! Postgres side of the billing-claim lease and subscription lifecycle
//! parity guards (SQLite covered by `billing_claim_test.rs` and the unit
//! tests in `sqlite/subscriptions.rs`).
//!
//! - `claim_due_for_billing` (`SELECT ... FOR UPDATE SKIP LOCKED`) hands
//!   concurrent workers disjoint batches; a live lease hides the row and
//!   refuses cycles from anyone but the holder; leases die on their own.
//! - A trial subscription joins the due set once its trial ends — not
//!   before — and the first post-trial cycle activates it.
//! - pause / resume / cancel / skip behave exactly like SQLite: a pause
//!   keeps the paid remainder, resume gives it back, cancel ends at the
//!   paid-through date (or now when immediate), skip moves exactly one
//!   calendar interval and refuses non-active subscriptions.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`);
//! skipped otherwise.

#![cfg(feature = "postgres")]

use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use rust_decimal_macros::dec;
use stateset_core::{
    BillingCycleFilter, BillingInterval, CancelSubscription, CommerceError, CreateBillingCycle,
    CreateCustomer, CreateSubscription, CreateSubscriptionPlan, PauseSubscription,
    SkipBillingCycle, Subscription, SubscriptionEventType, SubscriptionStatus,
};
use stateset_embedded::AsyncCommerce;
use uuid::Uuid;

/// The claim API has no tenant filter and the database is shared, so the
/// tests in this binary run one at a time and each uses its own (ancient)
/// due epoch: its rows sort first in the due set, anything else a claim
/// sweeps up is released untouched, and every row is expired on the way out.
static SERIAL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Wall-clock "now" at whole-second precision, so lease timestamps survive
/// Postgres's microsecond storage and compare exactly.
fn now_secs() -> DateTime<Utc> {
    chrono::Timelike::with_nanosecond(&Utc::now(), 0).expect("truncate")
}

fn epoch(year: i32) -> DateTime<Utc> {
    chrono::TimeZone::with_ymd_and_hms(&Utc, year, 6, 1, 12, 0, 0).single().expect("epoch")
}

/// Expire `subs` so they never re-enter anyone's due set.
async fn expire_all(commerce: &AsyncCommerce, subs: &[Subscription]) {
    for sub in subs {
        let _ = commerce
            .subscriptions()
            .cancel_subscription(
                sub.id.into_uuid(),
                CancelSubscription { immediate: Some(true), ..Default::default() },
            )
            .await;
    }
}

/// Release every claimed row that is not one of `mine`, returning mine.
async fn keep_mine(
    commerce: &AsyncCommerce,
    worker_id: &str,
    claimed: Vec<Subscription>,
    mine: &[stateset_core::SubscriptionId],
) -> Vec<Subscription> {
    let mut kept = Vec::new();
    for sub in claimed {
        if mine.contains(&sub.id) {
            kept.push(sub);
        } else {
            let _ =
                commerce.subscriptions().release_billing_claim(sub.id.into_uuid(), worker_id).await;
        }
    }
    kept
}

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

async fn plan(commerce: &AsyncCommerce, trial_days: i32) -> Uuid {
    let plan = commerce
        .subscriptions()
        .create_plan(CreateSubscriptionPlan {
            name: format!("Claim plan {}", Uuid::new_v4()),
            billing_interval: BillingInterval::Monthly,
            price: dec!(29.99),
            trial_days: Some(trial_days),
            ..Default::default()
        })
        .await
        .expect("create plan");
    commerce.subscriptions().activate_plan(plan.id).await.expect("activate plan");
    plan.id
}

async fn subscribe(commerce: &AsyncCommerce, plan_id: Uuid, start: DateTime<Utc>) -> Subscription {
    let unique = Uuid::new_v4().to_string();
    let customer = commerce
        .customers()
        .create(CreateCustomer {
            email: format!("claim-{}@example.com", &unique[..8]),
            first_name: "Claim".into(),
            last_name: "Test".into(),
            phone: None,
            accepts_marketing: None,
            tags: None,
            metadata: None,
        })
        .await
        .expect("create customer");
    commerce
        .subscriptions()
        .create_subscription(CreateSubscription {
            customer_id: customer.id,
            plan_id,
            start_date: Some(start),
            ..Default::default()
        })
        .await
        .expect("create subscription")
}

async fn reload(commerce: &AsyncCommerce, sub: &Subscription) -> Subscription {
    commerce
        .subscriptions()
        .get_subscription(sub.id.into_uuid())
        .await
        .expect("get")
        .expect("exists")
}

fn cycle(sub: &Subscription, n: i32, claimed_by: Option<&str>) -> CreateBillingCycle {
    CreateBillingCycle {
        subscription_id: sub.id,
        cycle_number: n,
        period_start: sub.current_period_end,
        period_end: sub.billing_interval.advance(sub.current_period_end, None),
        claimed_by: claimed_by.map(str::to_string),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_concurrent_claims_are_disjoint_and_bill_each_subscription_once() {
    let Some(commerce) = connect().await else { return };
    let _serial = SERIAL.lock().await;
    let commerce = Arc::new(commerce);
    let plan_id = plan(&commerce, 0).await;
    let now = now_secs();
    // Due since 1971: first in every claim's due ordering.
    let mut due = Vec::new();
    for _ in 0..9 {
        due.push(subscribe(&commerce, plan_id, epoch(1971)).await);
    }
    let due_ids: Vec<_> = due.iter().map(|s| s.id).collect();

    let workers = 3;
    let barrier = Arc::new(tokio::sync::Barrier::new(workers));
    let mut handles = Vec::new();
    for w in 0..workers {
        let commerce = Arc::clone(&commerce);
        let barrier = Arc::clone(&barrier);
        let due_ids = due_ids.clone();
        handles.push(tokio::spawn(async move {
            let worker_id = format!("pg-worker-{w}-{}", Uuid::new_v4());
            let subs = commerce.subscriptions();
            barrier.wait().await;
            // Each worker asks for MORE than its fair share so that only the
            // claim's atomicity keeps the batches disjoint.
            let claimed = subs.claim_due_for_billing(6, &worker_id, 300, now).await.expect("claim");
            let all_claimed: Vec<_> = claimed.iter().map(|s| s.id).collect();
            let mine = keep_mine(&commerce, &worker_id, claimed, &due_ids).await;
            let mut billed = Vec::new();
            for sub in mine {
                assert_eq!(sub.billing_lease_owner.as_deref(), Some(worker_id.as_str()));
                assert_eq!(sub.billing_lease_until, Some(now + Duration::seconds(300)));
                let created = subs
                    .create_billing_cycle(cycle(
                        &sub,
                        sub.billing_cycle_count + 2,
                        Some(&worker_id),
                    ))
                    .await
                    .expect("lease holder bills");
                subs.update_billing_cycle_status(
                    created.id,
                    stateset_core::BillingCycleStatus::Paid,
                )
                .await
                .expect("mark paid");
                // Settling the cycle already released the lease, so an
                // explicit release is a no-op that reports it was not held.
                assert!(
                    !subs
                        .release_billing_claim(sub.id.into_uuid(), &worker_id)
                        .await
                        .expect("release"),
                    "paying the cycle should have released the lease already"
                );
                billed.push((sub.id, sub.billing_cycle_count + 2));
            }
            (all_claimed, billed)
        }));
    }

    // These fixtures are overdue by decades, so paying one cycle advances them
    // by a single interval and leaves them due again. Since a settled cycle
    // releases its lease, another worker may legitimately claim such a
    // subscription to continue catching it up — so claims are NOT disjoint for
    // the whole run, only while a lease is held. What must never happen is two
    // workers billing the same PERIOD, which the `(subscription, cycle_number)`
    // key enforces.
    let mut seen_billed = std::collections::HashSet::new();
    for handle in handles {
        let (claimed, billed) = handle.await.expect("worker task");
        let mut batch = std::collections::HashSet::new();
        for id in claimed {
            assert!(batch.insert(id), "subscription {id} appeared twice in one claim batch");
        }
        for (id, cycle_number) in billed {
            assert!(
                seen_billed.insert((id, cycle_number)),
                "subscription {id} billed twice for cycle {cycle_number}"
            );
        }
    }
    assert_eq!(seen_billed.len(), 9, "every due subscription billed exactly once");

    for sub in &due {
        let after = reload(&commerce, sub).await;
        assert_eq!(after.billing_cycle_count, 1);
        assert_eq!(after.billing_lease_owner, None);
        let cycles = commerce
            .subscriptions()
            .list_billing_cycles(BillingCycleFilter {
                subscription_id: Some(sub.id),
                ..Default::default()
            })
            .await
            .expect("cycles");
        assert_eq!(cycles.len(), 2, "initial cycle + exactly one billed cycle");
    }
    expire_all(&commerce, &due).await;
}

#[tokio::test]
async fn postgres_leased_subscription_is_billable_only_by_the_holder_until_the_lease_dies() {
    let Some(commerce) = connect().await else { return };
    let _serial = SERIAL.lock().await;
    let plan_id = plan(&commerce, 0).await;
    let now = now_secs();
    let sub = subscribe(&commerce, plan_id, epoch(1972)).await;
    let subs = commerce.subscriptions();
    let w1 = format!("w1-{}", Uuid::new_v4());
    let w2 = format!("w2-{}", Uuid::new_v4());
    let is_mine = |s: &Subscription| s.id == sub.id;

    let claimed = subs.claim_due_for_billing(1000, &w1, 60, now).await.expect("claim");
    let claimed = keep_mine(&commerce, &w1, claimed, &[sub.id]).await;
    assert_eq!(claimed.len(), 1, "the due row is claimed");
    assert_eq!(claimed[0].billing_lease_until, Some(now + Duration::seconds(60)));
    // Hidden from the view and from other claims while leased.
    assert!(!subs.get_due_for_billing(now, None).await.expect("due").iter().any(is_mine));
    let other = subs.claim_due_for_billing(1000, &w2, 60, now).await.expect("claim");
    assert!(!other.iter().any(is_mine));
    keep_mine(&commerce, &w2, other, &[]).await;

    let err = subs.create_billing_cycle(cycle(&sub, 2, None)).await.expect_err("unclaimed");
    assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");
    let err = subs.create_billing_cycle(cycle(&sub, 2, Some(&w2))).await.expect_err("other worker");
    assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");
    subs.create_billing_cycle(cycle(&sub, 2, Some(&w1))).await.expect("holder bills");

    assert!(!subs.release_billing_claim(sub.id.into_uuid(), &w2).await.expect("release"));
    // Not released, but dead after 60s: re-claimable and billable by w2.
    let later = now + Duration::seconds(61);
    let reclaimed = subs.claim_due_for_billing(1000, &w2, 60, later).await.expect("claim");
    let reclaimed = keep_mine(&commerce, &w2, reclaimed, &[sub.id]).await;
    assert_eq!(reclaimed.len(), 1, "dead lease re-claimed");
    assert_eq!(reclaimed[0].billing_lease_owner.as_deref(), Some(w2.as_str()));
    subs.create_billing_cycle(cycle(&sub, 3, Some(&w2))).await.expect("new holder bills");
    assert!(subs.release_billing_claim(sub.id.into_uuid(), &w2).await.expect("release"));
    assert_eq!(reload(&commerce, &sub).await.billing_lease_until, None);
    expire_all(&commerce, &[sub]).await;
}

#[tokio::test]
async fn postgres_trial_is_due_once_it_ends_and_first_cycle_activates() {
    let Some(commerce) = connect().await else { return };
    let _serial = SERIAL.lock().await;
    let plan_id = plan(&commerce, 7).await;
    let now = Utc::now();
    let subs = commerce.subscriptions();

    let running = subscribe(&commerce, plan_id, now - Duration::days(3)).await;
    assert_eq!(running.status, SubscriptionStatus::Trial);
    let running_end = running.trial_ends_at.expect("trial end");
    assert!(
        !subs.get_due_for_billing(now, None).await.expect("due").iter().any(|s| s.id == running.id)
    );
    subs.create_billing_cycle(CreateBillingCycle {
        subscription_id: running.id,
        cycle_number: 2,
        period_start: now,
        period_end: running_end,
        claimed_by: None,
    })
    .await
    .expect("cycle inside the trial");
    assert_eq!(reload(&commerce, &running).await.status, SubscriptionStatus::Trial);

    let elapsed = subscribe(&commerce, plan_id, now - Duration::days(8)).await;
    assert_eq!(elapsed.status, SubscriptionStatus::Trial);
    let trial_end = elapsed.trial_ends_at.expect("trial end");
    assert!(
        subs.get_due_for_billing(now, None).await.expect("due").iter().any(|s| s.id == elapsed.id)
    );
    let worker = format!("trial-{}", Uuid::new_v4());
    let claimed = subs.claim_due_for_billing(1000, &worker, 60, now).await.expect("claim");
    let claimed = keep_mine(&commerce, &worker, claimed, &[elapsed.id]).await;
    assert_eq!(claimed.len(), 1, "the elapsed trial is claimable");
    subs.create_billing_cycle(CreateBillingCycle {
        subscription_id: elapsed.id,
        cycle_number: 2,
        period_start: trial_end,
        period_end: BillingInterval::Monthly.advance(trial_end, None),
        claimed_by: Some(worker.clone()),
    })
    .await
    .expect("first post-trial cycle");
    let activated = reload(&commerce, &elapsed).await;
    assert_eq!(activated.status, SubscriptionStatus::Active);
    let events = subs.get_subscription_events(elapsed.id.into_uuid()).await.expect("events");
    assert!(events.iter().any(|e| e.event_type == SubscriptionEventType::Activated), "{events:?}");
    assert!(subs.release_billing_claim(elapsed.id.into_uuid(), &worker).await.expect("release"));
    expire_all(&commerce, &[running, elapsed]).await;
}

#[tokio::test]
async fn postgres_pause_and_resume_carry_the_paid_remainder_forward() {
    let Some(commerce) = connect().await else { return };
    let plan_id = plan(&commerce, 0).await;
    // Paid through ~20 days from now.
    let sub = subscribe(&commerce, plan_id, Utc::now() - Duration::days(10)).await;
    let paid_through = sub.next_billing_date.expect("next billing date");
    let subs = commerce.subscriptions();

    let paused = subs
        .pause_subscription(sub.id.into_uuid(), PauseSubscription::default())
        .await
        .expect("pause");
    assert_eq!(paused.status, SubscriptionStatus::Paused);
    assert_eq!(paused.next_billing_date, None, "paused subscriptions never bill");
    assert_eq!(paused.current_period_end, paid_through, "paid-through date retained");
    let paused_at = paused.paused_at.expect("paused_at");
    // Cannot skip or re-pause while paused.
    let err = subs
        .skip_billing_cycle(sub.id.into_uuid(), SkipBillingCycle::default())
        .await
        .expect_err("skip while paused");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
    let err = subs
        .pause_subscription(sub.id.into_uuid(), PauseSubscription::default())
        .await
        .expect_err("double pause");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");

    let resumed = subs.resume_subscription(sub.id.into_uuid()).await.expect("resume");
    assert_eq!(resumed.status, SubscriptionStatus::Active);
    let remainder = paid_through - paused_at;
    let expected = resumed.current_period_start + remainder;
    let next = resumed.next_billing_date.expect("next billing date");
    assert!(
        (next - expected).num_seconds().abs() <= 1,
        "resume must return the paid remainder: next {next}, expected {expected}"
    );
    assert_eq!(resumed.current_period_end, next);
    assert_eq!(resumed.paused_at, None);
    let err = subs.resume_subscription(sub.id.into_uuid()).await.expect_err("double resume");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
    let events = subs.get_subscription_events(sub.id.into_uuid()).await.expect("events");
    assert!(events.iter().any(|e| e.event_type == SubscriptionEventType::Paused));
    assert!(events.iter().any(|e| e.event_type == SubscriptionEventType::Resumed));
}

#[tokio::test]
async fn postgres_cancel_ends_at_the_paid_through_date_or_immediately() {
    let Some(commerce) = connect().await else { return };
    let plan_id = plan(&commerce, 0).await;
    let subs = commerce.subscriptions();

    let sub = subscribe(&commerce, plan_id, Utc::now()).await;
    let cancelled = subs
        .cancel_subscription(sub.id.into_uuid(), CancelSubscription::default())
        .await
        .expect("cancel at period end");
    assert_eq!(cancelled.status, SubscriptionStatus::Cancelled);
    assert_eq!(cancelled.ends_at, Some(sub.current_period_end));
    assert_eq!(cancelled.next_billing_date, None);
    assert!(cancelled.cancelled_at.is_some());
    // Terminal: cannot cancel, pause, skip or resume.
    for err in [
        subs.cancel_subscription(sub.id.into_uuid(), CancelSubscription::default()).await.err(),
        subs.pause_subscription(sub.id.into_uuid(), PauseSubscription::default()).await.err(),
        subs.skip_billing_cycle(sub.id.into_uuid(), SkipBillingCycle::default()).await.err(),
        subs.resume_subscription(sub.id.into_uuid()).await.err(),
    ] {
        assert!(matches!(err, Some(CommerceError::ValidationError(_))), "got {err:?}");
    }

    let sub = subscribe(&commerce, plan_id, Utc::now()).await;
    let before = Utc::now();
    let expired = subs
        .cancel_subscription(
            sub.id.into_uuid(),
            CancelSubscription { immediate: Some(true), ..Default::default() },
        )
        .await
        .expect("cancel immediately");
    assert_eq!(expired.status, SubscriptionStatus::Expired);
    let ends_at = expired.ends_at.expect("ends_at");
    assert!(ends_at >= before && ends_at <= Utc::now() + Duration::seconds(1));
}

#[tokio::test]
async fn postgres_skip_moves_exactly_one_calendar_interval() {
    let Some(commerce) = connect().await else { return };
    let plan_id = plan(&commerce, 0).await;
    let subs = commerce.subscriptions();
    let sub = subscribe(&commerce, plan_id, Utc::now()).await;
    let next = sub.next_billing_date.expect("next billing date");

    let skipped = subs
        .skip_billing_cycle(sub.id.into_uuid(), SkipBillingCycle::default())
        .await
        .expect("skip");
    let expected = BillingInterval::Monthly.advance(next, None);
    assert_eq!(skipped.next_billing_date, Some(expected));
    assert_eq!(skipped.current_period_end, expected);
    assert_eq!(skipped.status, SubscriptionStatus::Active);
    let events = subs.get_subscription_events(sub.id.into_uuid()).await.expect("events");
    assert!(events.iter().any(|e| e.event_type == SubscriptionEventType::Skipped));

    // A second skip moves one more interval from the NEW date (no double
    // application of the first read).
    let again = subs
        .skip_billing_cycle(sub.id.into_uuid(), SkipBillingCycle::default())
        .await
        .expect("skip again");
    assert_eq!(again.next_billing_date, Some(BillingInterval::Monthly.advance(expected, None)));

    // Trials cannot skip.
    let trial_plan = plan(&commerce, 7).await;
    let trial = subscribe(&commerce, trial_plan, Utc::now()).await;
    let err = subs
        .skip_billing_cycle(trial.id.into_uuid(), SkipBillingCycle::default())
        .await
        .expect_err("skip on trial");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
}
