//! Domain events for commerce operations.
//!
//! Events are emitted when significant state changes occur in the commerce
//! system. They follow an append-only event-sourcing pattern and can be used
//! for:
//!
//! - **Audit logging** — immutable record of all state transitions
//! - **Sync with remote services** — replicate changes to external systems
//! - **Triggering side effects** — email, webhooks, analytics, etc.
//!
//! All events are serializable via [`serde`] using tagged JSON (`"type": "snake_case"`)
//! for easy consumption by downstream event processors.
//!
//! # Example
//!
//! ```rust
//! use stateset_core::events::CommerceEvent;
//! use stateset_core::{OrderId, CustomerId};
//!
//! let event = CommerceEvent::OrderCreated {
//!     order_id: OrderId::new(),
//!     customer_id: CustomerId::new(),
//!     total_amount: rust_decimal::Decimal::new(9999, 2),
//!     item_count: 3,
//!     timestamp: chrono::Utc::now(),
//! };
//!
//! let json = serde_json::to_string(&event).unwrap();
//! assert!(json.contains("\"type\":\"order_created\""));
//! ```

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_primitives::{CartId, CustomerId, OrderId, OrderItemId, PaymentId, ProductId, ReturnId, ShipmentId};
use uuid::Uuid;

use crate::models::{
    CustomerStatus, FulfillmentStatus, OrderStatus, PaymentStatus, ReturnReason, ReturnStatus,
    TrustLevel, X402Asset, X402Network,
};

/// Commerce domain event
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CommerceEvent {
    // Order events
    OrderCreated {
        order_id: OrderId,
        customer_id: CustomerId,
        total_amount: Decimal,
        item_count: usize,
        timestamp: DateTime<Utc>,
    },
    OrderStatusChanged {
        order_id: OrderId,
        from_status: OrderStatus,
        to_status: OrderStatus,
        timestamp: DateTime<Utc>,
    },
    OrderPaymentStatusChanged {
        order_id: OrderId,
        from_status: PaymentStatus,
        to_status: PaymentStatus,
        timestamp: DateTime<Utc>,
    },
    OrderFulfillmentStatusChanged {
        order_id: OrderId,
        from_status: FulfillmentStatus,
        to_status: FulfillmentStatus,
        timestamp: DateTime<Utc>,
    },
    OrderCancelled {
        order_id: OrderId,
        reason: Option<String>,
        timestamp: DateTime<Utc>,
    },
    OrderItemAdded {
        order_id: OrderId,
        item_id: OrderItemId,
        sku: String,
        quantity: i32,
        timestamp: DateTime<Utc>,
    },
    OrderItemRemoved {
        order_id: OrderId,
        item_id: OrderItemId,
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
        customer_id: CustomerId,
        email: String,
        timestamp: DateTime<Utc>,
    },
    CustomerUpdated {
        customer_id: CustomerId,
        fields_changed: Vec<String>,
        timestamp: DateTime<Utc>,
    },
    CustomerStatusChanged {
        customer_id: CustomerId,
        from_status: CustomerStatus,
        to_status: CustomerStatus,
        timestamp: DateTime<Utc>,
    },
    CustomerAddressAdded {
        customer_id: CustomerId,
        address_id: Uuid,
        timestamp: DateTime<Utc>,
    },

    // Product events
    ProductCreated {
        product_id: ProductId,
        name: String,
        slug: String,
        timestamp: DateTime<Utc>,
    },
    ProductUpdated {
        product_id: ProductId,
        fields_changed: Vec<String>,
        timestamp: DateTime<Utc>,
    },
    ProductStatusChanged {
        product_id: ProductId,
        from_status: String,
        to_status: String,
        timestamp: DateTime<Utc>,
    },
    ProductVariantAdded {
        product_id: ProductId,
        variant_id: Uuid,
        sku: String,
        timestamp: DateTime<Utc>,
    },
    ProductVariantUpdated {
        variant_id: Uuid,
        sku: String,
        timestamp: DateTime<Utc>,
    },

    // Custom Objects (custom states / metaobjects)
    CustomObjectTypeCreated {
        type_id: Uuid,
        handle: String,
        timestamp: DateTime<Utc>,
    },
    CustomObjectTypeUpdated {
        type_id: Uuid,
        handle: String,
        fields_changed: Vec<String>,
        timestamp: DateTime<Utc>,
    },
    CustomObjectTypeDeleted {
        type_id: Uuid,
        handle: String,
        timestamp: DateTime<Utc>,
    },
    CustomObjectCreated {
        object_id: Uuid,
        type_handle: String,
        owner_type: Option<String>,
        owner_id: Option<String>,
        timestamp: DateTime<Utc>,
    },
    CustomObjectUpdated {
        object_id: Uuid,
        type_handle: String,
        fields_changed: Vec<String>,
        timestamp: DateTime<Utc>,
    },
    CustomObjectDeleted {
        object_id: Uuid,
        type_handle: String,
        timestamp: DateTime<Utc>,
    },

    // Return events
    ReturnRequested {
        return_id: ReturnId,
        order_id: OrderId,
        customer_id: CustomerId,
        reason: ReturnReason,
        item_count: usize,
        timestamp: DateTime<Utc>,
    },
    ReturnStatusChanged {
        return_id: ReturnId,
        from_status: ReturnStatus,
        to_status: ReturnStatus,
        timestamp: DateTime<Utc>,
    },
    ReturnApproved {
        return_id: ReturnId,
        order_id: OrderId,
        timestamp: DateTime<Utc>,
    },
    ReturnRejected {
        return_id: ReturnId,
        order_id: OrderId,
        reason: String,
        timestamp: DateTime<Utc>,
    },
    ReturnCompleted {
        return_id: ReturnId,
        order_id: OrderId,
        refund_amount: Decimal,
        timestamp: DateTime<Utc>,
    },
    RefundIssued {
        return_id: ReturnId,
        order_id: OrderId,
        amount: Decimal,
        method: String,
        timestamp: DateTime<Utc>,
    },

    // x402 Payment Intent events
    X402IntentCreated {
        intent_id: Uuid,
        cart_id: Option<CartId>,
        payer_address: String,
        payee_address: String,
        amount: u64,
        asset: X402Asset,
        network: X402Network,
        timestamp: DateTime<Utc>,
    },
    X402IntentSigned {
        intent_id: Uuid,
        signing_hash: String,
        payer_public_key: String,
        timestamp: DateTime<Utc>,
    },
    X402IntentSequenced {
        intent_id: Uuid,
        sequence_number: u64,
        batch_id: Uuid,
        timestamp: DateTime<Utc>,
    },
    X402IntentSettled {
        intent_id: Uuid,
        tx_hash: String,
        block_number: u64,
        timestamp: DateTime<Utc>,
    },
    X402IntentFailed {
        intent_id: Uuid,
        reason: String,
        timestamp: DateTime<Utc>,
    },
    X402IntentExpired {
        intent_id: Uuid,
        timestamp: DateTime<Utc>,
    },

    // Agent Card events
    AgentCardCreated {
        agent_id: Uuid,
        name: String,
        wallet_address: String,
        trust_level: TrustLevel,
        timestamp: DateTime<Utc>,
    },
    AgentCardVerified {
        agent_id: Uuid,
        trust_level: TrustLevel,
        verification_method: String,
        timestamp: DateTime<Utc>,
    },
    AgentCardSuspended {
        agent_id: Uuid,
        reason: String,
        timestamp: DateTime<Utc>,
    },
    AgentCardReactivated {
        agent_id: Uuid,
        timestamp: DateTime<Utc>,
    },

    // A2A Commerce events
    A2AQuoteRequested {
        quote_id: Uuid,
        buyer_agent_id: Uuid,
        seller_agent_id: Uuid,
        total: Decimal,
        item_count: usize,
        timestamp: DateTime<Utc>,
    },
    A2AQuoteAccepted {
        quote_id: Uuid,
        buyer_agent_id: Uuid,
        seller_agent_id: Uuid,
        timestamp: DateTime<Utc>,
    },
    A2AQuoteRejected {
        quote_id: Uuid,
        buyer_agent_id: Uuid,
        reason: Option<String>,
        timestamp: DateTime<Utc>,
    },
    A2APurchaseInitiated {
        purchase_id: Uuid,
        quote_id: Option<Uuid>,
        buyer_agent_id: Uuid,
        seller_agent_id: Uuid,
        payment_intent_id: Uuid,
        total: Decimal,
        timestamp: DateTime<Utc>,
    },
    A2APurchasePaid {
        purchase_id: Uuid,
        payment_intent_id: Uuid,
        tx_hash: String,
        timestamp: DateTime<Utc>,
    },
    A2ADeliveryConfirmed {
        purchase_id: Uuid,
        order_id: Option<OrderId>,
        buyer_agent_id: Uuid,
        seller_agent_id: Uuid,
        rating: Option<u8>,
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
            Self::CustomObjectTypeCreated { .. } => "custom_object_type_created",
            Self::CustomObjectTypeUpdated { .. } => "custom_object_type_updated",
            Self::CustomObjectTypeDeleted { .. } => "custom_object_type_deleted",
            Self::CustomObjectCreated { .. } => "custom_object_created",
            Self::CustomObjectUpdated { .. } => "custom_object_updated",
            Self::CustomObjectDeleted { .. } => "custom_object_deleted",
            Self::ReturnRequested { .. } => "return_requested",
            Self::ReturnStatusChanged { .. } => "return_status_changed",
            Self::ReturnApproved { .. } => "return_approved",
            Self::ReturnRejected { .. } => "return_rejected",
            Self::ReturnCompleted { .. } => "return_completed",
            Self::RefundIssued { .. } => "refund_issued",
            // x402 events
            Self::X402IntentCreated { .. } => "x402_intent_created",
            Self::X402IntentSigned { .. } => "x402_intent_signed",
            Self::X402IntentSequenced { .. } => "x402_intent_sequenced",
            Self::X402IntentSettled { .. } => "x402_intent_settled",
            Self::X402IntentFailed { .. } => "x402_intent_failed",
            Self::X402IntentExpired { .. } => "x402_intent_expired",
            // Agent card events
            Self::AgentCardCreated { .. } => "agent_card_created",
            Self::AgentCardVerified { .. } => "agent_card_verified",
            Self::AgentCardSuspended { .. } => "agent_card_suspended",
            Self::AgentCardReactivated { .. } => "agent_card_reactivated",
            // A2A commerce events
            Self::A2AQuoteRequested { .. } => "a2a_quote_requested",
            Self::A2AQuoteAccepted { .. } => "a2a_quote_accepted",
            Self::A2AQuoteRejected { .. } => "a2a_quote_rejected",
            Self::A2APurchaseInitiated { .. } => "a2a_purchase_initiated",
            Self::A2APurchasePaid { .. } => "a2a_purchase_paid",
            Self::A2ADeliveryConfirmed { .. } => "a2a_delivery_confirmed",
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
            | Self::CustomObjectTypeCreated { timestamp, .. }
            | Self::CustomObjectTypeUpdated { timestamp, .. }
            | Self::CustomObjectTypeDeleted { timestamp, .. }
            | Self::CustomObjectCreated { timestamp, .. }
            | Self::CustomObjectUpdated { timestamp, .. }
            | Self::CustomObjectDeleted { timestamp, .. }
            | Self::ReturnRequested { timestamp, .. }
            | Self::ReturnStatusChanged { timestamp, .. }
            | Self::ReturnApproved { timestamp, .. }
            | Self::ReturnRejected { timestamp, .. }
            | Self::ReturnCompleted { timestamp, .. }
            | Self::RefundIssued { timestamp, .. }
            // x402 events
            | Self::X402IntentCreated { timestamp, .. }
            | Self::X402IntentSigned { timestamp, .. }
            | Self::X402IntentSequenced { timestamp, .. }
            | Self::X402IntentSettled { timestamp, .. }
            | Self::X402IntentFailed { timestamp, .. }
            | Self::X402IntentExpired { timestamp, .. }
            // Agent card events
            | Self::AgentCardCreated { timestamp, .. }
            | Self::AgentCardVerified { timestamp, .. }
            | Self::AgentCardSuspended { timestamp, .. }
            | Self::AgentCardReactivated { timestamp, .. }
            // A2A commerce events
            | Self::A2AQuoteRequested { timestamp, .. }
            | Self::A2AQuoteAccepted { timestamp, .. }
            | Self::A2AQuoteRejected { timestamp, .. }
            | Self::A2APurchaseInitiated { timestamp, .. }
            | Self::A2APurchasePaid { timestamp, .. }
            | Self::A2ADeliveryConfirmed { timestamp, .. } => *timestamp,
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
    fn get_events_since(
        &self,
        sequence: u64,
        limit: u32,
    ) -> crate::errors::Result<Vec<(u64, CommerceEvent)>>;

    /// Get events for aggregate
    fn get_events_for_aggregate(
        &self,
        aggregate_type: &str,
        aggregate_id: &str,
    ) -> crate::errors::Result<Vec<CommerceEvent>>;

    /// Get latest sequence number
    fn latest_sequence(&self) -> crate::errors::Result<u64>;
}
