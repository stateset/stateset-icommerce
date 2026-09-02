//! Serial Number Management domain models
//!
//! Models for individual unit tracking via serial numbers.

use crate::errors::CommerceError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};
use uuid::Uuid;

// ============================================================================
// Serial Number Types
// ============================================================================

/// A uniquely serialized unit of inventory
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SerialNumber {
    pub id: Uuid,
    pub serial: String,
    pub sku: String,
    pub status: SerialStatus,
    pub lot_id: Option<Uuid>,
    pub lot_number: Option<String>,
    pub current_location_id: Option<i32>,
    pub current_owner_id: Option<Uuid>,
    pub current_owner_type: Option<String>,
    pub warranty_id: Option<Uuid>,
    pub manufactured_at: Option<DateTime<Utc>>,
    pub received_at: Option<DateTime<Utc>>,
    pub sold_at: Option<DateTime<Utc>>,
    pub activated_at: Option<DateTime<Utc>>,
    pub last_service_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
    pub attributes: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Status of a serial number
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display, Serialize, Deserialize)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SerialStatus {
    /// In production/assembly
    InProduction,
    /// Available in inventory
    Available,
    /// Reserved for an order
    Reserved,
    /// Shipped to customer
    Shipped,
    /// Sold to customer
    Sold,
    /// Returned by customer
    Returned,
    /// Under repair/service
    InService,
    /// Under warranty claim
    InWarranty,
    /// Quarantined for quality
    Quarantined,
    /// Scrapped/destroyed
    Scrapped,
    /// Recalled
    Recalled,
    /// Lost/missing
    Lost,
    /// Transferred to another entity
    Transferred,
}

impl Default for SerialStatus {
    fn default() -> Self {
        Self::Available
    }
}

impl SerialStatus {
    /// Every status, in declaration order. Used by the transition table tests
    /// and by callers that need to enumerate the state space.
    pub const ALL: [Self; 13] = [
        Self::InProduction,
        Self::Available,
        Self::Reserved,
        Self::Shipped,
        Self::Sold,
        Self::Returned,
        Self::InService,
        Self::InWarranty,
        Self::Quarantined,
        Self::Scrapped,
        Self::Recalled,
        Self::Lost,
        Self::Transferred,
    ];

    /// The statuses a serial in `self` may move to.
    ///
    /// This is THE serial state machine. Every repository mutation that writes
    /// `serial_numbers.status` (`change_status`, `update`, `reserve`,
    /// `release_reservation`, `mark_sold`, `mark_shipped`, `mark_returned`,
    /// `quarantine`, `release_quarantine`, `scrap`, `transfer_ownership`, the
    /// lot-level bulk helpers) consults it and refuses anything not listed
    /// here, so a scrapped unit can never be shipped and a sold unit can never
    /// be re-reserved. The match is exhaustive on purpose: adding a status
    /// forces a decision about its edges.
    ///
    /// A status never lists itself — a self-transition is a no-op the caller
    /// almost certainly did not intend, and it is refused like any other
    /// invalid edge.
    #[must_use]
    pub const fn allowed_transitions(self) -> &'static [Self] {
        match self {
            Self::InProduction => &[Self::Available, Self::Quarantined, Self::Scrapped, Self::Lost],
            Self::Available => &[
                Self::Reserved,
                Self::Shipped,
                Self::Sold,
                Self::InService,
                Self::Quarantined,
                Self::Scrapped,
                Self::Recalled,
                Self::Lost,
                Self::Transferred,
            ],
            Self::Reserved => &[
                Self::Available,
                Self::Shipped,
                Self::Sold,
                Self::Quarantined,
                Self::Scrapped,
                Self::Recalled,
                Self::Lost,
            ],
            Self::Shipped => &[
                Self::Sold,
                Self::Returned,
                Self::InService,
                Self::InWarranty,
                Self::Recalled,
                Self::Lost,
                Self::Transferred,
            ],
            Self::Sold => &[
                Self::Returned,
                Self::InService,
                Self::InWarranty,
                Self::Recalled,
                Self::Lost,
                Self::Transferred,
            ],
            Self::Returned => &[
                Self::Available,
                Self::InService,
                Self::Quarantined,
                Self::Scrapped,
                Self::Recalled,
                Self::Lost,
            ],
            Self::InService => &[
                Self::Available,
                Self::Shipped,
                Self::Sold,
                Self::Quarantined,
                Self::Scrapped,
                Self::Lost,
            ],
            Self::InWarranty => &[
                Self::Shipped,
                Self::Sold,
                Self::Returned,
                Self::InService,
                Self::Scrapped,
                Self::Lost,
            ],
            Self::Quarantined => {
                &[Self::Available, Self::InService, Self::Scrapped, Self::Recalled, Self::Lost]
            }
            Self::Scrapped => &[],
            Self::Recalled => {
                &[Self::Available, Self::Returned, Self::Quarantined, Self::Scrapped, Self::Lost]
            }
            Self::Lost => &[Self::Available, Self::Scrapped],
            Self::Transferred => &[Self::Returned, Self::Lost],
        }
    }

    /// Whether a serial in `self` may move to `to` (see
    /// [`Self::allowed_transitions`]).
    #[must_use]
    pub fn can_transition_to(self, to: Self) -> bool {
        self.allowed_transitions().contains(&to)
    }

    /// Whether no transition leaves this status.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        self.allowed_transitions().is_empty()
    }
}

impl std::str::FromStr for SerialStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "in_production" => Ok(Self::InProduction),
            "available" => Ok(Self::Available),
            "reserved" => Ok(Self::Reserved),
            "shipped" => Ok(Self::Shipped),
            "sold" => Ok(Self::Sold),
            "returned" => Ok(Self::Returned),
            "in_service" => Ok(Self::InService),
            "in_warranty" => Ok(Self::InWarranty),
            "quarantined" => Ok(Self::Quarantined),
            "scrapped" => Ok(Self::Scrapped),
            "recalled" => Ok(Self::Recalled),
            "lost" => Ok(Self::Lost),
            "transferred" => Ok(Self::Transferred),
            _ => Err(format!("Unknown serial status: {s}")),
        }
    }
}

// ============================================================================
// Serial History Types
// ============================================================================

/// History event for a serial number
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SerialHistory {
    pub id: Uuid,
    pub serial_id: Uuid,
    pub event_type: SerialEventType,
    pub reference_type: Option<String>,
    pub reference_id: Option<Uuid>,
    pub from_status: SerialStatus,
    pub to_status: SerialStatus,
    pub from_location_id: Option<i32>,
    pub to_location_id: Option<i32>,
    pub from_owner_id: Option<Uuid>,
    pub to_owner_id: Option<Uuid>,
    pub performed_by: Option<String>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Type of serial number event
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, Serialize, Deserialize)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SerialEventType {
    Created,
    Received,
    LocationChanged,
    Reserved,
    Released,
    Picked,
    Packed,
    Shipped,
    Delivered,
    Sold,
    Activated,
    Returned,
    Repaired,
    Serviced,
    WarrantyClaimed,
    Quarantined,
    QuarantineReleased,
    Scrapped,
    Recalled,
    Lost,
    Found,
    Transferred,
    StatusChanged,
    AttributeUpdated,
}

impl Default for SerialEventType {
    fn default() -> Self {
        Self::Created
    }
}

// ============================================================================
// Serial Reservation Types
// ============================================================================

/// Reservation of a specific serial number
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SerialReservation {
    pub id: Uuid,
    pub serial_id: Uuid,
    pub reference_type: String,
    pub reference_id: Uuid,
    pub reserved_by: Option<String>,
    pub reserved_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub released_at: Option<DateTime<Utc>>,
}

// ============================================================================
// Input/Output Types
// ============================================================================

/// Input for creating a serial number
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreateSerialNumber {
    pub serial: Option<String>,
    pub sku: String,
    pub lot_id: Option<Uuid>,
    pub lot_number: Option<String>,
    pub location_id: Option<i32>,
    pub manufactured_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
    pub attributes: Option<serde_json::Value>,
}

/// Input for creating multiple serial numbers in bulk
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreateSerialNumbersBulk {
    pub sku: String,
    pub quantity: i32,
    pub prefix: Option<String>,
    pub lot_id: Option<Uuid>,
    pub lot_number: Option<String>,
    pub location_id: Option<i32>,
    pub manufactured_at: Option<DateTime<Utc>>,
}

/// Input for updating a serial number
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateSerialNumber {
    pub status: Option<SerialStatus>,
    pub location_id: Option<i32>,
    pub lot_id: Option<Uuid>,
    pub warranty_id: Option<Uuid>,
    pub notes: Option<String>,
    pub attributes: Option<serde_json::Value>,
}

/// Input for changing serial status with tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeSerialStatus {
    pub serial_id: Uuid,
    pub new_status: SerialStatus,
    pub reference_type: Option<String>,
    pub reference_id: Option<Uuid>,
    pub location_id: Option<i32>,
    pub owner_id: Option<Uuid>,
    pub owner_type: Option<String>,
    pub performed_by: Option<String>,
    pub notes: Option<String>,
}

impl Default for ChangeSerialStatus {
    fn default() -> Self {
        Self {
            serial_id: Uuid::nil(),
            new_status: SerialStatus::default(),
            reference_type: None,
            reference_id: None,
            location_id: None,
            owner_id: None,
            owner_type: None,
            performed_by: None,
            notes: None,
        }
    }
}

/// Input for reserving a serial number
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReserveSerialNumber {
    pub serial_id: Uuid,
    pub reference_type: String,
    pub reference_id: Uuid,
    pub reserved_by: Option<String>,
    pub expires_in_seconds: Option<i64>,
}

impl Default for ReserveSerialNumber {
    fn default() -> Self {
        Self {
            serial_id: Uuid::nil(),
            reference_type: String::new(),
            reference_id: Uuid::nil(),
            reserved_by: None,
            expires_in_seconds: None,
        }
    }
}

/// Input for transferring ownership of a serial number
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferSerialOwnership {
    pub serial_id: Uuid,
    pub new_owner_id: Uuid,
    pub new_owner_type: String,
    pub reference_type: Option<String>,
    pub reference_id: Option<Uuid>,
    pub notes: Option<String>,
}

impl Default for TransferSerialOwnership {
    fn default() -> Self {
        Self {
            serial_id: Uuid::nil(),
            new_owner_id: Uuid::nil(),
            new_owner_type: String::new(),
            reference_type: None,
            reference_id: None,
            notes: None,
        }
    }
}

/// Input for moving a serial to a new location
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveSerial {
    pub serial_id: Uuid,
    pub to_location_id: i32,
    pub performed_by: Option<String>,
    pub notes: Option<String>,
}

impl Default for MoveSerial {
    fn default() -> Self {
        Self { serial_id: Uuid::nil(), to_location_id: 0, performed_by: None, notes: None }
    }
}

/// Filter for listing serial numbers
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SerialFilter {
    pub serial: Option<String>,
    pub serial_prefix: Option<String>,
    pub sku: Option<String>,
    pub status: Option<SerialStatus>,
    pub statuses: Option<Vec<SerialStatus>>,
    pub lot_id: Option<Uuid>,
    pub lot_number: Option<String>,
    pub location_id: Option<i32>,
    pub owner_id: Option<Uuid>,
    pub owner_type: Option<String>,
    pub warranty_id: Option<Uuid>,
    pub has_warranty: Option<bool>,
    pub manufactured_after: Option<DateTime<Utc>>,
    pub manufactured_before: Option<DateTime<Utc>>,
    pub sold_after: Option<DateTime<Utc>>,
    pub sold_before: Option<DateTime<Utc>>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Filter for serial history
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SerialHistoryFilter {
    pub serial_id: Option<Uuid>,
    pub event_type: Option<SerialEventType>,
    pub reference_type: Option<String>,
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Result of scanning/looking up a serial number
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerialLookupResult {
    pub serial: SerialNumber,
    pub product_name: Option<String>,
    pub lot: Option<super::lot::Lot>,
    pub warranty_status: Option<WarrantyLookupStatus>,
    pub recent_history: Vec<SerialHistory>,
}

/// Warranty status for lookup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarrantyLookupStatus {
    pub warranty_id: Uuid,
    pub is_active: bool,
    pub expires_at: Option<DateTime<Utc>>,
    pub coverage_type: Option<String>,
}

/// Serial number validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerialValidation {
    pub is_valid: bool,
    pub serial_id: Option<Uuid>,
    pub status: Option<SerialStatus>,
    pub sku: Option<String>,
    pub error_message: Option<String>,
}

// ============================================================================
// Type Aliases for API compatibility
// ============================================================================

/// Alias for `CreateSerialNumber` for API convenience
pub type CreateSerial = CreateSerialNumber;

// ============================================================================
// Business Logic
// ============================================================================

impl SerialNumber {
    /// Check if serial is available for sale
    #[must_use]
    pub fn is_available(&self) -> bool {
        self.status == SerialStatus::Available
    }

    /// Check if serial is in customer's possession
    #[must_use]
    pub const fn is_with_customer(&self) -> bool {
        matches!(self.status, SerialStatus::Sold | SerialStatus::Shipped)
    }

    /// Check if serial can be reserved
    #[must_use]
    pub fn can_reserve(&self) -> bool {
        self.can_transition_to(SerialStatus::Reserved)
    }

    /// Check if serial can be shipped
    #[must_use]
    pub fn can_ship(&self) -> bool {
        self.can_transition_to(SerialStatus::Shipped)
    }

    /// Check if serial can be returned
    #[must_use]
    pub fn can_return(&self) -> bool {
        self.can_transition_to(SerialStatus::Returned)
    }

    /// Check if serial can be scrapped
    #[must_use]
    pub fn can_scrap(&self) -> bool {
        self.can_transition_to(SerialStatus::Scrapped)
    }

    /// Whether this serial's status may move to `to` under the serial state
    /// machine ([`SerialStatus::allowed_transitions`]).
    #[must_use]
    pub fn can_transition_to(&self, to: SerialStatus) -> bool {
        self.status.can_transition_to(to)
    }

    /// Refuse an invalid transition with a [`CommerceError::Conflict`] naming
    /// the serial, its current status and the requested one.
    ///
    /// # Errors
    ///
    /// Returns [`CommerceError::Conflict`] when the state machine does not
    /// allow `status -> to`.
    pub fn ensure_can_transition_to(&self, to: SerialStatus) -> Result<(), CommerceError> {
        if self.can_transition_to(to) {
            Ok(())
        } else {
            Err(CommerceError::Conflict(format!(
                "Serial {} ({}) cannot transition from {} to {}",
                self.serial, self.id, self.status, to
            )))
        }
    }

    /// Check if serial has been activated
    #[must_use]
    pub const fn is_activated(&self) -> bool {
        self.activated_at.is_some()
    }

    /// Get age in days since manufacture
    #[must_use]
    pub fn age_days(&self) -> Option<i64> {
        self.manufactured_at.map(|mfg| (Utc::now() - mfg).num_days())
    }

    /// Get days since sold
    #[must_use]
    pub fn days_since_sold(&self) -> Option<i64> {
        self.sold_at.map(|sold| (Utc::now() - sold).num_days())
    }
}

impl SerialReservation {
    /// Check if reservation is active: it still holds the serial.
    ///
    /// A reservation is opened by `reserve`, optionally confirmed, and closed
    /// (`released_at` set) by an explicit release, by the sale/shipment that
    /// consumes it, or by the expiry sweeper. Confirmation does NOT close it —
    /// a confirmed reservation is the strongest hold there is.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.released_at.is_none() && !self.is_expired()
    }

    /// Whether the reservation is still open in the store (not yet released,
    /// consumed or swept), regardless of expiry.
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.released_at.is_none()
    }

    /// Check if reservation has expired
    #[must_use]
    pub fn is_expired(&self) -> bool {
        if let Some(expires) = self.expires_at {
            Utc::now() > expires && self.confirmed_at.is_none()
        } else {
            false
        }
    }

    /// Check if reservation has been confirmed
    #[must_use]
    pub const fn is_confirmed(&self) -> bool {
        self.confirmed_at.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    // ============================================================================
    // Test Helpers
    // ============================================================================

    const ALL_STATUSES: [SerialStatus; 13] = SerialStatus::ALL;

    fn create_test_serial(status: SerialStatus) -> SerialNumber {
        let now = Utc::now();
        SerialNumber {
            id: Uuid::new_v4(),
            serial: "SN-0001".to_string(),
            sku: "SKU-001".to_string(),
            status,
            lot_id: None,
            lot_number: None,
            current_location_id: None,
            current_owner_id: None,
            current_owner_type: None,
            warranty_id: None,
            manufactured_at: None,
            received_at: None,
            sold_at: None,
            activated_at: None,
            last_service_at: None,
            notes: None,
            attributes: serde_json::json!({}),
            created_at: now,
            updated_at: now,
        }
    }

    fn create_test_reservation() -> SerialReservation {
        SerialReservation {
            id: Uuid::new_v4(),
            serial_id: Uuid::new_v4(),
            reference_type: "order".to_string(),
            reference_id: Uuid::new_v4(),
            reserved_by: None,
            reserved_at: Utc::now(),
            expires_at: None,
            confirmed_at: None,
            released_at: None,
        }
    }

    // ============================================================================
    // Status guards
    // ============================================================================

    #[test]
    fn is_available_only_when_available() {
        for status in ALL_STATUSES {
            let serial = create_test_serial(status);
            assert_eq!(serial.is_available(), status == SerialStatus::Available);
        }
    }

    #[test]
    fn can_reserve_only_from_available() {
        for status in ALL_STATUSES {
            let serial = create_test_serial(status);
            assert_eq!(serial.can_reserve(), status == SerialStatus::Available, "status {status}");
        }
    }

    #[test]
    fn can_ship_matches_transition_table() {
        for status in ALL_STATUSES {
            let serial = create_test_serial(status);
            let expected = matches!(
                status,
                SerialStatus::Available
                    | SerialStatus::Reserved
                    | SerialStatus::InService
                    | SerialStatus::InWarranty
            );
            assert_eq!(serial.can_ship(), expected, "status {status}");
        }
    }

    #[test]
    fn can_return_matches_transition_table() {
        for status in ALL_STATUSES {
            let serial = create_test_serial(status);
            let expected = matches!(
                status,
                SerialStatus::Sold
                    | SerialStatus::Shipped
                    | SerialStatus::InWarranty
                    | SerialStatus::Recalled
                    | SerialStatus::Transferred
            );
            assert_eq!(serial.can_return(), expected, "status {status}");
        }
    }

    #[test]
    fn can_scrap_excludes_customer_owned_and_terminal() {
        for status in ALL_STATUSES {
            let serial = create_test_serial(status);
            let expected = !matches!(
                status,
                SerialStatus::Sold
                    | SerialStatus::Shipped
                    | SerialStatus::Scrapped
                    | SerialStatus::Transferred
            );
            assert_eq!(serial.can_scrap(), expected, "status {status}");
        }
    }

    // ============================================================================
    // Transition table invariants
    // ============================================================================

    #[test]
    fn no_status_transitions_to_itself() {
        for status in ALL_STATUSES {
            assert!(!status.can_transition_to(status), "{status} lists itself");
        }
    }

    #[test]
    fn scrapped_is_the_only_terminal_status() {
        for status in ALL_STATUSES {
            assert_eq!(status.is_terminal(), status == SerialStatus::Scrapped, "{status}");
        }
    }

    #[test]
    fn scrapped_cannot_ship_sell_or_reserve() {
        let scrapped = create_test_serial(SerialStatus::Scrapped);
        assert!(!scrapped.can_ship());
        assert!(!scrapped.can_reserve());
        assert!(!scrapped.can_transition_to(SerialStatus::Sold));
        let err = scrapped.ensure_can_transition_to(SerialStatus::Shipped).unwrap_err();
        match err {
            CommerceError::Conflict(msg) => {
                assert!(msg.contains("scrapped"), "{msg}");
                assert!(msg.contains("shipped"), "{msg}");
                assert!(msg.contains("SN-0001"), "{msg}");
            }
            other => panic!("expected Conflict, got {other:?}"),
        }
    }

    #[test]
    fn every_non_terminal_status_can_reach_available_or_scrapped_eventually() {
        // Reachability: from any status, a path exists to `Scrapped` (write-off)
        // so no unit is stuck in a limbo status forever.
        for start in ALL_STATUSES {
            let mut seen = vec![start];
            let mut frontier = vec![start];
            while let Some(s) = frontier.pop() {
                for &next in s.allowed_transitions() {
                    if !seen.contains(&next) {
                        seen.push(next);
                        frontier.push(next);
                    }
                }
            }
            assert!(seen.contains(&SerialStatus::Scrapped), "{start} cannot reach scrapped");
        }
    }

    #[test]
    fn ensure_can_transition_to_accepts_listed_edges() {
        for from in ALL_STATUSES {
            for to in ALL_STATUSES {
                let serial = create_test_serial(from);
                assert_eq!(
                    serial.ensure_can_transition_to(to).is_ok(),
                    from.can_transition_to(to),
                    "{from} -> {to}"
                );
            }
        }
    }

    #[test]
    fn is_with_customer_for_sold_and_shipped() {
        assert!(create_test_serial(SerialStatus::Sold).is_with_customer());
        assert!(create_test_serial(SerialStatus::Shipped).is_with_customer());
        assert!(!create_test_serial(SerialStatus::Returned).is_with_customer());
        assert!(!create_test_serial(SerialStatus::Available).is_with_customer());
    }

    // ============================================================================
    // Activation and age
    // ============================================================================

    #[test]
    fn is_activated_tracks_activated_at() {
        let mut serial = create_test_serial(SerialStatus::Sold);
        assert!(!serial.is_activated());
        serial.activated_at = Some(Utc::now());
        assert!(serial.is_activated());
    }

    #[test]
    fn age_days_none_without_manufactured_at() {
        let serial = create_test_serial(SerialStatus::Available);
        assert_eq!(serial.age_days(), None);
        assert_eq!(serial.days_since_sold(), None);
    }

    #[test]
    fn age_days_and_days_since_sold_computed() {
        let mut serial = create_test_serial(SerialStatus::Sold);
        serial.manufactured_at = Some(Utc::now() - chrono::Duration::days(30));
        serial.sold_at = Some(Utc::now() - chrono::Duration::days(10));
        assert_eq!(serial.age_days(), Some(30));
        assert_eq!(serial.days_since_sold(), Some(10));
    }

    // ============================================================================
    // SerialReservation lifecycle
    // ============================================================================

    #[test]
    fn fresh_reservation_is_active() {
        let res = create_test_reservation();
        assert!(res.is_active());
        assert!(!res.is_expired());
        assert!(!res.is_confirmed());
    }

    #[test]
    fn released_reservation_is_not_active() {
        let mut res = create_test_reservation();
        res.released_at = Some(Utc::now());
        assert!(!res.is_active());
    }

    #[test]
    fn confirmed_reservation_stays_active_until_closed() {
        let mut res = create_test_reservation();
        res.confirmed_at = Some(Utc::now());
        assert!(res.is_active());
        assert!(res.is_open());
        assert!(res.is_confirmed());
        res.released_at = Some(Utc::now());
        assert!(!res.is_active());
        assert!(!res.is_open());
    }

    #[test]
    fn expired_reservation_is_expired_and_inactive() {
        let mut res = create_test_reservation();
        res.expires_at = Some(Utc::now() - chrono::Duration::seconds(1));
        assert!(res.is_expired());
        assert!(!res.is_active());
    }

    #[test]
    fn confirmation_suppresses_expiry() {
        let mut res = create_test_reservation();
        res.expires_at = Some(Utc::now() - chrono::Duration::seconds(1));
        res.confirmed_at = Some(Utc::now());
        assert!(!res.is_expired());
    }

    #[test]
    fn future_expiry_reservation_still_active() {
        let mut res = create_test_reservation();
        res.expires_at = Some(Utc::now() + chrono::Duration::hours(1));
        assert!(!res.is_expired());
        assert!(res.is_active());
    }

    // ============================================================================
    // Enum Display / FromStr round-trips and defaults
    // ============================================================================

    #[test]
    fn serial_status_display_from_str_round_trip() {
        for status in ALL_STATUSES {
            let parsed = SerialStatus::from_str(&status.to_string()).expect("round trip");
            assert_eq!(parsed, status);
        }
    }

    #[test]
    fn serial_status_from_str_case_insensitive_and_unknown() {
        assert_eq!(SerialStatus::from_str("IN_PRODUCTION"), Ok(SerialStatus::InProduction));
        assert!(SerialStatus::from_str("nonsense").is_err());
    }

    #[test]
    fn serial_event_type_round_trip() {
        for t in [
            SerialEventType::Created,
            SerialEventType::LocationChanged,
            SerialEventType::WarrantyClaimed,
            SerialEventType::QuarantineReleased,
            SerialEventType::AttributeUpdated,
        ] {
            assert_eq!(SerialEventType::from_str(&t.to_string()), Ok(t));
        }
        assert!(SerialEventType::from_str("nope").is_err());
    }

    #[test]
    fn defaults_are_sane() {
        assert_eq!(SerialStatus::default(), SerialStatus::Available);
        assert_eq!(SerialEventType::default(), SerialEventType::Created);
        let change = ChangeSerialStatus::default();
        assert_eq!(change.serial_id, Uuid::nil());
        assert_eq!(change.new_status, SerialStatus::Available);
        let reserve = ReserveSerialNumber::default();
        assert_eq!(reserve.serial_id, Uuid::nil());
        assert!(reserve.reference_type.is_empty());
    }
}
