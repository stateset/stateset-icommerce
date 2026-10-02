//! Subscriptions API.
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Subscriptions API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateSubscriptionPlanInput {
    pub name: String,
    pub description: Option<String>,
    pub code: Option<String>,
    #[napi(ts_type = "SubscriptionBillingIntervalInput")]
    pub billing_interval: String,
    pub custom_interval_days: Option<i32>,
    pub price: f64,
    pub setup_fee: Option<f64>,
    pub currency: Option<String>,
    pub trial_days: Option<i32>,
    pub trial_requires_payment_method: Option<bool>,
    pub min_cycles: Option<i32>,
    pub max_cycles: Option<i32>,
    pub discount_percent: Option<f64>,
    pub discount_amount: Option<f64>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UpdateSubscriptionPlanInput {
    pub name: Option<String>,
    pub description: Option<String>,
    pub price: Option<f64>,
    pub setup_fee: Option<f64>,
    pub trial_days: Option<i32>,
    pub trial_requires_payment_method: Option<bool>,
    pub min_cycles: Option<i32>,
    pub max_cycles: Option<i32>,
    pub discount_percent: Option<f64>,
    pub discount_amount: Option<f64>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct SubscriptionPlanFilterInput {
    #[napi(ts_type = "SubscriptionPlanStatus")]
    pub status: Option<String>,
    #[napi(ts_type = "SubscriptionBillingIntervalInput")]
    pub billing_interval: Option<String>,
    pub search: Option<String>,
    pub limit: Option<i32>,
    pub offset: Option<i32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct SubscriptionPlanOutput {
    pub id: String,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    #[napi(ts_type = "SubscriptionPlanStatus")]
    pub status: String,
    #[napi(ts_type = "SubscriptionBillingInterval")]
    pub billing_interval: String,
    pub custom_interval_days: Option<i32>,
    /// @deprecated Use the `priceExact` twin; float money will be removed in 2.0.
    pub price: f64,
    /// Exact base-10 price, straight from the engine's `Decimal`. Prefer this field for money.
    pub price_exact: String,
    /// @deprecated Use the `setupFeeExact` twin; float money will be removed in 2.0.
    pub setup_fee: Option<f64>,
    /// Exact base-10 setup fee, straight from the engine's `Decimal`. Prefer this field for money.
    pub setup_fee_exact: Option<String>,
    pub currency: String,
    pub trial_days: i32,
    pub trial_requires_payment_method: bool,
    pub min_cycles: Option<i32>,
    pub max_cycles: Option<i32>,
    pub discount_percent: Option<f64>,
    /// @deprecated Use the `discountAmountExact` twin; float money will be removed in 2.0.
    pub discount_amount: Option<f64>,
    /// Exact base-10 discount amount, straight from the engine's `Decimal`. Prefer this field for money.
    pub discount_amount_exact: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl TryFrom<stateset_core::SubscriptionPlan> for SubscriptionPlanOutput {
    type Error = Error;

    fn try_from(p: stateset_core::SubscriptionPlan) -> Result<Self> {
        let (price, price_exact) = money_pair(p.price, "subscription plan price")?;
        let (setup_fee, setup_fee_exact) =
            optional_money_pair(p.setup_fee, "subscription plan setup fee")?;
        let (discount_amount, discount_amount_exact) =
            optional_money_pair(p.discount_amount, "subscription plan discount amount")?;
        Ok(Self {
            id: p.id.to_string(),
            code: p.code,
            name: p.name,
            description: p.description,
            status: format!("{:?}", p.status).to_lowercase(),
            billing_interval: format!("{}", p.billing_interval),
            custom_interval_days: p.custom_interval_days,
            price,
            price_exact,
            setup_fee,
            setup_fee_exact,
            currency: p.currency.to_string(),
            trial_days: p.trial_days,
            trial_requires_payment_method: p.trial_requires_payment_method,
            min_cycles: p.min_cycles,
            max_cycles: p.max_cycles,
            discount_percent: optional_to_f64_checked(
                p.discount_percent,
                "subscription plan discount percent",
            )?,
            discount_amount,
            discount_amount_exact,
            created_at: p.created_at.to_rfc3339(),
            updated_at: p.updated_at.to_rfc3339(),
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateSubscriptionInput {
    pub customer_id: String,
    pub plan_id: String,
    pub payment_method_id: Option<String>,
    pub skip_trial: Option<bool>,
    pub price: Option<f64>,
    pub coupon_code: Option<String>,
    pub start_date: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UpdateSubscriptionInput {
    #[napi(ts_type = "SubscriptionStatus")]
    pub status: Option<String>,
    pub price: Option<f64>,
    pub payment_method_id: Option<String>,
    pub next_billing_date: Option<String>,
    pub discount_percent: Option<f64>,
    pub discount_amount: Option<f64>,
    pub coupon_code: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct SubscriptionFilterInput {
    pub customer_id: Option<String>,
    pub plan_id: Option<String>,
    #[napi(ts_type = "SubscriptionStatus")]
    pub status: Option<String>,
    pub from_date: Option<String>,
    pub to_date: Option<String>,
    pub search: Option<String>,
    pub limit: Option<i32>,
    pub offset: Option<i32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct SubscriptionOutput {
    pub id: String,
    pub subscription_number: String,
    pub customer_id: String,
    pub plan_id: String,
    pub plan_name: String,
    #[napi(ts_type = "SubscriptionStatus")]
    pub status: String,
    #[napi(ts_type = "SubscriptionBillingInterval")]
    pub billing_interval: String,
    pub custom_interval_days: Option<i32>,
    /// @deprecated Use the `priceExact` twin; float money will be removed in 2.0.
    pub price: f64,
    /// Exact base-10 price, straight from the engine's `Decimal`. Prefer this field for money.
    pub price_exact: String,
    pub currency: String,
    pub payment_method_id: Option<String>,
    pub started_at: String,
    pub current_period_start: String,
    pub current_period_end: String,
    pub next_billing_date: Option<String>,
    pub trial_ends_at: Option<String>,
    pub paused_at: Option<String>,
    pub resume_at: Option<String>,
    pub cancelled_at: Option<String>,
    pub ends_at: Option<String>,
    pub billing_cycle_count: i32,
    pub failed_payment_attempts: i32,
    pub discount_percent: Option<f64>,
    /// @deprecated Use the `discountAmountExact` twin; float money will be removed in 2.0.
    pub discount_amount: Option<f64>,
    /// Exact base-10 discount amount, straight from the engine's `Decimal`. Prefer this field for money.
    pub discount_amount_exact: Option<String>,
    pub coupon_code: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl TryFrom<stateset_core::Subscription> for SubscriptionOutput {
    type Error = Error;

    fn try_from(s: stateset_core::Subscription) -> Result<Self> {
        let (price, price_exact) = money_pair(s.price, "subscription price")?;
        let (discount_amount, discount_amount_exact) =
            optional_money_pair(s.discount_amount, "subscription discount amount")?;
        Ok(Self {
            id: s.id.to_string(),
            subscription_number: s.subscription_number,
            customer_id: s.customer_id.to_string(),
            plan_id: s.plan_id.to_string(),
            plan_name: s.plan_name,
            status: format!("{}", s.status),
            billing_interval: format!("{}", s.billing_interval),
            custom_interval_days: s.custom_interval_days,
            price,
            price_exact,
            currency: s.currency.to_string(),
            payment_method_id: s.payment_method_id,
            started_at: s.started_at.to_rfc3339(),
            current_period_start: s.current_period_start.to_rfc3339(),
            current_period_end: s.current_period_end.to_rfc3339(),
            next_billing_date: s.next_billing_date.map(|d| d.to_rfc3339()),
            trial_ends_at: s.trial_ends_at.map(|d| d.to_rfc3339()),
            paused_at: s.paused_at.map(|d| d.to_rfc3339()),
            resume_at: s.resume_at.map(|d| d.to_rfc3339()),
            cancelled_at: s.cancelled_at.map(|d| d.to_rfc3339()),
            ends_at: s.ends_at.map(|d| d.to_rfc3339()),
            billing_cycle_count: s.billing_cycle_count,
            failed_payment_attempts: s.failed_payment_attempts,
            discount_percent: optional_to_f64_checked(
                s.discount_percent,
                "subscription discount percent",
            )?,
            discount_amount,
            discount_amount_exact,
            coupon_code: s.coupon_code,
            created_at: s.created_at.to_rfc3339(),
            updated_at: s.updated_at.to_rfc3339(),
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct PauseSubscriptionInput {
    pub reason: Option<String>,
    pub resume_at: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct CancelSubscriptionInput {
    pub reason: Option<String>,
    pub immediate: Option<bool>,
    pub feedback: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct SkipBillingCycleInput {
    pub reason: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct BillingCycleFilterInput {
    pub subscription_id: Option<String>,
    #[napi(ts_type = "BillingCycleStatus")]
    pub status: Option<String>,
    pub from_date: Option<String>,
    pub to_date: Option<String>,
    pub limit: Option<i32>,
    pub offset: Option<i32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct BillingCycleOutput {
    pub id: String,
    pub subscription_id: String,
    pub cycle_number: i32,
    #[napi(ts_type = "BillingCycleStatus")]
    pub status: String,
    pub period_start: String,
    pub period_end: String,
    /// @deprecated Use the `subtotalExact` twin; float money will be removed in 2.0.
    pub subtotal: f64,
    /// Exact base-10 subtotal, straight from the engine's `Decimal`. Prefer this field for money.
    pub subtotal_exact: String,
    /// @deprecated Use the `discountExact` twin; float money will be removed in 2.0.
    pub discount: f64,
    /// Exact base-10 discount, straight from the engine's `Decimal`. Prefer this field for money.
    pub discount_exact: String,
    /// @deprecated Use the `taxExact` twin; float money will be removed in 2.0.
    pub tax: f64,
    /// Exact base-10 tax, straight from the engine's `Decimal`. Prefer this field for money.
    pub tax_exact: String,
    /// @deprecated Use the `totalExact` twin; float money will be removed in 2.0.
    pub total: f64,
    /// Exact base-10 total, straight from the engine's `Decimal`. Prefer this field for money.
    pub total_exact: String,
    pub currency: String,
    pub payment_id: Option<String>,
    pub billed_at: Option<String>,
    pub failure_reason: Option<String>,
    pub retry_count: i32,
    pub created_at: String,
    pub updated_at: String,
}

impl TryFrom<stateset_core::BillingCycle> for BillingCycleOutput {
    type Error = Error;

    fn try_from(b: stateset_core::BillingCycle) -> Result<Self> {
        let (subtotal, subtotal_exact) = money_pair(b.subtotal, "billing cycle subtotal")?;
        let (discount, discount_exact) = money_pair(b.discount, "billing cycle discount")?;
        let (tax, tax_exact) = money_pair(b.tax, "billing cycle tax")?;
        let (total, total_exact) = money_pair(b.total, "billing cycle total")?;
        Ok(Self {
            id: b.id.to_string(),
            subscription_id: b.subscription_id.to_string(),
            cycle_number: b.cycle_number,
            status: format!("{:?}", b.status).to_lowercase(),
            period_start: b.period_start.to_rfc3339(),
            period_end: b.period_end.to_rfc3339(),
            subtotal,
            subtotal_exact,
            discount,
            discount_exact,
            tax,
            tax_exact,
            total,
            total_exact,
            currency: b.currency.to_string(),
            payment_id: b.payment_id,
            billed_at: b.billed_at.map(|d| d.to_rfc3339()),
            failure_reason: b.failure_reason,
            retry_count: b.retry_count,
            created_at: b.created_at.to_rfc3339(),
            updated_at: b.updated_at.to_rfc3339(),
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct SubscriptionEventOutput {
    pub id: String,
    pub subscription_id: String,
    #[napi(ts_type = "SubscriptionEventType")]
    pub event_type: String,
    pub description: String,
    pub data: Option<String>,
    pub triggered_by: Option<String>,
    pub created_at: String,
}

impl From<stateset_core::SubscriptionEvent> for SubscriptionEventOutput {
    fn from(e: stateset_core::SubscriptionEvent) -> Self {
        Self {
            id: e.id.to_string(),
            subscription_id: e.subscription_id.to_string(),
            event_type: format!("{:?}", e.event_type).to_lowercase(),
            description: e.description,
            data: e.data.map(|d| serde_json::to_string(&d).unwrap_or_default()),
            triggered_by: e.triggered_by,
            created_at: e.created_at.to_rfc3339(),
        }
    }
}

pub(crate) fn parse_billing_interval(s: &str) -> Result<stateset_core::BillingInterval> {
    match s.to_lowercase().as_str() {
        "weekly" => Ok(stateset_core::BillingInterval::Weekly),
        "biweekly" => Ok(stateset_core::BillingInterval::Biweekly),
        "monthly" => Ok(stateset_core::BillingInterval::Monthly),
        "bimonthly" => Ok(stateset_core::BillingInterval::Bimonthly),
        "quarterly" => Ok(stateset_core::BillingInterval::Quarterly),
        "semiannual" => Ok(stateset_core::BillingInterval::Semiannual),
        "annual" => Ok(stateset_core::BillingInterval::Annual),
        "custom" => Ok(stateset_core::BillingInterval::Custom),
        _ => Err(coded(ErrCode::Validation, format!("Invalid billing interval: {s}"))),
    }
}

pub(crate) fn parse_plan_status(s: &str) -> Result<stateset_core::PlanStatus> {
    match s.to_lowercase().as_str() {
        "draft" => Ok(stateset_core::PlanStatus::Draft),
        "active" => Ok(stateset_core::PlanStatus::Active),
        "archived" => Ok(stateset_core::PlanStatus::Archived),
        _ => Err(coded(ErrCode::Validation, format!("Invalid plan status: {s}"))),
    }
}

pub(crate) fn parse_subscription_status(s: &str) -> Result<stateset_core::SubscriptionStatus> {
    match s.to_lowercase().as_str() {
        "pending" => Ok(stateset_core::SubscriptionStatus::Pending),
        "trial" => Ok(stateset_core::SubscriptionStatus::Trial),
        "active" => Ok(stateset_core::SubscriptionStatus::Active),
        "paused" => Ok(stateset_core::SubscriptionStatus::Paused),
        "past_due" => Ok(stateset_core::SubscriptionStatus::PastDue),
        "cancelled" => Ok(stateset_core::SubscriptionStatus::Cancelled),
        "expired" => Ok(stateset_core::SubscriptionStatus::Expired),
        _ => Err(coded(ErrCode::Validation, format!("Invalid subscription status: {s}"))),
    }
}

pub(crate) fn parse_billing_cycle_status(s: &str) -> Result<stateset_core::BillingCycleStatus> {
    match s.to_lowercase().as_str() {
        "scheduled" => Ok(stateset_core::BillingCycleStatus::Scheduled),
        "processing" => Ok(stateset_core::BillingCycleStatus::Processing),
        "paid" => Ok(stateset_core::BillingCycleStatus::Paid),
        "failed" => Ok(stateset_core::BillingCycleStatus::Failed),
        "skipped" => Ok(stateset_core::BillingCycleStatus::Skipped),
        "refunded" => Ok(stateset_core::BillingCycleStatus::Refunded),
        "voided" => Ok(stateset_core::BillingCycleStatus::Voided),
        _ => Err(coded(ErrCode::Validation, format!("Invalid billing cycle status: {s}"))),
    }
}

#[napi]
pub struct Subscriptions {
    pub(crate) commerce: Handle,
}

#[napi]
impl Subscriptions {
    // ========================================================================
    // Subscription Plans
    // ========================================================================

    /// Create a new subscription plan
    #[napi]
    pub async fn create_plan(
        &self,
        input: CreateSubscriptionPlanInput,
    ) -> Result<SubscriptionPlanOutput> {
        let commerce = self.commerce.get()?;
        let billing_interval = parse_billing_interval(&input.billing_interval)?;

        let plan = commerce
            .subscriptions()
            .create_plan(stateset_core::CreateSubscriptionPlan {
                name: input.name,
                description: input.description,
                code: input.code,
                billing_interval,
                custom_interval_days: input.custom_interval_days,
                price: decimal_from_f64(input.price, "subscription plan price")?,
                setup_fee: optional_decimal_from_f64(
                    input.setup_fee,
                    "subscription plan setup fee",
                )?,
                currency: parse_optional_currency(input.currency)?,
                trial_days: input.trial_days,
                trial_requires_payment_method: input.trial_requires_payment_method,
                min_cycles: input.min_cycles,
                max_cycles: input.max_cycles,
                discount_percent: optional_decimal_from_f64(
                    input.discount_percent,
                    "subscription plan discount percent",
                )?,
                discount_amount: optional_decimal_from_f64(
                    input.discount_amount,
                    "subscription plan discount amount",
                )?,
                items: None,
                metadata: None,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create plan", e))?;

        convert_output(plan)
    }

    /// Get a subscription plan by ID
    #[napi]
    pub async fn get_plan(&self, id: String) -> Result<Option<SubscriptionPlanOutput>> {
        let commerce = self.commerce.get()?;
        let uuid =
            uuid::Uuid::parse_str(&id).map_err(|e| wrap(ErrCode::Validation, "Invalid UUID", e))?;

        let plan = commerce
            .subscriptions()
            .get_plan(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get plan", e))?;

        convert_optional_output(plan)
    }

    /// Get a subscription plan by code
    #[napi]
    pub async fn get_plan_by_code(&self, code: String) -> Result<Option<SubscriptionPlanOutput>> {
        let commerce = self.commerce.get()?;

        let plan = commerce
            .subscriptions()
            .get_plan_by_code(&code)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get plan", e))?;

        convert_optional_output(plan)
    }

    /// List subscription plans
    #[napi]
    pub async fn list_plans(
        &self,
        filter: Option<SubscriptionPlanFilterInput>,
    ) -> Result<Vec<SubscriptionPlanOutput>> {
        let commerce = self.commerce.get()?;
        let f = filter.unwrap_or_default();

        let plans = commerce
            .subscriptions()
            .list_plans(stateset_core::SubscriptionPlanFilter {
                status: f.status.as_deref().map(parse_plan_status).transpose()?,
                billing_interval: f
                    .billing_interval
                    .as_deref()
                    .map(parse_billing_interval)
                    .transpose()?,
                search: f.search,
                limit: f.limit.map(|v| v as u32),
                offset: f.offset.map(|v| v as u32),
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list plans", e))?;

        convert_outputs(plans)
    }

    /// Update a subscription plan
    #[napi]
    pub async fn update_plan(
        &self,
        id: String,
        input: UpdateSubscriptionPlanInput,
    ) -> Result<SubscriptionPlanOutput> {
        let commerce = self.commerce.get()?;
        let uuid =
            uuid::Uuid::parse_str(&id).map_err(|e| wrap(ErrCode::Validation, "Invalid UUID", e))?;

        let plan = commerce
            .subscriptions()
            .update_plan(
                uuid,
                stateset_core::UpdateSubscriptionPlan {
                    name: input.name,
                    description: input.description,
                    status: None,
                    price: optional_decimal_from_f64(input.price, "subscription plan price")?,
                    setup_fee: optional_decimal_from_f64(
                        input.setup_fee,
                        "subscription plan setup fee",
                    )?,
                    trial_days: input.trial_days,
                    trial_requires_payment_method: input.trial_requires_payment_method,
                    min_cycles: input.min_cycles,
                    max_cycles: input.max_cycles,
                    discount_percent: optional_decimal_from_f64(
                        input.discount_percent,
                        "subscription plan discount percent",
                    )?,
                    discount_amount: optional_decimal_from_f64(
                        input.discount_amount,
                        "subscription plan discount amount",
                    )?,
                    metadata: None,
                },
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to update plan", e))?;

        convert_output(plan)
    }

    /// Activate a subscription plan
    #[napi]
    pub async fn activate_plan(&self, id: String) -> Result<SubscriptionPlanOutput> {
        let commerce = self.commerce.get()?;
        let uuid =
            uuid::Uuid::parse_str(&id).map_err(|e| wrap(ErrCode::Validation, "Invalid UUID", e))?;

        let plan = commerce
            .subscriptions()
            .activate_plan(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to activate plan", e))?;

        convert_output(plan)
    }

    /// Archive a subscription plan
    #[napi]
    pub async fn archive_plan(&self, id: String) -> Result<SubscriptionPlanOutput> {
        let commerce = self.commerce.get()?;
        let uuid =
            uuid::Uuid::parse_str(&id).map_err(|e| wrap(ErrCode::Validation, "Invalid UUID", e))?;

        let plan = commerce
            .subscriptions()
            .archive_plan(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to archive plan", e))?;

        convert_output(plan)
    }

    // ========================================================================
    // Subscriptions
    // ========================================================================

    /// Create a subscription for a customer
    #[napi]
    pub async fn subscribe(&self, input: CreateSubscriptionInput) -> Result<SubscriptionOutput> {
        let commerce = self.commerce.get()?;
        let customer_id = uuid::Uuid::parse_str(&input.customer_id)
            .map_err(|e| wrap(ErrCode::Validation, "Invalid customer UUID", e))?;
        let plan_id = uuid::Uuid::parse_str(&input.plan_id)
            .map_err(|e| wrap(ErrCode::Validation, "Invalid plan UUID", e))?;

        let start_date = parse_optional_datetime(input.start_date, "start date")?;

        let subscription = commerce
            .subscriptions()
            .subscribe(stateset_core::CreateSubscription {
                customer_id: customer_id.into(),
                plan_id,
                payment_method_id: input.payment_method_id,
                skip_trial: input.skip_trial,
                price: optional_decimal_from_f64(input.price, "subscription price")?,
                coupon_code: input.coupon_code,
                start_date,
                items: None,
                shipping_address: None,
                billing_address: None,
                metadata: None,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create subscription", e))?;

        convert_output(subscription)
    }

    /// Get a subscription by ID
    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<SubscriptionOutput>> {
        let commerce = self.commerce.get()?;
        let uuid =
            uuid::Uuid::parse_str(&id).map_err(|e| wrap(ErrCode::Validation, "Invalid UUID", e))?;

        let subscription = commerce
            .subscriptions()
            .get(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get subscription", e))?;

        convert_optional_output(subscription)
    }

    /// Get a subscription by number
    #[napi]
    pub async fn get_by_number(&self, number: String) -> Result<Option<SubscriptionOutput>> {
        let commerce = self.commerce.get()?;

        let subscription = commerce
            .subscriptions()
            .get_by_number(&number)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get subscription", e))?;

        convert_optional_output(subscription)
    }

    /// List subscriptions
    #[napi]
    pub async fn list(
        &self,
        filter: Option<SubscriptionFilterInput>,
    ) -> Result<Vec<SubscriptionOutput>> {
        let commerce = self.commerce.get()?;
        let f = filter.unwrap_or_default();

        let customer_id =
            parse_optional_id::<uuid::Uuid>(f.customer_id, "customer")?.map(CustomerId::from);
        let plan_id = parse_optional_id::<uuid::Uuid>(f.plan_id, "plan")?;
        let from_date = parse_optional_datetime(f.from_date, "from date")?;
        let to_date = parse_optional_datetime(f.to_date, "to date")?;

        let subscriptions = commerce
            .subscriptions()
            .list(stateset_core::SubscriptionFilter {
                customer_id,
                plan_id,
                status: f.status.as_deref().map(parse_subscription_status).transpose()?,
                from_date,
                to_date,
                search: f.search,
                limit: f.limit.map(|v| v as u32),
                offset: f.offset.map(|v| v as u32),
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list subscriptions", e))?;

        convert_outputs(subscriptions)
    }

    /// Update a subscription
    #[napi]
    pub async fn update(
        &self,
        id: String,
        input: UpdateSubscriptionInput,
    ) -> Result<SubscriptionOutput> {
        let commerce = self.commerce.get()?;
        let uuid =
            uuid::Uuid::parse_str(&id).map_err(|e| wrap(ErrCode::Validation, "Invalid UUID", e))?;

        let next_billing_date =
            parse_optional_datetime(input.next_billing_date, "next billing date")?;

        let subscription = commerce
            .subscriptions()
            .update(
                uuid.into(),
                stateset_core::UpdateSubscription {
                    status: input.status.as_deref().map(parse_subscription_status).transpose()?,
                    price: optional_decimal_from_f64(input.price, "subscription price")?,
                    payment_method_id: input.payment_method_id,
                    next_billing_date,
                    discount_percent: optional_decimal_from_f64(
                        input.discount_percent,
                        "subscription discount percent",
                    )?,
                    discount_amount: optional_decimal_from_f64(
                        input.discount_amount,
                        "subscription discount amount",
                    )?,
                    coupon_code: input.coupon_code,
                    shipping_address: None,
                    billing_address: None,
                    metadata: None,
                },
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to update subscription", e))?;

        convert_output(subscription)
    }

    /// Pause a subscription
    #[napi]
    pub async fn pause(
        &self,
        id: String,
        input: Option<PauseSubscriptionInput>,
    ) -> Result<SubscriptionOutput> {
        let commerce = self.commerce.get()?;
        let uuid =
            uuid::Uuid::parse_str(&id).map_err(|e| wrap(ErrCode::Validation, "Invalid UUID", e))?;

        let i = input.unwrap_or_default();
        let resume_at = parse_optional_datetime(i.resume_at, "resume at")?;

        let subscription = commerce
            .subscriptions()
            .pause(uuid.into(), stateset_core::PauseSubscription { reason: i.reason, resume_at })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to pause subscription", e))?;

        convert_output(subscription)
    }

    /// Resume a paused subscription
    #[napi]
    pub async fn resume(&self, id: String) -> Result<SubscriptionOutput> {
        let commerce = self.commerce.get()?;
        let uuid =
            uuid::Uuid::parse_str(&id).map_err(|e| wrap(ErrCode::Validation, "Invalid UUID", e))?;

        let subscription = commerce
            .subscriptions()
            .resume(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to resume subscription", e))?;

        convert_output(subscription)
    }

    /// Cancel a subscription
    #[napi]
    pub async fn cancel(
        &self,
        id: String,
        input: Option<CancelSubscriptionInput>,
    ) -> Result<SubscriptionOutput> {
        let commerce = self.commerce.get()?;
        let uuid =
            uuid::Uuid::parse_str(&id).map_err(|e| wrap(ErrCode::Validation, "Invalid UUID", e))?;

        let i = input.unwrap_or_default();

        let subscription = commerce
            .subscriptions()
            .cancel(
                uuid.into(),
                stateset_core::CancelSubscription {
                    reason: i.reason,
                    immediate: i.immediate,
                    feedback: i.feedback,
                },
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to cancel subscription", e))?;

        convert_output(subscription)
    }

    /// Skip the next billing cycle
    #[napi]
    pub async fn skip_billing(
        &self,
        id: String,
        input: Option<SkipBillingCycleInput>,
    ) -> Result<SubscriptionOutput> {
        let commerce = self.commerce.get()?;
        let uuid =
            uuid::Uuid::parse_str(&id).map_err(|e| wrap(ErrCode::Validation, "Invalid UUID", e))?;

        let i = input.unwrap_or_default();

        let subscription = commerce
            .subscriptions()
            .skip_next_cycle(uuid.into(), stateset_core::SkipBillingCycle { reason: i.reason })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to skip billing", e))?;

        convert_output(subscription)
    }

    // ========================================================================
    // Billing Cycles
    // ========================================================================

    /// List billing cycles for a subscription
    #[napi]
    pub async fn list_billing_cycles(
        &self,
        filter: Option<BillingCycleFilterInput>,
    ) -> Result<Vec<BillingCycleOutput>> {
        let commerce = self.commerce.get()?;
        let f = filter.unwrap_or_default();

        let subscription_id = parse_optional_id::<uuid::Uuid>(f.subscription_id, "subscription")?
            .map(SubscriptionId::from);
        let from_date = parse_optional_datetime(f.from_date, "from date")?;
        let to_date = parse_optional_datetime(f.to_date, "to date")?;

        let cycles = commerce
            .subscriptions()
            .list_billing_cycles(stateset_core::BillingCycleFilter {
                subscription_id,
                status: f.status.as_deref().map(parse_billing_cycle_status).transpose()?,
                from_date,
                to_date,
                limit: f.limit.map(|v| v as u32),
                offset: f.offset.map(|v| v as u32),
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list billing cycles", e))?;

        convert_outputs(cycles)
    }

    /// Get a billing cycle by ID
    #[napi]
    pub async fn get_billing_cycle(&self, id: String) -> Result<Option<BillingCycleOutput>> {
        let commerce = self.commerce.get()?;
        let uuid =
            uuid::Uuid::parse_str(&id).map_err(|e| wrap(ErrCode::Validation, "Invalid UUID", e))?;

        let cycle = commerce
            .subscriptions()
            .get_billing_cycle(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get billing cycle", e))?;

        convert_optional_output(cycle)
    }

    // ========================================================================
    // Events
    // ========================================================================

    /// Get events for a subscription
    #[napi]
    pub async fn get_events(
        &self,
        subscription_id: String,
    ) -> Result<Vec<SubscriptionEventOutput>> {
        let commerce = self.commerce.get()?;
        let uuid = uuid::Uuid::parse_str(&subscription_id)
            .map_err(|e| wrap(ErrCode::Validation, "Invalid UUID", e))?;

        let events = commerce
            .subscriptions()
            .get_events(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get events", e))?;

        Ok(events.into_iter().map(|e| e.into()).collect())
    }
}
