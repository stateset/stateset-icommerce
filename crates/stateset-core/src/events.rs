//! Domain events for commerce operations
//!
//! Events are emitted when significant state changes occur.
//! They can be used for:
//! - Audit logging
//! - Sync with remote services
//! - Triggering side effects (email, webhooks, etc.)

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::{
    CustomerStatus, FulfillmentStatus, OrderStatus, PaymentStatus, ReturnReason, ReturnStatus,
};

/// Commerce domain event
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CommerceEvent {
    // Order events
    OrderCreated {
        order_id: Uuid,
        customer_id: Uuid,
        total_amount: Decimal,
        item_count: usize,
        timestamp: DateTime<Utc>,
    },
    OrderStatusChanged {
        order_id: Uuid,
        from_status: OrderStatus,
        to_status: OrderStatus,
        timestamp: DateTime<Utc>,
    },
    OrderPaymentStatusChanged {
        order_id: Uuid,
        from_status: PaymentStatus,
        to_status: PaymentStatus,
        timestamp: DateTime<Utc>,
    },
    OrderFulfillmentStatusChanged {
        order_id: Uuid,
        from_status: FulfillmentStatus,
        to_status: FulfillmentStatus,
        timestamp: DateTime<Utc>,
    },
    OrderCancelled {
        order_id: Uuid,
        reason: Option<String>,
        timestamp: DateTime<Utc>,
    },
    OrderItemAdded {
        order_id: Uuid,
        item_id: Uuid,
        sku: String,
        quantity: i32,
        timestamp: DateTime<Utc>,
    },
    OrderItemRemoved {
        order_id: Uuid,
        item_id: Uuid,
        timestamp: DateTime<Utc>,
    },

    // Inventory events
    InventoryItemCreated {
        item_id: i64,
        sku: String,
        name: String,
        timestamp: DateTime<Utc>,
    },
    InventoryAdjusted {
        item_id: i64,
        sku: String,
        location_id: i32,
        quantity_change: Decimal,
        new_quantity: Decimal,
        reason: String,
        timestamp: DateTime<Utc>,
    },
    InventoryReserved {
        reservation_id: Uuid,
        sku: String,
        quantity: Decimal,
        reference_type: String,
        reference_id: String,
        timestamp: DateTime<Utc>,
    },
    InventoryReservationReleased {
        reservation_id: Uuid,
        sku: String,
        quantity: Decimal,
        timestamp: DateTime<Utc>,
    },
    InventoryReservationConfirmed {
        reservation_id: Uuid,
        sku: String,
        quantity: Decimal,
        timestamp: DateTime<Utc>,
    },
    LowStockAlert {
        sku: String,
        location_id: i32,
        current_quantity: Decimal,
        reorder_point: Decimal,
        timestamp: DateTime<Utc>,
    },

    // Customer events
    CustomerCreated {
        customer_id: Uuid,
        email: String,
        timestamp: DateTime<Utc>,
    },
    CustomerUpdated {
        customer_id: Uuid,
        fields_changed: Vec<String>,
        timestamp: DateTime<Utc>,
    },
    CustomerStatusChanged {
        customer_id: Uuid,
        from_status: CustomerStatus,
        to_status: CustomerStatus,
        timestamp: DateTime<Utc>,
    },
    CustomerAddressAdded {
        customer_id: Uuid,
        address_id: Uuid,
        timestamp: DateTime<Utc>,
    },

    // Product events
    ProductCreated {
        product_id: Uuid,
        name: String,
        slug: String,
        timestamp: DateTime<Utc>,
    },
    ProductUpdated {
        product_id: Uuid,
        fields_changed: Vec<String>,
        timestamp: DateTime<Utc>,
    },
    ProductStatusChanged {
        product_id: Uuid,
        from_status: String,
        to_status: String,
        timestamp: DateTime<Utc>,
    },
    ProductVariantAdded {
        product_id: Uuid,
        variant_id: Uuid,
        sku: String,
        timestamp: DateTime<Utc>,
    },
    ProductVariantUpdated {
        variant_id: Uuid,
        sku: String,
        timestamp: DateTime<Utc>,
    },

    // Return events
    ReturnRequested {
        return_id: Uuid,
        order_id: Uuid,
        customer_id: Uuid,
        reason: ReturnReason,
        item_count: usize,
        timestamp: DateTime<Utc>,
    },
    ReturnStatusChanged {
        return_id: Uuid,
        from_status: ReturnStatus,
        to_status: ReturnStatus,
        timestamp: DateTime<Utc>,
    },
    ReturnApproved {
        return_id: Uuid,
        order_id: Uuid,
        timestamp: DateTime<Utc>,
    },
    ReturnRejected {
        return_id: Uuid,
        order_id: Uuid,
        reason: String,
        timestamp: DateTime<Utc>,
    },
    ReturnCompleted {
        return_id: Uuid,
        order_id: Uuid,
        refund_amount: Decimal,
        timestamp: DateTime<Utc>,
    },
    RefundIssued {
        return_id: Uuid,
        order_id: Uuid,
        amount: Decimal,
        method: String,
        timestamp: DateTime<Utc>,
    },
}

impl CommerceEvent {
    /// Get event type as string
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::OrderCreated { .. } => "order_created",
            Self::OrderStatusChanged { .. } => "order_status_changed",
            Self::OrderPaymentStatusChanged { .. } => "order_payment_status_changed",
            Self::OrderFulfillmentStatusChanged { .. } => "order_fulfillment_status_changed",
            Self::OrderCancelled { .. } => "order_cancelled",
            Self::OrderItemAdded { .. } => "order_item_added",
            Self::OrderItemRemoved { .. } => "order_item_removed",
            Self::InventoryItemCreated { .. } => "inventory_item_created",
            Self::InventoryAdjusted { .. } => "inventory_adjusted",
            Self::InventoryReserved { .. } => "inventory_reserved",
            Self::InventoryReservationReleased { .. } => "inventory_reservation_released",
            Self::InventoryReservationConfirmed { .. } => "inventory_reservation_confirmed",
            Self::LowStockAlert { .. } => "low_stock_alert",
            Self::CustomerCreated { .. } => "customer_created",
            Self::CustomerUpdated { .. } => "customer_updated",
            Self::CustomerStatusChanged { .. } => "customer_status_changed",
            Self::CustomerAddressAdded { .. } => "customer_address_added",
            Self::ProductCreated { .. } => "product_created",
            Self::ProductUpdated { .. } => "product_updated",
            Self::ProductStatusChanged { .. } => "product_status_changed",
            Self::ProductVariantAdded { .. } => "product_variant_added",
            Self::ProductVariantUpdated { .. } => "product_variant_updated",
            Self::ReturnRequested { .. } => "return_requested",
            Self::ReturnStatusChanged { .. } => "return_status_changed",
            Self::ReturnApproved { .. } => "return_approved",
            Self::ReturnRejected { .. } => "return_rejected",
            Self::ReturnCompleted { .. } => "return_completed",
            Self::RefundIssued { .. } => "refund_issued",
        }
    }

    /// Get timestamp from event
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Self::OrderCreated { timestamp, .. }
            | Self::OrderStatusChanged { timestamp, .. }
            | Self::OrderPaymentStatusChanged { timestamp, .. }
            | Self::OrderFulfillmentStatusChanged { timestamp, .. }
            | Self::OrderCancelled { timestamp, .. }
            | Self::OrderItemAdded { timestamp, .. }
            | Self::OrderItemRemoved { timestamp, .. }
            | Self::InventoryItemCreated { timestamp, .. }
            | Self::InventoryAdjusted { timestamp, .. }
            | Self::InventoryReserved { timestamp, .. }
            | Self::InventoryReservationReleased { timestamp, .. }
            | Self::InventoryReservationConfirmed { timestamp, .. }
            | Self::LowStockAlert { timestamp, .. }
            | Self::CustomerCreated { timestamp, .. }
            | Self::CustomerUpdated { timestamp, .. }
            | Self::CustomerStatusChanged { timestamp, .. }
            | Self::CustomerAddressAdded { timestamp, .. }
            | Self::ProductCreated { timestamp, .. }
            | Self::ProductUpdated { timestamp, .. }
            | Self::ProductStatusChanged { timestamp, .. }
            | Self::ProductVariantAdded { timestamp, .. }
            | Self::ProductVariantUpdated { timestamp, .. }
            | Self::ReturnRequested { timestamp, .. }
            | Self::ReturnStatusChanged { timestamp, .. }
            | Self::ReturnApproved { timestamp, .. }
            | Self::ReturnRejected { timestamp, .. }
            | Self::ReturnCompleted { timestamp, .. }
            | Self::RefundIssued { timestamp, .. } => *timestamp,
        }
    }

    /// Serialize event to JSON
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string(self)
    }

    /// Deserialize event from JSON
    pub fn from_json(json: &str) -> serde_json::Result<Self> {
        serde_json::from_str(json)
    }
}

/// Event store for persisting and replaying events
pub trait EventStore {
    /// Append event to store
    fn append(&self, event: &CommerceEvent) -> crate::errors::Result<u64>;

    /// Get events since sequence number
    fn get_events_since(&self, sequence: u64, limit: u32) -> crate::errors::Result<Vec<(u64, CommerceEvent)>>;

    /// Get events for aggregate
    fn get_events_for_aggregate(&self, aggregate_type: &str, aggregate_id: &str) -> crate::errors::Result<Vec<CommerceEvent>>;

    /// Get latest sequence number
    fn latest_sequence(&self) -> crate::errors::Result<u64>;
}
