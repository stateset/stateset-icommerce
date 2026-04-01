//! Serial Number Management domain models
//!
//! Models for individual unit tracking via serial numbers.

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
        self.status == SerialStatus::Available
    }

    /// Check if serial can be shipped
    #[must_use]
    pub const fn can_ship(&self) -> bool {
        matches!(self.status, SerialStatus::Available | SerialStatus::Reserved)
    }

    /// Check if serial can be returned
    #[must_use]
    pub const fn can_return(&self) -> bool {
        matches!(self.status, SerialStatus::Sold | SerialStatus::Shipped)
    }

    /// Check if serial can be scrapped
    #[must_use]
    pub const fn can_scrap(&self) -> bool {
        !matches!(self.status, SerialStatus::Sold | SerialStatus::Shipped | SerialStatus::Scrapped)
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
    /// Check if reservation is active
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.released_at.is_none() && self.confirmed_at.is_none() && !self.is_expired()
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
