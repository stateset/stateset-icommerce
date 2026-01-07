//! Quality Control operations
//!
//! Comprehensive quality management system supporting:
//! - Inspections (receiving, in-process, final, random)
//! - Non-conformance reports (NCRs)
//! - Quality holds on inventory
//! - Defect code management
//!
//! # Example
//!
//! ```rust,ignore
//! use stateset_embedded::{Commerce, CreateInspection, InspectionType};
//! use uuid::Uuid;
//!
//! let commerce = Commerce::new("./store.db")?;
//!
//! // Create an inspection for received goods
//! let inspection = commerce.quality().create_inspection(CreateInspection {
//!     inspection_type: InspectionType::Receiving,
//!     reference_type: "purchase_order".into(),
//!     reference_id: Uuid::new_v4(),
//!     ..Default::default()
//! })?;
//!
//! println!("Created inspection #{}", inspection.inspection_number);
//! # Ok::<(), stateset_embedded::CommerceError>(())
//! ```

use stateset_core::{
    CreateDefectCode, CreateInspection, CreateNonConformance, CreateQualityHold,
    DefectCode, Inspection, InspectionFilter, InspectionItem, InspectionStatus,
    NonConformance, NonConformanceFilter, NcrStatus, QualityHold, QualityHoldFilter,
    QualityRepository, RecordInspectionResult, ReleaseQualityHold, Result,
    UpdateInspection, UpdateNonConformance,
};
use stateset_db::sqlite::SqliteQualityRepository;
use uuid::Uuid;

/// Quality control management interface.
pub struct Quality {
    repo: SqliteQualityRepository,
}

impl Quality {
    pub(crate) fn new(repo: SqliteQualityRepository) -> Self {
        Self { repo }
    }

    // ========================================================================
    // Inspections
    // ========================================================================

    /// Create a new inspection.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreateInspection, InspectionType};
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let inspection = commerce.quality().create_inspection(CreateInspection {
    ///     inspection_type: InspectionType::Receiving,
    ///     reference_type: "purchase_order".into(),
    ///     reference_id: Uuid::new_v4(),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn create_inspection(&self, input: CreateInspection) -> Result<Inspection> {
        self.repo.create_inspection(input)
    }

    /// Get an inspection by ID.
    pub fn get_inspection(&self, id: Uuid) -> Result<Option<Inspection>> {
        self.repo.get_inspection(id)
    }

    /// Get an inspection by its number.
    pub fn get_inspection_by_number(&self, number: &str) -> Result<Option<Inspection>> {
        self.repo.get_inspection_by_number(number)
    }

    /// List inspections with optional filtering.
    pub fn list_inspections(&self, filter: InspectionFilter) -> Result<Vec<Inspection>> {
        self.repo.list_inspections(filter)
    }

    /// Update an inspection.
    pub fn update_inspection(&self, id: Uuid, input: UpdateInspection) -> Result<Inspection> {
        self.repo.update_inspection(id, input)
    }

    /// Start an inspection (set status to InProgress).
    pub fn start_inspection(&self, id: Uuid) -> Result<Inspection> {
        self.repo.start_inspection(id)
    }

    /// Record inspection results for an item.
    pub fn record_inspection_result(&self, input: RecordInspectionResult) -> Result<InspectionItem> {
        self.repo.record_inspection_result(input)
    }

    /// Get inspection items for an inspection.
    pub fn get_inspection_items(&self, inspection_id: Uuid) -> Result<Vec<InspectionItem>> {
        self.repo.get_inspection_items(inspection_id)
    }

    /// Complete an inspection (mark as Passed or Failed based on results).
    pub fn complete_inspection(&self, id: Uuid) -> Result<Inspection> {
        self.repo.complete_inspection(id)
    }

    /// Delete an inspection (only if Pending).
    pub fn delete_inspection(&self, id: Uuid) -> Result<()> {
        self.repo.delete_inspection(id)
    }

    /// Count inspections matching filter.
    pub fn count_inspections(&self, filter: InspectionFilter) -> Result<u64> {
        self.repo.count_inspections(filter)
    }

    // ========================================================================
    // Non-Conformance Reports
    // ========================================================================

    /// Create a non-conformance report (NCR).
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreateNonConformance, NonConformanceSource, Severity};
    /// use rust_decimal_macros::dec;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let ncr = commerce.quality().create_ncr(CreateNonConformance {
    ///     source: NonConformanceSource::Inspection,
    ///     severity: Severity::Major,
    ///     sku: "SKU-001".into(),
    ///     quantity_affected: dec!(10),
    ///     description: "Parts do not meet specification".into(),
    ///     ..Default::default()
    /// })?;
    ///
    /// println!("Created NCR #{}", ncr.ncr_number);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn create_ncr(&self, input: CreateNonConformance) -> Result<NonConformance> {
        self.repo.create_ncr(input)
    }

    /// Get a non-conformance report by ID.
    pub fn get_ncr(&self, id: Uuid) -> Result<Option<NonConformance>> {
        self.repo.get_ncr(id)
    }

    /// Get a non-conformance report by its number.
    pub fn get_ncr_by_number(&self, number: &str) -> Result<Option<NonConformance>> {
        self.repo.get_ncr_by_number(number)
    }

    /// List non-conformance reports with optional filtering.
    pub fn list_ncrs(&self, filter: NonConformanceFilter) -> Result<Vec<NonConformance>> {
        self.repo.list_ncrs(filter)
    }

    /// Update a non-conformance report.
    ///
    /// Use this to set root cause, corrective action, disposition, etc.
    pub fn update_ncr(&self, id: Uuid, input: UpdateNonConformance) -> Result<NonConformance> {
        self.repo.update_ncr(id, input)
    }

    /// Close an NCR.
    pub fn close_ncr(&self, id: Uuid) -> Result<NonConformance> {
        self.repo.close_ncr(id)
    }

    /// Cancel an NCR.
    pub fn cancel_ncr(&self, id: Uuid) -> Result<NonConformance> {
        self.repo.cancel_ncr(id)
    }

    /// Count NCRs matching filter.
    pub fn count_ncrs(&self, filter: NonConformanceFilter) -> Result<u64> {
        self.repo.count_ncrs(filter)
    }

    // ========================================================================
    // Quality Holds
    // ========================================================================

    /// Create a quality hold on inventory.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreateQualityHold, HoldType};
    /// use rust_decimal_macros::dec;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let hold = commerce.quality().create_hold(CreateQualityHold {
    ///     sku: "SKU-001".into(),
    ///     lot_number: Some("LOT-2025-001".into()),
    ///     quantity_held: dec!(50),
    ///     reason: "Pending quality inspection".into(),
    ///     hold_type: HoldType::QualityInspection,
    ///     placed_by: Some("QA Team".into()),
    ///     ..Default::default()
    /// })?;
    ///
    /// println!("Hold placed on {} units", hold.quantity_held);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn create_hold(&self, input: CreateQualityHold) -> Result<QualityHold> {
        self.repo.create_hold(input)
    }

    /// Get a quality hold by ID.
    pub fn get_hold(&self, id: Uuid) -> Result<Option<QualityHold>> {
        self.repo.get_hold(id)
    }

    /// List quality holds with optional filtering.
    pub fn list_holds(&self, filter: QualityHoldFilter) -> Result<Vec<QualityHold>> {
        self.repo.list_holds(filter)
    }

    /// Release a quality hold.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, ReleaseQualityHold};
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// commerce.quality().release_hold(Uuid::new_v4(), ReleaseQualityHold {
    ///     released_by: "QA Manager".into(),
    ///     notes: Some("Inspection passed".into()),
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn release_hold(&self, id: Uuid, input: ReleaseQualityHold) -> Result<QualityHold> {
        self.repo.release_hold(id, input)
    }

    /// Get active holds for a SKU.
    pub fn get_active_holds_for_sku(&self, sku: &str) -> Result<Vec<QualityHold>> {
        self.repo.get_active_holds_for_sku(sku)
    }

    /// Get active holds for a lot.
    pub fn get_active_holds_for_lot(&self, lot_number: &str) -> Result<Vec<QualityHold>> {
        self.repo.get_active_holds_for_lot(lot_number)
    }

    /// Count active holds.
    pub fn count_active_holds(&self) -> Result<u64> {
        self.repo.count_active_holds()
    }

    // ========================================================================
    // Defect Codes
    // ========================================================================

    /// Create a defect code.
    pub fn create_defect_code(&self, input: CreateDefectCode) -> Result<DefectCode> {
        self.repo.create_defect_code(input)
    }

    /// Get a defect code by code.
    pub fn get_defect_code(&self, code: &str) -> Result<Option<DefectCode>> {
        self.repo.get_defect_code(code)
    }

    /// List all defect codes, optionally filtered by category.
    pub fn list_defect_codes(&self, category: Option<&str>) -> Result<Vec<DefectCode>> {
        self.repo.list_defect_codes(category)
    }

    /// Deactivate a defect code.
    pub fn deactivate_defect_code(&self, id: Uuid) -> Result<()> {
        self.repo.deactivate_defect_code(id)
    }

    // ========================================================================
    // Convenience Methods
    // ========================================================================

    /// Get all pending inspections.
    pub fn get_pending_inspections(&self) -> Result<Vec<Inspection>> {
        self.list_inspections(InspectionFilter {
            status: Some(InspectionStatus::Pending),
            ..Default::default()
        })
    }

    /// Get all open NCRs.
    pub fn get_open_ncrs(&self) -> Result<Vec<NonConformance>> {
        self.list_ncrs(NonConformanceFilter {
            status: Some(NcrStatus::Open),
            ..Default::default()
        })
    }

    /// Get all active holds.
    pub fn get_active_holds(&self) -> Result<Vec<QualityHold>> {
        self.list_holds(QualityHoldFilter {
            active_only: Some(true),
            ..Default::default()
        })
    }
}
