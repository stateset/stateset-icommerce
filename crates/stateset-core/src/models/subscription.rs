//! Subscription domain models
//!
//! Comprehensive subscription management supporting:
//! - Subscription plans with flexible billing intervals
//! - Customer subscriptions with lifecycle management
//! - Billing cycles and payment tracking
//! - Pause, resume, skip, and cancel functionality
//! - Trial periods and promotional pricing
//! - Product swapping and quantity changes

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::Address;

// ============================================================================
// Subscription Enums
// ============================================================================

/// Billing interval for subscriptions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BillingInterval {
    /// Weekly billing
    Weekly,
    /// Every two weeks
    Biweekly,
    /// Monthly billing
    #[default]
    Monthly,
    /// Every two months
    Bimonthly,
    /// Quarterly (every 3 months)
    Quarterly,
    /// Every 6 months
    Semiannual,
    /// Annual billing
    Annual,
    /// Custom interval in days
    Custom,
}

impl BillingInterval {
    /// Get the number of days in this interval
    pub fn days(&self) -> i64 {
        match self {
            BillingInterval::Weekly => 7,
            BillingInterval::Biweekly => 14,
            BillingInterval::Monthly => 30,
            BillingInterval::Bimonthly => 60,
            BillingInterval::Quarterly => 90,
            BillingInterval::Semiannual => 180,
            BillingInterval::Annual => 365,
            BillingInterval::Custom => 30, // Default, should use custom_interval_days
        }
    }
}

impl std::fmt::Display for BillingInterval {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Weekly => write!(f, "weekly"),
            Self::Biweekly => write!(f, "biweekly"),
            Self::Monthly => write!(f, "monthly"),
            Self::Bimonthly => write!(f, "bimonthly"),
            Self::Quarterly => write!(f, "quarterly"),
            Self::Semiannual => write!(f, "semiannual"),
            Self::Annual => write!(f, "annual"),
            Self::Custom => write!(f, "custom"),
        }
    }
}

/// Status of a subscription
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionStatus {
    /// Trial period (no charge yet)
    Trial,
    /// Active and billing normally
    #[default]
    Active,
    /// Paused by customer (no billing, can resume)
    Paused,
    /// Past due - payment failed, in retry period
    PastDue,
    /// Cancelled - will end at period end
    Cancelled,
    /// Expired - subscription has ended
    Expired,
    /// Pending - awaiting initial payment/activation
    Pending,
}

impl std::fmt::Display for SubscriptionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Trial => write!(f, "trial"),
            Self::Active => write!(f, "active"),
            Self::Paused => write!(f, "paused"),
            Self::PastDue => write!(f, "past_due"),
            Self::Cancelled => write!(f, "cancelled"),
            Self::Expired => write!(f, "expired"),
            Self::Pending => write!(f, "pending"),
        }
    }
}

/// Status of a subscription plan
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PlanStatus {
    /// Draft - not yet available
    Draft,
    /// Active - available for new subscriptions
    #[default]
    Active,
    /// Archived - no new subscriptions, existing continue
    Archived,
}

/// Status of a billing cycle
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BillingCycleStatus {
    /// Scheduled for future billing
    #[default]
    Scheduled,
    /// Payment is being processed
    Processing,
    /// Successfully billed
    Paid,
    /// Payment failed
    Failed,
    /// Skipped by customer request
    Skipped,
    /// Refunded
    Refunded,
    /// Voided/cancelled
    Voided,
}

/// Type of subscription event
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionEventType {
    /// Subscription created
    Created,
    /// Subscription activated
    Activated,
    /// Trial started
    TrialStarted,
    /// Trial ended
    TrialEnded,
    /// Successfully renewed/billed
    Renewed,
    /// Payment failed
    PaymentFailed,
    /// Payment retry succeeded
    PaymentRetrySucceeded,
    /// Subscription paused
    Paused,
    /// Subscription resumed
    Resumed,
    /// Upcoming renewal skipped
    Skipped,
    /// Subscription cancelled
    Cancelled,
    /// Subscription expired
    Expired,
    /// Plan changed
    PlanChanged,
    /// Items modified
    ItemsModified,
    /// Quantity changed
    QuantityChanged,
    /// Address updated
    AddressUpdated,
    /// Payment method updated
    PaymentMethodUpdated,
    /// Discount applied
    DiscountApplied,
    /// Discount removed
    DiscountRemoved,
    /// Refund issued
    Refunded,
}

// ============================================================================
// Subscription Plan Model
// ============================================================================

/// A subscription plan template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionPlan {
    pub id: Uuid,
    /// Unique code for the plan
    pub code: String,
    /// Display name
    pub name: String,
    /// Description
    pub description: Option<String>,
    pub status: PlanStatus,

    // Billing configuration
    pub billing_interval: BillingInterval,
    /// Custom interval in days (when billing_interval is Custom)
    pub custom_interval_days: Option<i32>,
    /// Base price per billing cycle
    pub price: Decimal,
    /// Setup/activation fee (charged once)
    pub setup_fee: Option<Decimal>,
    pub currency: String,

    // Trial configuration
    /// Trial period in days (0 = no trial)
    pub trial_days: i32,
    /// Whether payment method is required for trial
    pub trial_requires_payment_method: bool,

    // Limits
    /// Minimum number of billing cycles
    pub min_cycles: Option<i32>,
    /// Maximum number of billing cycles (None = unlimited)
    pub max_cycles: Option<i32>,

    // Included products/items
    pub items: Vec<SubscriptionPlanItem>,

    // Discounts
    /// Percentage discount for this plan
    pub discount_percent: Option<Decimal>,
    /// Fixed discount amount
    pub discount_amount: Option<Decimal>,

    // Metadata
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// An item included in a subscription plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionPlanItem {
    pub id: Uuid,
    pub plan_id: Uuid,
    pub product_id: Uuid,
    pub variant_id: Option<Uuid>,
    pub sku: String,
    pub name: String,
    /// Default quantity
    pub quantity: i32,
    /// Minimum quantity (for customizable plans)
    pub min_quantity: Option<i32>,
    /// Maximum quantity (for customizable plans)
    pub max_quantity: Option<i32>,
    /// Whether customer can remove this item
    pub is_required: bool,
    /// Unit price override (None = use plan price)
    pub unit_price: Option<Decimal>,
}

// ============================================================================
// Subscription Model
// ============================================================================

/// A customer's subscription
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscription {
    pub id: Uuid,
    /// Human-readable subscription number
    pub subscription_number: String,
    pub customer_id: Uuid,
    pub plan_id: Uuid,
    /// Snapshot of plan name at time of subscription
    pub plan_name: String,
    pub status: SubscriptionStatus,

    // Billing
    pub billing_interval: BillingInterval,
    pub custom_interval_days: Option<i32>,
    /// Current price per cycle
    pub price: Decimal,
    pub currency: String,
    /// Payment method ID (from payment provider)
    pub payment_method_id: Option<String>,

    // Dates
    /// When the subscription started
    pub started_at: DateTime<Utc>,
    /// When current billing period started
    pub current_period_start: DateTime<Utc>,
    /// When current billing period ends
    pub current_period_end: DateTime<Utc>,
    /// Next billing date
    pub next_billing_date: Option<DateTime<Utc>>,
    /// Trial end date (if applicable)
    pub trial_ends_at: Option<DateTime<Utc>>,
    /// When subscription was cancelled (if applicable)
    pub cancelled_at: Option<DateTime<Utc>>,
    /// When subscription ends (for cancelled subscriptions)
    pub ends_at: Option<DateTime<Utc>>,
    /// When subscription was paused
    pub paused_at: Option<DateTime<Utc>>,
    /// When to auto-resume (if paused with a date)
    pub resume_at: Option<DateTime<Utc>>,

    // Cycle tracking
    /// Number of completed billing cycles
    pub billing_cycle_count: i32,
    /// Number of failed payment attempts in current cycle
    pub failed_payment_attempts: i32,

    // Items
    pub items: Vec<SubscriptionItem>,

    // Addresses
    pub shipping_address: Option<Address>,
    pub billing_address: Option<Address>,

    // Applied discounts
    pub discount_percent: Option<Decimal>,
    pub discount_amount: Option<Decimal>,
    /// Coupon code applied (if any)
    pub coupon_code: Option<String>,

    // Metadata
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A line item in a subscription
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionItem {
    pub id: Uuid,
    pub subscription_id: Uuid,
    pub product_id: Uuid,
    pub variant_id: Option<Uuid>,
    pub sku: String,
    pub name: String,
    pub quantity: i32,
    /// Price per unit
    pub unit_price: Decimal,
    /// Line total (quantity * unit_price)
    pub line_total: Decimal,
}

// ============================================================================
// Billing Cycle Model
// ============================================================================

/// A billing cycle/period for a subscription
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillingCycle {
    pub id: Uuid,
    pub subscription_id: Uuid,
    /// Cycle number (1, 2, 3, ...)
    pub cycle_number: i32,
    pub status: BillingCycleStatus,

    // Period
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    /// When billing was attempted
    pub billed_at: Option<DateTime<Utc>>,

    // Amounts
    pub subtotal: Decimal,
    pub discount: Decimal,
    pub tax: Decimal,
    pub total: Decimal,
    pub currency: String,

    // Payment
    /// Payment ID from payment provider
    pub payment_id: Option<String>,
    /// Order ID if an order was created
    pub order_id: Option<Uuid>,
    /// Invoice ID if an invoice was created
    pub invoice_id: Option<Uuid>,

    // Failure tracking
    pub failure_reason: Option<String>,
    pub retry_count: i32,
    pub next_retry_at: Option<DateTime<Utc>>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ============================================================================
// Subscription Event (Audit Log)
// ============================================================================

/// An event in a subscription's history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionEvent {
    pub id: Uuid,
    pub subscription_id: Uuid,
    pub event_type: SubscriptionEventType,
    /// Human-readable description
    pub description: String,
    /// Detailed event data
    pub data: Option<serde_json::Value>,
    /// Who triggered this event (customer_id, "system", "admin")
    pub triggered_by: Option<String>,
    pub created_at: DateTime<Utc>,
}

// ============================================================================
// CRUD DTOs
// ============================================================================

/// Create a new subscription plan
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreateSubscriptionPlan {
    pub code: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub billing_interval: BillingInterval,
    pub custom_interval_days: Option<i32>,
    pub price: Decimal,
    pub setup_fee: Option<Decimal>,
    pub currency: Option<String>,
    pub trial_days: Option<i32>,
    pub trial_requires_payment_method: Option<bool>,
    pub min_cycles: Option<i32>,
    pub max_cycles: Option<i32>,
    pub items: Option<Vec<CreateSubscriptionPlanItem>>,
    pub discount_percent: Option<Decimal>,
    pub discount_amount: Option<Decimal>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSubscriptionPlanItem {
    pub product_id: Uuid,
    pub variant_id: Option<Uuid>,
    pub sku: String,
    pub name: String,
    pub quantity: i32,
    pub min_quantity: Option<i32>,
    pub max_quantity: Option<i32>,
    pub is_required: Option<bool>,
    pub unit_price: Option<Decimal>,
}

/// Update a subscription plan
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateSubscriptionPlan {
    pub name: Option<String>,
    pub description: Option<String>,
    pub status: Option<PlanStatus>,
    pub price: Option<Decimal>,
    pub setup_fee: Option<Decimal>,
    pub trial_days: Option<i32>,
    pub trial_requires_payment_method: Option<bool>,
    pub min_cycles: Option<i32>,
    pub max_cycles: Option<i32>,
    pub discount_percent: Option<Decimal>,
    pub discount_amount: Option<Decimal>,
    pub metadata: Option<serde_json::Value>,
}

/// Create a new subscription
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSubscription {
    pub customer_id: Uuid,
    pub plan_id: Uuid,
    /// Override plan items with custom items
    pub items: Option<Vec<CreateSubscriptionItem>>,
    /// Override plan price
    pub price: Option<Decimal>,
    pub payment_method_id: Option<String>,
    pub shipping_address: Option<Address>,
    pub billing_address: Option<Address>,
    /// Skip trial period
    pub skip_trial: Option<bool>,
    /// Start date (default: now)
    pub start_date: Option<DateTime<Utc>>,
    /// Coupon code to apply
    pub coupon_code: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

impl Default for CreateSubscription {
    fn default() -> Self {
        Self {
            customer_id: Uuid::nil(),
            plan_id: Uuid::nil(),
            items: None,
            price: None,
            payment_method_id: None,
            shipping_address: None,
            billing_address: None,
            skip_trial: None,
            start_date: None,
            coupon_code: None,
            metadata: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSubscriptionItem {
    pub product_id: Uuid,
    pub variant_id: Option<Uuid>,
    pub sku: String,
    pub name: String,
    pub quantity: i32,
    pub unit_price: Option<Decimal>,
}

/// Update a subscription
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateSubscription {
    pub status: Option<SubscriptionStatus>,
    pub price: Option<Decimal>,
    pub payment_method_id: Option<String>,
    pub shipping_address: Option<Address>,
    pub billing_address: Option<Address>,
    pub next_billing_date: Option<DateTime<Utc>>,
    pub discount_percent: Option<Decimal>,
    pub discount_amount: Option<Decimal>,
    pub coupon_code: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Pause subscription request
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PauseSubscription {
    /// Optional resume date
    pub resume_at: Option<DateTime<Utc>>,
    pub reason: Option<String>,
}

/// Cancel subscription request
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CancelSubscription {
    /// Cancel immediately or at period end
    pub immediate: Option<bool>,
    pub reason: Option<String>,
    /// Feedback for cancellation
    pub feedback: Option<String>,
}

/// Skip next billing cycle
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SkipBillingCycle {
    pub reason: Option<String>,
}

/// Change subscription plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeSubscriptionPlan {
    pub new_plan_id: Uuid,
    /// Prorate charges (default: true)
    pub prorate: Option<bool>,
    /// Apply immediately or at next billing
    pub immediate: Option<bool>,
}

/// Modify subscription items
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModifySubscriptionItems {
    /// Items to add
    pub add: Option<Vec<CreateSubscriptionItem>>,
    /// Item IDs to remove
    pub remove: Option<Vec<Uuid>>,
    /// Items to update (id -> new quantity)
    pub update_quantities: Option<Vec<UpdateItemQuantity>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateItemQuantity {
    pub item_id: Uuid,
    pub quantity: i32,
}

// ============================================================================
// Filter DTOs
// ============================================================================

/// Filter for listing subscription plans
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SubscriptionPlanFilter {
    pub status: Option<PlanStatus>,
    pub billing_interval: Option<BillingInterval>,
    pub search: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Filter for listing subscriptions
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SubscriptionFilter {
    pub customer_id: Option<Uuid>,
    pub plan_id: Option<Uuid>,
    pub status: Option<SubscriptionStatus>,
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
    pub search: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Filter for listing billing cycles
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BillingCycleFilter {
    pub subscription_id: Option<Uuid>,
    pub status: Option<BillingCycleStatus>,
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Generate a unique subscription number
pub fn generate_subscription_number() -> String {
    let id = Uuid::new_v4();
    let bytes = id.as_bytes();
    let random = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) % 1000000;
    format!("SUB-{:06}", random)
}

/// Generate a unique plan code
pub fn generate_plan_code(name: &str) -> String {
    let slug: String = name
        .to_uppercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ')
        .take(20)
        .collect::<String>()
        .trim()
        .replace(' ', "-");

    let id = Uuid::new_v4();
    let bytes = id.as_bytes();
    let random = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) % 10000;

    if slug.is_empty() {
        format!("PLAN-{:04}", random)
    } else {
        format!("{}-{:04}", slug, random)
    }
}

impl Subscription {
    /// Check if subscription is in an active billing state
    pub fn is_active(&self) -> bool {
        matches!(
            self.status,
            SubscriptionStatus::Active | SubscriptionStatus::Trial
        )
    }

    /// Check if subscription can be paused
    pub fn can_pause(&self) -> bool {
        matches!(
            self.status,
            SubscriptionStatus::Active | SubscriptionStatus::Trial
        )
    }

    /// Check if subscription can be resumed
    pub fn can_resume(&self) -> bool {
        self.status == SubscriptionStatus::Paused
    }

    /// Check if subscription can be cancelled
    pub fn can_cancel(&self) -> bool {
        !matches!(
            self.status,
            SubscriptionStatus::Cancelled | SubscriptionStatus::Expired
        )
    }

    /// Check if subscription is in trial period
    pub fn is_in_trial(&self) -> bool {
        if self.status != SubscriptionStatus::Trial {
            return false;
        }

        if let Some(trial_ends) = self.trial_ends_at {
            return Utc::now() < trial_ends;
        }

        false
    }

    /// Calculate remaining trial days
    pub fn trial_days_remaining(&self) -> Option<i64> {
        if !self.is_in_trial() {
            return None;
        }

        self.trial_ends_at.map(|ends| {
            let now = Utc::now();
            if ends > now {
                (ends - now).num_days()
            } else {
                0
            }
        })
    }

    /// Calculate total subscription value
    pub fn calculate_total(&self) -> Decimal {
        self.items.iter().map(|item| item.line_total).sum()
    }

    /// Get next billing amount (after discounts)
    pub fn next_billing_amount(&self) -> Decimal {
        let subtotal = self.calculate_total();
        let mut total = subtotal;

        if let Some(pct) = self.discount_percent {
            total -= subtotal * pct;
        }
        if let Some(amt) = self.discount_amount {
            total -= amt;
        }

        if total < Decimal::ZERO {
            Decimal::ZERO
        } else {
            total
        }
    }
}

impl SubscriptionItem {
    /// Calculate line total
    pub fn calculate_total(quantity: i32, unit_price: Decimal) -> Decimal {
        unit_price * Decimal::from(quantity)
    }
}

impl BillingCycle {
    /// Check if cycle can be refunded
    pub fn can_refund(&self) -> bool {
        self.status == BillingCycleStatus::Paid
    }

    /// Check if cycle can be retried
    pub fn can_retry(&self) -> bool {
        self.status == BillingCycleStatus::Failed
    }
}
