#![cfg(feature = "sqlite")]

//! Regression tests for subscription billing-cycle guards (SQLite backend,
//! sync `Commerce` engine).
//!
//! Headline defect: a paid billing cycle never advanced the subscription.
//! `billing_cycle_count` was inserted as 0 and incremented nowhere, and
//! `mark_cycle_paid` did not touch `next_billing_date` — so a worker that
//! polled `get_due_for_billing`, billed, marked the cycle paid and polled
//! again found the SAME subscription still due and charged the customer a
//! second time for the same period. Nothing stopped it: there was no
//! uniqueness on `(subscription_id, cycle_number)` either.
//!
//! Covers:
//! - a bill -> mark-paid -> poll-again worker loop bills each period exactly
//!   once and advances `next_billing_date` by exactly one interval per pass;
//! - marking an already-paid cycle paid is refused (not silently re-applied);
//! - the billing-cycle status allowlist (paid cycles cannot be failed,
//!   skipped or re-paid; refunded/voided are terminal);
//! - `retry_count` increments once per genuine failure and cannot be inflated
//!   on a settled cycle;
//! - a duplicate `cycle_number` is rejected by the database
//!   (`077_billing_cycle_uniqueness`), and voiding a cycle frees the slot;
//! - the advance is calendar-correct (a monthly subscription keeps its day of
//!   month instead of drifting backwards ~5 days a year) and never rewinds
//!   the schedule or resurrects billing on a cancelled subscription.

use chrono::{DateTime, Datelike, Duration, TimeZone, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_embedded::{
    BillingCycle, BillingCycleFilter, BillingCycleStatus, BillingInterval, CancelSubscription,
    Commerce, CreateCustomer, CreateSubscription, CreateSubscriptionPlan, CustomerId,
    PauseSubscription, Subscription, SubscriptionStatus,
};
use uuid::Uuid;

// ============================================================================
// Fixtures
// ============================================================================

fn commerce() -> Commerce {
    Commerce::new(":memory:").expect("Failed to create in-memory Commerce")
}

fn customer(commerce: &Commerce) -> CustomerId {
    commerce
        .customers()
        .create(CreateCustomer {
            email: format!("billing-guard-{}@example.com", Uuid::new_v4()),
            first_name: "Billing".into(),
            last_name: "Guard".into(),
            ..Default::default()
        })
        .expect("create customer")
        .id
}

/// An ACTIVE plan with no trial, so subscriptions start `Active` and billing.
fn active_plan(
    commerce: &Commerce,
    name: &str,
    interval: BillingInterval,
    custom_interval_days: Option<i32>,
    price: Decimal,
) -> Uuid {
    let plan = commerce
        .subscriptions()
        .create_plan(CreateSubscriptionPlan {
            name: name.into(),
            billing_interval: interval,
            custom_interval_days,
            price,
            trial_days: Some(0),
            ..Default::default()
        })
        .expect("create plan");
    commerce.subscriptions().activate_plan(plan.id).expect("activate plan");
    plan.id
}

/// A subscription that started in the past, so it is already due for billing.
fn subscribe_started_at(
    commerce: &Commerce,
    interval: BillingInterval,
    custom_interval_days: Option<i32>,
    start: DateTime<Utc>,
) -> Subscription {
    let customer_id = customer(commerce);
    let plan_id = active_plan(
        commerce,
        &format!("Plan {}", Uuid::new_v4()),
        interval,
        custom_interval_days,
        dec!(29.99),
    );
    commerce
        .subscriptions()
        .subscribe(CreateSubscription {
            customer_id,
            plan_id,
            skip_trial: Some(true),
            start_date: Some(start),
            ..Default::default()
        })
        .expect("subscribe")
}

/// A monthly subscription anchored on 2026-01-15, already overdue.
fn monthly_overdue(commerce: &Commerce) -> Subscription {
    subscribe_started_at(
        commerce,
        BillingInterval::Monthly,
        None,
        Utc.with_ymd_and_hms(2026, 1, 15, 12, 0, 0).single().expect("anchor date"),
    )
}

fn cycles_of(commerce: &Commerce, sub: &Subscription) -> Vec<BillingCycle> {
    let mut cycles = commerce
        .subscriptions()
        .list_billing_cycles(BillingCycleFilter {
            subscription_id: Some(sub.id),
            ..Default::default()
        })
        .expect("list billing cycles");
    cycles.sort_by_key(|c| c.cycle_number);
    cycles
}

fn reload(commerce: &Commerce, sub: &Subscription) -> Subscription {
    commerce.subscriptions().get(sub.id).expect("get subscription").expect("subscription exists")
}

/// One pass of a naive billing worker: poll for due subscriptions, bill the
/// next cycle, mark it paid. Returns one entry per successful charge.
///
/// This is deliberately the *naive* worker the defect report describes — it
/// re-derives the cycle to bill from `billing_cycle_count` on every pass and
/// has no memory of its own. If the engine does not advance the subscription,
/// this worker charges the same period twice.
fn run_billing_worker(commerce: &Commerce) -> Vec<(i32, DateTime<Utc>, DateTime<Utc>)> {
    let due = commerce.subscriptions().get_due_for_billing(Utc::now()).expect("poll due");
    let mut charged = Vec::new();

    for sub in due {
        let cycle_number = sub.billing_cycle_count + 1;
        let existing = cycles_of(commerce, &sub);
        let cycle = match existing.iter().find(|c| c.cycle_number == cycle_number) {
            Some(c) => c.clone(),
            None => commerce
                .subscriptions()
                .create_billing_cycle(
                    sub.id,
                    cycle_number,
                    sub.current_period_start,
                    sub.current_period_end,
                )
                .expect("create billing cycle"),
        };

        // The "charge the customer" step. It succeeds exactly once per cycle.
        commerce.subscriptions().mark_cycle_paid(cycle.id).expect("mark cycle paid");
        charged.push((cycle.cycle_number, cycle.period_start, cycle.period_end));
    }

    charged
}

// ============================================================================
// Headline: the double-billing loop
// ============================================================================

/// THE regression test. Run a full bill -> mark-paid -> poll-again worker pass
/// TWICE against a subscription that is due for billing, and assert the
/// customer is charged exactly once per period.
///
/// Before the fix, pass 2 saw `billing_cycle_count == 0` and an unchanged
/// `next_billing_date`, re-derived cycle number 1, found the cycle it had
/// already charged, and charged it again — two charges, one period.
#[test]
fn billing_worker_loop_charges_each_period_exactly_once() {
    let commerce = commerce();
    let sub = monthly_overdue(&commerce);
    assert_eq!(sub.billing_cycle_count, 0);
    let period_1_end = sub.current_period_end;

    let pass_1 = run_billing_worker(&commerce);
    let pass_2 = run_billing_worker(&commerce);

    assert_eq!(pass_1.len(), 1, "pass 1 must charge the one due subscription once");
    assert_eq!(pass_2.len(), 1, "pass 2 must charge the NEXT period, not the same one again");

    let mut charged: Vec<_> = pass_1.iter().chain(pass_2.iter()).collect();
    charged.sort_by_key(|(n, _, _)| *n);

    assert_eq!(
        charged.iter().map(|(n, _, _)| *n).collect::<Vec<_>>(),
        vec![1, 2],
        "each pass must bill a distinct cycle number"
    );
    assert_ne!(
        (charged[0].1, charged[0].2),
        (charged[1].1, charged[1].2),
        "the two charges must cover different billing periods (this is the double-bill)"
    );
    assert_eq!(
        charged[0].2, charged[1].1,
        "billing periods must be contiguous: period 2 starts where period 1 ended"
    );

    // And the engine state agrees: two settled cycles, both paid, count == 2.
    let after = reload(&commerce, &sub);
    assert_eq!(after.billing_cycle_count, 2, "each paid cycle must increment billing_cycle_count");

    let cycles = cycles_of(&commerce, &sub);
    assert_eq!(cycles.len(), 2, "exactly two billing cycles must exist");
    assert!(cycles.iter().all(|c| c.status == BillingCycleStatus::Paid));

    // The billing clock advanced by exactly one calendar month per pass, from
    // the cycle that was paid — not from "now".
    let expected_next = period_1_end + chrono::Months::new(2);
    assert_eq!(
        after.next_billing_date,
        Some(expected_next),
        "next_billing_date must advance by exactly one interval per settled cycle"
    );
}

/// A third pass keeps the invariant: one charge per period, no gaps, no
/// repeats, however far behind the worker is.
#[test]
fn billing_worker_loop_stays_contiguous_over_three_passes() {
    let commerce = commerce();
    let sub = monthly_overdue(&commerce);

    let mut charged = Vec::new();
    for _ in 0..3 {
        charged.extend(run_billing_worker(&commerce));
    }

    assert_eq!(charged.len(), 3);
    assert_eq!(charged.iter().map(|(n, _, _)| *n).collect::<Vec<_>>(), vec![1, 2, 3]);
    for window in charged.windows(2) {
        assert_eq!(window[0].2, window[1].1, "periods must chain end-to-start with no gap");
    }
    assert_eq!(reload(&commerce, &sub).billing_cycle_count, 3);
}

// ============================================================================
// Advance arithmetic
// ============================================================================

/// A monthly subscription anchored on the 15th stays on the 15th. The old
/// flat-30-day arithmetic walked the billing day backwards ~5 days a year.
#[test]
fn monthly_advance_uses_calendar_months_not_thirty_days() {
    let commerce = commerce();
    let sub = monthly_overdue(&commerce);

    // The initial period is still the legacy 30 days wide (set by
    // `create_subscription`), so period 1 ends 2026-02-14.
    let cycle_1 = cycles_of(&commerce, &sub).remove(0);
    assert_eq!(cycle_1.period_end, cycle_1.period_start + Duration::days(30));

    commerce.subscriptions().mark_cycle_paid(cycle_1.id).expect("mark paid");
    let after = reload(&commerce, &sub);

    let next = after.next_billing_date.expect("next billing date");
    assert_eq!(next, cycle_1.period_end + chrono::Months::new(1));
    assert_eq!(next.day(), cycle_1.period_end.day(), "the billing day of month must not drift");
    assert_ne!(
        next,
        cycle_1.period_end + Duration::days(30),
        "March is 31 days: a flat 30-day advance would land a day early"
    );
    assert_eq!(after.current_period_start, cycle_1.period_end);
    assert_eq!(after.current_period_end, next);
}

#[test]
fn weekly_advance_moves_exactly_seven_days() {
    let commerce = commerce();
    let sub = subscribe_started_at(
        &commerce,
        BillingInterval::Weekly,
        None,
        Utc::now() - Duration::days(30),
    );
    let cycle = cycles_of(&commerce, &sub).remove(0);

    commerce.subscriptions().mark_cycle_paid(cycle.id).expect("mark paid");

    assert_eq!(
        reload(&commerce, &sub).next_billing_date,
        Some(cycle.period_end + Duration::days(7))
    );
}

#[test]
fn custom_interval_advance_uses_custom_interval_days() {
    let commerce = commerce();
    let sub = subscribe_started_at(
        &commerce,
        BillingInterval::Custom,
        Some(45),
        Utc::now() - Duration::days(90),
    );
    let cycle = cycles_of(&commerce, &sub).remove(0);

    commerce.subscriptions().mark_cycle_paid(cycle.id).expect("mark paid");

    assert_eq!(
        reload(&commerce, &sub).next_billing_date,
        Some(cycle.period_end + Duration::days(45))
    );
}

/// The advance anchors on the PAID CYCLE's `period_end`, not on `Utc::now()`.
/// A worker that runs late must not push the schedule out by its own latency.
#[test]
fn advance_anchors_on_the_paid_cycle_not_on_now() {
    let commerce = commerce();
    // Started 8 months ago: the cycle being paid is long past.
    let sub = subscribe_started_at(
        &commerce,
        BillingInterval::Monthly,
        None,
        Utc.with_ymd_and_hms(2026, 1, 15, 12, 0, 0).single().expect("anchor date"),
    );
    let cycle = cycles_of(&commerce, &sub).remove(0);

    commerce.subscriptions().mark_cycle_paid(cycle.id).expect("mark paid");
    let next = reload(&commerce, &sub).next_billing_date.expect("next billing date");

    assert_eq!(next, cycle.period_end + chrono::Months::new(1));
    assert!(next < Utc::now(), "anchoring on `now` would have skipped every missed period");
}

// ============================================================================
// Status transition allowlist
// ============================================================================

#[test]
fn marking_an_already_paid_cycle_paid_is_refused() {
    let commerce = commerce();
    let sub = monthly_overdue(&commerce);
    let cycle = cycles_of(&commerce, &sub).remove(0);

    commerce.subscriptions().mark_cycle_paid(cycle.id).expect("first payment");
    let after_first = reload(&commerce, &sub);

    let err = commerce
        .subscriptions()
        .mark_cycle_paid(cycle.id)
        .expect_err("re-paying a paid cycle must be refused");
    assert!(
        matches!(err, stateset_embedded::CommerceError::ValidationError(_)),
        "expected a validation error, got {err:?}"
    );

    // And the refusal changed nothing: no second advance, no second count.
    let after_second = reload(&commerce, &sub);
    assert_eq!(after_second.billing_cycle_count, after_first.billing_cycle_count);
    assert_eq!(after_second.next_billing_date, after_first.next_billing_date);
}

#[test]
fn paid_cycle_cannot_be_marked_failed() {
    let commerce = commerce();
    let sub = monthly_overdue(&commerce);
    let cycle = cycles_of(&commerce, &sub).remove(0);
    commerce.subscriptions().mark_cycle_paid(cycle.id).expect("mark paid");

    let err = commerce
        .subscriptions()
        .mark_cycle_failed(cycle.id)
        .expect_err("failing a paid cycle must be refused");
    assert!(matches!(err, stateset_embedded::CommerceError::ValidationError(_)));

    let reloaded =
        commerce.subscriptions().get_billing_cycle(cycle.id).expect("get").expect("cycle");
    assert_eq!(reloaded.status, BillingCycleStatus::Paid, "status must remain paid");
    assert_eq!(reloaded.retry_count, 0, "a refused failure must not inflate retry_count");
}

#[test]
fn paid_cycle_may_be_refunded_and_a_refunded_cycle_is_terminal() {
    let commerce = commerce();
    let sub = monthly_overdue(&commerce);
    let cycle = cycles_of(&commerce, &sub).remove(0);
    commerce.subscriptions().mark_cycle_paid(cycle.id).expect("mark paid");

    let refunded = commerce
        .subscriptions()
        .update_billing_cycle_status(cycle.id, BillingCycleStatus::Refunded)
        .expect("paid -> refunded is allowed");
    assert_eq!(refunded.status, BillingCycleStatus::Refunded);

    for next in [
        BillingCycleStatus::Paid,
        BillingCycleStatus::Failed,
        BillingCycleStatus::Scheduled,
        BillingCycleStatus::Skipped,
        BillingCycleStatus::Refunded,
    ] {
        assert!(
            commerce.subscriptions().update_billing_cycle_status(cycle.id, next).is_err(),
            "refunded -> {next} must be refused"
        );
    }
}

#[test]
fn skipped_cycle_cannot_be_paid() {
    let commerce = commerce();
    let sub = monthly_overdue(&commerce);
    let cycle = cycles_of(&commerce, &sub).remove(0);

    commerce
        .subscriptions()
        .update_billing_cycle_status(cycle.id, BillingCycleStatus::Skipped)
        .expect("scheduled -> skipped is allowed");

    let err = commerce
        .subscriptions()
        .mark_cycle_paid(cycle.id)
        .expect_err("paying a skipped cycle must be refused");
    assert!(matches!(err, stateset_embedded::CommerceError::ValidationError(_)));
    assert_eq!(reload(&commerce, &sub).billing_cycle_count, 0);
}

#[test]
fn scheduled_cycle_cannot_be_re_scheduled() {
    let commerce = commerce();
    let sub = monthly_overdue(&commerce);
    let cycle = cycles_of(&commerce, &sub).remove(0);

    assert!(
        commerce
            .subscriptions()
            .update_billing_cycle_status(cycle.id, BillingCycleStatus::Scheduled)
            .is_err(),
        "a no-op self transition must be refused rather than silently rewriting the row"
    );
}

// ============================================================================
// Retry counting
// ============================================================================

/// `retry_count` moves one step per genuine collection failure — and a failed
/// cycle is still retryable, so `Failed -> Paid` works and settles normally.
#[test]
fn failed_cycle_retry_count_increments_once_per_genuine_failure() {
    let commerce = commerce();
    let sub = monthly_overdue(&commerce);
    let cycle = cycles_of(&commerce, &sub).remove(0);
    assert_eq!(cycle.retry_count, 0);

    let first = commerce.subscriptions().mark_cycle_failed(cycle.id).expect("first failure");
    assert_eq!(first.retry_count, 1);
    assert!(first.billed_at.is_some());

    let second = commerce.subscriptions().mark_cycle_failed(cycle.id).expect("second failure");
    assert_eq!(second.retry_count, 2, "each genuine attempt increments once");

    // The subscription is untouched while collection keeps failing.
    let mid = reload(&commerce, &sub);
    assert_eq!(mid.billing_cycle_count, 0);
    assert_eq!(mid.next_billing_date, sub.next_billing_date);

    // A retry that finally succeeds settles the cycle and advances once.
    let paid = commerce.subscriptions().mark_cycle_paid(cycle.id).expect("retry succeeds");
    assert_eq!(paid.status, BillingCycleStatus::Paid);
    assert_eq!(paid.retry_count, 2, "settling must not touch retry_count");

    let after = reload(&commerce, &sub);
    assert_eq!(after.billing_cycle_count, 1);
    assert_eq!(after.next_billing_date, Some(cycle.period_end + chrono::Months::new(1)));
}

/// A terminal cycle's `retry_count` cannot be inflated by a retry loop.
#[test]
fn retry_count_cannot_be_inflated_on_a_terminal_cycle() {
    let commerce = commerce();
    let sub = monthly_overdue(&commerce);
    let cycle = cycles_of(&commerce, &sub).remove(0);

    commerce
        .subscriptions()
        .update_billing_cycle_status(cycle.id, BillingCycleStatus::Voided)
        .expect("scheduled -> voided is allowed");

    for _ in 0..5 {
        assert!(commerce.subscriptions().mark_cycle_failed(cycle.id).is_err());
    }

    let reloaded =
        commerce.subscriptions().get_billing_cycle(cycle.id).expect("get").expect("cycle");
    assert_eq!(reloaded.retry_count, 0);
    assert_eq!(reloaded.status, BillingCycleStatus::Voided);
}

// ============================================================================
// Cycle uniqueness (migration 077 / 084)
// ============================================================================

/// The database refuses a second cycle for a period that already has one.
/// This is the backstop for writers that bypass the application layer.
#[test]
fn duplicate_cycle_number_is_rejected() {
    let commerce = commerce();
    let sub = monthly_overdue(&commerce);
    let cycle = cycles_of(&commerce, &sub).remove(0);
    assert_eq!(cycle.cycle_number, 1);

    let err = commerce
        .subscriptions()
        .create_billing_cycle(sub.id, 1, cycle.period_start, cycle.period_end)
        .expect_err("a duplicate cycle_number must be rejected");
    assert!(
        matches!(err, stateset_embedded::CommerceError::Conflict(_)),
        "expected a conflict, got {err:?}"
    );

    assert_eq!(cycles_of(&commerce, &sub).len(), 1, "no duplicate row may be created");
}

/// Different periods on the same subscription are of course fine, and the same
/// cycle number on a DIFFERENT subscription is fine too (the key is scoped).
#[test]
fn cycle_numbers_are_unique_per_subscription_only() {
    let commerce = commerce();
    let sub_a = monthly_overdue(&commerce);
    let sub_b = monthly_overdue(&commerce);

    let start = sub_a.current_period_end;
    commerce
        .subscriptions()
        .create_billing_cycle(sub_a.id, 2, start, start + Duration::days(30))
        .expect("a second period on the same subscription is fine");

    // `sub_b` already owns cycle 1 of its own; that must not clash with
    // `sub_a`'s cycle 1.
    assert_eq!(cycles_of(&commerce, &sub_b).len(), 1);
    assert_eq!(cycles_of(&commerce, &sub_a).len(), 2);
}

/// Voiding a cycle frees its `(subscription, cycle_number)` slot so a
/// corrected cycle can be created for the same period.
#[test]
fn voiding_a_cycle_frees_its_cycle_number() {
    let commerce = commerce();
    let sub = monthly_overdue(&commerce);
    let cycle = cycles_of(&commerce, &sub).remove(0);

    assert!(
        commerce
            .subscriptions()
            .create_billing_cycle(sub.id, 1, cycle.period_start, cycle.period_end)
            .is_err()
    );

    commerce
        .subscriptions()
        .update_billing_cycle_status(cycle.id, BillingCycleStatus::Voided)
        .expect("void");

    commerce
        .subscriptions()
        .create_billing_cycle(sub.id, 1, cycle.period_start, cycle.period_end)
        .expect("voiding frees the slot for a corrected cycle");

    assert_eq!(cycles_of(&commerce, &sub).len(), 2);
}

// ============================================================================
// The advance never rewinds or resurrects
// ============================================================================

/// A late payment for an OLD cycle must not rewind the billing schedule.
#[test]
fn paying_an_older_cycle_does_not_rewind_the_schedule() {
    let commerce = commerce();
    let sub = monthly_overdue(&commerce);
    let cycle_1 = cycles_of(&commerce, &sub).remove(0);

    // Settle cycle 1 normally, then queue and settle cycle 2.
    commerce.subscriptions().mark_cycle_paid(cycle_1.id).expect("pay cycle 1");
    let after_1 = reload(&commerce, &sub);
    let cycle_2 = commerce
        .subscriptions()
        .create_billing_cycle(sub.id, 2, after_1.current_period_start, after_1.current_period_end)
        .expect("create cycle 2");
    commerce.subscriptions().mark_cycle_paid(cycle_2.id).expect("pay cycle 2");
    let after_2 = reload(&commerce, &sub);

    // Now a straggler cycle for a period BEFORE cycle 1 settles late.
    let straggler = commerce
        .subscriptions()
        .create_billing_cycle(
            sub.id,
            0,
            cycle_1.period_start - Duration::days(30),
            cycle_1.period_start,
        )
        .expect("create back-dated cycle");
    commerce.subscriptions().mark_cycle_paid(straggler.id).expect("pay straggler");

    let after_3 = reload(&commerce, &sub);
    assert_eq!(
        after_3.next_billing_date, after_2.next_billing_date,
        "a late payment for an older period must never rewind next_billing_date"
    );
    assert_eq!(after_3.billing_cycle_count, 3, "but the settled cycle is still counted");
}

/// A cycle that settles after cancellation must not put the subscription back
/// into the billing queue.
#[test]
fn paying_a_cycle_on_a_cancelled_subscription_does_not_resurrect_billing() {
    let commerce = commerce();
    let sub = monthly_overdue(&commerce);
    let cycle = cycles_of(&commerce, &sub).remove(0);

    commerce
        .subscriptions()
        .cancel(sub.id, CancelSubscription { immediate: Some(true), ..Default::default() })
        .expect("cancel");

    commerce.subscriptions().mark_cycle_paid(cycle.id).expect("an in-flight charge still settles");

    let after = reload(&commerce, &sub);
    assert_eq!(after.status, SubscriptionStatus::Expired);
    assert_eq!(after.next_billing_date, None, "a cancelled subscription must not be rescheduled");
    assert_eq!(after.billing_cycle_count, 1, "the settled cycle is still counted");
    assert!(
        commerce
            .subscriptions()
            .get_due_for_billing(Utc::now() + Duration::days(365))
            .expect("poll due")
            .iter()
            .all(|s| s.id != sub.id)
    );
}

/// Same for a paused subscription: `pause` clears `next_billing_date`, and a
/// cycle settling afterwards must not set it again behind the customer's back.
#[test]
fn paying_a_cycle_on_a_paused_subscription_does_not_resurrect_billing() {
    let commerce = commerce();
    let sub = monthly_overdue(&commerce);
    let cycle = cycles_of(&commerce, &sub).remove(0);

    commerce.subscriptions().pause(sub.id, PauseSubscription::default()).expect("pause");
    commerce.subscriptions().mark_cycle_paid(cycle.id).expect("in-flight charge settles");

    let after = reload(&commerce, &sub);
    assert_eq!(after.status, SubscriptionStatus::Paused);
    assert_eq!(after.next_billing_date, None);
    assert_eq!(after.billing_cycle_count, 1);
}

// ============================================================================
// Audit trail
// ============================================================================

/// The advance is auditable: settling a cycle records a `renewed` event.
#[test]
fn settling_a_cycle_records_a_renewed_event() {
    let commerce = commerce();
    let sub = monthly_overdue(&commerce);
    let cycle = cycles_of(&commerce, &sub).remove(0);

    commerce.subscriptions().mark_cycle_paid(cycle.id).expect("mark paid");

    let events = commerce.subscriptions().get_events(sub.id).expect("get events");
    let renewals = events
        .iter()
        .filter(|e| e.event_type == stateset_embedded::SubscriptionEventType::Renewed)
        .count();
    assert_eq!(renewals, 1, "exactly one renewal event per settled cycle");
}
