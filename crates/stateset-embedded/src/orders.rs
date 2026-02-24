//! Order operations

use rust_decimal::prelude::ToPrimitive;
use stateset_core::{
    CreateOrder, CreateOrderItem, CustomerId, Order, OrderFilter, OrderId, OrderItem, OrderItemId,
    OrderStatus, PaymentStatus, Result, UpdateOrder,
};
use stateset_db::Database;
use stateset_observability::Metrics;
use std::sync::Arc;

#[cfg(feature = "events")]
use crate::events::EventSystem;
#[cfg(feature = "events")]
use chrono::Utc;
#[cfg(feature = "events")]
use stateset_core::CommerceEvent;

/// Order operations interface.
pub struct Orders {
    db: Arc<dyn Database>,
    metrics: Metrics,
    #[cfg(feature = "events")]
    event_system: Arc<EventSystem>,
}

impl std::fmt::Debug for Orders {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Orders").finish_non_exhaustive()
    }
}

impl Orders {
    #[cfg(feature = "events")]
    pub(crate) fn new(
        db: Arc<dyn Database>,
        event_system: Arc<EventSystem>,
        metrics: Metrics,
    ) -> Self {
        Self { db, metrics, event_system }
    }

    #[cfg(not(feature = "events"))]
    pub(crate) fn new(db: Arc<dyn Database>, metrics: Metrics) -> Self {
        Self { db, metrics }
    }

    #[cfg(feature = "events")]
    fn emit(&self, event: CommerceEvent) {
        self.event_system.emit(event);
    }

    #[cfg(feature = "events")]
    fn emit_order_change_events(&self, previous: &Order, updated: &Order) {
        if previous.status != updated.status {
            self.emit(CommerceEvent::OrderStatusChanged {
                order_id: updated.id,
                from_status: previous.status,
                to_status: updated.status,
                timestamp: updated.updated_at,
            });
            if updated.status == OrderStatus::Cancelled {
                self.emit(CommerceEvent::OrderCancelled {
                    order_id: updated.id,
                    reason: updated.notes.clone(),
                    timestamp: updated.updated_at,
                });
            }
        }

        if previous.payment_status != updated.payment_status {
            self.emit(CommerceEvent::OrderPaymentStatusChanged {
                order_id: updated.id,
                from_status: previous.payment_status,
                to_status: updated.payment_status,
                timestamp: updated.updated_at,
            });
        }

        if previous.fulfillment_status != updated.fulfillment_status {
            self.emit(CommerceEvent::OrderFulfillmentStatusChanged {
                order_id: updated.id,
                from_status: previous.fulfillment_status,
                to_status: updated.fulfillment_status,
                timestamp: updated.updated_at,
            });
        }
    }

    /// Create a new order.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// # use rust_decimal_macros::dec;
    /// # let commerce = Commerce::new(":memory:")?;
    /// let order = commerce.orders().create(CreateOrder {
    ///     customer_id: stateset_core::CustomerId::new(),
    ///     items: vec![CreateOrderItem {
    ///         product_id: stateset_core::ProductId::new(),
    ///         sku: "SKU-001".into(),
    ///         name: "Widget".into(),
    ///         quantity: 2,
    ///         unit_price: dec!(29.99),
    ///         ..Default::default()
    ///     }],
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), CommerceError>(())
    /// ```
    #[tracing::instrument(skip(self, input), fields(customer_id = %input.customer_id, items = input.items.len()))]
    pub fn create(&self, input: CreateOrder) -> Result<Order> {
        tracing::info!("creating order");
        let order = self.db.orders().create(input)?;
        self.metrics.record_order_created(
            &order.customer_id.to_string(),
            order.total_amount.to_f64().unwrap_or(0.0),
        );

        #[cfg(feature = "events")]
        {
            self.emit(CommerceEvent::OrderCreated {
                order_id: order.id,
                customer_id: order.customer_id,
                total_amount: order.total_amount,
                item_count: order.items.len(),
                timestamp: order.created_at,
            });
        }
        Ok(order)
    }

    /// Get an order by ID.
    pub fn get(&self, id: OrderId) -> Result<Option<Order>> {
        self.db.orders().get(id)
    }

    /// Get an order by order number.
    pub fn get_by_number(&self, order_number: &str) -> Result<Option<Order>> {
        self.db.orders().get_by_number(order_number)
    }

    /// Update an order.
    pub fn update(&self, id: OrderId, input: UpdateOrder) -> Result<Order> {
        #[cfg(feature = "events")]
        let previous = self.db.orders().get(id)?;

        let updated = self.db.orders().update(id, input)?;

        #[cfg(feature = "events")]
        if let Some(previous) = previous {
            self.emit_order_change_events(&previous, &updated);
        }

        Ok(updated)
    }

    /// Update order status.
    #[tracing::instrument(skip(self), fields(order_id = %id, status = ?status))]
    pub fn update_status(&self, id: OrderId, status: OrderStatus) -> Result<Order> {
        tracing::info!("updating order status");
        let mut tracking_number = None;
        let mut payment_status = None;
        if status == OrderStatus::Shipped {
            if let Some(order) = self.get(id)? {
                if order.tracking_number.is_none() {
                    tracking_number = Some(format!("AUTO-{}", id));
                }
            }
        }
        if status == OrderStatus::Refunded {
            payment_status = Some(PaymentStatus::Refunded);
        }
        self.update(
            id,
            UpdateOrder {
                status: Some(status),
                payment_status,
                tracking_number,
                ..Default::default()
            },
        )
    }

    /// List orders with optional filtering.
    pub fn list(&self, filter: OrderFilter) -> Result<Vec<Order>> {
        self.db.orders().list(filter)
    }

    /// List orders for a specific customer.
    pub fn list_for_customer(&self, customer_id: CustomerId) -> Result<Vec<Order>> {
        self.db.orders().list(OrderFilter { customer_id: Some(customer_id), ..Default::default() })
    }

    /// Delete an order.
    pub fn delete(&self, id: OrderId) -> Result<()> {
        self.db.orders().delete(id)
    }

    /// Add an item to an order.
    pub fn add_item(&self, order_id: OrderId, item: CreateOrderItem) -> Result<OrderItem> {
        let order_item = self.db.orders().add_item(order_id, item)?;
        #[cfg(feature = "events")]
        {
            self.emit(CommerceEvent::OrderItemAdded {
                order_id,
                item_id: order_item.id,
                sku: order_item.sku.clone(),
                quantity: order_item.quantity,
                timestamp: Utc::now(),
            });
        }
        Ok(order_item)
    }

    /// Remove an item from an order.
    pub fn remove_item(&self, order_id: OrderId, item_id: OrderItemId) -> Result<()> {
        self.db.orders().remove_item(order_id, item_id)?;
        #[cfg(feature = "events")]
        {
            self.emit(CommerceEvent::OrderItemRemoved { order_id, item_id, timestamp: Utc::now() });
        }
        Ok(())
    }

    /// Count orders matching a filter.
    pub fn count(&self, filter: OrderFilter) -> Result<u64> {
        self.db.orders().count(filter)
    }

    /// Cancel an order.
    #[tracing::instrument(skip(self), fields(order_id = %id))]
    pub fn cancel(&self, id: OrderId) -> Result<Order> {
        tracing::info!("cancelling order");
        self.update_status(id, OrderStatus::Cancelled)
    }

    /// Mark an order as shipped.
    #[tracing::instrument(skip(self), fields(order_id = %id, has_tracking = tracking_number.is_some()))]
    pub fn ship(&self, id: OrderId, tracking_number: Option<&str>) -> Result<Order> {
        tracing::info!("shipping order");
        if let Some(order) = self.get(id)? {
            match order.status {
                OrderStatus::Pending => {
                    self.update_status(id, OrderStatus::Confirmed)?;
                    self.update_status(id, OrderStatus::Processing)?;
                }
                OrderStatus::Confirmed => {
                    self.update_status(id, OrderStatus::Processing)?;
                }
                _ => {}
            }
        }
        self.update(
            id,
            UpdateOrder {
                status: Some(OrderStatus::Shipped),
                tracking_number: tracking_number.map(|s| s.to_string()),
                ..Default::default()
            },
        )
    }

    /// Mark an order as delivered.
    #[tracing::instrument(skip(self), fields(order_id = %id))]
    pub fn deliver(&self, id: OrderId) -> Result<Order> {
        tracing::info!("marking order as delivered");
        self.update_status(id, OrderStatus::Delivered)
    }
}
