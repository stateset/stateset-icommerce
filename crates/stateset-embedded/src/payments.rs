//! Payment operations for processing transactions and refunds
//!
//! # Example
//!
//! ```ignore
//! use stateset_embedded::{Commerce, CreatePayment, PaymentMethodType, OrderId};
//! use rust_decimal_macros::dec;
//!
//! let commerce = Commerce::new("./store.db")?;
//!
//! // Create a payment for an order
//! let payment = commerce.payments().create(CreatePayment {
//!     order_id: Some(OrderId::new()),
//!     payment_method: PaymentMethodType::CreditCard,
//!     amount: dec!(99.99),
//!     card_brand: Some(stateset_embedded::CardBrand::Visa),
//!     card_last4: Some("4242".into()),
//!     ..Default::default()
//! })?;
//!
//! // Mark payment as completed
//! let payment = commerce.payments().mark_completed(payment.id)?;
//!
//! // Process a refund
//! let refund = commerce.payments().create_refund(stateset_embedded::CreateRefund {
//!     payment_id: payment.id,
//!     amount: Some(dec!(25.00)),
//!     reason: Some("Partial refund - damaged item".into()),
//!     ..Default::default()
//! })?;
//! # Ok::<(), stateset_embedded::CommerceError>(())
//! ```

use crate::Database;
#[cfg(feature = "events")]
use crate::payment_providers::{PaymentProvider, ProviderCharge, ProviderDecision};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
#[cfg(feature = "events")]
use stateset_core::PaymentTransactionStatus;
use stateset_core::{
    CommerceError, CreatePayment, CreatePaymentMethod, CreateRefund, CustomerId, OrderId, Payment,
    PaymentError, PaymentFilter, PaymentId, PaymentMethod, Refund, Result, UpdatePayment, Validate,
};
use stateset_observability::Metrics;
use std::sync::Arc;
use uuid::Uuid;

/// Payment operations for transaction processing and refunds
pub struct Payments {
    db: Arc<dyn Database>,
    metrics: Metrics,
}

impl std::fmt::Debug for Payments {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Payments").finish_non_exhaustive()
    }
}

impl Payments {
    pub(crate) fn new(db: Arc<dyn Database>, metrics: Metrics) -> Self {
        Self { db, metrics }
    }

    /// Create a new payment
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CreatePayment, PaymentMethodType, CardBrand, OrderId, CurrencyCode};
    /// use rust_decimal_macros::dec;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// let payment = commerce.payments().create(CreatePayment {
    ///     order_id: Some(OrderId::new()),
    ///     payment_method: PaymentMethodType::CreditCard,
    ///     amount: dec!(149.99),
    ///     currency: Some(CurrencyCode::USD),
    ///     card_brand: Some(CardBrand::Visa),
    ///     card_last4: Some("4242".into()),
    ///     billing_email: Some("customer@example.com".into()),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    #[tracing::instrument(skip(self, input), fields(amount = %input.amount, method = ?input.payment_method))]
    pub fn create(&self, input: CreatePayment) -> Result<Payment> {
        tracing::info!("creating payment");
        // Reject a negative amount or malformed billing email before persisting.
        input.validate()?;
        self.db.payments().create(input)
    }

    /// Get a payment by ID
    pub fn get(&self, id: PaymentId) -> Result<Option<Payment>> {
        self.db.payments().get(id)
    }

    /// Get a payment by payment number (e.g., "PAY-20231215123456")
    pub fn get_by_number(&self, payment_number: &str) -> Result<Option<Payment>> {
        self.db.payments().get_by_number(payment_number)
    }

    /// Get a payment by external ID (e.g., Stripe payment intent ID)
    pub fn get_by_external_id(&self, external_id: &str) -> Result<Option<Payment>> {
        self.db.payments().get_by_external_id(external_id)
    }

    /// Update a payment
    pub fn update(&self, id: PaymentId, input: stateset_core::UpdatePayment) -> Result<Payment> {
        self.db.payments().update(id, input)
    }

    /// List payments with optional filtering
    pub fn list(&self, filter: PaymentFilter) -> Result<Vec<Payment>> {
        self.db.payments().list(filter)
    }

    /// Get all payments for an order
    pub fn for_order(&self, order_id: OrderId) -> Result<Vec<Payment>> {
        self.db.payments().for_order(order_id)
    }

    /// Get all payments for an invoice
    pub fn for_invoice(&self, invoice_id: Uuid) -> Result<Vec<Payment>> {
        self.db.payments().for_invoice(invoice_id.into())
    }

    /// Payments for an order that are still holding captured money: every
    /// payment in a capturing status (pending / processing / `requires_action` /
    /// completed / `partially_refunded` / disputed) whose amount exceeds what has
    /// been refunded. An empty result means nothing is outstanding and the
    /// order can be cancelled without stranding a capture.
    pub fn open_captures_for_order(&self, order_id: OrderId) -> Result<Vec<Payment>> {
        self.db.payments().open_captures_for_order(order_id)
    }

    /// Mark payment as processing
    pub fn mark_processing(&self, id: PaymentId) -> Result<Payment> {
        self.db.payments().mark_processing(id)
    }

    /// Mark payment as completed
    ///
    /// This records the payment timestamp and marks the transaction as successful.
    #[tracing::instrument(skip(self), fields(payment_id = %id))]
    pub fn mark_completed(&self, id: PaymentId) -> Result<Payment> {
        tracing::info!("marking payment as completed");
        let payment = self.db.payments().mark_completed(id)?;
        self.metrics.record_payment_completed(
            &payment.id.to_string(),
            payment.amount.to_f64().unwrap_or(0.0),
        );
        Ok(payment)
    }

    /// Mark payment as failed
    ///
    /// # Arguments
    ///
    /// * `id` - Payment ID
    /// * `reason` - Human-readable failure reason
    /// * `code` - Optional error code from payment processor
    pub fn mark_failed(&self, id: PaymentId, reason: &str, code: Option<&str>) -> Result<Payment> {
        self.db.payments().mark_failed(id, reason, code)
    }

    /// Cancel a payment
    pub fn cancel(&self, id: PaymentId) -> Result<Payment> {
        self.db.payments().cancel(id)
    }

    /// Hold funds through an upstream processor without moving them.
    ///
    /// This is the first half of real card capture: the processor places a
    /// hold (Stripe: a manually-captured `PaymentIntent` in `requires_capture`)
    /// and the engine records the upstream reference on the payment. A
    /// decline is persisted via `mark_failed`, never thrown away.
    #[cfg(feature = "events")]
    pub fn authorize_with_provider(
        &self,
        id: PaymentId,
        provider: &dyn PaymentProvider,
        payment_method: Option<String>,
    ) -> Result<ProviderDecision> {
        let payment = self
            .db
            .payments()
            .get(id)?
            .ok_or_else(|| CommerceError::Payment(PaymentError::NotFound(id.into())))?;
        // Refuse before going upstream: a hold placed for a payment the
        // engine can no longer move to `Processing` (failed, cancelled,
        // already captured) would be an orphaned authorization on the card.
        if !matches!(
            payment.status,
            PaymentTransactionStatus::Pending | PaymentTransactionStatus::RequiresAction
        ) {
            return Err(CommerceError::ValidationError(format!(
                "cannot authorize a payment in status {}; create a new payment to retry",
                payment.status
            )));
        }
        // Scope the key to the payment method: retrying the same card replays
        // the original decision, while retrying with a different card after a
        // decline is a new upstream request rather than an idempotency clash.
        let idempotency_key = format!(
            "authorize-{}-{}",
            payment.payment_number,
            payment_method.as_deref().unwrap_or("default")
        );
        let decision = provider.authorize(&ProviderCharge {
            amount: payment.amount,
            currency: payment.currency.to_string(),
            payment_method,
            idempotency_key,
            description: Some(format!(
                "order {}",
                payment.order_id.map(|o| o.to_string()).unwrap_or_default()
            )),
        })?;
        self.persist_provider_decision(id, &decision)?;
        Ok(decision)
    }

    /// Move previously held funds through an upstream processor.
    ///
    /// The second half of real card capture. `amount` must be `None` or the
    /// full payment amount: the payment row has no captured-amount column, so
    /// a partial capture would record money that never moved and is refused.
    /// The upstream reference recorded at authorize time is reused; when the
    /// payment has none yet (record-only row), `authorize_with_provider` runs
    /// first with `payment_method`, and capture only follows a hold —
    /// a 3-D Secure challenge, an immediate capture, or a decline is returned
    /// as-is for the caller to act on.
    #[cfg(feature = "events")]
    pub fn capture_with_provider(
        &self,
        id: PaymentId,
        provider: &dyn PaymentProvider,
        amount: Option<Decimal>,
        payment_method: Option<String>,
    ) -> Result<ProviderDecision> {
        let payment = self
            .db
            .payments()
            .get(id)?
            .ok_or_else(|| CommerceError::Payment(PaymentError::NotFound(id.into())))?;
        if amount.is_some_and(|amount| amount != payment.amount) {
            return Err(CommerceError::ValidationError(format!(
                "partial capture is not supported: capture amount must equal the payment amount {}",
                payment.amount
            )));
        }
        let reference = match payment.external_id.clone() {
            Some(reference) => reference,
            None => match self.authorize_with_provider(id, provider, payment_method)? {
                ProviderDecision::Authorized { reference } => reference,
                // Already settled, awaiting the customer, or refused: there is
                // no hold to capture, and the caller needs this outcome.
                other => return Ok(other),
            },
        };
        let decision = provider.capture(&reference, amount, &payment.currency.to_string())?;
        self.persist_provider_decision(id, &decision)?;
        Ok(decision)
    }

    /// Return captured funds through the same upstream processor.
    ///
    /// Records the engine-side refund row first (so the refunds invariant
    /// holds even if the upstream call fails), then completes it once the
    /// processor reports the refund succeeded, fails it with the processor's
    /// reason, or leaves it pending when the processor has not settled yet.
    #[cfg(feature = "events")]
    pub fn refund_with_provider(
        &self,
        payment_id: PaymentId,
        provider: &dyn PaymentProvider,
        amount: Option<Decimal>,
        reason: Option<String>,
    ) -> Result<ProviderDecision> {
        let payment = self
            .db
            .payments()
            .get(payment_id)?
            .ok_or_else(|| CommerceError::Payment(PaymentError::NotFound(payment_id.into())))?;
        let reference = payment.external_id.clone().ok_or_else(|| {
            CommerceError::ValidationError(
                "payment has no upstream reference to refund against".into(),
            )
        })?;
        let refund = self.db.payments().create_refund(CreateRefund {
            payment_id,
            amount,
            reason,
            ..Default::default()
        })?;
        // One key per engine refund row: partial refunds against the same
        // payment are distinct upstream operations, and a retry of this row
        // replays rather than double-refunding.
        let idempotency_key = format!("refund-{}", refund.id);
        match provider.refund(&reference, amount, &payment.currency.to_string(), &idempotency_key) {
            Ok(ProviderDecision::Declined { code, message }) => {
                self.db.payments().fail_refund(refund.id, &format!("{code}: {message}"))?;
                Ok(ProviderDecision::Declined { code, message })
            }
            // Accepted but not settled: leave the engine refund pending until
            // the processor confirms, so nothing reads as refunded early.
            Ok(pending @ ProviderDecision::Pending { .. }) => Ok(pending),
            Ok(decision) => {
                self.db.payments().complete_refund(refund.id)?;
                Ok(decision)
            }
            Err(e) => {
                self.db.payments().fail_refund(refund.id, &e.to_string())?;
                Err(e)
            }
        }
    }

    /// Persist one provider outcome onto the payment row.
    #[cfg(feature = "events")]
    fn persist_provider_decision(&self, id: PaymentId, decision: &ProviderDecision) -> Result<()> {
        match decision {
            ProviderDecision::Authorized { reference }
            | ProviderDecision::Captured { reference } => {
                self.db.payments().update(
                    id,
                    UpdatePayment { external_id: Some(reference.clone()), ..Default::default() },
                )?;
                if matches!(decision, ProviderDecision::Captured { .. }) {
                    self.mark_completed(id)?;
                } else {
                    self.mark_processing(id)?;
                }
            }
            ProviderDecision::RequiresAction { reference, .. }
            | ProviderDecision::Pending { reference } => {
                self.db.payments().update(
                    id,
                    UpdatePayment { external_id: Some(reference.clone()), ..Default::default() },
                )?;
            }
            ProviderDecision::Declined { code, message } => {
                self.mark_failed(id, message, Some(code.as_str()))?;
            }
        }
        Ok(())
    }

    /// Create a refund for a payment
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CreateRefund, PaymentId};
    /// use rust_decimal_macros::dec;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// // Full refund (omit amount for full refund)
    /// let refund = commerce.payments().create_refund(CreateRefund {
    ///     payment_id: PaymentId::new(),
    ///     reason: Some("Customer request".into()),
    ///     ..Default::default()
    /// })?;
    ///
    /// // Partial refund
    /// let refund = commerce.payments().create_refund(CreateRefund {
    ///     payment_id: PaymentId::new(),
    ///     amount: Some(dec!(50.00)),
    ///     reason: Some("Partial refund for damaged item".into()),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    #[tracing::instrument(skip(self, input), fields(payment_id = %input.payment_id))]
    pub fn create_refund(&self, input: CreateRefund) -> Result<Refund> {
        tracing::info!("creating refund");
        // Reject a nil payment reference or non-positive requested amount before
        // touching the database at all.
        input.validate()?;
        // Defense-in-depth: validate the refund against the payment's current
        // status and remaining refundable balance before delegating. The DB
        // backends enforce this too, but rejecting early keeps invalid refunds
        // out of the persistence layer entirely.
        let payment = self.db.payments().get(input.payment_id)?.ok_or(CommerceError::NotFound)?;
        payment.validate_refund(input.amount)?;
        self.db.payments().create_refund(input)
    }

    /// Get a refund by ID
    pub fn get_refund(&self, id: Uuid) -> Result<Option<Refund>> {
        self.db.payments().get_refund(id)
    }

    /// Get all refunds for a payment
    pub fn get_refunds(&self, payment_id: PaymentId) -> Result<Vec<Refund>> {
        self.db.payments().get_refunds(payment_id)
    }

    /// Complete a refund
    ///
    /// This marks the refund as processed and updates the payment's refunded amount.
    pub fn complete_refund(&self, id: Uuid) -> Result<Refund> {
        self.db.payments().complete_refund(id)
    }

    /// Mark a refund as failed
    pub fn fail_refund(&self, id: Uuid, reason: &str) -> Result<Refund> {
        self.db.payments().fail_refund(id, reason)
    }

    /// Create a stored payment method for a customer
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CreatePaymentMethod, PaymentMethodType, CardBrand, CustomerId};
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// let method = commerce.payments().create_payment_method(CreatePaymentMethod {
    ///     customer_id: CustomerId::new(),
    ///     method_type: PaymentMethodType::CreditCard,
    ///     is_default: Some(true),
    ///     card_brand: Some(CardBrand::Visa),
    ///     card_last4: Some("4242".into()),
    ///     card_exp_month: Some(12),
    ///     card_exp_year: Some(2025),
    ///     cardholder_name: Some("Alice Smith".into()),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn create_payment_method(&self, input: CreatePaymentMethod) -> Result<PaymentMethod> {
        self.db.payments().create_payment_method(input)
    }

    /// Get all payment methods for a customer
    pub fn get_payment_methods(&self, customer_id: CustomerId) -> Result<Vec<PaymentMethod>> {
        self.db.payments().get_payment_methods(customer_id)
    }

    /// Delete a payment method
    pub fn delete_payment_method(&self, id: Uuid) -> Result<()> {
        self.db.payments().delete_payment_method(id)
    }

    /// Set a payment method as the default for a customer
    pub fn set_default_payment_method(
        &self,
        customer_id: CustomerId,
        method_id: Uuid,
    ) -> Result<()> {
        self.db.payments().set_default_payment_method(customer_id, method_id)
    }

    /// Count payments matching a filter
    pub fn count(&self, filter: PaymentFilter) -> Result<u64> {
        self.db.payments().count(filter)
    }
}
