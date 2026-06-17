//! Vendor return operations (return-to-supplier).

use stateset_core::{CreateVendorReturn, Result, VendorReturn, VendorReturnFilter, VendorReturnId};
use stateset_db::{Database, DatabaseCapability};
use std::sync::Arc;

/// Vendor return operations.
pub struct VendorReturns {
    db: Arc<dyn Database>,
}

impl std::fmt::Debug for VendorReturns {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VendorReturns").finish_non_exhaustive()
    }
}

impl VendorReturns {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    /// Whether vendor returns are supported by the active backend.
    #[must_use]
    pub fn is_supported(&self) -> bool {
        self.db.supports_capability(DatabaseCapability::VendorReturns)
    }

    fn ensure(&self) -> Result<()> {
        self.db.ensure_capability(DatabaseCapability::VendorReturns)
    }

    /// Create a new vendor return.
    pub fn create(&self, input: CreateVendorReturn) -> Result<VendorReturn> {
        self.ensure()?;
        self.db.vendor_returns().create(input)
    }

    /// Get a vendor return by ID.
    pub fn get(&self, id: VendorReturnId) -> Result<Option<VendorReturn>> {
        self.ensure()?;
        self.db.vendor_returns().get(id)
    }

    /// List vendor returns with optional filtering.
    pub fn list(&self, filter: VendorReturnFilter) -> Result<Vec<VendorReturn>> {
        self.ensure()?;
        self.db.vendor_returns().list(filter)
    }

    /// Submit a draft vendor return to the supplier.
    pub fn submit(&self, id: VendorReturnId) -> Result<VendorReturn> {
        self.ensure()?;
        self.db.vendor_returns().submit(id)
    }

    /// Process a vendor return, optionally generating a vendor credit.
    pub fn process(&self, id: VendorReturnId, generate_credit: bool) -> Result<VendorReturn> {
        self.ensure()?;
        self.db.vendor_returns().process(id, generate_credit)
    }

    /// Cancel a vendor return.
    pub fn cancel(&self, id: VendorReturnId) -> Result<VendorReturn> {
        self.ensure()?;
        self.db.vendor_returns().cancel(id)
    }
}
