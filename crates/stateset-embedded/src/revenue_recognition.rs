//! Revenue recognition (ASC 606) operations
//!
//! # Example
//!
//! ```ignore
//! use stateset_embedded::Commerce;
//!
//! let commerce = Commerce::new("./store.db")?;
//! let contract = commerce.revenue_recognition().create_contract(input)?;
//! let schedule = commerce.revenue_recognition().generate_schedule(obligation_id)?;
//! let schedule = commerce.revenue_recognition().recognize_period(obligation_id, through)?;
//! # Ok::<(), stateset_embedded::CommerceError>(())
//! ```

use chrono::NaiveDate;
use stateset_core::{
    CreateRevenueContract, PerformanceObligation, Result, RevenueContract, RevenueContractFilter,
    RevenueSchedule, UpdateRevenueContract,
};
use stateset_db::{Database, DatabaseCapability};
use std::sync::Arc;
use uuid::Uuid;

/// Revenue recognition (ASC 606) operations.
pub struct RevenueRecognition {
    db: Arc<dyn Database>,
}

impl std::fmt::Debug for RevenueRecognition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RevenueRecognition").finish_non_exhaustive()
    }
}

impl RevenueRecognition {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    /// Whether revenue recognition is supported by the active backend.
    #[must_use]
    pub fn is_supported(&self) -> bool {
        self.db.supports_capability(DatabaseCapability::RevenueRecognition)
    }

    fn ensure(&self) -> Result<()> {
        self.db.ensure_capability(DatabaseCapability::RevenueRecognition)
    }

    /// Create a new revenue contract with its performance obligations.
    pub fn create_contract(&self, input: CreateRevenueContract) -> Result<RevenueContract> {
        self.ensure()?;
        self.db.revenue_recognition().create_contract(input)
    }

    /// Get a revenue contract by ID (with obligations).
    pub fn get_contract(&self, id: Uuid) -> Result<Option<RevenueContract>> {
        self.ensure()?;
        self.db.revenue_recognition().get_contract(id)
    }

    /// List revenue contracts with optional filtering.
    pub fn list_contracts(&self, filter: RevenueContractFilter) -> Result<Vec<RevenueContract>> {
        self.ensure()?;
        self.db.revenue_recognition().list_contracts(filter)
    }

    /// Update a revenue contract; status changes are transition-guarded.
    pub fn update_contract(
        &self,
        id: Uuid,
        input: UpdateRevenueContract,
    ) -> Result<RevenueContract> {
        self.ensure()?;
        self.db.revenue_recognition().update_contract(id, input)
    }

    /// List the performance obligations under a contract.
    pub fn list_obligations(&self, contract_id: Uuid) -> Result<Vec<PerformanceObligation>> {
        self.ensure()?;
        self.db.revenue_recognition().list_obligations(contract_id)
    }

    /// Generate and persist the recognition schedule for an obligation.
    pub fn generate_schedule(&self, obligation_id: Uuid) -> Result<RevenueSchedule> {
        self.ensure()?;
        self.db.revenue_recognition().generate_schedule(obligation_id)
    }

    /// Get the persisted recognition schedule for an obligation, if generated.
    pub fn get_schedule(&self, obligation_id: Uuid) -> Result<Option<RevenueSchedule>> {
        self.ensure()?;
        self.db.revenue_recognition().get_schedule(obligation_id)
    }

    /// Recognize deferred entries with a period start on or before `through`.
    ///
    /// If the active GL auto-posting configuration has
    /// `auto_post_revenue_recognition` enabled, a balanced journal entry is
    /// also created and posted (debit deferred/unearned revenue, credit sales
    /// revenue) for the newly recognized amount.
    pub fn recognize_period(
        &self,
        obligation_id: Uuid,
        through: NaiveDate,
    ) -> Result<RevenueSchedule> {
        self.ensure()?;
        self.db.revenue_recognition().recognize_period(obligation_id, through)
    }
}
