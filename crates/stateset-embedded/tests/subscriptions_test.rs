//! Integration tests for subscription management features

use rust_decimal_macros::dec;
use stateset_embedded::{
    Commerce, CreateSubscriptionPlan, CreateSubscription, BillingInterval,
    SubscriptionPlanFilter, SubscriptionFilter, PlanStatus, SubscriptionStatus,
    PauseSubscription, CancelSubscription, SkipBillingCycle, UpdateSubscriptionPlan,
    CreateCustomer,
};
use uuid::Uuid;

// ============================================================================
// Subscription Plan Tests
// ============================================================================

#[test]
fn test_create_subscription_plan() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let plan = commerce.subscriptions().create_plan(CreateSubscriptionPlan {
        name: "Monthly Coffee Box".into(),
        billing_interval: BillingInterval::Monthly,
        price: dec!(29.99),
        currency: Some("USD".into()),
        trial_days: Some(14),
        description: Some("Fresh roasted coffee delivered monthly".into()),
        ..Default::default()
    }).expect("Failed to create plan");

    assert!(!plan.id.is_nil());
    assert!(!plan.code.is_empty());
    assert_eq!(plan.name, "Monthly Coffee Box");
    assert_eq!(plan.billing_interval, BillingInterval::Monthly);
    assert_eq!(plan.price, dec!(29.99));
    assert_eq!(plan.currency, "USD");
    assert_eq!(plan.trial_days, 14);
    assert_eq!(plan.status, PlanStatus::Draft);
}

#[test]
fn test_create_plan_with_different_intervals() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let intervals = vec![
        (BillingInterval::Weekly, "Weekly Plan"),
        (BillingInterval::Biweekly, "Biweekly Plan"),
        (BillingInterval::Monthly, "Monthly Plan"),
        (BillingInterval::Quarterly, "Quarterly Plan"),
        (BillingInterval::Annual, "Annual Plan"),
    ];

    for (interval, name) in intervals {
        let plan = commerce.subscriptions().create_plan(CreateSubscriptionPlan {
            name: name.into(),
            billing_interval: interval,
            price: dec!(49.99),
            ..Default::default()
        }).expect(&format!("Failed to create {} plan", name));

        assert_eq!(plan.billing_interval, interval);
        assert_eq!(plan.name, name);
    }
}

#[test]
fn test_get_plan_by_id() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let created = commerce.subscriptions().create_plan(CreateSubscriptionPlan {
        name: "Test Plan".into(),
        billing_interval: BillingInterval::Monthly,
        price: dec!(19.99),
        ..Default::default()
    }).expect("Failed to create plan");

    let retrieved = commerce.subscriptions().get_plan(created.id)
        .expect("Failed to get plan")
        .expect("Plan not found");

    assert_eq!(retrieved.id, created.id);
    assert_eq!(retrieved.name, "Test Plan");
}

#[test]
fn test_get_plan_by_code() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let created = commerce.subscriptions().create_plan(CreateSubscriptionPlan {
        code: Some("PREMIUM-MONTHLY".into()),
        name: "Premium Monthly".into(),
        billing_interval: BillingInterval::Monthly,
        price: dec!(99.99),
        ..Default::default()
    }).expect("Failed to create plan");

    let retrieved = commerce.subscriptions().get_plan_by_code("PREMIUM-MONTHLY")
        .expect("Failed to get plan by code")
        .expect("Plan not found");

    assert_eq!(retrieved.id, created.id);
    assert_eq!(retrieved.code, "PREMIUM-MONTHLY");
}

#[test]
fn test_list_plans() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Create several plans
    for i in 1..=5 {
        commerce.subscriptions().create_plan(CreateSubscriptionPlan {
            name: format!("Plan {}", i),
            billing_interval: BillingInterval::Monthly,
            price: dec!(9.99) * rust_decimal::Decimal::from(i),
            ..Default::default()
        }).expect("Failed to create plan");
    }

    let plans = commerce.subscriptions().list_plans(SubscriptionPlanFilter::default())
        .expect("Failed to list plans");

    // Should include created plans plus any seeded plans
    assert!(plans.len() >= 5);
}

#[test]
fn test_list_plans_by_status() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Create draft plan
    let draft = commerce.subscriptions().create_plan(CreateSubscriptionPlan {
        name: "Draft Plan".into(),
        billing_interval: BillingInterval::Monthly,
        price: dec!(29.99),
        ..Default::default()
    }).expect("Failed to create draft plan");

    // Activate it
    commerce.subscriptions().activate_plan(draft.id)
        .expect("Failed to activate plan");

    // Create another draft
    commerce.subscriptions().create_plan(CreateSubscriptionPlan {
        name: "Another Draft".into(),
        billing_interval: BillingInterval::Monthly,
        price: dec!(19.99),
        ..Default::default()
    }).expect("Failed to create another draft");

    // List active plans
    let active_plans = commerce.subscriptions().list_plans(SubscriptionPlanFilter {
        status: Some(PlanStatus::Active),
        ..Default::default()
    }).expect("Failed to list active plans");

    // Should have at least our activated plan
    assert!(active_plans.iter().any(|p| p.id == draft.id));

    // List draft plans
    let draft_plans = commerce.subscriptions().list_plans(SubscriptionPlanFilter {
        status: Some(PlanStatus::Draft),
        ..Default::default()
    }).expect("Failed to list draft plans");

    assert!(!draft_plans.is_empty());
}

#[test]
fn test_activate_plan() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let plan = commerce.subscriptions().create_plan(CreateSubscriptionPlan {
        name: "Plan to Activate".into(),
        billing_interval: BillingInterval::Monthly,
        price: dec!(29.99),
        ..Default::default()
    }).expect("Failed to create plan");

    assert_eq!(plan.status, PlanStatus::Draft);

    let activated = commerce.subscriptions().activate_plan(plan.id)
        .expect("Failed to activate plan");

    assert_eq!(activated.status, PlanStatus::Active);
}

#[test]
fn test_archive_plan() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let plan = commerce.subscriptions().create_plan(CreateSubscriptionPlan {
        name: "Plan to Archive".into(),
        billing_interval: BillingInterval::Monthly,
        price: dec!(29.99),
        ..Default::default()
    }).expect("Failed to create plan");

    // Activate first
    commerce.subscriptions().activate_plan(plan.id)
        .expect("Failed to activate plan");

    // Then archive
    let archived = commerce.subscriptions().archive_plan(plan.id)
        .expect("Failed to archive plan");

    assert_eq!(archived.status, PlanStatus::Archived);
}

#[test]
fn test_update_plan() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let plan = commerce.subscriptions().create_plan(CreateSubscriptionPlan {
        name: "Original Name".into(),
        billing_interval: BillingInterval::Monthly,
        price: dec!(29.99),
        ..Default::default()
    }).expect("Failed to create plan");

    let updated = commerce.subscriptions().update_plan(plan.id, UpdateSubscriptionPlan {
        name: Some("Updated Name".into()),
        price: Some(dec!(39.99)),
        description: Some("New description".into()),
        ..Default::default()
    }).expect("Failed to update plan");

    assert_eq!(updated.name, "Updated Name");
    assert_eq!(updated.price, dec!(39.99));
    assert_eq!(updated.description, Some("New description".into()));
}

#[test]
fn test_plan_with_setup_fee() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let plan = commerce.subscriptions().create_plan(CreateSubscriptionPlan {
        name: "Plan with Setup Fee".into(),
        billing_interval: BillingInterval::Monthly,
        price: dec!(49.99),
        setup_fee: Some(dec!(9.99)),
        ..Default::default()
    }).expect("Failed to create plan");

    assert_eq!(plan.setup_fee, Some(dec!(9.99)));
}

// ============================================================================
// Subscription Tests
// ============================================================================

fn create_test_customer(commerce: &Commerce) -> Uuid {
    let customer = commerce.customers().create(CreateCustomer {
        email: format!("test-{}@example.com", Uuid::new_v4()),
        first_name: "Test".into(),
        last_name: "Customer".into(),
        ..Default::default()
    }).expect("Failed to create customer");
    customer.id
}

fn create_active_plan(commerce: &Commerce, name: &str, price: rust_decimal::Decimal) -> Uuid {
    let plan = commerce.subscriptions().create_plan(CreateSubscriptionPlan {
        name: name.into(),
        billing_interval: BillingInterval::Monthly,
        price,
        trial_days: Some(14),
        ..Default::default()
    }).expect("Failed to create plan");

    commerce.subscriptions().activate_plan(plan.id)
        .expect("Failed to activate plan");

    plan.id
}

#[test]
fn test_create_subscription() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let customer_id = create_test_customer(&commerce);
    let plan_id = create_active_plan(&commerce, "Monthly Plan", dec!(29.99));

    let subscription = commerce.subscriptions().subscribe(CreateSubscription {
        customer_id,
        plan_id,
        ..Default::default()
    }).expect("Failed to create subscription");

    assert!(!subscription.id.is_nil());
    assert!(!subscription.subscription_number.is_empty());
    assert!(subscription.subscription_number.starts_with("SUB-"));
    assert_eq!(subscription.customer_id, customer_id);
    assert_eq!(subscription.plan_id, plan_id);
    assert_eq!(subscription.status, SubscriptionStatus::Trial); // Has trial
    assert!(subscription.trial_ends_at.is_some());
}

#[test]
fn test_create_subscription_skip_trial() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let customer_id = create_test_customer(&commerce);
    let plan_id = create_active_plan(&commerce, "Monthly Plan", dec!(29.99));

    let subscription = commerce.subscriptions().subscribe(CreateSubscription {
        customer_id,
        plan_id,
        skip_trial: Some(true),
        ..Default::default()
    }).expect("Failed to create subscription");

    assert_eq!(subscription.status, SubscriptionStatus::Active);
    assert!(subscription.trial_ends_at.is_none());
}

#[test]
fn test_get_subscription_by_id() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let customer_id = create_test_customer(&commerce);
    let plan_id = create_active_plan(&commerce, "Test Plan", dec!(19.99));

    let created = commerce.subscriptions().subscribe(CreateSubscription {
        customer_id,
        plan_id,
        skip_trial: Some(true),
        ..Default::default()
    }).expect("Failed to create subscription");

    let retrieved = commerce.subscriptions().get(created.id)
        .expect("Failed to get subscription")
        .expect("Subscription not found");

    assert_eq!(retrieved.id, created.id);
    assert_eq!(retrieved.customer_id, customer_id);
}

#[test]
fn test_get_subscription_by_number() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let customer_id = create_test_customer(&commerce);
    let plan_id = create_active_plan(&commerce, "Test Plan", dec!(19.99));

    let created = commerce.subscriptions().subscribe(CreateSubscription {
        customer_id,
        plan_id,
        skip_trial: Some(true),
        ..Default::default()
    }).expect("Failed to create subscription");

    let retrieved = commerce.subscriptions().get_by_number(&created.subscription_number)
        .expect("Failed to get subscription by number")
        .expect("Subscription not found");

    assert_eq!(retrieved.id, created.id);
}

#[test]
fn test_list_subscriptions() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let customer_id = create_test_customer(&commerce);
    let plan_id = create_active_plan(&commerce, "Test Plan", dec!(19.99));

    // Create multiple subscriptions
    for _ in 0..3 {
        commerce.subscriptions().subscribe(CreateSubscription {
            customer_id,
            plan_id,
            skip_trial: Some(true),
            ..Default::default()
        }).expect("Failed to create subscription");
    }

    let subscriptions = commerce.subscriptions().list(SubscriptionFilter::default())
        .expect("Failed to list subscriptions");

    assert!(subscriptions.len() >= 3);
}

#[test]
fn test_list_subscriptions_by_customer() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let customer1 = create_test_customer(&commerce);
    let customer2 = create_test_customer(&commerce);
    let plan_id = create_active_plan(&commerce, "Test Plan", dec!(19.99));

    // Create subscriptions for customer1
    for _ in 0..2 {
        commerce.subscriptions().subscribe(CreateSubscription {
            customer_id: customer1,
            plan_id,
            skip_trial: Some(true),
            ..Default::default()
        }).expect("Failed to create subscription");
    }

    // Create subscription for customer2
    commerce.subscriptions().subscribe(CreateSubscription {
        customer_id: customer2,
        plan_id,
        skip_trial: Some(true),
        ..Default::default()
    }).expect("Failed to create subscription");

    // Filter by customer1
    let customer1_subs = commerce.subscriptions().list(SubscriptionFilter {
        customer_id: Some(customer1),
        ..Default::default()
    }).expect("Failed to list subscriptions");

    assert_eq!(customer1_subs.len(), 2);
    assert!(customer1_subs.iter().all(|s| s.customer_id == customer1));
}

#[test]
fn test_list_subscriptions_by_status() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let customer_id = create_test_customer(&commerce);
    let plan_id = create_active_plan(&commerce, "Test Plan", dec!(19.99));

    // Create active subscription
    commerce.subscriptions().subscribe(CreateSubscription {
        customer_id,
        plan_id,
        skip_trial: Some(true),
        ..Default::default()
    }).expect("Failed to create subscription");

    // Create trial subscription
    commerce.subscriptions().subscribe(CreateSubscription {
        customer_id,
        plan_id,
        ..Default::default()
    }).expect("Failed to create trial subscription");

    // Filter by active status
    let active_subs = commerce.subscriptions().list(SubscriptionFilter {
        status: Some(SubscriptionStatus::Active),
        ..Default::default()
    }).expect("Failed to list active subscriptions");

    assert!(!active_subs.is_empty());
    assert!(active_subs.iter().all(|s| s.status == SubscriptionStatus::Active));

    // Filter by trial status
    let trial_subs = commerce.subscriptions().list(SubscriptionFilter {
        status: Some(SubscriptionStatus::Trial),
        ..Default::default()
    }).expect("Failed to list trial subscriptions");

    assert!(!trial_subs.is_empty());
    assert!(trial_subs.iter().all(|s| s.status == SubscriptionStatus::Trial));
}

// ============================================================================
// Subscription Lifecycle Tests
// ============================================================================

#[test]
fn test_pause_subscription() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let customer_id = create_test_customer(&commerce);
    let plan_id = create_active_plan(&commerce, "Test Plan", dec!(19.99));

    let subscription = commerce.subscriptions().subscribe(CreateSubscription {
        customer_id,
        plan_id,
        skip_trial: Some(true),
        ..Default::default()
    }).expect("Failed to create subscription");

    assert_eq!(subscription.status, SubscriptionStatus::Active);

    let paused = commerce.subscriptions().pause(subscription.id, PauseSubscription {
        reason: Some("Customer requested".into()),
        ..Default::default()
    }).expect("Failed to pause subscription");

    assert_eq!(paused.status, SubscriptionStatus::Paused);
    assert!(paused.paused_at.is_some());
}

#[test]
fn test_resume_subscription() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let customer_id = create_test_customer(&commerce);
    let plan_id = create_active_plan(&commerce, "Test Plan", dec!(19.99));

    let subscription = commerce.subscriptions().subscribe(CreateSubscription {
        customer_id,
        plan_id,
        skip_trial: Some(true),
        ..Default::default()
    }).expect("Failed to create subscription");

    // Pause first
    commerce.subscriptions().pause(subscription.id, PauseSubscription::default())
        .expect("Failed to pause subscription");

    // Then resume
    let resumed = commerce.subscriptions().resume(subscription.id)
        .expect("Failed to resume subscription");

    assert_eq!(resumed.status, SubscriptionStatus::Active);
    assert!(resumed.paused_at.is_none());
}

#[test]
fn test_cancel_subscription_at_period_end() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let customer_id = create_test_customer(&commerce);
    let plan_id = create_active_plan(&commerce, "Test Plan", dec!(19.99));

    let subscription = commerce.subscriptions().subscribe(CreateSubscription {
        customer_id,
        plan_id,
        skip_trial: Some(true),
        ..Default::default()
    }).expect("Failed to create subscription");

    let cancelled = commerce.subscriptions().cancel(subscription.id, CancelSubscription {
        reason: Some("No longer needed".into()),
        immediate: Some(false),
        ..Default::default()
    }).expect("Failed to cancel subscription");

    assert_eq!(cancelled.status, SubscriptionStatus::Cancelled);
    assert!(cancelled.cancelled_at.is_some());
    assert!(cancelled.ends_at.is_some());
    // ends_at should be at period end, not now
    assert!(cancelled.ends_at.unwrap() > cancelled.cancelled_at.unwrap());
}

#[test]
fn test_cancel_subscription_immediately() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let customer_id = create_test_customer(&commerce);
    let plan_id = create_active_plan(&commerce, "Test Plan", dec!(19.99));

    let subscription = commerce.subscriptions().subscribe(CreateSubscription {
        customer_id,
        plan_id,
        skip_trial: Some(true),
        ..Default::default()
    }).expect("Failed to create subscription");

    let cancelled = commerce.subscriptions().cancel(subscription.id, CancelSubscription {
        immediate: Some(true),
        reason: Some("Refund requested".into()),
        ..Default::default()
    }).expect("Failed to cancel subscription");

    assert_eq!(cancelled.status, SubscriptionStatus::Expired);
    assert!(cancelled.cancelled_at.is_some());
}

#[test]
fn test_skip_billing_cycle() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let customer_id = create_test_customer(&commerce);
    let plan_id = create_active_plan(&commerce, "Test Plan", dec!(19.99));

    let subscription = commerce.subscriptions().subscribe(CreateSubscription {
        customer_id,
        plan_id,
        skip_trial: Some(true),
        ..Default::default()
    }).expect("Failed to create subscription");

    let original_next_billing = subscription.next_billing_date;

    let skipped = commerce.subscriptions().skip_next_cycle(subscription.id, SkipBillingCycle {
        reason: Some("Customer traveling".into()),
    }).expect("Failed to skip billing cycle");

    assert_eq!(skipped.status, SubscriptionStatus::Active);
    // Next billing date should be pushed forward
    assert!(skipped.next_billing_date.unwrap() > original_next_billing.unwrap());
}

#[test]
fn test_cannot_pause_cancelled_subscription() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let customer_id = create_test_customer(&commerce);
    let plan_id = create_active_plan(&commerce, "Test Plan", dec!(19.99));

    let subscription = commerce.subscriptions().subscribe(CreateSubscription {
        customer_id,
        plan_id,
        skip_trial: Some(true),
        ..Default::default()
    }).expect("Failed to create subscription");

    // Cancel first
    commerce.subscriptions().cancel(subscription.id, CancelSubscription {
        immediate: Some(true),
        ..Default::default()
    }).expect("Failed to cancel subscription");

    // Try to pause - should fail
    let result = commerce.subscriptions().pause(subscription.id, PauseSubscription::default());
    assert!(result.is_err());
}

#[test]
fn test_cannot_resume_active_subscription() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let customer_id = create_test_customer(&commerce);
    let plan_id = create_active_plan(&commerce, "Test Plan", dec!(19.99));

    let subscription = commerce.subscriptions().subscribe(CreateSubscription {
        customer_id,
        plan_id,
        skip_trial: Some(true),
        ..Default::default()
    }).expect("Failed to create subscription");

    // Try to resume active subscription - should fail
    let result = commerce.subscriptions().resume(subscription.id);
    assert!(result.is_err());
}

// ============================================================================
// Subscription Events Tests
// ============================================================================

#[test]
fn test_subscription_events_created_on_subscribe() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let customer_id = create_test_customer(&commerce);
    let plan_id = create_active_plan(&commerce, "Test Plan", dec!(19.99));

    let subscription = commerce.subscriptions().subscribe(CreateSubscription {
        customer_id,
        plan_id,
        skip_trial: Some(true),
        ..Default::default()
    }).expect("Failed to create subscription");

    let events = commerce.subscriptions().get_events(subscription.id, None)
        .expect("Failed to get events");

    // Should have at least created and activated events
    assert!(events.len() >= 2);
    assert!(events.iter().any(|e| e.description.contains("created")));
    assert!(events.iter().any(|e| e.description.contains("activated")));
}

#[test]
fn test_subscription_events_on_pause_resume() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let customer_id = create_test_customer(&commerce);
    let plan_id = create_active_plan(&commerce, "Test Plan", dec!(19.99));

    let subscription = commerce.subscriptions().subscribe(CreateSubscription {
        customer_id,
        plan_id,
        skip_trial: Some(true),
        ..Default::default()
    }).expect("Failed to create subscription");

    // Pause
    commerce.subscriptions().pause(subscription.id, PauseSubscription {
        reason: Some("Going on vacation".into()),
        ..Default::default()
    }).expect("Failed to pause");

    // Resume
    commerce.subscriptions().resume(subscription.id)
        .expect("Failed to resume");

    let events = commerce.subscriptions().get_events(subscription.id, None)
        .expect("Failed to get events");

    // Should have paused and resumed events
    assert!(events.iter().any(|e| e.description.contains("aused"))); // Paused
    assert!(events.iter().any(|e| e.description.contains("esumed"))); // Resumed
}

#[test]
fn test_subscription_events_limit() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let customer_id = create_test_customer(&commerce);
    let plan_id = create_active_plan(&commerce, "Test Plan", dec!(19.99));

    let subscription = commerce.subscriptions().subscribe(CreateSubscription {
        customer_id,
        plan_id,
        skip_trial: Some(true),
        ..Default::default()
    }).expect("Failed to create subscription");

    // Do some operations to create events
    commerce.subscriptions().pause(subscription.id, PauseSubscription::default())
        .expect("Failed to pause");
    commerce.subscriptions().resume(subscription.id)
        .expect("Failed to resume");

    // Get limited events
    let events = commerce.subscriptions().get_events(subscription.id, Some(2))
        .expect("Failed to get events");

    assert!(events.len() <= 2);
}

// ============================================================================
// Billing Cycle Tests
// ============================================================================

#[test]
fn test_create_billing_cycle() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let customer_id = create_test_customer(&commerce);
    let plan_id = create_active_plan(&commerce, "Test Plan", dec!(19.99));

    let subscription = commerce.subscriptions().subscribe(CreateSubscription {
        customer_id,
        plan_id,
        skip_trial: Some(true),
        ..Default::default()
    }).expect("Failed to create subscription");

    let now = chrono::Utc::now();
    let period_end = now + chrono::Duration::days(30);

    let cycle = commerce.subscriptions().create_billing_cycle(
        subscription.id,
        1,
        now,
        period_end,
    ).expect("Failed to create billing cycle");

    assert!(!cycle.id.is_nil());
    assert_eq!(cycle.subscription_id, subscription.id);
    assert_eq!(cycle.cycle_number, 1);
}

#[test]
fn test_list_billing_cycles() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let customer_id = create_test_customer(&commerce);
    let plan_id = create_active_plan(&commerce, "Test Plan", dec!(19.99));

    let subscription = commerce.subscriptions().subscribe(CreateSubscription {
        customer_id,
        plan_id,
        skip_trial: Some(true),
        ..Default::default()
    }).expect("Failed to create subscription");

    // Create a few billing cycles
    let now = chrono::Utc::now();
    for i in 1i32..=3 {
        let start = now + chrono::Duration::days((i - 1) as i64 * 30);
        let end = start + chrono::Duration::days(30);
        commerce.subscriptions().create_billing_cycle(subscription.id, i, start, end)
            .expect("Failed to create billing cycle");
    }

    let cycles = commerce.subscriptions().list_billing_cycles(
        stateset_embedded::BillingCycleFilter {
            subscription_id: Some(subscription.id),
            ..Default::default()
        }
    ).expect("Failed to list billing cycles");

    assert_eq!(cycles.len(), 3);
}

#[test]
fn test_mark_billing_cycle_paid() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let customer_id = create_test_customer(&commerce);
    let plan_id = create_active_plan(&commerce, "Test Plan", dec!(19.99));

    let subscription = commerce.subscriptions().subscribe(CreateSubscription {
        customer_id,
        plan_id,
        skip_trial: Some(true),
        ..Default::default()
    }).expect("Failed to create subscription");

    let now = chrono::Utc::now();
    let cycle = commerce.subscriptions().create_billing_cycle(
        subscription.id,
        1,
        now,
        now + chrono::Duration::days(30),
    ).expect("Failed to create billing cycle");

    let paid = commerce.subscriptions().mark_cycle_paid(cycle.id, "pay_123456".into())
        .expect("Failed to mark cycle paid");

    assert_eq!(paid.status, stateset_embedded::BillingCycleStatus::Paid);
    assert_eq!(paid.payment_id, Some("pay_123456".into()));
    assert!(paid.billed_at.is_some());
}

#[test]
fn test_mark_billing_cycle_failed() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let customer_id = create_test_customer(&commerce);
    let plan_id = create_active_plan(&commerce, "Test Plan", dec!(19.99));

    let subscription = commerce.subscriptions().subscribe(CreateSubscription {
        customer_id,
        plan_id,
        skip_trial: Some(true),
        ..Default::default()
    }).expect("Failed to create subscription");

    let now = chrono::Utc::now();
    let cycle = commerce.subscriptions().create_billing_cycle(
        subscription.id,
        1,
        now,
        now + chrono::Duration::days(30),
    ).expect("Failed to create billing cycle");

    let failed = commerce.subscriptions().mark_cycle_failed(cycle.id, "Card declined")
        .expect("Failed to mark cycle failed");

    assert_eq!(failed.status, stateset_embedded::BillingCycleStatus::Failed);
    assert_eq!(failed.failure_reason, Some("Card declined".into()));
}

// ============================================================================
// Convenience Method Tests
// ============================================================================

#[test]
fn test_get_active_plans() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Create and activate a plan
    let plan = commerce.subscriptions().create_plan(CreateSubscriptionPlan {
        name: "Active Plan".into(),
        billing_interval: BillingInterval::Monthly,
        price: dec!(29.99),
        ..Default::default()
    }).expect("Failed to create plan");

    commerce.subscriptions().activate_plan(plan.id)
        .expect("Failed to activate plan");

    let active_plans = commerce.subscriptions().get_active_plans()
        .expect("Failed to get active plans");

    assert!(!active_plans.is_empty());
    assert!(active_plans.iter().all(|p| p.status == PlanStatus::Active));
}

#[test]
fn test_get_customer_subscriptions() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let customer_id = create_test_customer(&commerce);
    let plan_id = create_active_plan(&commerce, "Test Plan", dec!(19.99));

    // Create subscription for this customer
    commerce.subscriptions().subscribe(CreateSubscription {
        customer_id,
        plan_id,
        skip_trial: Some(true),
        ..Default::default()
    }).expect("Failed to create subscription");

    let subs = commerce.subscriptions().get_customer_subscriptions(customer_id)
        .expect("Failed to get customer subscriptions");

    assert_eq!(subs.len(), 1);
    assert_eq!(subs[0].customer_id, customer_id);
}

#[test]
fn test_is_active() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let customer_id = create_test_customer(&commerce);
    let plan_id = create_active_plan(&commerce, "Test Plan", dec!(19.99));

    let subscription = commerce.subscriptions().subscribe(CreateSubscription {
        customer_id,
        plan_id,
        skip_trial: Some(true),
        ..Default::default()
    }).expect("Failed to create subscription");

    assert!(commerce.subscriptions().is_active(subscription.id)
        .expect("Failed to check if active"));

    // Cancel it
    commerce.subscriptions().cancel(subscription.id, CancelSubscription {
        immediate: Some(true),
        ..Default::default()
    }).expect("Failed to cancel");

    assert!(!commerce.subscriptions().is_active(subscription.id)
        .expect("Failed to check if active"));
}

#[test]
fn test_is_in_trial() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let customer_id = create_test_customer(&commerce);
    let plan_id = create_active_plan(&commerce, "Test Plan", dec!(19.99));

    // Create subscription with trial
    let trial_sub = commerce.subscriptions().subscribe(CreateSubscription {
        customer_id,
        plan_id,
        ..Default::default()
    }).expect("Failed to create subscription");

    assert!(commerce.subscriptions().is_in_trial(trial_sub.id)
        .expect("Failed to check trial status"));

    // Create subscription without trial
    let active_sub = commerce.subscriptions().subscribe(CreateSubscription {
        customer_id,
        plan_id,
        skip_trial: Some(true),
        ..Default::default()
    }).expect("Failed to create subscription");

    assert!(!commerce.subscriptions().is_in_trial(active_sub.id)
        .expect("Failed to check trial status"));
}

// ============================================================================
// Error Case Tests
// ============================================================================

#[test]
fn test_subscribe_to_inactive_plan_fails() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let customer_id = create_test_customer(&commerce);

    // Create a draft plan (not activated)
    let plan = commerce.subscriptions().create_plan(CreateSubscriptionPlan {
        name: "Draft Plan".into(),
        billing_interval: BillingInterval::Monthly,
        price: dec!(29.99),
        ..Default::default()
    }).expect("Failed to create plan");

    // Try to subscribe to inactive plan
    let result = commerce.subscriptions().subscribe(CreateSubscription {
        customer_id,
        plan_id: plan.id,
        ..Default::default()
    });

    assert!(result.is_err());
}

#[test]
fn test_subscribe_to_nonexistent_plan_fails() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let customer_id = create_test_customer(&commerce);

    let result = commerce.subscriptions().subscribe(CreateSubscription {
        customer_id,
        plan_id: Uuid::new_v4(), // Random non-existent ID
        ..Default::default()
    });

    assert!(result.is_err());
}

#[test]
fn test_get_nonexistent_subscription() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let result = commerce.subscriptions().get(Uuid::new_v4())
        .expect("Should not error");

    assert!(result.is_none());
}

#[test]
fn test_get_nonexistent_plan() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let result = commerce.subscriptions().get_plan(Uuid::new_v4())
        .expect("Should not error");

    assert!(result.is_none());
}
