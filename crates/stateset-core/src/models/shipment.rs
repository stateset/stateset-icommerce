//! Shipment models for tracking order fulfillment and delivery

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Shipping carrier for deliveries
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ShippingCarrier {
    #[default]
    Other,
    Ups,
    FedEx,
    Usps,
    Dhl,
    OnTrac,
    LaserShip,
}

impl ShippingCarrier {
    /// Get the tracking URL base for this carrier
    pub fn tracking_url_base(&self) -> Option<&'static str> {
        match self {
            ShippingCarrier::Ups => Some("https://www.ups.com/track?tracknum="),
            ShippingCarrier::FedEx => Some("https://www.fedex.com/apps/fedextrack/?tracknumbers="),
            ShippingCarrier::Usps => Some("https://tools.usps.com/go/TrackConfirmAction?tLabels="),
            ShippingCarrier::Dhl => Some("https://www.dhl.com/us-en/home/tracking/tracking-express.html?submit=1&tracking-id="),
            ShippingCarrier::OnTrac => Some("https://www.ontrac.com/tracking/?number="),
            ShippingCarrier::LaserShip => Some("https://www.lasership.com/track/"),
            ShippingCarrier::Other => None,
        }
    }

    /// Generate a full tracking URL for a tracking number
    pub fn tracking_url(&self, tracking_number: &str) -> Option<String> {
        self.tracking_url_base().map(|base| format!("{}{}", base, tracking_number))
    }
}

impl std::fmt::Display for ShippingCarrier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShippingCarrier::Ups => write!(f, "ups"),
            ShippingCarrier::FedEx => write!(f, "fedex"),
            ShippingCarrier::Usps => write!(f, "usps"),
            ShippingCarrier::Dhl => write!(f, "dhl"),
            ShippingCarrier::OnTrac => write!(f, "ontrac"),
            ShippingCarrier::LaserShip => write!(f, "lasership"),
            ShippingCarrier::Other => write!(f, "other"),
        }
    }
}

/// Shipping method/speed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ShippingMethod {
    #[default]
    Standard,
    Express,
    Overnight,
    TwoDay,
    Ground,
    International,
    SameDay,
    Freight,
}

impl std::fmt::Display for ShippingMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShippingMethod::Standard => write!(f, "standard"),
            ShippingMethod::Express => write!(f, "express"),
            ShippingMethod::Overnight => write!(f, "overnight"),
            ShippingMethod::TwoDay => write!(f, "two_day"),
            ShippingMethod::Ground => write!(f, "ground"),
            ShippingMethod::International => write!(f, "international"),
            ShippingMethod::SameDay => write!(f, "same_day"),
            ShippingMethod::Freight => write!(f, "freight"),
        }
    }
}

/// Status of a shipment through its lifecycle
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ShipmentStatus {
    /// Initial state - shipment created but not yet processed
    #[default]
    Pending,
    /// Being prepared for shipping
    Processing,
    /// Packed and ready to ship
    ReadyToShip,
    /// Handed off to carrier
    Shipped,
    /// In carrier's network
    InTransit,
    /// On delivery vehicle
    OutForDelivery,
    /// Successfully delivered
    Delivered,
    /// Delivery attempt failed
    Failed,
    /// Returned to sender
    Returned,
    /// Shipment cancelled
    Cancelled,
    /// On hold for investigation
    OnHold,
}

impl ShipmentStatus {
    /// Check if this status can transition to the target status
    pub fn can_transition_to(&self, target: ShipmentStatus) -> bool {
        use ShipmentStatus::*;
        matches!(
            (self, target),
            // Forward flow
            (Pending, Processing) |
            (Pending, Cancelled) |
            (Processing, ReadyToShip) |
            (Processing, Cancelled) |
            (ReadyToShip, Shipped) |
            (ReadyToShip, Cancelled) |
            (Shipped, InTransit) |
            (InTransit, OutForDelivery) |
            (InTransit, Failed) |
            (OutForDelivery, Delivered) |
            (OutForDelivery, Failed) |
            // Recovery paths
            (Failed, InTransit) |
            (Failed, Returned) |
            (Delivered, Returned) |
            // Hold operations
            (Pending, OnHold) |
            (Processing, OnHold) |
            (OnHold, Processing) |
            (OnHold, Cancelled)
        )
    }

    /// Check if this is a terminal status
    pub fn is_terminal(&self) -> bool {
        matches!(self, ShipmentStatus::Delivered | ShipmentStatus::Cancelled | ShipmentStatus::Returned)
    }
}

impl std::fmt::Display for ShipmentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShipmentStatus::Pending => write!(f, "pending"),
            ShipmentStatus::Processing => write!(f, "processing"),
            ShipmentStatus::ReadyToShip => write!(f, "ready_to_ship"),
            ShipmentStatus::Shipped => write!(f, "shipped"),
            ShipmentStatus::InTransit => write!(f, "in_transit"),
            ShipmentStatus::OutForDelivery => write!(f, "out_for_delivery"),
            ShipmentStatus::Delivered => write!(f, "delivered"),
            ShipmentStatus::Failed => write!(f, "failed"),
            ShipmentStatus::Returned => write!(f, "returned"),
            ShipmentStatus::Cancelled => write!(f, "cancelled"),
            ShipmentStatus::OnHold => write!(f, "on_hold"),
        }
    }
}

/// A shipment tracks the physical delivery of items from an order
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shipment {
    /// Unique identifier
    pub id: Uuid,
    /// Human-readable shipment number (e.g., "SHP-ABC123")
    pub shipment_number: String,
    /// Order this shipment belongs to
    pub order_id: Uuid,
    /// Current status
    pub status: ShipmentStatus,
    /// Shipping carrier
    pub carrier: ShippingCarrier,
    /// Shipping method/speed
    pub shipping_method: ShippingMethod,
    /// Carrier tracking number
    pub tracking_number: Option<String>,
    /// Auto-generated tracking URL
    pub tracking_url: Option<String>,

    // Recipient information
    /// Recipient name
    pub recipient_name: String,
    /// Recipient email for notifications
    pub recipient_email: Option<String>,
    /// Recipient phone
    pub recipient_phone: Option<String>,
    /// Full shipping address
    pub shipping_address: String,

    // Package details
    /// Package weight in kg
    pub weight_kg: Option<Decimal>,
    /// Package dimensions (e.g., "10x8x6 cm")
    pub dimensions: Option<String>,
    /// Shipping cost
    pub shipping_cost: Option<Decimal>,
    /// Insurance amount
    pub insurance_amount: Option<Decimal>,
    /// Whether signature is required on delivery
    pub signature_required: bool,

    // Timestamps
    /// When the shipment was handed to carrier
    pub shipped_at: Option<DateTime<Utc>>,
    /// Expected delivery date
    pub estimated_delivery: Option<DateTime<Utc>>,
    /// Actual delivery timestamp
    pub delivered_at: Option<DateTime<Utc>>,

    /// Notes about the shipment
    pub notes: Option<String>,
    /// Items in this shipment
    pub items: Vec<ShipmentItem>,
    /// Tracking events/history
    pub events: Vec<ShipmentEvent>,
    /// Version for optimistic locking
    pub version: i32,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Shipment {
    /// Generate a unique shipment number based on timestamp
    pub fn generate_shipment_number() -> String {
        let now = chrono::Utc::now();
        format!("SHP-{}", now.format("%Y%m%d%H%M%S"))
    }

    /// Calculate transit time in days (if delivered)
    pub fn transit_days(&self) -> Option<f64> {
        match (self.shipped_at, self.delivered_at) {
            (Some(shipped), Some(delivered)) => {
                let duration = delivered - shipped;
                Some(duration.num_hours() as f64 / 24.0)
            }
            _ => None,
        }
    }

    /// Check if delivery is late
    pub fn is_late(&self) -> bool {
        match (self.estimated_delivery, self.delivered_at) {
            (Some(estimated), Some(delivered)) => delivered > estimated,
            (Some(estimated), None) if !self.status.is_terminal() => Utc::now() > estimated,
            _ => false,
        }
    }
}

/// An item within a shipment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShipmentItem {
    pub id: Uuid,
    pub shipment_id: Uuid,
    /// Reference to order item
    pub order_item_id: Option<Uuid>,
    /// Product being shipped
    pub product_id: Option<Uuid>,
    /// SKU of the item
    pub sku: String,
    /// Item name/description
    pub name: String,
    /// Quantity in this shipment
    pub quantity: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A tracking event in a shipment's history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShipmentEvent {
    pub id: Uuid,
    pub shipment_id: Uuid,
    /// Type of event (e.g., "picked_up", "departed_facility", "arrived_at_hub")
    pub event_type: String,
    /// Location where event occurred
    pub location: Option<String>,
    /// Description of the event
    pub description: Option<String>,
    /// When the event occurred (may differ from created_at for imported events)
    pub event_time: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

/// Input for creating a new shipment
#[derive(Debug, Clone, Default)]
pub struct CreateShipment {
    pub order_id: Uuid,
    pub carrier: Option<ShippingCarrier>,
    pub shipping_method: Option<ShippingMethod>,
    pub tracking_number: Option<String>,
    pub recipient_name: String,
    pub recipient_email: Option<String>,
    pub recipient_phone: Option<String>,
    pub shipping_address: String,
    pub weight_kg: Option<Decimal>,
    pub dimensions: Option<String>,
    pub shipping_cost: Option<Decimal>,
    pub insurance_amount: Option<Decimal>,
    pub signature_required: Option<bool>,
    pub estimated_delivery: Option<DateTime<Utc>>,
    pub notes: Option<String>,
    pub items: Option<Vec<CreateShipmentItem>>,
}

/// Input for creating a shipment item
#[derive(Debug, Clone, Default)]
pub struct CreateShipmentItem {
    pub order_item_id: Option<Uuid>,
    pub product_id: Option<Uuid>,
    pub sku: String,
    pub name: String,
    pub quantity: i32,
}

/// Input for updating a shipment
#[derive(Debug, Clone, Default)]
pub struct UpdateShipment {
    pub status: Option<ShipmentStatus>,
    pub carrier: Option<ShippingCarrier>,
    pub tracking_number: Option<String>,
    pub recipient_name: Option<String>,
    pub recipient_email: Option<String>,
    pub recipient_phone: Option<String>,
    pub shipping_address: Option<String>,
    pub weight_kg: Option<Decimal>,
    pub dimensions: Option<String>,
    pub shipping_cost: Option<Decimal>,
    pub estimated_delivery: Option<DateTime<Utc>>,
    pub notes: Option<String>,
}

/// Input for adding a tracking event
#[derive(Debug, Clone)]
pub struct AddShipmentEvent {
    pub event_type: String,
    pub location: Option<String>,
    pub description: Option<String>,
    pub event_time: Option<DateTime<Utc>>,
}

/// Filter for querying shipments
#[derive(Debug, Clone, Default)]
pub struct ShipmentFilter {
    pub order_id: Option<Uuid>,
    pub status: Option<ShipmentStatus>,
    pub carrier: Option<ShippingCarrier>,
    pub tracking_number: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}
