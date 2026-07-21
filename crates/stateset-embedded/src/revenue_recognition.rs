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

#[cfg(feature = "events")]
use crate::events::EventSystem;
#[cfg(feature = "events")]
use chrono::Utc;
#[cfg(feature = "events")]
use stateset_core::CommerceEvent;

/// Revenue recognition (ASC 606) operations.
pub struct RevenueRecognition {
    db: Arc<dyn Database>,
    #[cfg(feature = "events")]
    event_system: Arc<EventSystem>,
}

impl std::fmt::Debug for RevenueRecognition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RevenueRecognition").finish_non_exhaustive()
    }
}

impl RevenueRecognition {
    #[cfg(feature = "events")]
    pub(crate) fn new(db: Arc<dyn Database>, event_system: Arc<EventSystem>) -> Self {
        Self { db, event_system }
    }

    #[cfg(not(feature = "events"))]
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    #[cfg(feature = "events")]
    fn emit(&self, event: CommerceEvent) {
        self.event_system.emit(event);
    }

    /// Emit [`CommerceEvent::RevenueContractCompleted`] when `contract`
    /// transitioned into the completed status.
    #[cfg(feature = "events")]
    fn emit_contract_completed_if_transitioned(
        &self,
        previous_status: stateset_core::RevenueContractStatus,
        contract: &stateset_core::RevenueContract,
    ) {
        if previous_status != stateset_core::RevenueContractStatus::Completed
            && contract.status == stateset_core::RevenueContractStatus::Completed
        {
            self.emit(CommerceEvent::RevenueContractCompleted {
                contract_id: contract.id,
                contract_number: contract.contract_number.clone(),
                transaction_price: contract.transaction_price,
                currency: contract.currency,
                timestamp: Utc::now(),
            });
        }
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
        #[cfg(feature = "events")]
        let previous_status = self
            .db
            .revenue_recognition()
            .get_contract(id)?
            .map(|c| c.status)
            .unwrap_or(stateset_core::RevenueContractStatus::Draft);
        let contract = self.db.revenue_recognition().update_contract(id, input)?;
        #[cfg(feature = "events")]
        self.emit_contract_completed_if_transitioned(previous_status, &contract);
        Ok(contract)
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
        #[cfg(feature = "events")]
        let previously_recognized = self
            .db
            .revenue_recognition()
            .get_schedule(obligation_id)?
            .map(|s| s.recognized_total())
            .unwrap_or(rust_decimal::Decimal::ZERO);
        let schedule = self.db.revenue_recognition().recognize_period(obligation_id, through)?;
        #[cfg(feature = "events")]
        {
            let newly_recognized = schedule.recognized_total() - previously_recognized;
            if newly_recognized > rust_decimal::Decimal::ZERO {
                self.emit(CommerceEvent::RevenueRecognized {
                    obligation_id,
                    amount: newly_recognized,
                    total_recognized: schedule.recognized_total(),
                    timestamp: Utc::now(),
                });
                // The store completes the contract when every obligation is
                // fully recognized; detect that transition here. A contract
                // completed by an earlier call cannot have deferred entries
                // left, so `newly_recognized > 0` guarantees this is a fresh
                // transition when the owning contract is now completed.
                if schedule.deferred_total().is_zero() {
                    let completed =
                        self.db.revenue_recognition().list_contracts(RevenueContractFilter {
                            status: Some(stateset_core::RevenueContractStatus::Completed),
                            limit: Some(u32::MAX),
                            ..Default::default()
                        })?;
                    if let Some(contract) = completed
                        .iter()
                        .find(|c| c.obligations.iter().any(|o| o.id == obligation_id))
                    {
                        self.emit_contract_completed_if_transitioned(
                            stateset_core::RevenueContractStatus::Active,
                            contract,
                        );
                    }
                }
            }
        }
        Ok(schedule)
    }
}
