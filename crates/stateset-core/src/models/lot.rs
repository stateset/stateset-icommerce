//! Lot/Batch Tracking domain models
//!
//! Models for lot tracking, traceability, and batch management.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// Lot Types
// ============================================================================

/// A lot/batch of inventory items
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Lot {
    pub id: Uuid,
    pub lot_number: String,
    pub sku: String,
    pub status: LotStatus,
    pub quantity_produced: Decimal,
    pub quantity_remaining: Decimal,
    pub quantity_reserved: Decimal,
    pub quantity_quarantined: Decimal,
    pub production_date: DateTime<Utc>,
    pub expiration_date: Option<DateTime<Utc>>,
    pub best_before_date: Option<DateTime<Utc>>,
    pub supplier_lot: Option<String>,
    pub supplier_id: Option<Uuid>,
    pub work_order_id: Option<Uuid>,
    pub purchase_order_id: Option<Uuid>,
    pub cost_per_unit: Option<Decimal>,
    pub attributes: serde_json::Value,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Status of a lot
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LotStatus {
    /// Lot is active and available
    Active,
    /// Lot is in quarantine pending inspection
    Quarantine,
    /// Lot has expired
    Expired,
    /// Lot is fully consumed
    Consumed,
    /// Lot is on hold (quality issue)
    OnHold,
    /// Lot has been recalled
    Recalled,
    /// Lot has been scrapped
    Scrapped,
}

impl Default for LotStatus {
    fn default() -> Self {
        Self::Active
    }
}

impl std::fmt::Display for LotStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Quarantine => write!(f, "quarantine"),
            Self::Expired => write!(f, "expired"),
            Self::Consumed => write!(f, "consumed"),
            Self::OnHold => write!(f, "on_hold"),
            Self::Recalled => write!(f, "recalled"),
            Self::Scrapped => write!(f, "scrapped"),
        }
    }
}

impl std::str::FromStr for LotStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "active" => Ok(Self::Active),
            "quarantine" => Ok(Self::Quarantine),
            "expired" => Ok(Self::Expired),
            "consumed" => Ok(Self::Consumed),
            "on_hold" => Ok(Self::OnHold),
            "recalled" => Ok(Self::Recalled),
            "scrapped" => Ok(Self::Scrapped),
            _ => Err(format!("Unknown lot status: {}", s)),
        }
    }
}

// ============================================================================
// Lot Transaction Types
// ============================================================================

/// Transaction record for lot movements
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LotTransaction {
    pub id: Uuid,
    pub lot_id: Uuid,
    pub transaction_type: LotTransactionType,
    pub quantity: Decimal,
    pub reference_type: String,
    pub reference_id: Uuid,
    pub from_location_id: Option<i32>,
    pub to_location_id: Option<i32>,
    pub reason: Option<String>,
    pub performed_by: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Type of lot transaction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LotTransactionType {
    /// Initial creation/receipt of lot
    Received,
    /// Consumed in production or sale
    Consumed,
    /// Manual adjustment
    Adjusted,
    /// Reserved for an order
    Reserved,
    /// Released from reservation
    Released,
    /// Moved to quarantine
    Quarantined,
    /// Released from quarantine
    QuarantineReleased,
    /// Transferred between locations
    Transferred,
    /// Scrapped
    Scrapped,
    /// Returned from customer
    Returned,
    /// Split from another lot
    Split,
    /// Merged with another lot
    Merged,
}

impl Default for LotTransactionType {
    fn default() -> Self {
        Self::Received
    }
}

impl std::fmt::Display for LotTransactionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Received => write!(f, "received"),
            Self::Consumed => write!(f, "consumed"),
            Self::Adjusted => write!(f, "adjusted"),
            Self::Reserved => write!(f, "reserved"),
            Self::Released => write!(f, "released"),
            Self::Quarantined => write!(f, "quarantined"),
            Self::QuarantineReleased => write!(f, "quarantine_released"),
            Self::Transferred => write!(f, "transferred"),
            Self::Scrapped => write!(f, "scrapped"),
            Self::Returned => write!(f, "returned"),
            Self::Split => write!(f, "split"),
            Self::Merged => write!(f, "merged"),
        }
    }
}

impl std::str::FromStr for LotTransactionType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "received" => Ok(Self::Received),
            "consumed" => Ok(Self::Consumed),
            "adjusted" => Ok(Self::Adjusted),
            "reserved" => Ok(Self::Reserved),
            "released" => Ok(Self::Released),
            "quarantined" => Ok(Self::Quarantined),
            "quarantine_released" => Ok(Self::QuarantineReleased),
            "transferred" => Ok(Self::Transferred),
            "scrapped" => Ok(Self::Scrapped),
            "returned" => Ok(Self::Returned),
            "split" => Ok(Self::Split),
            "merged" => Ok(Self::Merged),
            _ => Err(format!("Unknown lot transaction type: {}", s)),
        }
    }
}

// ============================================================================
// Lot Certificate Types
// ============================================================================

/// Certificate/document associated with a lot
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LotCertificate {
    pub id: Uuid,
    pub lot_id: Uuid,
    pub certificate_type: CertificateType,
    pub certificate_number: Option<String>,
    pub document_url: Option<String>,
    pub issued_by: Option<String>,
    pub issued_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Type of certificate
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CertificateType {
    /// Certificate of Analysis
    Coa,
    /// Certificate of Conformance
    Coc,
    /// Material Safety Data Sheet
    Msds,
    /// Safety Data Sheet
    Sds,
    /// Test Report
    TestReport,
    /// Inspection Report
    InspectionReport,
    /// Country of Origin
    CountryOfOrigin,
    /// Other
    Other,
}

impl Default for CertificateType {
    fn default() -> Self {
        Self::Coa
    }
}

impl std::fmt::Display for CertificateType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Coa => write!(f, "coa"),
            Self::Coc => write!(f, "coc"),
            Self::Msds => write!(f, "msds"),
            Self::Sds => write!(f, "sds"),
            Self::TestReport => write!(f, "test_report"),
            Self::InspectionReport => write!(f, "inspection_report"),
            Self::CountryOfOrigin => write!(f, "country_of_origin"),
            Self::Other => write!(f, "other"),
        }
    }
}

impl std::str::FromStr for CertificateType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "coa" => Ok(Self::Coa),
            "coc" => Ok(Self::Coc),
            "msds" => Ok(Self::Msds),
            "sds" => Ok(Self::Sds),
            "test_report" => Ok(Self::TestReport),
            "inspection_report" => Ok(Self::InspectionReport),
            "country_of_origin" => Ok(Self::CountryOfOrigin),
            "other" => Ok(Self::Other),
            _ => Err(format!("Unknown certificate type: {}", s)),
        }
    }
}

// ============================================================================
// Lot Location Types
// ============================================================================

/// Inventory of a lot at a specific location
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LotLocation {
    pub lot_id: Uuid,
    pub location_id: i32,
    pub quantity: Decimal,
    pub updated_at: DateTime<Utc>,
}

// ============================================================================
// Traceability Types
// ============================================================================

/// Result of a traceability query
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceabilityResult {
    pub lot: Lot,
    /// Upstream trace - where did this lot come from
    pub upstream: Vec<TraceNode>,
    /// Downstream trace - where did this lot go
    pub downstream: Vec<TraceNode>,
}

/// A node in the traceability chain
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceNode {
    pub node_type: TraceNodeType,
    pub node_id: Uuid,
    pub reference_number: Option<String>,
    pub lot_number: Option<String>,
    pub serial_number: Option<String>,
    pub quantity: Decimal,
    pub timestamp: DateTime<Utc>,
    pub entity_name: Option<String>,
}

/// Type of node in traceability chain
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceNodeType {
    PurchaseOrder,
    Receipt,
    WorkOrder,
    Order,
    Shipment,
    Return,
    Transfer,
    Adjustment,
}

impl std::fmt::Display for TraceNodeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PurchaseOrder => write!(f, "purchase_order"),
            Self::Receipt => write!(f, "receipt"),
            Self::WorkOrder => write!(f, "work_order"),
            Self::Order => write!(f, "order"),
            Self::Shipment => write!(f, "shipment"),
            Self::Return => write!(f, "return"),
            Self::Transfer => write!(f, "transfer"),
            Self::Adjustment => write!(f, "adjustment"),
        }
    }
}

// ============================================================================
// Input/Output Types
// ============================================================================

/// Input for creating a lot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateLot {
    pub lot_number: Option<String>,
    pub sku: String,
    pub quantity: Decimal,
    pub production_date: Option<DateTime<Utc>>,
    pub expiration_date: Option<DateTime<Utc>>,
    pub best_before_date: Option<DateTime<Utc>>,
    pub supplier_lot: Option<String>,
    pub supplier_id: Option<Uuid>,
    pub work_order_id: Option<Uuid>,
    pub purchase_order_id: Option<Uuid>,
    pub cost_per_unit: Option<Decimal>,
    pub attributes: Option<serde_json::Value>,
    pub notes: Option<String>,
    pub initial_location_id: Option<i32>,
}

impl Default for CreateLot {
    fn default() -> Self {
        Self {
            lot_number: None,
            sku: String::new(),
            quantity: Decimal::ZERO,
            production_date: None,
            expiration_date: None,
            best_before_date: None,
            supplier_lot: None,
            supplier_id: None,
            work_order_id: None,
            purchase_order_id: None,
            cost_per_unit: None,
            attributes: None,
            notes: None,
            initial_location_id: None,
        }
    }
}

/// Input for updating a lot
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateLot {
    pub status: Option<LotStatus>,
    pub expiration_date: Option<DateTime<Utc>>,
    pub best_before_date: Option<DateTime<Utc>>,
    pub cost_per_unit: Option<Decimal>,
    pub attributes: Option<serde_json::Value>,
    pub notes: Option<String>,
}

/// Input for adjusting lot quantity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdjustLot {
    pub lot_id: Uuid,
    pub quantity_change: Decimal,
    pub reason: String,
    pub reference_type: Option<String>,
    pub reference_id: Option<Uuid>,
    pub location_id: Option<i32>,
    pub performed_by: Option<String>,
}

impl Default for AdjustLot {
    fn default() -> Self {
        Self {
            lot_id: Uuid::nil(),
            quantity_change: Decimal::ZERO,
            reason: String::new(),
            reference_type: None,
            reference_id: None,
            location_id: None,
            performed_by: None,
        }
    }
}

/// Input for consuming from a lot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsumeLot {
    pub lot_id: Uuid,
    pub quantity: Decimal,
    pub reference_type: String,
    pub reference_id: Uuid,
    pub location_id: Option<i32>,
    pub performed_by: Option<String>,
}

impl Default for ConsumeLot {
    fn default() -> Self {
        Self {
            lot_id: Uuid::nil(),
            quantity: Decimal::ZERO,
            reference_type: String::new(),
            reference_id: Uuid::nil(),
            location_id: None,
            performed_by: None,
        }
    }
}

/// Input for reserving from a lot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReserveLot {
    pub lot_id: Uuid,
    pub quantity: Decimal,
    pub reference_type: String,
    pub reference_id: Uuid,
    pub expires_in_seconds: Option<i64>,
}

impl Default for ReserveLot {
    fn default() -> Self {
        Self {
            lot_id: Uuid::nil(),
            quantity: Decimal::ZERO,
            reference_type: String::new(),
            reference_id: Uuid::nil(),
            expires_in_seconds: None,
        }
    }
}

/// Input for transferring lot between locations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferLot {
    pub lot_id: Uuid,
    pub quantity: Decimal,
    pub from_location_id: i32,
    pub to_location_id: i32,
    pub reason: Option<String>,
    pub performed_by: Option<String>,
}

impl Default for TransferLot {
    fn default() -> Self {
        Self {
            lot_id: Uuid::nil(),
            quantity: Decimal::ZERO,
            from_location_id: 0,
            to_location_id: 0,
            reason: None,
            performed_by: None,
        }
    }
}

/// Input for splitting a lot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitLot {
    pub lot_id: Uuid,
    pub quantity: Decimal,
    pub new_lot_number: Option<String>,
    pub reason: Option<String>,
}

impl Default for SplitLot {
    fn default() -> Self {
        Self {
            lot_id: Uuid::nil(),
            quantity: Decimal::ZERO,
            new_lot_number: None,
            reason: None,
        }
    }
}

/// Input for merging lots
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MergeLots {
    pub source_lot_ids: Vec<Uuid>,
    pub target_lot_number: Option<String>,
    pub reason: Option<String>,
}

/// Filter for listing lots
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LotFilter {
    pub sku: Option<String>,
    pub lot_number: Option<String>,
    pub status: Option<LotStatus>,
    pub supplier_id: Option<Uuid>,
    pub work_order_id: Option<Uuid>,
    pub purchase_order_id: Option<Uuid>,
    pub expiring_before: Option<DateTime<Utc>>,
    pub expiring_after: Option<DateTime<Utc>>,
    pub has_quantity: Option<bool>,
    pub location_id: Option<i32>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Input for adding a certificate to a lot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddLotCertificate {
    pub lot_id: Uuid,
    pub certificate_type: CertificateType,
    pub certificate_number: Option<String>,
    pub document_url: Option<String>,
    pub issued_by: Option<String>,
    pub issued_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
}

impl Default for AddLotCertificate {
    fn default() -> Self {
        Self {
            lot_id: Uuid::nil(),
            certificate_type: CertificateType::default(),
            certificate_number: None,
            document_url: None,
            issued_by: None,
            issued_at: None,
            expires_at: None,
            notes: None,
        }
    }
}

// ============================================================================
// Business Logic
// ============================================================================

impl Lot {
    /// Check if lot has available quantity
    pub fn has_available(&self) -> bool {
        self.quantity_available() > Decimal::ZERO
    }

    /// Get available quantity (not reserved or quarantined)
    pub fn quantity_available(&self) -> Decimal {
        self.quantity_remaining - self.quantity_reserved - self.quantity_quarantined
    }

    /// Check if lot is expired
    pub fn is_expired(&self) -> bool {
        if let Some(exp) = self.expiration_date {
            Utc::now() > exp
        } else {
            false
        }
    }

    /// Check if lot is expiring soon (within days)
    pub fn is_expiring_soon(&self, days: i64) -> bool {
        if let Some(exp) = self.expiration_date {
            let threshold = Utc::now() + chrono::Duration::days(days);
            exp <= threshold && !self.is_expired()
        } else {
            false
        }
    }

    /// Check if lot can be consumed
    pub fn can_consume(&self, quantity: Decimal) -> bool {
        self.status == LotStatus::Active && self.quantity_available() >= quantity
    }

    /// Check if lot can be reserved
    pub fn can_reserve(&self, quantity: Decimal) -> bool {
        self.status == LotStatus::Active && self.quantity_available() >= quantity
    }

    /// Get days until expiration
    pub fn days_until_expiration(&self) -> Option<i64> {
        self.expiration_date
            .map(|exp| (exp - Utc::now()).num_days())
    }

    /// Get shelf life percentage remaining
    pub fn shelf_life_remaining(&self) -> Option<Decimal> {
        if let Some(exp) = self.expiration_date {
            let total_days = (exp - self.production_date).num_days();
            if total_days > 0 {
                let remaining_days = (exp - Utc::now()).num_days();
                Some(
                    Decimal::from(remaining_days.max(0)) / Decimal::from(total_days)
                        * Decimal::from(100),
                )
            } else {
                None
            }
        } else {
            None
        }
    }
}
