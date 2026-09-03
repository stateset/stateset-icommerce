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

impl NcrStatus {
    /// Whether the NCR has reached a state no further work leaves.
    ///
    /// A `Closed` NCR is a finished quality record and a `Cancelled` one was
    /// opened in error; both are evidence, so the repositories refuse to edit
    /// or re-status them. Every other status is still in flight.
    ///
    /// The match is exhaustive on purpose: adding a status forces a decision
    /// about whether it ends the NCR.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        match self {
            Self::Closed | Self::Cancelled => true,
            Self::Open
            | Self::UnderReview
            | Self::PendingDisposition
            | Self::CorrectiveAction
            | Self::PreventiveAction
            | Self::Verification => false,
        }
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
    /// Keyset cursor: return records after this `(sort_key, id)` pair.
    /// Sort key is `created_at` (DESC ordering).
    pub after_cursor: Option<(String, String)>,
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
    /// Keyset cursor: return records after this `(sort_key, id)` pair.
    /// Sort key is `created_at` (DESC ordering).
    pub after_cursor: Option<(String, String)>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use std::str::FromStr;

    // ============================================================================
    // Test Helpers
    // ============================================================================

    fn create_test_inspection(status: InspectionStatus) -> Inspection {
        let now = Utc::now();
        Inspection {
            id: Uuid::new_v4(),
            inspection_number: "INSP-001".to_string(),
            inspection_type: InspectionType::Receiving,
            reference_type: "receipt".to_string(),
            reference_id: Uuid::new_v4(),
            status,
            inspector_id: None,
            scheduled_at: None,
            started_at: None,
            completed_at: None,
            notes: None,
            items: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    fn create_test_item(
        result: InspectionResult,
        inspected: Decimal,
        passed: Decimal,
    ) -> InspectionItem {
        InspectionItem {
            id: Uuid::new_v4(),
            inspection_id: Uuid::new_v4(),
            sku: "SKU-001".to_string(),
            lot_number: None,
            serial_number: None,
            quantity_inspected: inspected,
            quantity_passed: passed,
            quantity_failed: inspected - passed,
            defect_codes: Vec::new(),
            measurements: None,
            result,
            notes: None,
            created_at: Utc::now(),
        }
    }

    fn create_test_ncr(status: NcrStatus, disposition: Option<Disposition>) -> NonConformance {
        let now = Utc::now();
        NonConformance {
            id: Uuid::new_v4(),
            ncr_number: "NCR-001".to_string(),
            inspection_id: None,
            source: NonConformanceSource::Inspection,
            severity: Severity::Minor,
            status,
            sku: "SKU-001".to_string(),
            lot_number: None,
            serial_number: None,
            quantity_affected: dec!(5),
            description: "Defect".to_string(),
            root_cause: None,
            corrective_action: None,
            preventive_action: None,
            disposition,
            disposition_quantity: None,
            assigned_to: None,
            created_at: now,
            updated_at: now,
            closed_at: None,
        }
    }

    fn create_test_hold() -> QualityHold {
        QualityHold {
            id: Uuid::new_v4(),
            sku: "SKU-001".to_string(),
            lot_number: None,
            serial_number: None,
            location_id: None,
            quantity_held: dec!(10),
            reason: "inspection pending".to_string(),
            hold_type: HoldType::QualityInspection,
            ncr_id: None,
            inspection_id: None,
            placed_by: "qa".to_string(),
            released_by: None,
            release_notes: None,
            placed_at: Utc::now(),
            released_at: None,
            expires_at: None,
        }
    }

    // ============================================================================
    // Inspection lifecycle guards
    // ============================================================================

    #[test]
    fn can_start_from_pending_and_scheduled_only() {
        for status in [
            InspectionStatus::Pending,
            InspectionStatus::Scheduled,
            InspectionStatus::InProgress,
            InspectionStatus::Passed,
            InspectionStatus::Failed,
            InspectionStatus::PartialPass,
            InspectionStatus::OnHold,
            InspectionStatus::Cancelled,
        ] {
            let insp = create_test_inspection(status);
            let expected =
                matches!(status, InspectionStatus::Pending | InspectionStatus::Scheduled);
            assert_eq!(insp.can_start(), expected, "status {status}");
        }
    }

    #[test]
    fn can_complete_only_from_in_progress() {
        assert!(create_test_inspection(InspectionStatus::InProgress).can_complete());
        assert!(!create_test_inspection(InspectionStatus::Pending).can_complete());
        assert!(!create_test_inspection(InspectionStatus::Passed).can_complete());
        assert!(!create_test_inspection(InspectionStatus::Cancelled).can_complete());
    }

    #[test]
    fn all_items_inspected_vacuously_true_when_empty() {
        assert!(create_test_inspection(InspectionStatus::InProgress).all_items_inspected());
    }

    #[test]
    fn all_items_inspected_false_with_pending_item() {
        let mut insp = create_test_inspection(InspectionStatus::InProgress);
        insp.items.push(create_test_item(InspectionResult::Pass, dec!(10), dec!(10)));
        insp.items.push(create_test_item(InspectionResult::Pending, dec!(5), dec!(0)));
        assert!(!insp.all_items_inspected());
        insp.items.pop();
        assert!(insp.all_items_inspected());
    }

    // ============================================================================
    // pass_rate
    // ============================================================================

    #[test]
    fn pass_rate_none_when_nothing_inspected() {
        let insp = create_test_inspection(InspectionStatus::InProgress);
        assert_eq!(insp.pass_rate(), None);
        let mut zero = create_test_inspection(InspectionStatus::InProgress);
        zero.items.push(create_test_item(InspectionResult::Pending, Decimal::ZERO, Decimal::ZERO));
        assert_eq!(zero.pass_rate(), None);
    }

    #[test]
    fn pass_rate_computes_percentage() {
        let mut insp = create_test_inspection(InspectionStatus::InProgress);
        insp.items.push(create_test_item(InspectionResult::Pass, dec!(60), dec!(60)));
        insp.items.push(create_test_item(InspectionResult::Fail, dec!(40), dec!(15)));
        assert_eq!(insp.pass_rate(), Some(dec!(75)));
    }

    #[test]
    fn pass_rate_zero_when_all_failed() {
        let mut insp = create_test_inspection(InspectionStatus::InProgress);
        insp.items.push(create_test_item(InspectionResult::Fail, dec!(10), Decimal::ZERO));
        assert_eq!(insp.pass_rate(), Some(Decimal::ZERO));
    }

    // ============================================================================
    // calculate_overall_result
    // ============================================================================

    #[test]
    fn overall_result_in_progress_when_empty_or_pending() {
        let insp = create_test_inspection(InspectionStatus::InProgress);
        assert_eq!(insp.calculate_overall_result(), InspectionStatus::InProgress);
        let mut pending = create_test_inspection(InspectionStatus::InProgress);
        pending.items.push(create_test_item(InspectionResult::Pending, dec!(1), Decimal::ZERO));
        assert_eq!(pending.calculate_overall_result(), InspectionStatus::InProgress);
    }

    #[test]
    fn overall_result_passed_when_all_pass() {
        let mut insp = create_test_inspection(InspectionStatus::InProgress);
        insp.items.push(create_test_item(InspectionResult::Pass, dec!(5), dec!(5)));
        insp.items.push(create_test_item(InspectionResult::Pass, dec!(3), dec!(3)));
        assert_eq!(insp.calculate_overall_result(), InspectionStatus::Passed);
    }

    #[test]
    fn overall_result_partial_pass_when_mixed() {
        let mut insp = create_test_inspection(InspectionStatus::InProgress);
        insp.items.push(create_test_item(InspectionResult::Pass, dec!(5), dec!(5)));
        insp.items.push(create_test_item(InspectionResult::Fail, dec!(5), Decimal::ZERO));
        assert_eq!(insp.calculate_overall_result(), InspectionStatus::PartialPass);
    }

    #[test]
    fn overall_result_conditional_pass_counts_as_partial() {
        let mut insp = create_test_inspection(InspectionStatus::InProgress);
        insp.items.push(create_test_item(InspectionResult::ConditionalPass, dec!(5), dec!(5)));
        assert_eq!(insp.calculate_overall_result(), InspectionStatus::PartialPass);
    }

    #[test]
    fn overall_result_failed_when_all_fail() {
        let mut insp = create_test_inspection(InspectionStatus::InProgress);
        insp.items.push(create_test_item(InspectionResult::Fail, dec!(5), Decimal::ZERO));
        assert_eq!(insp.calculate_overall_result(), InspectionStatus::Failed);
    }

    // ============================================================================
    // NonConformance
    // ============================================================================

    #[test]
    fn ncr_can_close_requires_late_status_and_disposition() {
        for status in
            [NcrStatus::Verification, NcrStatus::CorrectiveAction, NcrStatus::PreventiveAction]
        {
            assert!(create_test_ncr(status, Some(Disposition::Rework)).can_close());
            assert!(!create_test_ncr(status, None).can_close(), "no disposition, {status}");
        }
    }

    #[test]
    fn ncr_cannot_close_from_early_or_terminal_statuses() {
        for status in [
            NcrStatus::Open,
            NcrStatus::UnderReview,
            NcrStatus::PendingDisposition,
            NcrStatus::Closed,
            NcrStatus::Cancelled,
        ] {
            assert!(!create_test_ncr(status, Some(Disposition::Scrap)).can_close(), "{status}");
        }
    }

    #[test]
    fn ncr_requires_immediate_action_only_for_critical() {
        let mut ncr = create_test_ncr(NcrStatus::Open, None);
        assert!(!ncr.requires_immediate_action());
        ncr.severity = Severity::Critical;
        assert!(ncr.requires_immediate_action());
        ncr.severity = Severity::Major;
        assert!(!ncr.requires_immediate_action());
    }

    #[test]
    fn ncr_has_disposition() {
        assert!(!create_test_ncr(NcrStatus::Open, None).has_disposition());
        assert!(create_test_ncr(NcrStatus::Open, Some(Disposition::UseAsIs)).has_disposition());
    }

    // ============================================================================
    // QualityHold
    // ============================================================================

    #[test]
    fn hold_is_active_until_released() {
        let mut hold = create_test_hold();
        assert!(hold.is_active());
        hold.released_at = Some(Utc::now());
        assert!(!hold.is_active());
    }

    #[test]
    fn hold_is_expired_only_past_expiry_and_unreleased() {
        let mut hold = create_test_hold();
        assert!(!hold.is_expired()); // no expiry
        hold.expires_at = Some(Utc::now() + chrono::Duration::hours(1));
        assert!(!hold.is_expired()); // future expiry
        hold.expires_at = Some(Utc::now() - chrono::Duration::hours(1));
        assert!(hold.is_expired()); // past expiry, not released
        hold.released_at = Some(Utc::now());
        assert!(!hold.is_expired()); // released holds are not "expired"
    }

    // ============================================================================
    // Enum Display / FromStr round-trips and defaults
    // ============================================================================

    #[test]
    fn ncr_status_display_from_str_round_trip() {
        for status in [
            NcrStatus::Open,
            NcrStatus::UnderReview,
            NcrStatus::PendingDisposition,
            NcrStatus::CorrectiveAction,
            NcrStatus::PreventiveAction,
            NcrStatus::Verification,
            NcrStatus::Closed,
            NcrStatus::Cancelled,
        ] {
            assert_eq!(NcrStatus::from_str(&status.to_string()), Ok(status));
        }
        assert_eq!(NcrStatus::from_str("UNDER_REVIEW"), Ok(NcrStatus::UnderReview));
        assert!(NcrStatus::from_str("bogus").is_err());
    }

    #[test]
    fn inspection_status_round_trip() {
        for status in [
            InspectionStatus::Pending,
            InspectionStatus::InProgress,
            InspectionStatus::PartialPass,
            InspectionStatus::OnHold,
        ] {
            assert_eq!(InspectionStatus::from_str(&status.to_string()), Ok(status));
        }
        assert!(InspectionStatus::from_str("nope").is_err());
    }

    #[test]
    fn inspection_type_and_result_round_trip() {
        for t in [InspectionType::Incoming, InspectionType::InProcess, InspectionType::Return] {
            assert_eq!(InspectionType::from_str(&t.to_string()), Ok(t));
        }
        for r in [InspectionResult::Pass, InspectionResult::ConditionalPass] {
            assert_eq!(InspectionResult::from_str(&r.to_string()), Ok(r));
        }
    }

    #[test]
    fn disposition_and_hold_type_round_trip() {
        for d in [Disposition::UseAsIs, Disposition::ReturnToVendor, Disposition::SortAndScreen] {
            assert_eq!(Disposition::from_str(&d.to_string()), Ok(d));
        }
        for h in [HoldType::QualityInspection, HoldType::RegulatoryHold, HoldType::Quarantine] {
            assert_eq!(HoldType::from_str(&h.to_string()), Ok(h));
        }
    }

    #[test]
    fn defaults_are_sane() {
        assert_eq!(InspectionType::default(), InspectionType::Incoming);
        assert_eq!(InspectionStatus::default(), InspectionStatus::Pending);
        assert_eq!(InspectionResult::default(), InspectionResult::Pending);
        assert_eq!(NcrStatus::default(), NcrStatus::Open);
        assert_eq!(Severity::default(), Severity::Minor);
        assert_eq!(HoldType::default(), HoldType::QualityInspection);
        let create = CreateNonConformance::default();
        assert_eq!(create.quantity_affected, Decimal::ZERO);
        assert!(create.sku.is_empty());
        let hold = CreateQualityHold::default();
        assert_eq!(hold.quantity, Decimal::ZERO);
        assert!(hold.placed_by.is_empty());
    }
}
