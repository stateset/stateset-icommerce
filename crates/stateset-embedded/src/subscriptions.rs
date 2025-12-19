//! Subscription management operations
//!
//! Comprehensive subscription system supporting:
//! - Subscription plans with flexible billing intervals
//! - Customer subscriptions with full lifecycle management
//! - Trial periods and promotional pricing
//! - Pause, resume, skip, and cancel functionality
//! - Billing cycle tracking and payment integration
//!
//! # Example
//!
//! ```rust,no_run
//! use stateset_embedded::{Commerce, CreateSubscriptionPlan, CreateSubscription, BillingInterval};
//! use rust_decimal_macros::dec;
//!
//! let commerce = Commerce::new("./store.db")?;
//!
//! // Create a subscription plan
//! let plan = commerce.subscriptions().create_plan(CreateSubscriptionPlan {
//!     name: "Monthly Coffee Box".into(),
//!     billing_interval: BillingInterval::Monthly,
//!     price: dec!(29.99),
//!     trial_days: Some(14),
//!     ..Default::default()
//! })?;
//!
//! // Activate the plan
//! commerce.subscriptions().activate_plan(plan.id)?;
//!
//! // Subscribe a customer
//! let subscription = commerce.subscriptions().subscribe(CreateSubscription {
//!     customer_id: customer.id,
//!     plan_id: plan.id,
//!     ..Default::default()
//! })?;
//!
//! println!("Subscription #{} created", subscription.subscription_number);
//! # Ok::<(), stateset_embedded::CommerceError>(())
//! ```

use chrono::{DateTime, Utc};
use stateset_core::{
    BillingCycle, BillingCycleFilter, BillingCycleStatus, CancelSubscription,
    CreateSubscription, CreateSubscriptionPlan, PauseSubscription, Result,
    SkipBillingCycle, Subscription, SubscriptionEvent, SubscriptionFilter,
    SubscriptionPlan, SubscriptionPlanFilter, UpdateSubscription, UpdateSubscriptionPlan,
};
use stateset_db::sqlite::SqliteSubscriptionRepository;
use uuid::Uuid;

/// Subscription management interface.
pub struct Subscriptions {
    repo: SqliteSubscriptionRepository,
}

impl Subscriptions {
    pub(crate) fn new(repo: SqliteSubscriptionRepository) -> Self {
        Self { repo }
    }

    // ========================================================================
    // Subscription Plans
    // ========================================================================

    /// Create a new subscription plan.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CreateSubscriptionPlan, BillingInterval};
    /// use rust_decimal_macros::dec;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let plan = commerce.subscriptions().create_plan(CreateSubscriptionPlan {
    ///     name: "Weekly Meal Kit".into(),
    ///     billing_interval: BillingInterval::Weekly,
    ///     price: dec!(79.99),
    ///     trial_days: Some(7),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn create_plan(&self, input: CreateSubscriptionPlan) -> Result<SubscriptionPlan> {
        self.repo.create_plan(input)
    }

    /// Get a subscription plan by ID.
    pub fn get_plan(&self, id: Uuid) -> Result<Option<SubscriptionPlan>> {
        self.repo.get_plan(id)
    }

    /// Get a subscription plan by its code.
    pub fn get_plan_by_code(&self, code: &str) -> Result<Option<SubscriptionPlan>> {
        self.repo.get_plan_by_code(code)
    }

    /// List subscription plans with optional filtering.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, SubscriptionPlanFilter, PlanStatus};
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// // List active plans
    /// let plans = commerce.subscriptions().list_plans(SubscriptionPlanFilter {
    ///     status: Some(PlanStatus::Active),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn list_plans(&self, filter: SubscriptionPlanFilter) -> Result<Vec<SubscriptionPlan>> {
        self.repo.list_plans(filter)
    }

    /// Update a subscription plan.
    pub fn update_plan(&self, id: Uuid, input: UpdateSubscriptionPlan) -> Result<SubscriptionPlan> {
        self.repo.update_plan(id, input)
    }

    /// Activate a subscription plan (make it available for new subscriptions).
    pub fn activate_plan(&self, id: Uuid) -> Result<SubscriptionPlan> {
        self.repo.activate_plan(id)
    }

    /// Archive a subscription plan (no new subscriptions, existing ones continue).
    pub fn archive_plan(&self, id: Uuid) -> Result<SubscriptionPlan> {
        self.repo.archive_plan(id)
    }

    // ========================================================================
    // Subscriptions
    // ========================================================================

    /// Create a new subscription for a customer.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CreateSubscription};
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let subscription = commerce.subscriptions().subscribe(CreateSubscription {
    ///     customer_id: Uuid::new_v4(),
    ///     plan_id: Uuid::new_v4(),
    ///     payment_method_id: Some("pm_1234".into()),
    ///     ..Default::default()
    /// })?;
    ///
    /// println!("Created subscription #{}", subscription.subscription_number);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn subscribe(&self, input: CreateSubscription) -> Result<Subscription> {
        self.repo.create_subscription(input)
    }

    /// Get a subscription by ID.
    pub fn get(&self, id: Uuid) -> Result<Option<Subscription>> {
        self.repo.get_subscription(id)
    }

    /// Get a subscription by its number (e.g., "SUB-123456").
    pub fn get_by_number(&self, number: &str) -> Result<Option<Subscription>> {
        self.repo.get_subscription_by_number(number)
    }

    /// List subscriptions with optional filtering.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, SubscriptionFilter, SubscriptionStatus};
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// // List active subscriptions for a customer
    /// let subs = commerce.subscriptions().list(SubscriptionFilter {
    ///     customer_id: Some(Uuid::new_v4()),
    ///     status: Some(SubscriptionStatus::Active),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn list(&self, filter: SubscriptionFilter) -> Result<Vec<Subscription>> {
        self.repo.list_subscriptions(filter)
    }

    /// Update a subscription.
    pub fn update(&self, id: Uuid, input: UpdateSubscription) -> Result<Subscription> {
        self.repo.update_subscription(id, input)
    }

    // ========================================================================
    // Subscription Lifecycle
    // ========================================================================

    /// Pause a subscription.
    ///
    /// Pausing stops billing but preserves the subscription. The customer
    /// can resume at any time.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, PauseSubscription};
    /// use uuid::Uuid;
    /// use chrono::{Utc, Duration};
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// // Pause for 30 days
    /// commerce.subscriptions().pause(Uuid::new_v4(), PauseSubscription {
    ///     resume_at: Some(Utc::now() + Duration::days(30)),
    ///     reason: Some("Customer requested vacation hold".into()),
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn pause(&self, id: Uuid, input: PauseSubscription) -> Result<Subscription> {
        self.repo.pause_subscription(id, input)
    }

    /// Resume a paused subscription.
    ///
    /// This reactivates billing and creates a new billing period.
    pub fn resume(&self, id: Uuid) -> Result<Subscription> {
        self.repo.resume_subscription(id)
    }

    /// Cancel a subscription.
    ///
    /// By default, cancellation takes effect at the end of the current
    /// billing period. Use `immediate: true` to cancel immediately.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CancelSubscription};
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// // Cancel at end of period
    /// commerce.subscriptions().cancel(Uuid::new_v4(), CancelSubscription {
    ///     reason: Some("Customer found alternative".into()),
    ///     ..Default::default()
    /// })?;
    ///
    /// // Immediate cancellation
    /// commerce.subscriptions().cancel(Uuid::new_v4(), CancelSubscription {
    ///     immediate: Some(true),
    ///     reason: Some("Refund requested".into()),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn cancel(&self, id: Uuid, input: CancelSubscription) -> Result<Subscription> {
        self.repo.cancel_subscription(id, input)
    }

    /// Skip the next billing cycle.
    ///
    /// The subscription remains active, but the next billing date is
    /// pushed forward by one interval.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, SkipBillingCycle};
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// commerce.subscriptions().skip_next_cycle(Uuid::new_v4(), SkipBillingCycle {
    ///     reason: Some("Customer traveling".into()),
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn skip_next_cycle(&self, id: Uuid, input: SkipBillingCycle) -> Result<Subscription> {
        self.repo.skip_billing_cycle(id, input)
    }

    // ========================================================================
    // Billing Cycles
    // ========================================================================

    /// Create a billing cycle for a subscription.
    ///
    /// This is typically called by the billing system when processing
    /// subscription renewals.
    pub fn create_billing_cycle(
        &self,
        subscription_id: Uuid,
        cycle_number: i32,
        period_start: DateTime<Utc>,
        period_end: DateTime<Utc>,
    ) -> Result<BillingCycle> {
        self.repo.create_billing_cycle(subscription_id, cycle_number, period_start, period_end)
    }

    /// Get a billing cycle by ID.
    pub fn get_billing_cycle(&self, id: Uuid) -> Result<Option<BillingCycle>> {
        self.repo.get_billing_cycle(id)
    }

    /// List billing cycles with optional filtering.
    pub fn list_billing_cycles(&self, filter: BillingCycleFilter) -> Result<Vec<BillingCycle>> {
        self.repo.list_billing_cycles(filter)
    }

    /// Update billing cycle status (mark as paid, failed, etc.).
    pub fn update_billing_cycle_status(
        &self,
        id: Uuid,
        status: BillingCycleStatus,
        payment_id: Option<String>,
        failure_reason: Option<String>,
    ) -> Result<BillingCycle> {
        self.repo.update_billing_cycle_status(id, status, payment_id, failure_reason)
    }

    /// Mark a billing cycle as paid.
    pub fn mark_cycle_paid(&self, id: Uuid, payment_id: String) -> Result<BillingCycle> {
        self.repo.update_billing_cycle_status(id, BillingCycleStatus::Paid, Some(payment_id), None)
    }

    /// Mark a billing cycle as failed.
    pub fn mark_cycle_failed(&self, id: Uuid, reason: &str) -> Result<BillingCycle> {
        self.repo.update_billing_cycle_status(id, BillingCycleStatus::Failed, None, Some(reason.to_string()))
    }

    // ========================================================================
    // Events
    // ========================================================================

    /// Get subscription events (audit log).
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::Commerce;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let events = commerce.subscriptions().get_events(Uuid::new_v4(), Some(10))?;
    /// for event in events {
    ///     println!("{}: {}", event.event_type, event.description);
    /// }
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn get_events(&self, subscription_id: Uuid, limit: Option<u32>) -> Result<Vec<SubscriptionEvent>> {
        self.repo.get_subscription_events(subscription_id, limit)
    }

    // ========================================================================
    // Convenience Methods
    // ========================================================================

    /// Get all active plans.
    pub fn get_active_plans(&self) -> Result<Vec<SubscriptionPlan>> {
        self.list_plans(SubscriptionPlanFilter {
            status: Some(stateset_core::PlanStatus::Active),
            ..Default::default()
        })
    }

    /// Get all subscriptions for a customer.
    pub fn get_customer_subscriptions(&self, customer_id: Uuid) -> Result<Vec<Subscription>> {
        self.list(SubscriptionFilter {
            customer_id: Some(customer_id),
            ..Default::default()
        })
    }

    /// Get active subscriptions for a customer.
    pub fn get_active_customer_subscriptions(&self, customer_id: Uuid) -> Result<Vec<Subscription>> {
        self.list(SubscriptionFilter {
            customer_id: Some(customer_id),
            status: Some(stateset_core::SubscriptionStatus::Active),
            ..Default::default()
        })
    }

    /// Check if a subscription is active.
    pub fn is_active(&self, id: Uuid) -> Result<bool> {
        if let Some(sub) = self.get(id)? {
            Ok(sub.is_active())
        } else {
            Ok(false)
        }
    }

    /// Check if a subscription is in trial period.
    pub fn is_in_trial(&self, id: Uuid) -> Result<bool> {
        if let Some(sub) = self.get(id)? {
            Ok(sub.is_in_trial())
        } else {
            Ok(false)
        }
    }

    /// Get subscriptions due for billing.
    ///
    /// Returns subscriptions where `next_billing_date` is on or before the
    /// specified date.
    pub fn get_due_for_billing(&self, before: DateTime<Utc>) -> Result<Vec<Subscription>> {
        let subs = self.list(SubscriptionFilter {
            status: Some(stateset_core::SubscriptionStatus::Active),
            ..Default::default()
        })?;

        Ok(subs.into_iter()
            .filter(|s| {
                if let Some(next_billing) = s.next_billing_date {
                    next_billing <= before
                } else {
                    false
                }
            })
            .collect())
    }

    /// Get subscriptions with trials ending soon.
    ///
    /// Returns subscriptions in trial status where trial ends within the
    /// specified number of days.
    pub fn get_trials_ending(&self, within_days: i64) -> Result<Vec<Subscription>> {
        let cutoff = Utc::now() + chrono::Duration::days(within_days);

        let subs = self.list(SubscriptionFilter {
            status: Some(stateset_core::SubscriptionStatus::Trial),
            ..Default::default()
        })?;

        Ok(subs.into_iter()
            .filter(|s| {
                if let Some(trial_ends) = s.trial_ends_at {
                    trial_ends <= cutoff
                } else {
                    false
                }
            })
            .collect())
    }
}
