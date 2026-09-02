//! Lot/Batch Tracking domain models
//!
//! Models for lot tracking, traceability, and batch management.
//!
//! # How lots relate to inventory
//!
//! A lot is the itemised view of a SKU's stock: `quantity_remaining` is what
//! the lot still holds, `quantity_reserved` what is promised to orders, and
//! `quantity_quarantined` what a quality event has blocked. The sellable
//! remainder is [`Lot::quantity_available`].
//!
//! Lots are **linked to `inventory_balances`** (the per-SKU, per-location
//! on-hand / allocated / available triple) whenever two things are true:
//!
//! 1. the lot has a placement in `lot_locations` (created with
//!    `initial_location_id`, or moved there by a transfer), and
//! 2. an `inventory_items` row exists for the lot's SKU.
//!
//! When both hold, every lot mutation adjusts the balance for
//! `(sku, location)` **in the same database transaction**:
//!
//! | lot operation              | inventory effect at the lot's location        |
//! |----------------------------|-----------------------------------------------|
//! | create (with a location)   | on-hand `+= quantity` (receipt)                |
//! | adjust                     | on-hand `+= quantity_change`                   |
//! | consume / confirm          | on-hand `-= quantity` (confirm also frees the hold) |
//! | reserve / release          | allocated `+= / -= quantity` (a hold)          |
//! | quarantine / release       | allocated `+= / -= quarantined units` (a hold) |
//! | expiry sweep               | allocated `+= available units` (a hold)        |
//! | transfer                   | on-hand moves from source to destination       |
//!
//! Inventory has no dedicated quarantine bucket, so a quarantined (or expired)
//! lot **holds** its blocked units via `quantity_allocated`: the stock stays
//! on hand, but `quantity_available` — which the order path checks — drops.
//! Each on-hand change also writes an `inventory_transactions` row referencing
//! the lot. The invariant maintained is, per `(sku, location)`:
//!
//! ```text
//! Σ active lots (remaining − reserved − quarantined) == inventory available
//! ```
//!
//! Lots without a location, or SKUs without an inventory item, float free
//! (the pre-existing behaviour). `split` / `merge` move units *between lots*
//! and leave inventory untouched; the derived lots carry no placement.
//! Balances are floored at zero when a legacy lot was never received into
//! inventory, so the linkage can be adopted on live data without failing
//! consumption.
//!
//! Quarantining a lot also quarantines its `Available` / `Reserved` serials
//! (closing their reservations) and releasing it returns them to `Available`;
//! a failed inspection does the same through the quality repository.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};
use uuid::Uuid;

// ============================================================================
// Lot Types
// ============================================================================

/// A lot/batch of inventory items
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display, Serialize, Deserialize)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
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

impl LotStatus {
    /// Whether a lot may move from `self` to `next` through the guarded
    /// workflow operations (`quarantine`, `release_quarantine`, `expire_lots`,
    /// `consume`).
    ///
    /// The graph is deliberately conservative:
    ///
    /// * `Active` is the only sellable state and may go anywhere.
    /// * `Quarantine` / `OnHold` are reversible blocks: back to `Active`, or
    ///   onward to a terminal disposition. `OnHold` may also escalate to
    ///   `Quarantine`.
    /// * `Expired` and `Recalled` may only be disposed of (`Scrapped`) or, for
    ///   `Expired`, recalled.
    /// * `Consumed` and `Scrapped` are terminal.
    ///
    /// `update()` deliberately bypasses this table: it is the administrative
    /// override for data repair.
    #[must_use]
    pub const fn can_transition_to(self, next: Self) -> bool {
        match self {
            Self::Active => !matches!(next, Self::Active),
            Self::Quarantine => matches!(next, Self::Active | Self::Scrapped | Self::Recalled),
            Self::OnHold => matches!(
                next,
                Self::Active | Self::Quarantine | Self::Scrapped | Self::Recalled | Self::Expired
            ),
            Self::Expired => matches!(next, Self::Scrapped | Self::Recalled),
            Self::Recalled => matches!(next, Self::Scrapped),
            Self::Consumed | Self::Scrapped => false,
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
            _ => Err(format!("Unknown lot status: {s}")),
        }
    }
}

// ============================================================================
// Lot Transaction Types
// ============================================================================

/// Transaction record for lot movements
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, Serialize, Deserialize)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, Serialize, Deserialize)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
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

// ============================================================================
// Lot Location Types
// ============================================================================

/// Inventory of a lot at a specific location
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceabilityResult {
    pub lot: Lot,
    /// Upstream trace - where did this lot come from
    pub upstream: Vec<TraceNode>,
    /// Downstream trace - where did this lot go
    pub downstream: Vec<TraceNode>,
}

/// A node in the traceability chain
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[non_exhaustive]
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
        Self { lot_id: Uuid::nil(), quantity: Decimal::ZERO, new_lot_number: None, reason: None }
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
    #[must_use]
    pub fn has_available(&self) -> bool {
        self.quantity_available() > Decimal::ZERO
    }

    /// Get available quantity (not reserved or quarantined)
    #[must_use]
    pub fn quantity_available(&self) -> Decimal {
        self.quantity_remaining - self.quantity_reserved - self.quantity_quarantined
    }

    /// Check if lot is expired (relative to the wall clock)
    #[must_use]
    pub fn is_expired(&self) -> bool {
        self.is_expired_at(Utc::now())
    }

    /// Check if lot is expired as of `now`.
    ///
    /// A lot with no `expiration_date` never expires. The comparison is strict:
    /// a lot is still good *at* its expiration instant and expired after it.
    #[must_use]
    pub fn is_expired_at(&self, now: DateTime<Utc>) -> bool {
        self.expiration_date.is_some_and(|exp| now > exp)
    }

    /// Whether units may leave this lot as of `now`: the lot must be `Active`
    /// *and* unexpired. Status alone is not enough — `expire_lots` is a sweeper
    /// that may not have run yet, so a lot whose `expiration_date` has passed
    /// is refused even while its status still reads `Active`.
    #[must_use]
    pub fn consumable_at(&self, now: DateTime<Utc>) -> bool {
        self.status == LotStatus::Active && !self.is_expired_at(now)
    }

    /// Check if lot is expiring soon (within days)
    #[must_use]
    pub fn is_expiring_soon(&self, days: i64) -> bool {
        if let Some(exp) = self.expiration_date {
            let threshold = Utc::now() + chrono::Duration::days(days);
            exp <= threshold && !self.is_expired()
        } else {
            false
        }
    }

    /// Check if lot can be consumed (relative to the wall clock)
    #[must_use]
    pub fn can_consume(&self, quantity: Decimal) -> bool {
        self.can_consume_at(quantity, Utc::now())
    }

    /// Check if `quantity` can be consumed from this lot as of `now`:
    /// `Active`, unexpired, and enough unreserved/unquarantined units.
    #[must_use]
    pub fn can_consume_at(&self, quantity: Decimal, now: DateTime<Utc>) -> bool {
        self.consumable_at(now) && self.quantity_available() >= quantity
    }

    /// Check if lot can be reserved (relative to the wall clock)
    #[must_use]
    pub fn can_reserve(&self, quantity: Decimal) -> bool {
        self.can_reserve_at(quantity, Utc::now())
    }

    /// Check if `quantity` can be reserved from this lot as of `now`; same
    /// preconditions as [`Self::can_consume_at`].
    #[must_use]
    pub fn can_reserve_at(&self, quantity: Decimal, now: DateTime<Utc>) -> bool {
        self.consumable_at(now) && self.quantity_available() >= quantity
    }

    /// Get days until expiration
    #[must_use]
    pub fn days_until_expiration(&self) -> Option<i64> {
        self.expiration_date.map(|exp| (exp - Utc::now()).num_days())
    }

    /// Get shelf life percentage remaining
    #[must_use]
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

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use std::str::FromStr;

    // ============================================================================
    // Test Helpers
    // ============================================================================

    fn create_test_lot() -> Lot {
        let now = Utc::now();
        Lot {
            id: Uuid::new_v4(),
            lot_number: "LOT-001".to_string(),
            sku: "SKU-001".to_string(),
            status: LotStatus::Active,
            quantity_produced: dec!(100),
            quantity_remaining: dec!(100),
            quantity_reserved: Decimal::ZERO,
            quantity_quarantined: Decimal::ZERO,
            production_date: now,
            expiration_date: None,
            best_before_date: None,
            supplier_lot: None,
            supplier_id: None,
            work_order_id: None,
            purchase_order_id: None,
            cost_per_unit: Some(dec!(2.50)),
            attributes: serde_json::json!({}),
            notes: None,
            created_at: now,
            updated_at: now,
        }
    }

    // ============================================================================
    // Quantity / Availability
    // ============================================================================

    #[test]
    fn quantity_available_subtracts_reserved_and_quarantined() {
        let mut lot = create_test_lot();
        lot.quantity_reserved = dec!(30);
        lot.quantity_quarantined = dec!(20);
        assert_eq!(lot.quantity_available(), dec!(50));
    }

    #[test]
    fn has_available_true_when_positive() {
        let lot = create_test_lot();
        assert!(lot.has_available());
    }

    #[test]
    fn has_available_false_when_zero() {
        let mut lot = create_test_lot();
        lot.quantity_reserved = dec!(100);
        assert_eq!(lot.quantity_available(), Decimal::ZERO);
        assert!(!lot.has_available());
    }

    #[test]
    fn quantity_available_can_go_negative_when_over_reserved() {
        let mut lot = create_test_lot();
        lot.quantity_reserved = dec!(120);
        assert_eq!(lot.quantity_available(), dec!(-20));
        assert!(!lot.has_available());
    }

    // ============================================================================
    // can_consume / can_reserve guards
    // ============================================================================

    #[test]
    fn can_consume_exact_boundary() {
        let lot = create_test_lot();
        assert!(lot.can_consume(dec!(100)));
        assert!(!lot.can_consume(dec!(100.0001)));
    }

    #[test]
    fn can_consume_zero_quantity() {
        let lot = create_test_lot();
        assert!(lot.can_consume(Decimal::ZERO));
    }

    #[test]
    fn can_consume_rejected_for_non_active_statuses() {
        for status in [
            LotStatus::Quarantine,
            LotStatus::Expired,
            LotStatus::Consumed,
            LotStatus::OnHold,
            LotStatus::Recalled,
            LotStatus::Scrapped,
        ] {
            let mut lot = create_test_lot();
            lot.status = status;
            assert!(!lot.can_consume(dec!(1)), "should not consume from {status}");
        }
    }

    #[test]
    fn can_reserve_exact_boundary_and_over() {
        let mut lot = create_test_lot();
        lot.quantity_reserved = dec!(40);
        assert!(lot.can_reserve(dec!(60)));
        assert!(!lot.can_reserve(dec!(61)));
    }

    #[test]
    fn can_reserve_rejected_when_on_hold() {
        let mut lot = create_test_lot();
        lot.status = LotStatus::OnHold;
        assert!(!lot.can_reserve(dec!(1)));
    }

    #[test]
    fn can_consume_and_reserve_refuse_expired_lot_regardless_of_status() {
        let mut lot = create_test_lot();
        lot.expiration_date = Some(Utc::now() - chrono::Duration::days(1));
        assert_eq!(lot.status, LotStatus::Active, "status alone would allow it");
        assert!(!lot.can_consume(dec!(1)));
        assert!(!lot.can_reserve(dec!(1)));
        assert!(!lot.consumable_at(Utc::now()));

        // The same lot evaluated *before* its expiry instant is fine.
        let earlier = Utc::now() - chrono::Duration::days(2);
        assert!(lot.can_consume_at(dec!(1), earlier));
        assert!(lot.can_reserve_at(dec!(1), earlier));
        assert!(lot.consumable_at(earlier));
    }

    #[test]
    fn is_expired_at_is_strict_and_none_safe() {
        let mut lot = create_test_lot();
        assert!(!lot.is_expired_at(Utc::now()));
        let exp = Utc::now();
        lot.expiration_date = Some(exp);
        assert!(!lot.is_expired_at(exp), "still good at the instant itself");
        assert!(lot.is_expired_at(exp + chrono::Duration::seconds(1)));
    }

    // ============================================================================
    // Status transitions
    // ============================================================================

    #[test]
    fn lot_status_transitions_are_exhaustive_and_conservative() {
        use LotStatus::*;
        let all = [Active, Quarantine, Expired, Consumed, OnHold, Recalled, Scrapped];
        // Workflow guards used by the repositories.
        assert!(Active.can_transition_to(Quarantine));
        assert!(OnHold.can_transition_to(Quarantine));
        assert!(Quarantine.can_transition_to(Active));
        assert!(Active.can_transition_to(Expired));
        // Nothing resurrects a terminal lot.
        for next in all {
            assert!(!Consumed.can_transition_to(next), "consumed -> {next}");
            assert!(!Scrapped.can_transition_to(next), "scrapped -> {next}");
        }
        // Blocked stock must be released (or disposed of) through its own
        // workflow, never laundered straight back to Active.
        for from in [Expired, Recalled] {
            assert!(!from.can_transition_to(Active), "{from} -> active");
            assert!(!from.can_transition_to(Quarantine), "{from} -> quarantine");
        }
        // Self-transitions are never allowed (double quarantine / double release).
        for s in all {
            assert!(!s.can_transition_to(s), "{s} -> {s}");
        }
    }

    // ============================================================================
    // Expiration
    // ============================================================================

    #[test]
    fn is_expired_false_without_expiration_date() {
        let lot = create_test_lot();
        assert!(!lot.is_expired());
        assert_eq!(lot.days_until_expiration(), None);
    }

    #[test]
    fn is_expired_true_for_past_date() {
        let mut lot = create_test_lot();
        lot.expiration_date = Some(Utc::now() - chrono::Duration::days(1));
        assert!(lot.is_expired());
    }

    #[test]
    fn is_expiring_soon_within_window() {
        let mut lot = create_test_lot();
        lot.expiration_date = Some(Utc::now() + chrono::Duration::days(5));
        assert!(lot.is_expiring_soon(7));
        assert!(!lot.is_expiring_soon(2));
    }

    #[test]
    fn is_expiring_soon_false_when_already_expired() {
        let mut lot = create_test_lot();
        lot.expiration_date = Some(Utc::now() - chrono::Duration::days(1));
        assert!(!lot.is_expiring_soon(30));
    }

    #[test]
    fn days_until_expiration_positive_for_future() {
        let mut lot = create_test_lot();
        lot.expiration_date = Some(Utc::now() + chrono::Duration::days(10));
        assert_eq!(lot.days_until_expiration(), Some(9)); // truncated by sub-day remainder
    }

    #[test]
    fn shelf_life_remaining_none_without_expiration() {
        let lot = create_test_lot();
        assert_eq!(lot.shelf_life_remaining(), None);
    }

    #[test]
    fn shelf_life_remaining_none_when_total_days_zero() {
        let mut lot = create_test_lot();
        lot.expiration_date = Some(lot.production_date);
        assert_eq!(lot.shelf_life_remaining(), None);
    }

    #[test]
    fn shelf_life_remaining_clamps_expired_to_zero() {
        let mut lot = create_test_lot();
        lot.production_date = Utc::now() - chrono::Duration::days(100);
        lot.expiration_date = Some(Utc::now() - chrono::Duration::days(10));
        assert_eq!(lot.shelf_life_remaining(), Some(Decimal::ZERO));
    }

    #[test]
    fn shelf_life_remaining_roughly_half_midway() {
        let mut lot = create_test_lot();
        lot.production_date = Utc::now() - chrono::Duration::days(50);
        lot.expiration_date = Some(Utc::now() + chrono::Duration::days(50));
        let pct = lot.shelf_life_remaining().expect("should have shelf life");
        assert!(pct >= dec!(48) && pct <= dec!(52), "expected ~50, got {pct}");
    }

    // ============================================================================
    // Enum Display / FromStr round-trips and defaults
    // ============================================================================

    #[test]
    fn lot_status_display_from_str_round_trip() {
        for status in [
            LotStatus::Active,
            LotStatus::Quarantine,
            LotStatus::Expired,
            LotStatus::Consumed,
            LotStatus::OnHold,
            LotStatus::Recalled,
            LotStatus::Scrapped,
        ] {
            let parsed = LotStatus::from_str(&status.to_string()).expect("round trip");
            assert_eq!(parsed, status);
        }
    }

    #[test]
    fn lot_status_from_str_case_insensitive_and_unknown() {
        assert_eq!(LotStatus::from_str("ON_HOLD"), Ok(LotStatus::OnHold));
        assert!(LotStatus::from_str("bogus").is_err());
    }

    #[test]
    fn lot_transaction_type_round_trip() {
        for t in [
            LotTransactionType::Received,
            LotTransactionType::Consumed,
            LotTransactionType::QuarantineReleased,
            LotTransactionType::Split,
            LotTransactionType::Merged,
        ] {
            assert_eq!(LotTransactionType::from_str(&t.to_string()), Ok(t));
        }
        assert!(LotTransactionType::from_str("nope").is_err());
    }

    #[test]
    fn certificate_type_round_trip_and_default() {
        assert_eq!(CertificateType::default(), CertificateType::Coa);
        for t in [CertificateType::Coa, CertificateType::Msds, CertificateType::CountryOfOrigin] {
            assert_eq!(CertificateType::from_str(&t.to_string()), Ok(t));
        }
    }

    #[test]
    fn trace_node_type_display() {
        assert_eq!(TraceNodeType::PurchaseOrder.to_string(), "purchase_order");
        assert_eq!(TraceNodeType::WorkOrder.to_string(), "work_order");
        assert_eq!(TraceNodeType::Adjustment.to_string(), "adjustment");
    }

    #[test]
    fn defaults_are_sane() {
        assert_eq!(LotStatus::default(), LotStatus::Active);
        assert_eq!(LotTransactionType::default(), LotTransactionType::Received);
        let create = CreateLot::default();
        assert_eq!(create.quantity, Decimal::ZERO);
        assert!(create.sku.is_empty());
        let consume = ConsumeLot::default();
        assert_eq!(consume.lot_id, Uuid::nil());
        assert_eq!(consume.quantity, Decimal::ZERO);
    }
}
