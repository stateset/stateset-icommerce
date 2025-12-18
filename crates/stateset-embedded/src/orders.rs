//! Order operations

use stateset_core::{
    CreateOrder, CreateOrderItem, Order, OrderFilter, OrderItem, OrderStatus,
    Result, UpdateOrder,
};
use stateset_db::Database;
use std::sync::Arc;
use uuid::Uuid;

/// Order operations interface.
pub struct Orders {
    db: Arc<dyn Database>,
}

impl Orders {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
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
    pub fn create(&self, input: CreateOrder) -> Result<Order> {
        self.db.orders().create(input)
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
        self.db.orders().update(id, input)
    }

    /// Update order status.
    pub fn update_status(&self, id: Uuid, status: OrderStatus) -> Result<Order> {
        self.db.orders().update(
            id,
            UpdateOrder {
                status: Some(status),
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
        self.db.orders().add_item(order_id, item)
    }

    /// Remove an item from an order.
    pub fn remove_item(&self, order_id: Uuid, item_id: Uuid) -> Result<()> {
        self.db.orders().remove_item(order_id, item_id)
    }

    /// Count orders matching a filter.
    pub fn count(&self, filter: OrderFilter) -> Result<u64> {
        self.db.orders().count(filter)
    }

    /// Cancel an order.
    pub fn cancel(&self, id: Uuid) -> Result<Order> {
        self.update_status(id, OrderStatus::Cancelled)
    }

    /// Mark an order as shipped.
    pub fn ship(&self, id: Uuid, tracking_number: Option<&str>) -> Result<Order> {
        self.db.orders().update(
            id,
            UpdateOrder {
                status: Some(OrderStatus::Shipped),
                tracking_number: tracking_number.map(|s| s.to_string()),
                ..Default::default()
            },
        )
    }

    /// Mark an order as delivered.
    pub fn deliver(&self, id: Uuid) -> Result<Order> {
        self.update_status(id, OrderStatus::Delivered)
    }
}
