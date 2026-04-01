//! Quality Control domain models
//!
//! Models for inspections, non-conformance reports (NCRs), and quality holds.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};
use uuid::Uuid;

// ============================================================================
// Inspection Types
// ============================================================================

/// Quality inspection for goods
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Inspection {
    pub id: Uuid,
    pub inspection_number: String,
    pub inspection_type: InspectionType,
    pub reference_type: String,
    pub reference_id: Uuid,
    pub status: InspectionStatus,
    pub inspector_id: Option<String>,
    pub scheduled_at: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
    pub items: Vec<InspectionItem>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Type of inspection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, Serialize, Deserialize)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum InspectionType {
    /// Incoming goods inspection (alias for Receiving)
    Incoming,
    /// Inspection of received goods
    Receiving,
    /// In-process quality check during manufacturing
    InProcess,
    /// Final inspection before shipping
    Final,
    /// Random quality audit
    Random,
    /// Customer return inspection
    Return,
}

impl Default for InspectionType {
    fn default() -> Self {
        Self::Incoming
    }
}

/// Status of an inspection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, Serialize, Deserialize)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum InspectionStatus {
    /// Inspection has been created but not yet scheduled.
    Pending,
    /// Inspection is scheduled for a future time.
    Scheduled,
    /// Inspector is actively performing the inspection.
    InProgress,
    /// All items passed the inspection criteria.
    Passed,
    /// One or more items failed the inspection criteria.
    Failed,
    /// Some items passed and some failed.
    PartialPass,
    /// Inspection is temporarily paused pending additional information.
    OnHold,
    /// Inspection was cancelled before completion.
    Cancelled,
}

impl Default for InspectionStatus {
    fn default() -> Self {
        Self::Pending
    }
}

/// Line item in an inspection
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InspectionItem {
    pub id: Uuid,
    pub inspection_id: Uuid,
    pub sku: String,
    pub lot_number: Option<String>,
    pub serial_number: Option<String>,
    pub quantity_inspected: Decimal,
    pub quantity_passed: Decimal,
    pub quantity_failed: Decimal,
    pub defect_codes: Vec<String>,
    pub measurements: Option<serde_json::Value>,
    pub result: InspectionResult,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Result of inspecting an item
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, Serialize, Deserialize)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum InspectionResult {
    /// Result has not yet been recorded.
    Pending,
    /// Item fully meets the quality criteria.
    Pass,
    /// Item does not meet the quality criteria.
    Fail,
    /// Item meets criteria only under specific conditions or with minor rework.
    ConditionalPass,
}

impl Default for InspectionResult {
    fn default() -> Self {
        Self::Pending
    }
}

// ============================================================================
// Non-Conformance Report (NCR) Types
// ============================================================================

/// Non-Conformance Report for quality issues
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NonConformance {
    pub id: Uuid,
    pub ncr_number: String,
    pub inspection_id: Option<Uuid>,
    pub source: NonConformanceSource,
    pub severity: Severity,
    pub status: NcrStatus,
    pub sku: String,
    pub lot_number: Option<String>,
    pub serial_number: Option<String>,
    pub quantity_affected: Decimal,
    pub description: String,
    pub root_cause: Option<String>,
    pub corrective_action: Option<String>,
    pub preventive_action: Option<String>,
    pub disposition: Option<Disposition>,
    pub disposition_quantity: Option<Decimal>,
    pub assigned_to: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
}

/// Source of the non-conformance
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, Serialize, Deserialize)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum NonConformanceSource {
    /// Defect discovered during a formal quality inspection.
    Inspection,
    /// Non-conformance reported by a customer.
    CustomerComplaint,
    /// Defect identified during an internal quality audit.
    InternalAudit,
    /// Problem attributed to a supplier's material or process.
    SupplierIssue,
    /// Defect introduced during the manufacturing process.
    ProductionDefect,
    /// Goods damaged in transit or during shipment.
    ShippingDamage,
}

impl Default for NonConformanceSource {
    fn default() -> Self {
        Self::Inspection
    }
}

/// Severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, Serialize, Deserialize)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Severity {
    /// Defect poses an immediate safety or compliance risk; requires urgent action.
    Critical,
    /// Significant defect likely to affect product function or customer satisfaction.
    Major,
    /// Small defect with limited impact on product use or appearance.
    Minor,
    /// Noteworthy finding that does not rise to the level of a defect.
    Observation,
}

impl Default for Severity {
    fn default() -> Self {
        Self::Minor
    }
}

/// Status of an NCR
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display, Serialize, Deserialize)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum NcrStatus {
    /// NCR has been created and is awaiting assignment.
    Open,
    /// NCR is being assessed by the quality team.
    UnderReview,
    /// Investigation is complete; awaiting a disposition decision.
    PendingDisposition,
    /// Corrective actions are being implemented to address the root cause.
    CorrectiveAction,
    /// Preventive actions are being implemented to avoid recurrence.
    PreventiveAction,
    /// Actions have been taken; effectiveness is being verified.
    Verification,
    /// All actions verified effective; NCR is closed.
    Closed,
    /// NCR was opened in error or deemed not applicable.
    Cancelled,
}

impl Default for NcrStatus {
    fn default() -> Self {
        Self::Open
    }
}

impl std::str::FromStr for NcrStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "open" => Ok(Self::Open),
            "under_review" => Ok(Self::UnderReview),
            "pending_disposition" => Ok(Self::PendingDisposition),
            "corrective_action" => Ok(Self::CorrectiveAction),
            "preventive_action" => Ok(Self::PreventiveAction),
            "verification" => Ok(Self::Verification),
            "closed" => Ok(Self::Closed),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(format!("Unknown NCR status: {s}")),
        }
    }
}

/// Disposition decision for non-conforming material
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, Serialize, Deserialize)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Disposition {
    /// Material is accepted in its current state without modification.
    UseAsIs,
    /// Material will be re-processed to meet the original specification.
    Rework,
    /// Material will be fixed to an acceptable but possibly different specification.
    Repair,
    /// Material is disposed of; cannot be used or sold.
    Scrap,
    /// Material is returned to the supplier for credit or replacement.
    ReturnToVendor,
    /// Material is reclassified to a lower-grade specification.
    Downgrade,
    /// Each unit is individually inspected to separate conforming from non-conforming.
    SortAndScreen,
}

// ============================================================================
// Quality Hold Types
// ============================================================================

/// Quality hold to prevent inventory movement
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityHold {
    pub id: Uuid,
    pub sku: String,
    pub lot_number: Option<String>,
    pub serial_number: Option<String>,
    pub location_id: Option<i32>,
    pub quantity_held: Decimal,
    pub reason: String,
    pub hold_type: HoldType,
    pub ncr_id: Option<Uuid>,
    pub inspection_id: Option<Uuid>,
    pub placed_by: String,
    pub released_by: Option<String>,
    pub release_notes: Option<String>,
    pub placed_at: DateTime<Utc>,
    pub released_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
}

/// Type of quality hold
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, Serialize, Deserialize)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum HoldType {
    /// Held pending a quality inspection decision.
    QualityInspection,
    /// Returned by a customer; awaiting disposition.
    CustomerReturn,
    /// Subject to a product recall.
    Recall,
    /// Goods were damaged and cannot be sold as-is.
    Damaged,
    /// Goods have passed or are approaching their expiry date.
    Expired,
    /// Isolated to prevent potential contamination or spread.
    Quarantine,
    /// Held due to a regulatory agency requirement or investigation.
    RegulatoryHold,
    /// Held while an internal investigation is in progress.
    InvestigationHold,
}

impl Default for HoldType {
    fn default() -> Self {
        Self::QualityInspection
    }
}

// ============================================================================
// Defect Code Types
// ============================================================================

/// Defect code definition
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DefectCode {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub category: String,
    pub severity: Severity,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

// ============================================================================
// Input/Output Types
// ============================================================================

/// Input for creating an inspection
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CreateInspection {
    pub inspection_type: InspectionType,
    pub reference_type: String,
    pub reference_id: Uuid,
    pub inspector_id: Option<String>,
    pub scheduled_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
    pub items: Vec<CreateInspectionItem>,
}

/// Input for creating an inspection item
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateInspectionItem {
    pub sku: String,
    pub lot_number: Option<String>,
    pub serial_number: Option<String>,
    pub quantity_to_inspect: Decimal,
}

impl Default for CreateInspectionItem {
    fn default() -> Self {
        Self {
            sku: String::new(),
            lot_number: None,
            serial_number: None,
            quantity_to_inspect: Decimal::ZERO,
        }
    }
}

/// Input for updating an inspection
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct UpdateInspection {
    pub status: Option<InspectionStatus>,
    pub inspector_id: Option<String>,
    pub scheduled_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
}

/// Input for recording inspection results
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordInspectionResult {
    pub item_id: Uuid,
    pub quantity_passed: Decimal,
    pub quantity_failed: Decimal,
    pub result: InspectionResult,
    pub defect_codes: Vec<String>,
    pub measurements: Option<serde_json::Value>,
    pub notes: Option<String>,
}

/// Filter for listing inspections
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct InspectionFilter {
    pub inspection_type: Option<InspectionType>,
    pub status: Option<InspectionStatus>,
    pub reference_type: Option<String>,
    pub reference_id: Option<Uuid>,
    pub inspector_id: Option<String>,
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Input for creating an NCR
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateNonConformance {
    pub inspection_id: Option<Uuid>,
    pub source: NonConformanceSource,
    pub severity: Severity,
    pub sku: String,
    pub lot_number: Option<String>,
    pub serial_number: Option<String>,
    pub quantity_affected: Decimal,
    pub description: String,
    pub assigned_to: Option<String>,
}

impl Default for CreateNonConformance {
    fn default() -> Self {
        Self {
            inspection_id: None,
            source: NonConformanceSource::default(),
            severity: Severity::default(),
            sku: String::new(),
            lot_number: None,
            serial_number: None,
            quantity_affected: Decimal::ZERO,
            description: String::new(),
            assigned_to: None,
        }
    }
}

/// Input for updating an NCR
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct UpdateNonConformance {
    pub status: Option<NcrStatus>,
    pub severity: Option<Severity>,
    pub root_cause: Option<String>,
    pub corrective_action: Option<String>,
    pub preventive_action: Option<String>,
    pub disposition: Option<Disposition>,
    pub disposition_quantity: Option<Decimal>,
    pub assigned_to: Option<String>,
}

/// Filter for listing NCRs
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct NonConformanceFilter {
    pub source: Option<NonConformanceSource>,
    pub severity: Option<Severity>,
    pub status: Option<NcrStatus>,
    pub sku: Option<String>,
    pub lot_number: Option<String>,
    pub assigned_to: Option<String>,
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Input for creating a quality hold
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateQualityHold {
    pub sku: String,
    pub lot_number: Option<String>,
    pub serial_number: Option<String>,
    pub location_id: Option<i32>,
    pub quantity: Decimal,
    pub reason: String,
    pub hold_type: HoldType,
    pub ncr_id: Option<Uuid>,
    pub inspection_id: Option<Uuid>,
    pub placed_by: String,
    pub expires_at: Option<DateTime<Utc>>,
}

impl Default for CreateQualityHold {
    fn default() -> Self {
        Self {
            sku: String::new(),
            lot_number: None,
            serial_number: None,
            location_id: None,
            quantity: Decimal::ZERO,
            reason: String::new(),
            hold_type: HoldType::default(),
            ncr_id: None,
            inspection_id: None,
            placed_by: String::new(),
            expires_at: None,
        }
    }
}

/// Input for releasing a quality hold
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseQualityHold {
    pub released_by: String,
    pub release_notes: Option<String>,
}

/// Filter for listing quality holds
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct QualityHoldFilter {
    pub sku: Option<String>,
    pub lot_number: Option<String>,
    pub hold_type: Option<HoldType>,
    pub location_id: Option<i32>,
    pub active_only: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Input for creating a defect code
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CreateDefectCode {
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub category: String,
    pub severity: Severity,
}

// ============================================================================
// Type Aliases for API compatibility
// ============================================================================

/// Alias for `CreateNonConformance` for API convenience
pub type CreateNcr = CreateNonConformance;

/// Input for completing an inspection
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompleteInspection {
    pub quantity_passed: Decimal,
    pub quantity_failed: Decimal,
    pub inspector_id: Option<String>,
    pub notes: Option<String>,
}

impl Default for CompleteInspection {
    fn default() -> Self {
        Self {
            quantity_passed: Decimal::ZERO,
            quantity_failed: Decimal::ZERO,
            inspector_id: None,
            notes: None,
        }
    }
}

// ============================================================================
// Business Logic
// ============================================================================

impl Inspection {
    /// Check if inspection can be started
    #[must_use]
    pub const fn can_start(&self) -> bool {
        matches!(self.status, InspectionStatus::Pending | InspectionStatus::Scheduled)
    }

    /// Check if inspection can be completed
    #[must_use]
    pub const fn can_complete(&self) -> bool {
        matches!(self.status, InspectionStatus::InProgress)
    }

    /// Check if all items have been inspected
    #[must_use]
    pub fn all_items_inspected(&self) -> bool {
        self.items.iter().all(|item| item.result != InspectionResult::Pending)
    }

    /// Get overall pass rate
    #[must_use]
    pub fn pass_rate(&self) -> Option<Decimal> {
        let total_inspected: Decimal = self.items.iter().map(|i| i.quantity_inspected).sum();
        if total_inspected > Decimal::ZERO {
            let total_passed: Decimal = self.items.iter().map(|i| i.quantity_passed).sum();
            Some((total_passed / total_inspected) * Decimal::from(100))
        } else {
            None
        }
    }

    /// Calculate overall result based on items
    #[must_use]
    pub fn calculate_overall_result(&self) -> InspectionStatus {
        if self.items.is_empty() || self.items.iter().any(|i| i.result == InspectionResult::Pending)
        {
            return InspectionStatus::InProgress;
        }

        let all_passed = self.items.iter().all(|i| i.result == InspectionResult::Pass);
        let any_passed = self.items.iter().any(|i| {
            i.result == InspectionResult::Pass || i.result == InspectionResult::ConditionalPass
        });

        if all_passed {
            InspectionStatus::Passed
        } else if any_passed {
            InspectionStatus::PartialPass
        } else {
            InspectionStatus::Failed
        }
    }
}

impl NonConformance {
    /// Check if NCR can be closed
    #[must_use]
    pub const fn can_close(&self) -> bool {
        matches!(
            self.status,
            NcrStatus::Verification | NcrStatus::CorrectiveAction | NcrStatus::PreventiveAction
        ) && self.disposition.is_some()
    }

    /// Check if NCR requires immediate action based on severity
    #[must_use]
    pub const fn requires_immediate_action(&self) -> bool {
        matches!(self.severity, Severity::Critical)
    }

    /// Check if disposition has been set
    #[must_use]
    pub const fn has_disposition(&self) -> bool {
        self.disposition.is_some()
    }
}

impl QualityHold {
    /// Check if hold is active
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.released_at.is_none()
    }

    /// Check if hold has expired
    #[must_use]
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            Utc::now() > expires_at && self.released_at.is_none()
        } else {
            false
        }
    }
}
