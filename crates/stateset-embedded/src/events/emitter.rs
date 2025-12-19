//! Event emitter for publishing events to the event bus

use super::bus::EventBus;
use stateset_core::CommerceEvent;
use std::sync::Arc;

/// Event emitter that publishes events to the event bus
#[derive(Clone)]
pub struct EventEmitter {
    bus: Arc<EventBus>,
}

impl EventEmitter {
    /// Create a new event emitter connected to the given bus
    pub fn new(bus: Arc<EventBus>) -> Self {
        Self { bus }
    }

    /// Emit an event (non-blocking)
    pub fn emit(&self, event: CommerceEvent) {
        let receivers = self.bus.publish(event.clone());
        tracing::debug!(
            event_type = event.event_type(),
            receivers,
            "Event emitted"
        );
    }

    /// Emit multiple events
    pub fn emit_all(&self, events: impl IntoIterator<Item = CommerceEvent>) {
        for event in events {
            self.emit(event);
        }
    }

    /// Get the number of active subscribers
    pub fn subscriber_count(&self) -> usize {
        self.bus.receiver_count()
    }

    /// Get total events published through this emitter's bus
    pub fn total_events(&self) -> u64 {
        self.bus.events_published()
    }
}

/// Helper macros and builders for creating events
impl EventEmitter {
    /// Create and emit an OrderCreated event
    pub fn order_created(
        &self,
        order_id: uuid::Uuid,
        customer_id: uuid::Uuid,
        total_amount: rust_decimal::Decimal,
        item_count: usize,
    ) {
        self.emit(CommerceEvent::OrderCreated {
            order_id,
            customer_id,
            total_amount,
            item_count,
            timestamp: chrono::Utc::now(),
        });
    }

    /// Create and emit an OrderStatusChanged event
    pub fn order_status_changed(
        &self,
        order_id: uuid::Uuid,
        from_status: stateset_core::OrderStatus,
        to_status: stateset_core::OrderStatus,
    ) {
        self.emit(CommerceEvent::OrderStatusChanged {
            order_id,
            from_status,
            to_status,
            timestamp: chrono::Utc::now(),
        });
    }

    /// Create and emit an OrderCancelled event
    pub fn order_cancelled(&self, order_id: uuid::Uuid, reason: Option<String>) {
        self.emit(CommerceEvent::OrderCancelled {
            order_id,
            reason,
            timestamp: chrono::Utc::now(),
        });
    }

    /// Create and emit an InventoryAdjusted event
    pub fn inventory_adjusted(
        &self,
        item_id: i64,
        sku: String,
        location_id: i32,
        quantity_change: rust_decimal::Decimal,
        new_quantity: rust_decimal::Decimal,
        reason: String,
    ) {
        self.emit(CommerceEvent::InventoryAdjusted {
            item_id,
            sku,
            location_id,
            quantity_change,
            new_quantity,
            reason,
            timestamp: chrono::Utc::now(),
        });
    }

    /// Create and emit a LowStockAlert event
    pub fn low_stock_alert(
        &self,
        sku: String,
        location_id: i32,
        current_quantity: rust_decimal::Decimal,
        reorder_point: rust_decimal::Decimal,
    ) {
        self.emit(CommerceEvent::LowStockAlert {
            sku,
            location_id,
            current_quantity,
            reorder_point,
            timestamp: chrono::Utc::now(),
        });
    }

    /// Create and emit a CustomerCreated event
    pub fn customer_created(&self, customer_id: uuid::Uuid, email: String) {
        self.emit(CommerceEvent::CustomerCreated {
            customer_id,
            email,
            timestamp: chrono::Utc::now(),
        });
    }

    /// Create and emit a ProductCreated event
    pub fn product_created(&self, product_id: uuid::Uuid, name: String, slug: String) {
        self.emit(CommerceEvent::ProductCreated {
            product_id,
            name,
            slug,
            timestamp: chrono::Utc::now(),
        });
    }

    /// Create and emit a ReturnRequested event
    pub fn return_requested(
        &self,
        return_id: uuid::Uuid,
        order_id: uuid::Uuid,
        customer_id: uuid::Uuid,
        reason: stateset_core::ReturnReason,
        item_count: usize,
    ) {
        self.emit(CommerceEvent::ReturnRequested {
            return_id,
            order_id,
            customer_id,
            reason,
            item_count,
            timestamp: chrono::Utc::now(),
        });
    }

    /// Create and emit a ReturnApproved event
    pub fn return_approved(&self, return_id: uuid::Uuid, order_id: uuid::Uuid) {
        self.emit(CommerceEvent::ReturnApproved {
            return_id,
            order_id,
            timestamp: chrono::Utc::now(),
        });
    }

    /// Create and emit a RefundIssued event
    pub fn refund_issued(
        &self,
        return_id: uuid::Uuid,
        order_id: uuid::Uuid,
        amount: rust_decimal::Decimal,
        method: String,
    ) {
        self.emit(CommerceEvent::RefundIssued {
            return_id,
            order_id,
            amount,
            method,
            timestamp: chrono::Utc::now(),
        });
    }
}
