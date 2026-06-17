//! Vendor credit operations (supplier-owed credits).

use stateset_core::{
    ApplyVendorCredit, CreateVendorCredit, Result, VendorCredit, VendorCreditApplication,
    VendorCreditApplicationId, VendorCreditFilter, VendorCreditId,
};
use stateset_db::{Database, DatabaseCapability};
use std::sync::Arc;

/// Vendor credit operations.
pub struct VendorCredits {
    db: Arc<dyn Database>,
}

impl std::fmt::Debug for VendorCredits {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VendorCredits").finish_non_exhaustive()
    }
}

impl VendorCredits {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    /// Whether vendor credits are supported by the active backend.
    #[must_use]
    pub fn is_supported(&self) -> bool {
        self.db.supports_capability(DatabaseCapability::VendorCredits)
    }

    fn ensure(&self) -> Result<()> {
        self.db.ensure_capability(DatabaseCapability::VendorCredits)
    }

    /// Create a new vendor credit.
    pub fn create(&self, input: CreateVendorCredit) -> Result<VendorCredit> {
        self.ensure()?;
        self.db.vendor_credits().create(input)
    }

    /// Get a vendor credit by ID.
    pub fn get(&self, id: VendorCreditId) -> Result<Option<VendorCredit>> {
        self.ensure()?;
        self.db.vendor_credits().get(id)
    }

    /// List vendor credits with optional filtering.
    pub fn list(&self, filter: VendorCreditFilter) -> Result<Vec<VendorCredit>> {
        self.ensure()?;
        self.db.vendor_credits().list(filter)
    }

    /// Apply a vendor credit against a bill or payment obligation.
    pub fn apply(&self, id: VendorCreditId, input: ApplyVendorCredit) -> Result<VendorCredit> {
        self.ensure()?;
        self.db.vendor_credits().apply(id, input)
    }

    /// List applications for a vendor credit.
    pub fn list_applications(&self, id: VendorCreditId) -> Result<Vec<VendorCreditApplication>> {
        self.ensure()?;
        self.db.vendor_credits().list_applications(id)
    }

    /// Reverse a previously-recorded application.
    pub fn reverse_application(
        &self,
        id: VendorCreditId,
        application_id: VendorCreditApplicationId,
    ) -> Result<VendorCredit> {
        self.ensure()?;
        self.db.vendor_credits().reverse_application(id, application_id)
    }

    /// Cancel a vendor credit.
    pub fn cancel(&self, id: VendorCreditId) -> Result<VendorCredit> {
        self.ensure()?;
        self.db.vendor_credits().cancel(id)
    }
}
