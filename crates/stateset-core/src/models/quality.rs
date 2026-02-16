//! Quality Control domain models
//!
//! Models for inspections, non-conformance reports (NCRs), and quality holds.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// Inspection Types
// ============================================================================

/// Quality inspection for goods
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

impl std::fmt::Display for InspectionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Incoming => write!(f, "incoming"),
            Self::Receiving => write!(f, "receiving"),
            Self::InProcess => write!(f, "in_process"),
            Self::Final => write!(f, "final"),
            Self::Random => write!(f, "random"),
            Self::Return => write!(f, "return"),
        }
    }
}

impl std::str::FromStr for InspectionType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "incoming" => Ok(Self::Incoming),
            "receiving" => Ok(Self::Receiving),
            "in_process" => Ok(Self::InProcess),
            "final" => Ok(Self::Final),
            "random" => Ok(Self::Random),
            "return" => Ok(Self::Return),
            _ => Err(format!("Unknown inspection type: {}", s)),
        }
    }
}

/// Status of an inspection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum InspectionStatus {
    Pending,
    Scheduled,
    InProgress,
    Passed,
    Failed,
    PartialPass,
    OnHold,
    Cancelled,
}

impl Default for InspectionStatus {
    fn default() -> Self {
        Self::Pending
    }
}

impl std::fmt::Display for InspectionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Scheduled => write!(f, "scheduled"),
            Self::InProgress => write!(f, "in_progress"),
            Self::Passed => write!(f, "passed"),
            Self::Failed => write!(f, "failed"),
            Self::PartialPass => write!(f, "partial_pass"),
            Self::OnHold => write!(f, "on_hold"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl std::str::FromStr for InspectionStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(Self::Pending),
            "scheduled" => Ok(Self::Scheduled),
            "in_progress" => Ok(Self::InProgress),
            "passed" => Ok(Self::Passed),
            "failed" => Ok(Self::Failed),
            "partial_pass" => Ok(Self::PartialPass),
            "on_hold" => Ok(Self::OnHold),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(format!("Unknown inspection status: {}", s)),
        }
    }
}

/// Line item in an inspection
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum InspectionResult {
    Pending,
    Pass,
    Fail,
    ConditionalPass,
}

impl Default for InspectionResult {
    fn default() -> Self {
        Self::Pending
    }
}

impl std::fmt::Display for InspectionResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Pass => write!(f, "pass"),
            Self::Fail => write!(f, "fail"),
            Self::ConditionalPass => write!(f, "conditional_pass"),
        }
    }
}

impl std::str::FromStr for InspectionResult {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(Self::Pending),
            "pass" => Ok(Self::Pass),
            "fail" => Ok(Self::Fail),
            "conditional_pass" => Ok(Self::ConditionalPass),
            _ => Err(format!("Unknown inspection result: {}", s)),
        }
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum NonConformanceSource {
    Inspection,
    CustomerComplaint,
    InternalAudit,
    SupplierIssue,
    ProductionDefect,
    ShippingDamage,
}

impl Default for NonConformanceSource {
    fn default() -> Self {
        Self::Inspection
    }
}

impl std::fmt::Display for NonConformanceSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Inspection => write!(f, "inspection"),
            Self::CustomerComplaint => write!(f, "customer_complaint"),
            Self::InternalAudit => write!(f, "internal_audit"),
            Self::SupplierIssue => write!(f, "supplier_issue"),
            Self::ProductionDefect => write!(f, "production_defect"),
            Self::ShippingDamage => write!(f, "shipping_damage"),
        }
    }
}

impl std::str::FromStr for NonConformanceSource {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "inspection" => Ok(Self::Inspection),
            "customer_complaint" => Ok(Self::CustomerComplaint),
            "internal_audit" => Ok(Self::InternalAudit),
            "supplier_issue" => Ok(Self::SupplierIssue),
            "production_defect" => Ok(Self::ProductionDefect),
            "shipping_damage" => Ok(Self::ShippingDamage),
            _ => Err(format!("Unknown non-conformance source: {}", s)),
        }
    }
}

/// Severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Severity {
    Critical,
    Major,
    Minor,
    Observation,
}

impl Default for Severity {
    fn default() -> Self {
        Self::Minor
    }
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Critical => write!(f, "critical"),
            Self::Major => write!(f, "major"),
            Self::Minor => write!(f, "minor"),
            Self::Observation => write!(f, "observation"),
        }
    }
}

impl std::str::FromStr for Severity {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "critical" => Ok(Self::Critical),
            "major" => Ok(Self::Major),
            "minor" => Ok(Self::Minor),
            "observation" => Ok(Self::Observation),
            _ => Err(format!("Unknown severity: {}", s)),
        }
    }
}

/// Status of an NCR
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum NcrStatus {
    Open,
    UnderReview,
    PendingDisposition,
    CorrectiveAction,
    PreventiveAction,
    Verification,
    Closed,
    Cancelled,
}

impl Default for NcrStatus {
    fn default() -> Self {
        Self::Open
    }
}

impl std::fmt::Display for NcrStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Open => write!(f, "open"),
            Self::UnderReview => write!(f, "under_review"),
            Self::PendingDisposition => write!(f, "pending_disposition"),
            Self::CorrectiveAction => write!(f, "corrective_action"),
            Self::PreventiveAction => write!(f, "preventive_action"),
            Self::Verification => write!(f, "verification"),
            Self::Closed => write!(f, "closed"),
            Self::Cancelled => write!(f, "cancelled"),
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
            _ => Err(format!("Unknown NCR status: {}", s)),
        }
    }
}

/// Disposition decision for non-conforming material
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Disposition {
    UseAsIs,
    Rework,
    Repair,
    Scrap,
    ReturnToVendor,
    Downgrade,
    SortAndScreen,
}

impl std::fmt::Display for Disposition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UseAsIs => write!(f, "use_as_is"),
            Self::Rework => write!(f, "rework"),
            Self::Repair => write!(f, "repair"),
            Self::Scrap => write!(f, "scrap"),
            Self::ReturnToVendor => write!(f, "return_to_vendor"),
            Self::Downgrade => write!(f, "downgrade"),
            Self::SortAndScreen => write!(f, "sort_and_screen"),
        }
    }
}

impl std::str::FromStr for Disposition {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "use_as_is" => Ok(Self::UseAsIs),
            "rework" => Ok(Self::Rework),
            "repair" => Ok(Self::Repair),
            "scrap" => Ok(Self::Scrap),
            "return_to_vendor" => Ok(Self::ReturnToVendor),
            "downgrade" => Ok(Self::Downgrade),
            "sort_and_screen" => Ok(Self::SortAndScreen),
            _ => Err(format!("Unknown disposition: {}", s)),
        }
    }
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum HoldType {
    QualityInspection,
    CustomerReturn,
    Recall,
    Damaged,
    Expired,
    Quarantine,
    RegulatoryHold,
    InvestigationHold,
}

impl Default for HoldType {
    fn default() -> Self {
        Self::QualityInspection
    }
}

impl std::fmt::Display for HoldType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::QualityInspection => write!(f, "quality_inspection"),
            Self::CustomerReturn => write!(f, "customer_return"),
            Self::Recall => write!(f, "recall"),
            Self::Damaged => write!(f, "damaged"),
            Self::Expired => write!(f, "expired"),
            Self::Quarantine => write!(f, "quarantine"),
            Self::RegulatoryHold => write!(f, "regulatory_hold"),
            Self::InvestigationHold => write!(f, "investigation_hold"),
        }
    }
}

impl std::str::FromStr for HoldType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "quality_inspection" => Ok(Self::QualityInspection),
            "customer_return" => Ok(Self::CustomerReturn),
            "recall" => Ok(Self::Recall),
            "damaged" => Ok(Self::Damaged),
            "expired" => Ok(Self::Expired),
            "quarantine" => Ok(Self::Quarantine),
            "regulatory_hold" => Ok(Self::RegulatoryHold),
            "investigation_hold" => Ok(Self::InvestigationHold),
            _ => Err(format!("Unknown hold type: {}", s)),
        }
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
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateInspection {
    pub status: Option<InspectionStatus>,
    pub inspector_id: Option<String>,
    pub scheduled_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
}

/// Input for recording inspection results
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseQualityHold {
    pub released_by: String,
    pub release_notes: Option<String>,
}

/// Filter for listing quality holds
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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

/// Alias for CreateNonConformance for API convenience
pub type CreateNcr = CreateNonConformance;

/// Input for completing an inspection
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub fn can_start(&self) -> bool {
        matches!(self.status, InspectionStatus::Pending | InspectionStatus::Scheduled)
    }

    /// Check if inspection can be completed
    pub fn can_complete(&self) -> bool {
        matches!(self.status, InspectionStatus::InProgress)
    }

    /// Check if all items have been inspected
    pub fn all_items_inspected(&self) -> bool {
        self.items.iter().all(|item| item.result != InspectionResult::Pending)
    }

    /// Get overall pass rate
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
    pub fn can_close(&self) -> bool {
        matches!(
            self.status,
            NcrStatus::Verification | NcrStatus::CorrectiveAction | NcrStatus::PreventiveAction
        ) && self.disposition.is_some()
    }

    /// Check if NCR requires immediate action based on severity
    pub fn requires_immediate_action(&self) -> bool {
        matches!(self.severity, Severity::Critical)
    }

    /// Check if disposition has been set
    pub fn has_disposition(&self) -> bool {
        self.disposition.is_some()
    }
}

impl QualityHold {
    /// Check if hold is active
    pub fn is_active(&self) -> bool {
        self.released_at.is_none()
    }

    /// Check if hold has expired
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            Utc::now() > expires_at && self.released_at.is_none()
        } else {
            false
        }
    }
}
