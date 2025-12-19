//! Payment operations for processing transactions and refunds
//!
//! # Example
//!
//! ```rust,no_run
//! use stateset_embedded::{Commerce, CreatePayment, PaymentMethodType};
//! use rust_decimal_macros::dec;
//! use uuid::Uuid;
//!
//! let commerce = Commerce::new("./store.db")?;
//!
//! // Create a payment for an order
//! let payment = commerce.payments().create(CreatePayment {
//!     order_id: Some(Uuid::new_v4()),
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
use stateset_core::{
    CreatePayment, CreatePaymentMethod, CreateRefund, Payment, PaymentFilter,
    PaymentMethod, Refund, Result,
};
use std::sync::Arc;
use uuid::Uuid;

/// Payment operations for transaction processing and refunds
pub struct Payments {
    db: Arc<dyn Database>,
}

impl Payments {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    /// Create a new payment
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CreatePayment, PaymentMethodType, CardBrand};
    /// use rust_decimal_macros::dec;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// let payment = commerce.payments().create(CreatePayment {
    ///     order_id: Some(Uuid::new_v4()),
    ///     payment_method: PaymentMethodType::CreditCard,
    ///     amount: dec!(149.99),
    ///     currency: Some("USD".into()),
    ///     card_brand: Some(CardBrand::Visa),
    ///     card_last4: Some("4242".into()),
    ///     billing_email: Some("customer@example.com".into()),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn create(&self, input: CreatePayment) -> Result<Payment> {
        self.db.payments().create(input)
    }

    /// Get a payment by ID
    pub fn get(&self, id: Uuid) -> Result<Option<Payment>> {
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
    pub fn update(&self, id: Uuid, input: stateset_core::UpdatePayment) -> Result<Payment> {
        self.db.payments().update(id, input)
    }

    /// List payments with optional filtering
    pub fn list(&self, filter: PaymentFilter) -> Result<Vec<Payment>> {
        self.db.payments().list(filter)
    }

    /// Get all payments for an order
    pub fn for_order(&self, order_id: Uuid) -> Result<Vec<Payment>> {
        self.db.payments().for_order(order_id)
    }

    /// Get all payments for an invoice
    pub fn for_invoice(&self, invoice_id: Uuid) -> Result<Vec<Payment>> {
        self.db.payments().for_invoice(invoice_id)
    }

    /// Mark payment as processing
    pub fn mark_processing(&self, id: Uuid) -> Result<Payment> {
        self.db.payments().mark_processing(id)
    }

    /// Mark payment as completed
    ///
    /// This records the payment timestamp and marks the transaction as successful.
    pub fn mark_completed(&self, id: Uuid) -> Result<Payment> {
        self.db.payments().mark_completed(id)
    }

    /// Mark payment as failed
    ///
    /// # Arguments
    ///
    /// * `id` - Payment ID
    /// * `reason` - Human-readable failure reason
    /// * `code` - Optional error code from payment processor
    pub fn mark_failed(&self, id: Uuid, reason: &str, code: Option<&str>) -> Result<Payment> {
        self.db.payments().mark_failed(id, reason, code)
    }

    /// Cancel a payment
    pub fn cancel(&self, id: Uuid) -> Result<Payment> {
        self.db.payments().cancel(id)
    }

    /// Create a refund for a payment
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CreateRefund};
    /// use rust_decimal_macros::dec;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// // Full refund (omit amount for full refund)
    /// let refund = commerce.payments().create_refund(CreateRefund {
    ///     payment_id: Uuid::new_v4(),
    ///     reason: Some("Customer request".into()),
    ///     ..Default::default()
    /// })?;
    ///
    /// // Partial refund
    /// let refund = commerce.payments().create_refund(CreateRefund {
    ///     payment_id: Uuid::new_v4(),
    ///     amount: Some(dec!(50.00)),
    ///     reason: Some("Partial refund for damaged item".into()),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn create_refund(&self, input: CreateRefund) -> Result<Refund> {
        self.db.payments().create_refund(input)
    }

    /// Get a refund by ID
    pub fn get_refund(&self, id: Uuid) -> Result<Option<Refund>> {
        self.db.payments().get_refund(id)
    }

    /// Get all refunds for a payment
    pub fn get_refunds(&self, payment_id: Uuid) -> Result<Vec<Refund>> {
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
    /// use stateset_embedded::{Commerce, CreatePaymentMethod, PaymentMethodType, CardBrand};
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// let method = commerce.payments().create_payment_method(CreatePaymentMethod {
    ///     customer_id: Uuid::new_v4(),
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
    pub fn get_payment_methods(&self, customer_id: Uuid) -> Result<Vec<PaymentMethod>> {
        self.db.payments().get_payment_methods(customer_id)
    }

    /// Delete a payment method
    pub fn delete_payment_method(&self, id: Uuid) -> Result<()> {
        self.db.payments().delete_payment_method(id)
    }

    /// Set a payment method as the default for a customer
    pub fn set_default_payment_method(&self, customer_id: Uuid, method_id: Uuid) -> Result<()> {
        self.db.payments().set_default_payment_method(customer_id, method_id)
    }

    /// Count payments matching a filter
    pub fn count(&self, filter: PaymentFilter) -> Result<u64> {
        self.db.payments().count(filter)
    }
}
