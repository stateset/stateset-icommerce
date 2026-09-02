#![cfg(feature = "sqlite")]

//! Billing-worker claim leases (SQLite backend, sync `Commerce` engine).
//!
//! `get_due_for_billing` is a read-only list: two workers polling at the same
//! instant both saw the same due subscription and both charged the customer
//! before the cycle-uniqueness backstop could stop the second one. Workers
//! now CLAIM a batch first (`claim_due_for_billing`): the claim leases each
//! row inside the write transaction, a live lease hides the row from every
//! other worker, and `create_billing_cycle` refuses a subscription whose live
//! lease belongs to someone else.
//!
//! Covers:
//! - N concurrent workers claiming the same due set receive disjoint batches
//!   and every due subscription is billed exactly once end to end;
//! - an unclaimed caller / another worker cannot bill a leased subscription;
//! - a lease dies on its own after `lease_secs`, so a crashed worker never
//!   wedges billing, and `release_billing_claim` only honours the owner;
//! - a trial subscription joins the due set once its trial ends — not
//!   before — and the first post-trial cycle activates it.

use std::sync::{Arc, Barrier};
use std::thread;

use chrono::{DateTime, Duration, Utc};
use rust_decimal_macros::dec;
use stateset_embedded::{
    BillingCycleFilter, BillingInterval, Commerce, CommerceError, CreateCustomer,
    CreateSubscription, CreateSubscriptionPlan, Subscription, SubscriptionEventType,
    SubscriptionStatus,
};
use uuid::Uuid;

fn commerce() -> Commerce {
    Commerce::new(":memory:").expect("in-memory Commerce")
}

fn plan(commerce: &Commerce, trial_days: i32) -> Uuid {
    let plan = commerce
        .subscriptions()
        .create_plan(CreateSubscriptionPlan {
            name: format!("Plan {}", Uuid::new_v4()),
            billing_interval: BillingInterval::Monthly,
            price: dec!(29.99),
            trial_days: Some(trial_days),
            ..Default::default()
        })
        .expect("create plan");
    commerce.subscriptions().activate_plan(plan.id).expect("activate plan");
    plan.id
}

fn subscribe(commerce: &Commerce, plan_id: Uuid, start: DateTime<Utc>) -> Subscription {
    let customer_id = commerce
        .customers()
        .create(CreateCustomer {
            email: format!("claim-{}@example.com", Uuid::new_v4()),
            first_name: "Claim".into(),
            last_name: "Test".into(),
            ..Default::default()
        })
        .expect("create customer")
        .id;
    commerce
        .subscriptions()
        .subscribe(CreateSubscription {
            customer_id,
            plan_id,
            start_date: Some(start),
            ..Default::default()
        })
        .expect("subscribe")
}

fn reload(commerce: &Commerce, id: stateset_core::SubscriptionId) -> Subscription {
    commerce.subscriptions().get(id).expect("get").expect("exists")
}

#[test]
fn concurrent_workers_claim_disjoint_batches_and_bill_each_subscription_once() {
    let commerce = Arc::new(commerce());
    let plan_id = plan(&commerce, 0);
    let now = Utc::now();
    // Monthly subscriptions started 40 days ago were due 10 days ago.
    let due: Vec<_> =
        (0..9).map(|_| subscribe(&commerce, plan_id, now - Duration::days(40)).id).collect();
    let fresh = subscribe(&commerce, plan_id, now);
    assert_eq!(commerce.subscriptions().get_due_for_billing(now).expect("due").len(), 9);

    // Three workers race for the same due set, each billing what it claims.
    let workers = 3;
    let barrier = Arc::new(Barrier::new(workers));
    let handles: Vec<_> = (0..workers)
        .map(|w| {
            let commerce = Arc::clone(&commerce);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                let worker_id = format!("worker-{w}");
                let subs = commerce.subscriptions();
                barrier.wait();
                let claimed = subs.claim_due_for_billing(4, &worker_id, 300, now).expect("claim");
                let mut billed = Vec::new();
                for sub in claimed {
                    assert_eq!(sub.billing_lease_owner.as_deref(), Some(worker_id.as_str()));
                    let period_end = sub.billing_interval.advance(sub.current_period_end, None);
                    let cycle = subs
                        .create_claimed_billing_cycle(
                            &worker_id,
                            sub.id,
                            sub.billing_cycle_count + 2,
                            sub.current_period_end,
                            period_end,
                        )
                        .expect("lease holder bills");
                    subs.mark_cycle_paid(cycle.id).expect("mark paid");
                    assert!(subs.release_billing_claim(sub.id, &worker_id).expect("release"));
                    billed.push(sub.id);
                }
                billed
            })
        })
        .collect();

    let mut seen = std::collections::HashSet::new();
    for handle in handles {
        for id in handle.join().expect("worker thread") {
            assert!(due.contains(&id), "billed a subscription that was not due");
            assert!(seen.insert(id), "subscription {id} billed by two workers");
        }
    }
    assert_eq!(seen.len(), 9, "every due subscription billed exactly once");

    for id in &due {
        let sub = reload(&commerce, *id);
        assert_eq!(sub.billing_cycle_count, 1, "{id} advanced by one paid cycle");
        assert!(sub.next_billing_date.is_some_and(|d| d > now), "{id} clock moved forward");
        assert_eq!(sub.billing_lease_owner, None, "{id} lease released");
        let cycles = commerce
            .subscriptions()
            .list_billing_cycles(BillingCycleFilter {
                subscription_id: Some(*id),
                ..Default::default()
            })
            .expect("cycles");
        assert_eq!(cycles.len(), 2, "{id}: initial cycle + exactly one billed cycle");
    }
    assert_eq!(reload(&commerce, fresh.id).billing_cycle_count, 0);
    assert!(commerce.subscriptions().get_due_for_billing(now).expect("due").is_empty());
}

#[test]
fn a_leased_subscription_can_only_be_billed_by_the_lease_holder() {
    let commerce = commerce();
    let plan_id = plan(&commerce, 0);
    let now = Utc::now();
    let sub = subscribe(&commerce, plan_id, now - Duration::days(40));
    let subs = commerce.subscriptions();

    let claimed = subs.claim_due_for_billing(10, "w1", 300, now).expect("claim");
    assert_eq!(claimed.len(), 1);
    assert!(subs.claim_due_for_billing(10, "w2", 300, now).expect("claim").is_empty());
    assert!(subs.get_due_for_billing(now).expect("due").is_empty(), "leased rows are hidden");

    let (start, end) = (sub.current_period_end, sub.current_period_end + Duration::days(30));
    let err = subs.create_billing_cycle(sub.id, 2, start, end).expect_err("unclaimed caller");
    assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");
    let err =
        subs.create_claimed_billing_cycle("w2", sub.id, 2, start, end).expect_err("another worker");
    assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");
    subs.create_claimed_billing_cycle("w1", sub.id, 2, start, end).expect("lease holder");

    // Only the owner can release; afterwards anyone may bill again.
    assert!(!subs.release_billing_claim(sub.id, "w2").expect("release"));
    assert!(subs.release_billing_claim(sub.id, "w1").expect("release"));
    subs.create_billing_cycle(sub.id, 3, end, end + Duration::days(30)).expect("unleased");
}

#[test]
fn a_lease_expires_on_its_own() {
    let commerce = commerce();
    let plan_id = plan(&commerce, 0);
    let now = Utc::now();
    let sub = subscribe(&commerce, plan_id, now - Duration::days(40));
    let subs = commerce.subscriptions();

    let claimed = subs.claim_due_for_billing(10, "crashed-worker", 60, now).expect("claim");
    assert_eq!(claimed[0].billing_lease_until, Some(now + Duration::seconds(60)));
    // Still leased one second before expiry...
    let almost = now + Duration::seconds(59);
    assert!(subs.claim_due_for_billing(10, "w2", 60, almost).expect("claim").is_empty());
    // ...re-claimable once it has died, and billable by the new holder.
    let later = now + Duration::seconds(61);
    assert_eq!(subs.get_due_for_billing(later).expect("due").len(), 1);
    let reclaimed = subs.claim_due_for_billing(10, "w2", 60, later).expect("claim");
    assert_eq!(reclaimed.len(), 1);
    assert_eq!(reclaimed[0].billing_lease_owner.as_deref(), Some("w2"));
    let (start, end) = (sub.current_period_end, sub.current_period_end + Duration::days(30));
    subs.create_claimed_billing_cycle("w2", sub.id, 2, start, end).expect("new holder bills");
}

#[test]
fn trial_subscription_is_due_once_its_trial_ends_and_activates_on_first_cycle() {
    let commerce = commerce();
    let plan_id = plan(&commerce, 7);
    let now = Utc::now();
    let subs = commerce.subscriptions();

    // Trial still running: not due; a cycle billed inside the trial does not
    // activate it.
    let running = subscribe(&commerce, plan_id, now - Duration::days(3));
    assert_eq!(running.status, SubscriptionStatus::Trial);
    let running_end = running.trial_ends_at.expect("trial end");
    assert!(subs.get_due_for_billing(now).expect("due").is_empty());
    subs.create_billing_cycle(running.id, 2, now, running_end).expect("cycle inside the trial");
    assert_eq!(reload(&commerce, running.id).status, SubscriptionStatus::Trial);

    // Trial elapsed: due; claim, bill the first post-trial cycle, activated
    // in the same transaction with an audited event.
    let elapsed = subscribe(&commerce, plan_id, now - Duration::days(8));
    assert_eq!(elapsed.status, SubscriptionStatus::Trial);
    let trial_end = elapsed.trial_ends_at.expect("trial end");
    let due = subs.get_due_for_billing(now).expect("due");
    assert_eq!(due.iter().map(|s| s.id).collect::<Vec<_>>(), vec![elapsed.id]);

    let claimed = subs.claim_due_for_billing(10, "w1", 60, now).expect("claim");
    assert_eq!(claimed.len(), 1);
    let cycle = subs
        .create_claimed_billing_cycle(
            "w1",
            elapsed.id,
            2,
            trial_end,
            BillingInterval::Monthly.advance(trial_end, None),
        )
        .expect("first post-trial cycle");
    assert_eq!(cycle.total, dec!(29.99));
    let activated = reload(&commerce, elapsed.id);
    assert_eq!(activated.status, SubscriptionStatus::Active);
    let events = subs.get_events(elapsed.id).expect("events");
    assert!(events.iter().any(|e| e.event_type == SubscriptionEventType::Activated), "{events:?}");
    subs.mark_cycle_paid(cycle.id).expect("paid");
    assert!(subs.release_billing_claim(elapsed.id, "w1").expect("release"));
    assert!(subs.get_due_for_billing(now).expect("due").is_empty());
}
