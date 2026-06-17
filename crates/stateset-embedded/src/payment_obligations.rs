//! Payment obligation operations (scheduled AP payments).

use chrono::NaiveDate;
use rust_decimal::Decimal;
use stateset_core::{
    CreatePaymentObligation, PaymentObligation, PaymentObligationDashboard,
    PaymentObligationFilter, PaymentObligationId, PaymentObligationStatus, Result,
};
use stateset_db::{Database, DatabaseCapability};
use std::sync::Arc;
use uuid::Uuid;

/// Payment obligation operations.
pub struct PaymentObligations {
    db: Arc<dyn Database>,
}

impl std::fmt::Debug for PaymentObligations {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PaymentObligations").finish_non_exhaustive()
    }
}

impl PaymentObligations {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    /// Whether payment obligations are supported by the active backend.
    #[must_use]
    pub fn is_supported(&self) -> bool {
        self.db.supports_capability(DatabaseCapability::PaymentObligations)
    }

    fn ensure(&self) -> Result<()> {
        self.db.ensure_capability(DatabaseCapability::PaymentObligations)
    }

    /// Create a new payment obligation.
    pub fn create(&self, input: CreatePaymentObligation) -> Result<PaymentObligation> {
        self.ensure()?;
        self.db.payment_obligations().create(input)
    }

    /// Get a payment obligation by ID.
    pub fn get(&self, id: PaymentObligationId) -> Result<Option<PaymentObligation>> {
        self.ensure()?;
        self.db.payment_obligations().get(id)
    }

    /// List payment obligations with optional filtering.
    pub fn list(&self, filter: PaymentObligationFilter) -> Result<Vec<PaymentObligation>> {
        self.ensure()?;
        self.db.payment_obligations().list(filter)
    }

    /// Record a payment against an obligation.
    pub fn record_payment(
        &self,
        id: PaymentObligationId,
        amount: Decimal,
    ) -> Result<PaymentObligation> {
        self.ensure()?;
        self.db.payment_obligations().record_payment(id, amount)
    }

    /// Set the obligation status (e.g. schedule or cancel).
    pub fn set_status(
        &self,
        id: PaymentObligationId,
        status: PaymentObligationStatus,
    ) -> Result<PaymentObligation> {
        self.ensure()?;
        self.db.payment_obligations().set_status(id, status)
    }

    /// Link an AP bill to an obligation.
    pub fn link_bill(&self, id: PaymentObligationId, bill_id: Uuid) -> Result<PaymentObligation> {
        self.ensure()?;
        self.db.payment_obligations().link_bill(id, bill_id)
    }

    /// Aggregate dashboard summary as of the given date.
    pub fn dashboard(&self, today: NaiveDate) -> Result<PaymentObligationDashboard> {
        self.ensure()?;
        self.db.payment_obligations().dashboard(today)
    }
}
