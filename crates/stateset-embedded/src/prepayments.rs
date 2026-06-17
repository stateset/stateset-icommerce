//! Prepayment operations (advance payments to suppliers).

use stateset_core::{
    ApplyPrepayment, CreatePrepayment, Prepayment, PrepaymentApplication, PrepaymentApplicationId,
    PrepaymentFilter, PrepaymentId, Result,
};
use stateset_db::{Database, DatabaseCapability};
use std::sync::Arc;

/// Prepayment operations.
pub struct Prepayments {
    db: Arc<dyn Database>,
}

impl std::fmt::Debug for Prepayments {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Prepayments").finish_non_exhaustive()
    }
}

impl Prepayments {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    /// Whether prepayments are supported by the active backend.
    #[must_use]
    pub fn is_supported(&self) -> bool {
        self.db.supports_capability(DatabaseCapability::Prepayments)
    }

    fn ensure(&self) -> Result<()> {
        self.db.ensure_capability(DatabaseCapability::Prepayments)
    }

    /// Create a new prepayment.
    pub fn create(&self, input: CreatePrepayment) -> Result<Prepayment> {
        self.ensure()?;
        self.db.prepayments().create(input)
    }

    /// Get a prepayment by ID.
    pub fn get(&self, id: PrepaymentId) -> Result<Option<Prepayment>> {
        self.ensure()?;
        self.db.prepayments().get(id)
    }

    /// List prepayments with optional filtering.
    pub fn list(&self, filter: PrepaymentFilter) -> Result<Vec<Prepayment>> {
        self.ensure()?;
        self.db.prepayments().list(filter)
    }

    /// Apply a prepayment against a bill or payment obligation.
    pub fn apply(&self, id: PrepaymentId, input: ApplyPrepayment) -> Result<Prepayment> {
        self.ensure()?;
        self.db.prepayments().apply(id, input)
    }

    /// List applications for a prepayment.
    pub fn list_applications(&self, id: PrepaymentId) -> Result<Vec<PrepaymentApplication>> {
        self.ensure()?;
        self.db.prepayments().list_applications(id)
    }

    /// Reverse a previously-recorded application.
    pub fn reverse_application(
        &self,
        id: PrepaymentId,
        application_id: PrepaymentApplicationId,
    ) -> Result<Prepayment> {
        self.ensure()?;
        self.db.prepayments().reverse_application(id, application_id)
    }

    /// Refund the remaining balance, closing the prepayment.
    pub fn refund(&self, id: PrepaymentId) -> Result<Prepayment> {
        self.ensure()?;
        self.db.prepayments().refund(id)
    }
}
