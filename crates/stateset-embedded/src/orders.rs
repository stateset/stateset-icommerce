//! Order operations

use rust_decimal::Decimal;
use stateset_core::{
    CommerceError, CreateBackorder, CreateOrder, CreateOrderItem, InventoryReservation, Order,
    OrderFilter, OrderItem, OrderStatus, PaymentStatus, ReserveInventory, Result, UpdateOrder,
};
use stateset_db::Database;
use std::sync::Arc;
use uuid::Uuid;

#[cfg(feature = "events")]
use crate::events::EventSystem;
#[cfg(feature = "events")]
use chrono::Utc;
#[cfg(feature = "events")]
use stateset_core::CommerceEvent;

/// Order operations interface.
pub struct Orders {
    db: Arc<dyn Database>,
    #[cfg(feature = "events")]
    event_system: Arc<EventSystem>,
}

impl Orders {
    #[cfg(feature = "events")]
    pub(crate) fn new(db: Arc<dyn Database>, event_system: Arc<EventSystem>) -> Self {
        Self { db, event_system }
    }

    #[cfg(not(feature = "events"))]
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
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

    fn reservations_for_order(&self, order_id: Uuid) -> Result<Vec<InventoryReservation>> {
        self.db
            .inventory()
            .list_reservations_by_reference("order", &order_id.to_string())
    }

    fn confirm_reservations_for_order(&self, order_id: Uuid) -> Result<()> {
        let reservations = self.reservations_for_order(order_id)?;
        let mut first_error = None;
        for reservation in reservations {
            if let Err(err) = self.db.inventory().confirm_reservation(reservation.id) {
                if first_error.is_none() {
                    first_error = Some(err);
                }
            }
        }
        if let Some(err) = first_error {
            return Err(err);
        }
        Ok(())
    }

    fn release_reservations_for_order(&self, order_id: Uuid) -> Result<()> {
        let reservations = self.reservations_for_order(order_id)?;
        let mut first_error = None;
        for reservation in reservations {
            if let Err(err) = self.db.inventory().release_reservation(reservation.id) {
                if first_error.is_none() {
                    first_error = Some(err);
                }
            }
        }
        if let Some(err) = first_error {
            return Err(err);
        }
        Ok(())
    }

    fn cancel_backorders_for_order(&self, order_id: Uuid) -> Result<()> {
        let backorders = self.db.backorder().get_backorders_for_order(order_id)?;
        let mut first_error = None;
        for backorder in backorders {
            if let Err(err) = self.db.backorder().cancel_backorder(backorder.id) {
                if first_error.is_none() {
                    first_error = Some(err);
                }
            }
        }
        if let Some(err) = first_error {
            return Err(err);
        }
        Ok(())
    }

    fn cleanup_order_after_failure(
        &self,
        order_id: Uuid,
        reservation_ids: &[Uuid],
        backorder_ids: &[Uuid],
    ) {
        for reservation_id in reservation_ids {
            if let Err(err) = self.db.inventory().release_reservation(*reservation_id) {
                tracing::warn!(
                    reservation_id = %reservation_id,
                    error = ?err,
                    "failed to release inventory reservation during order rollback"
                );
            }
        }

        for backorder_id in backorder_ids {
            if let Err(err) = self.db.backorder().cancel_backorder(*backorder_id) {
                tracing::warn!(
                    backorder_id = %backorder_id,
                    error = ?err,
                    "failed to cancel backorder during order rollback"
                );
            }
        }

        if let Err(err) = self.db.orders().delete(order_id) {
            tracing::warn!(
                order_id = %order_id,
                error = ?err,
                "failed to delete order during rollback"
            );
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
    ///     customer_id: uuid::Uuid::new_v4(),
    ///     items: vec![CreateOrderItem {
    ///         product_id: uuid::Uuid::new_v4(),
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
        let inventory = self.db.inventory();
        let backorders = self.db.backorder();

        let order = self.db.orders().create(input)?;

        let mut reservation_ids = Vec::new();
        let mut backorder_ids = Vec::new();
        let reference_id = order.id.to_string();

        for item in &order.items {
            if item.quantity <= 0 {
                continue;
            }

            let has_inventory = match inventory.get_item_by_sku(&item.sku) {
                Ok(Some(_)) => true,
                Ok(None) => false,
                Err(err) => {
                    self.cleanup_order_after_failure(order.id, &reservation_ids, &backorder_ids);
                    return Err(err);
                }
            };

            if !has_inventory {
                continue;
            }

            let available = match inventory.get_stock(&item.sku) {
                Ok(Some(stock)) => stock.total_available,
                Ok(None) => Decimal::ZERO,
                Err(err) => {
                    self.cleanup_order_after_failure(order.id, &reservation_ids, &backorder_ids);
                    return Err(err);
                }
            };

            let requested = Decimal::from(item.quantity);
            let reserve_qty = if available > Decimal::ZERO {
                requested.min(available)
            } else {
                Decimal::ZERO
            };

            let mut reserved = Decimal::ZERO;
            if reserve_qty > Decimal::ZERO {
                match inventory.reserve(ReserveInventory {
                    sku: item.sku.clone(),
                    location_id: None,
                    quantity: reserve_qty,
                    reference_type: "order".to_string(),
                    reference_id: reference_id.clone(),
                    expires_in_seconds: None,
                }) {
                    Ok(reservation) => {
                        reservation_ids.push(reservation.id);
                        reserved = reserve_qty;
                    }
                    Err(CommerceError::InsufficientStock { .. }) => {
                        reserved = Decimal::ZERO;
                    }
                    Err(err) => {
                        self.cleanup_order_after_failure(order.id, &reservation_ids, &backorder_ids);
                        return Err(err);
                    }
                }
            }

            let remaining = requested - reserved;
            if remaining > Decimal::ZERO {
                match backorders.create_backorder(CreateBackorder {
                    order_id: order.id,
                    order_line_id: Some(item.id),
                    customer_id: order.customer_id,
                    sku: item.sku.clone(),
                    quantity: remaining,
                    priority: None,
                    expected_date: None,
                    promised_date: None,
                    source_location_id: None,
                    notes: Some("Auto backorder: insufficient stock".to_string()),
                }) {
                    Ok(backorder) => backorder_ids.push(backorder.id),
                    Err(err) => {
                        self.cleanup_order_after_failure(order.id, &reservation_ids, &backorder_ids);
                        return Err(err);
                    }
                }
            }
        }

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
    pub fn get(&self, id: Uuid) -> Result<Option<Order>> {
        self.db.orders().get(id)
    }

    /// Get an order by order number.
    pub fn get_by_number(&self, order_number: &str) -> Result<Option<Order>> {
        self.db.orders().get_by_number(order_number)
    }

    /// Update an order.
    pub fn update(&self, id: Uuid, input: UpdateOrder) -> Result<Order> {
        let next_status = input.status;
        if matches!(next_status, Some(OrderStatus::Shipped)) {
            self.confirm_reservations_for_order(id)?;
        }

        #[cfg(feature = "events")]
        let previous = self.db.orders().get(id)?;

        let updated = self.db.orders().update(id, input)?;

        #[cfg(feature = "events")]
        if let Some(previous) = previous {
            self.emit_order_change_events(&previous, &updated);
        }

        if matches!(next_status, Some(OrderStatus::Cancelled)) {
            self.release_reservations_for_order(id)?;
            self.cancel_backorders_for_order(id)?;
        }

        Ok(updated)
    }

    /// Update order status.
    #[tracing::instrument(skip(self), fields(order_id = %id, status = ?status))]
    pub fn update_status(&self, id: Uuid, status: OrderStatus) -> Result<Order> {
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
    pub fn list_for_customer(&self, customer_id: Uuid) -> Result<Vec<Order>> {
        self.db.orders().list(OrderFilter {
            customer_id: Some(customer_id),
            ..Default::default()
        })
    }

    /// Delete an order.
    pub fn delete(&self, id: Uuid) -> Result<()> {
        self.db.orders().delete(id)
    }

    /// Add an item to an order.
    pub fn add_item(&self, order_id: Uuid, item: CreateOrderItem) -> Result<OrderItem> {
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
    pub fn remove_item(&self, order_id: Uuid, item_id: Uuid) -> Result<()> {
        self.db.orders().remove_item(order_id, item_id)?;
        #[cfg(feature = "events")]
        {
            self.emit(CommerceEvent::OrderItemRemoved {
                order_id,
                item_id,
                timestamp: Utc::now(),
            });
        }
        Ok(())
    }

    /// Count orders matching a filter.
    pub fn count(&self, filter: OrderFilter) -> Result<u64> {
        self.db.orders().count(filter)
    }

    /// Cancel an order.
    #[tracing::instrument(skip(self), fields(order_id = %id))]
    pub fn cancel(&self, id: Uuid) -> Result<Order> {
        tracing::info!("cancelling order");
        self.update_status(id, OrderStatus::Cancelled)
    }

    /// Mark an order as shipped.
    #[tracing::instrument(skip(self), fields(order_id = %id, has_tracking = tracking_number.is_some()))]
    pub fn ship(&self, id: Uuid, tracking_number: Option<&str>) -> Result<Order> {
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
    pub fn deliver(&self, id: Uuid) -> Result<Order> {
        tracing::info!("marking order as delivered");
        self.update_status(id, OrderStatus::Delivered)
    }
}
