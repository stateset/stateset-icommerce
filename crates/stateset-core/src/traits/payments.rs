//! Payment, subscription, currency, tax, fraud, payment-obligation, and prepayment repositories.

use super::*;

/// Payment repository trait
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait PaymentRepository: Send + Sync {
    /// Create a new payment
    fn create(&self, input: CreatePayment) -> Result<Payment>;

    /// Get payment by ID
    fn get(&self, id: PaymentId) -> Result<Option<Payment>>;

    /// Get payment by payment number
    fn get_by_number(&self, payment_number: &str) -> Result<Option<Payment>>;

    /// Get payment by external ID (e.g., Stripe payment intent)
    fn get_by_external_id(&self, external_id: &str) -> Result<Option<Payment>>;

    /// Update a payment
    fn update(&self, id: PaymentId, input: UpdatePayment) -> Result<Payment>;

    /// List payments with filter
    fn list(&self, filter: PaymentFilter) -> Result<Vec<Payment>>;

    /// Get payments for an order
    fn for_order(&self, order_id: OrderId) -> Result<Vec<Payment>>;

    /// Get payments for an invoice
    fn for_invoice(&self, invoice_id: InvoiceId) -> Result<Vec<Payment>>;

    // Status transitions
    /// Mark payment as processing
    fn mark_processing(&self, id: PaymentId) -> Result<Payment>;

    /// Mark payment as completed (paid)
    fn mark_completed(&self, id: PaymentId) -> Result<Payment>;

    /// Mark payment as failed
    fn mark_failed(&self, id: PaymentId, reason: &str, code: Option<&str>) -> Result<Payment>;

    /// Cancel payment
    fn cancel(&self, id: PaymentId) -> Result<Payment>;

    // Refund operations
    /// Create a refund for a payment
    fn create_refund(&self, input: CreateRefund) -> Result<Refund>;

    /// Get refund by ID
    fn get_refund(&self, id: Uuid) -> Result<Option<Refund>>;

    /// Get refunds for a payment
    fn get_refunds(&self, payment_id: PaymentId) -> Result<Vec<Refund>>;

    /// Process refund (mark as completed)
    fn complete_refund(&self, id: Uuid) -> Result<Refund>;

    /// Fail refund
    fn fail_refund(&self, id: Uuid, reason: &str) -> Result<Refund>;

    // Payment method operations
    /// Create a payment method for a customer
    fn create_payment_method(&self, input: CreatePaymentMethod) -> Result<PaymentMethod>;

    /// Get payment methods for a customer
    fn get_payment_methods(&self, customer_id: CustomerId) -> Result<Vec<PaymentMethod>>;

    /// Delete a payment method
    fn delete_payment_method(&self, id: Uuid) -> Result<()>;

    /// Set default payment method
    fn set_default_payment_method(&self, customer_id: CustomerId, method_id: Uuid) -> Result<()>;

    /// Count payments matching filter
    fn count(&self, filter: PaymentFilter) -> Result<u64>;

    // === Batch Operations ===

    /// Create multiple payments - partial success allowed
    fn create_batch(&self, inputs: Vec<CreatePayment>) -> Result<BatchResult<Payment>>;

    /// Create multiple payments - atomic (all-or-nothing)
    fn create_batch_atomic(&self, inputs: Vec<CreatePayment>) -> Result<Vec<Payment>>;

    /// Update multiple payments - partial success allowed
    fn update_batch(
        &self,
        updates: Vec<(PaymentId, UpdatePayment)>,
    ) -> Result<BatchResult<Payment>>;

    /// Update multiple payments - atomic (all-or-nothing)
    fn update_batch_atomic(&self, updates: Vec<(PaymentId, UpdatePayment)>)
    -> Result<Vec<Payment>>;

    /// Delete multiple payments - partial success allowed
    fn delete_batch(&self, ids: Vec<PaymentId>) -> Result<BatchResult<Uuid>>;

    /// Delete multiple payments - atomic (all-or-nothing)
    fn delete_batch_atomic(&self, ids: Vec<PaymentId>) -> Result<()>;

    /// Get multiple payments by ID
    fn get_batch(&self, ids: Vec<PaymentId>) -> Result<Vec<Payment>>;
}

/// Subscriptions repository trait
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait SubscriptionRepository: Send + Sync {
    /// Create a subscription plan
    fn create_plan(&self, input: CreateSubscriptionPlan) -> Result<SubscriptionPlan>;
    /// Get a subscription plan by ID
    fn get_plan(&self, id: Uuid) -> Result<Option<SubscriptionPlan>>;
    /// Get a subscription plan by code
    fn get_plan_by_code(&self, code: &str) -> Result<Option<SubscriptionPlan>>;
    /// List subscription plans matching a filter
    fn list_plans(&self, filter: SubscriptionPlanFilter) -> Result<Vec<SubscriptionPlan>>;
    /// Update a subscription plan
    fn update_plan(&self, id: Uuid, input: UpdateSubscriptionPlan) -> Result<SubscriptionPlan>;
    /// Activate a subscription plan
    fn activate_plan(&self, id: Uuid) -> Result<SubscriptionPlan>;
    /// Archive a subscription plan
    fn archive_plan(&self, id: Uuid) -> Result<SubscriptionPlan>;

    /// Create a subscription
    fn create_subscription(&self, input: CreateSubscription) -> Result<Subscription>;
    /// Get a subscription by ID
    fn get_subscription(&self, id: SubscriptionId) -> Result<Option<Subscription>>;
    /// Get a subscription by number
    fn get_subscription_by_number(&self, number: &str) -> Result<Option<Subscription>>;
    /// List subscriptions matching a filter
    fn list_subscriptions(&self, filter: SubscriptionFilter) -> Result<Vec<Subscription>>;
    /// Update a subscription
    fn update_subscription(
        &self,
        id: SubscriptionId,
        input: UpdateSubscription,
    ) -> Result<Subscription>;
    /// Cancel a subscription
    fn cancel_subscription(
        &self,
        id: SubscriptionId,
        input: CancelSubscription,
    ) -> Result<Subscription>;
    /// Pause a subscription
    fn pause_subscription(
        &self,
        id: SubscriptionId,
        input: PauseSubscription,
    ) -> Result<Subscription>;
    /// Resume a paused subscription
    fn resume_subscription(&self, id: SubscriptionId) -> Result<Subscription>;

    /// Create a billing cycle
    fn create_billing_cycle(&self, input: CreateBillingCycle) -> Result<BillingCycle>;
    /// Get a billing cycle by ID
    fn get_billing_cycle(&self, id: Uuid) -> Result<Option<BillingCycle>>;
    /// List billing cycles matching a filter
    fn list_billing_cycles(&self, filter: BillingCycleFilter) -> Result<Vec<BillingCycle>>;
    /// Update the status of a billing cycle
    fn update_billing_cycle_status(
        &self,
        id: Uuid,
        status: BillingCycleStatus,
    ) -> Result<BillingCycle>;
    /// Skip a billing cycle
    fn skip_billing_cycle(
        &self,
        id: SubscriptionId,
        input: SkipBillingCycle,
    ) -> Result<Subscription>;

    /// Record a subscription event
    fn record_event(
        &self,
        subscription_id: SubscriptionId,
        event_type: SubscriptionEventType,
        notes: Option<String>,
    ) -> Result<SubscriptionEvent>;
    /// Get all events for a subscription
    fn get_subscription_events(
        &self,
        subscription_id: SubscriptionId,
    ) -> Result<Vec<SubscriptionEvent>>;
}

/// Currency and exchange rate repository trait
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait CurrencyRepository: Send + Sync {
    /// Get current exchange rate between two currencies
    fn get_rate(&self, from: Currency, to: Currency) -> Result<Option<ExchangeRate>>;

    /// Get all exchange rates for a base currency
    fn get_rates_for(&self, base: Currency) -> Result<Vec<ExchangeRate>>;

    /// List all exchange rates with optional filter
    fn list_rates(&self, filter: ExchangeRateFilter) -> Result<Vec<ExchangeRate>>;

    /// Set an exchange rate
    fn set_rate(&self, input: SetExchangeRate) -> Result<ExchangeRate>;

    /// Set multiple exchange rates at once
    fn set_rates(&self, rates: Vec<SetExchangeRate>) -> Result<Vec<ExchangeRate>>;

    /// Delete an exchange rate
    fn delete_rate(&self, id: Uuid) -> Result<()>;

    /// Convert money between currencies
    fn convert(&self, input: ConvertCurrency) -> Result<ConversionResult>;

    /// Get store currency settings
    fn get_settings(&self) -> Result<StoreCurrencySettings>;

    /// Update store currency settings
    fn update_settings(&self, settings: StoreCurrencySettings) -> Result<StoreCurrencySettings>;

    // === Batch Operations ===

    /// Set multiple exchange rates - atomic (all-or-nothing)
    /// Note: `set_rates` already exists as a partial-success batch operation
    fn set_rates_atomic(&self, rates: Vec<SetExchangeRate>) -> Result<Vec<ExchangeRate>>;

    /// Delete multiple exchange rates - partial success allowed
    fn delete_rates_batch(&self, ids: Vec<Uuid>) -> Result<BatchResult<Uuid>>;

    /// Delete multiple exchange rates - atomic (all-or-nothing)
    fn delete_rates_atomic(&self, ids: Vec<Uuid>) -> Result<()>;

    /// Get multiple exchange rates by currency pairs
    fn get_rates_batch(&self, pairs: Vec<(Currency, Currency)>) -> Result<Vec<ExchangeRate>>;
}

/// Tax repository trait
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait TaxRepository: Send + Sync {
    /// Create a tax jurisdiction
    fn create_jurisdiction(&self, input: CreateTaxJurisdiction) -> Result<TaxJurisdiction>;
    /// Get a tax jurisdiction by ID
    fn get_jurisdiction(&self, id: Uuid) -> Result<Option<TaxJurisdiction>>;
    /// Get a tax jurisdiction by code
    fn get_jurisdiction_by_code(&self, code: &str) -> Result<Option<TaxJurisdiction>>;
    /// List tax jurisdictions matching a filter
    fn list_jurisdictions(&self, filter: TaxJurisdictionFilter) -> Result<Vec<TaxJurisdiction>>;

    /// Create a tax rate
    fn create_rate(&self, input: CreateTaxRate) -> Result<TaxRate>;
    /// Get a tax rate by ID
    fn get_rate(&self, id: Uuid) -> Result<Option<TaxRate>>;
    /// List tax rates matching a filter
    fn list_rates(&self, filter: TaxRateFilter) -> Result<Vec<TaxRate>>;
    /// Get applicable tax rates for an address and category on a date
    fn get_rates_for_address(
        &self,
        address: &TaxAddress,
        category: ProductTaxCategory,
        date: chrono::NaiveDate,
    ) -> Result<Vec<TaxRate>>;

    /// Create a tax exemption
    fn create_exemption(&self, input: CreateTaxExemption) -> Result<TaxExemption>;
    /// Get a tax exemption by ID
    fn get_exemption(&self, id: Uuid) -> Result<Option<TaxExemption>>;
    /// Get all exemptions for a customer
    fn get_customer_exemptions(&self, customer_id: Uuid) -> Result<Vec<TaxExemption>>;

    /// Get tax settings
    fn get_settings(&self) -> Result<TaxSettings>;
    /// Update tax settings
    fn update_settings(&self, settings: TaxSettings) -> Result<TaxSettings>;

    /// Calculate tax for a request
    fn calculate_tax(&self, request: TaxCalculationRequest) -> Result<TaxCalculationResult>;
    /// Persist a tax calculation for audit/reporting
    fn save_calculation(
        &self,
        result: &TaxCalculationResult,
        order_id: Option<Uuid>,
        cart_id: Option<Uuid>,
        customer_id: Option<Uuid>,
        address: &TaxAddress,
        currency: &str,
    ) -> Result<()>;
}

/// Fraud detection repository trait.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait FraudRepository: Send + Sync {
    /// Create a fraud assessment for an order
    fn create_assessment(&self, input: CreateFraudAssessment) -> Result<FraudAssessment>;

    /// Get fraud assessment for an order
    fn get_assessment(&self, order_id: OrderId) -> Result<Option<FraudAssessment>>;

    /// List fraud assessments with filter
    fn list_assessments(&self, filter: FraudAssessmentFilter) -> Result<Vec<FraudAssessment>>;

    /// Update assessment after manual review
    fn review_assessment(
        &self,
        order_id: OrderId,
        decision: FraudDecision,
        reviewer: String,
        notes: Option<String>,
    ) -> Result<FraudAssessment>;

    /// Create a fraud rule
    fn create_rule(&self, input: CreateFraudRule) -> Result<FraudRule>;

    /// Get a fraud rule by ID
    fn get_rule(&self, id: FraudRuleId) -> Result<Option<FraudRule>>;

    /// Update a fraud rule
    fn update_rule(&self, id: FraudRuleId, input: UpdateFraudRule) -> Result<FraudRule>;

    /// List fraud rules with filter
    fn list_rules(&self, filter: FraudRuleFilter) -> Result<Vec<FraudRule>>;

    /// Delete a fraud rule
    fn delete_rule(&self, id: FraudRuleId) -> Result<()>;

    /// Get all enabled rules
    fn get_active_rules(&self) -> Result<Vec<FraudRule>>;
}

/// Payment obligation repository trait.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait PaymentObligationRepository: Send + Sync {
    /// Create a new payment obligation.
    fn create(&self, input: CreatePaymentObligation) -> Result<PaymentObligation>;

    /// Get a payment obligation by ID.
    fn get(&self, id: PaymentObligationId) -> Result<Option<PaymentObligation>>;

    /// List payment obligations with filter.
    fn list(&self, filter: PaymentObligationFilter) -> Result<Vec<PaymentObligation>>;

    /// Record a payment against an obligation, advancing its status.
    fn record_payment(
        &self,
        id: PaymentObligationId,
        amount: rust_decimal::Decimal,
    ) -> Result<PaymentObligation>;

    /// Set the obligation status explicitly (e.g. schedule or cancel).
    fn set_status(
        &self,
        id: PaymentObligationId,
        status: PaymentObligationStatus,
    ) -> Result<PaymentObligation>;

    /// Link an AP bill to an obligation (idempotent).
    fn link_bill(&self, id: PaymentObligationId, bill_id: uuid::Uuid) -> Result<PaymentObligation>;

    /// Aggregate dashboard summary as of the given date.
    fn dashboard(&self, today: chrono::NaiveDate) -> Result<PaymentObligationDashboard>;
}

/// Prepayment repository trait.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait PrepaymentRepository: Send + Sync {
    /// Create a new prepayment.
    fn create(&self, input: CreatePrepayment) -> Result<Prepayment>;

    /// Get a prepayment by ID.
    fn get(&self, id: PrepaymentId) -> Result<Option<Prepayment>>;

    /// List prepayments with filter.
    fn list(&self, filter: PrepaymentFilter) -> Result<Vec<Prepayment>>;

    /// Apply a prepayment against a bill or payment obligation, drawing down
    /// the remaining balance and recording an application.
    fn apply(&self, id: PrepaymentId, input: ApplyPrepayment) -> Result<Prepayment>;

    /// List applications for a prepayment.
    fn list_applications(&self, id: PrepaymentId) -> Result<Vec<PrepaymentApplication>>;

    /// Reverse a previously-recorded application, restoring the balance.
    fn reverse_application(
        &self,
        id: PrepaymentId,
        application_id: PrepaymentApplicationId,
    ) -> Result<Prepayment>;

    /// Refund the remaining balance, closing the prepayment.
    fn refund(&self, id: PrepaymentId) -> Result<Prepayment>;
}
