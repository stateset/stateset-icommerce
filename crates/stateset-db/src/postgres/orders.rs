//! PostgreSQL order repository implementation

use super::map_db_error;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;
use sqlx::FromRow;
use stateset_core::{
    Address, CommerceError, CreateOrder, CreateOrderItem, FulfillmentStatus, Order, OrderFilter,
    OrderItem, OrderRepository, OrderStatus, PaymentStatus, Result, UpdateOrder,
};
use uuid::Uuid;

/// PostgreSQL implementation of OrderRepository
#[derive(Clone)]
pub struct PgOrderRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct OrderRow {
    id: Uuid,
    order_number: String,
    customer_id: Uuid,
    status: String,
    order_date: DateTime<Utc>,
    total_amount: Decimal,
    currency: String,
    payment_status: String,
    fulfillment_status: String,
    payment_method: Option<String>,
    shipping_method: Option<String>,
    tracking_number: Option<String>,
    notes: Option<String>,
    shipping_address: Option<serde_json::Value>,
    billing_address: Option<serde_json::Value>,
    version: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct OrderItemRow {
    id: Uuid,
    order_id: Uuid,
    product_id: Uuid,
    variant_id: Option<Uuid>,
    sku: String,
    name: String,
    quantity: i32,
    unit_price: Decimal,
    discount: Decimal,
    tax_amount: Decimal,
    total: Decimal,
    created_at: DateTime<Utc>,
}

impl PgOrderRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_order(row: OrderRow, items: Vec<OrderItem>) -> Order {
        Order {
            id: row.id,
            order_number: row.order_number,
            customer_id: row.customer_id,
            status: parse_order_status(&row.status),
            order_date: row.order_date,
            total_amount: row.total_amount,
            currency: row.currency,
            payment_status: parse_payment_status(&row.payment_status),
            fulfillment_status: parse_fulfillment_status(&row.fulfillment_status),
            payment_method: row.payment_method,
            shipping_method: row.shipping_method,
            tracking_number: row.tracking_number,
            notes: row.notes,
            shipping_address: row.shipping_address.and_then(|v| serde_json::from_value(v).ok()),
            billing_address: row.billing_address.and_then(|v| serde_json::from_value(v).ok()),
            items,
            version: row.version,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }

    fn row_to_item(row: OrderItemRow) -> OrderItem {
        OrderItem {
            id: row.id,
            order_id: row.order_id,
            product_id: row.product_id,
            variant_id: row.variant_id,
            sku: row.sku,
            name: row.name,
            quantity: row.quantity,
            unit_price: row.unit_price,
            discount: row.discount,
            tax_amount: row.tax_amount,
            total: row.total,
        }
    }

    /// Create an order (async)
    pub async fn create_async(&self, input: CreateOrder) -> Result<Order> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        // Get next order number
        let order_number: (i64,) = sqlx::query_as("SELECT nextval('order_number_seq')")
            .fetch_one(&self.pool)
            .await
            .map_err(map_db_error)?;

        let order_number = format!("ORD-{}", order_number.0);

        // Calculate total
        let total: Decimal = input
            .items
            .iter()
            .map(|i| {
                let subtotal = i.unit_price * Decimal::from(i.quantity);
                subtotal - i.discount.unwrap_or(Decimal::ZERO) + i.tax_amount.unwrap_or(Decimal::ZERO)
            })
            .sum();

        let shipping_address_json = input
            .shipping_address
            .as_ref()
            .map(|a| serde_json::to_value(a).unwrap_or_default());
        let billing_address_json = input
            .billing_address
            .as_ref()
            .map(|a| serde_json::to_value(a).unwrap_or_default());

        sqlx::query(
            r#"
            INSERT INTO orders (id, order_number, customer_id, status, order_date, total_amount,
                               currency, payment_status, fulfillment_status, payment_method,
                               shipping_method, notes, shipping_address, billing_address,
                               created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
            "#,
        )
        .bind(id)
        .bind(&order_number)
        .bind(input.customer_id)
        .bind("pending")
        .bind(now)
        .bind(total)
        .bind(input.currency.as_deref().unwrap_or("USD"))
        .bind("pending")
        .bind("unfulfilled")
        .bind(&input.payment_method)
        .bind(&input.shipping_method)
        .bind(&input.notes)
        .bind(&shipping_address_json)
        .bind(&billing_address_json)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        // Insert order items
        let mut items = Vec::new();
        for item_input in &input.items {
            let item_id = Uuid::new_v4();
            let discount = item_input.discount.unwrap_or(Decimal::ZERO);
            let tax = item_input.tax_amount.unwrap_or(Decimal::ZERO);
            let item_total = OrderItem::calculate_total(item_input.quantity, item_input.unit_price, discount, tax);

            sqlx::query(
                r#"
                INSERT INTO order_items (id, order_id, product_id, variant_id, sku, name,
                                         quantity, unit_price, discount, tax_amount, total, created_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                "#,
            )
            .bind(item_id)
            .bind(id)
            .bind(item_input.product_id)
            .bind(item_input.variant_id)
            .bind(&item_input.sku)
            .bind(&item_input.name)
            .bind(item_input.quantity)
            .bind(item_input.unit_price)
            .bind(discount)
            .bind(tax)
            .bind(item_total)
            .bind(now)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

            items.push(OrderItem {
                id: item_id,
                order_id: id,
                product_id: item_input.product_id,
                variant_id: item_input.variant_id,
                sku: item_input.sku.clone(),
                name: item_input.name.clone(),
                quantity: item_input.quantity,
                unit_price: item_input.unit_price,
                discount,
                tax_amount: tax,
                total: item_total,
            });
        }

        Ok(Order {
            id,
            order_number,
            customer_id: input.customer_id,
            status: OrderStatus::Pending,
            order_date: now,
            total_amount: total,
            currency: input.currency.unwrap_or_else(|| "USD".to_string()),
            payment_status: PaymentStatus::Pending,
            fulfillment_status: FulfillmentStatus::Unfulfilled,
            payment_method: input.payment_method,
            shipping_method: input.shipping_method,
            tracking_number: None,
            notes: input.notes,
            shipping_address: input.shipping_address,
            billing_address: input.billing_address,
            items,
            version: 1,
            created_at: now,
            updated_at: now,
        })
    }

    /// Get an order by ID (async)
    pub async fn get_async(&self, id: Uuid) -> Result<Option<Order>> {
        let row = sqlx::query_as::<_, OrderRow>("SELECT * FROM orders WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        match row {
            Some(order_row) => {
                let items = self.get_items_async(id).await?;
                Ok(Some(Self::row_to_order(order_row, items)))
            }
            None => Ok(None),
        }
    }

    /// Get order by number (async)
    pub async fn get_by_number_async(&self, order_number: &str) -> Result<Option<Order>> {
        let row = sqlx::query_as::<_, OrderRow>("SELECT * FROM orders WHERE order_number = $1")
            .bind(order_number)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        match row {
            Some(order_row) => {
                let items = self.get_items_async(order_row.id).await?;
                Ok(Some(Self::row_to_order(order_row, items)))
            }
            None => Ok(None),
        }
    }

    /// Get order items (async)
    pub async fn get_items_async(&self, order_id: Uuid) -> Result<Vec<OrderItem>> {
        let rows = sqlx::query_as::<_, OrderItemRow>("SELECT * FROM order_items WHERE order_id = $1")
            .bind(order_id)
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(rows.into_iter().map(Self::row_to_item).collect())
    }

    /// Update an order (async)
    pub async fn update_async(&self, id: Uuid, input: UpdateOrder) -> Result<Order> {
        let now = Utc::now();

        let existing = self.get_async(id).await?.ok_or(CommerceError::OrderNotFound(id))?;

        let new_status = input.status.unwrap_or(existing.status);
        let new_payment_status = input.payment_status.unwrap_or(existing.payment_status);
        let new_fulfillment_status = input.fulfillment_status.unwrap_or(existing.fulfillment_status);
        let new_tracking = input.tracking_number.or(existing.tracking_number);
        let new_notes = input.notes.or(existing.notes);
        let new_shipping = input.shipping_address.or(existing.shipping_address);
        let new_billing = input.billing_address.or(existing.billing_address);

        let shipping_json = new_shipping.as_ref().map(|a| serde_json::to_value(a).unwrap_or_default());
        let billing_json = new_billing.as_ref().map(|a| serde_json::to_value(a).unwrap_or_default());

        sqlx::query(
            r#"
            UPDATE orders
            SET status = $1, payment_status = $2, fulfillment_status = $3,
                tracking_number = $4, notes = $5, shipping_address = $6,
                billing_address = $7, updated_at = $8
            WHERE id = $9
            "#,
        )
        .bind(new_status.to_string())
        .bind(new_payment_status.to_string())
        .bind(new_fulfillment_status.to_string())
        .bind(&new_tracking)
        .bind(&new_notes)
        .bind(&shipping_json)
        .bind(&billing_json)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::OrderNotFound(id))
    }

    /// List orders (async)
    pub async fn list_async(&self, filter: OrderFilter) -> Result<Vec<Order>> {
        let limit = filter.limit.unwrap_or(100) as i64;
        let offset = filter.offset.unwrap_or(0) as i64;

        let rows = sqlx::query_as::<_, OrderRow>(
            "SELECT * FROM orders ORDER BY created_at DESC LIMIT $1 OFFSET $2",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        let mut orders = Vec::new();
        for row in rows {
            let items = self.get_items_async(row.id).await?;
            orders.push(Self::row_to_order(row, items));
        }

        Ok(orders)
    }

    /// Delete an order (async)
    pub async fn delete_async(&self, id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM orders WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(())
    }

    /// Add item to order (async)
    pub async fn add_item_async(&self, order_id: Uuid, item: CreateOrderItem) -> Result<OrderItem> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let discount = item.discount.unwrap_or(Decimal::ZERO);
        let tax = item.tax_amount.unwrap_or(Decimal::ZERO);
        let total = OrderItem::calculate_total(item.quantity, item.unit_price, discount, tax);

        sqlx::query(
            r#"
            INSERT INTO order_items (id, order_id, product_id, variant_id, sku, name,
                                     quantity, unit_price, discount, tax_amount, total, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            "#,
        )
        .bind(id)
        .bind(order_id)
        .bind(item.product_id)
        .bind(item.variant_id)
        .bind(&item.sku)
        .bind(&item.name)
        .bind(item.quantity)
        .bind(item.unit_price)
        .bind(discount)
        .bind(tax)
        .bind(total)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(OrderItem {
            id,
            order_id,
            product_id: item.product_id,
            variant_id: item.variant_id,
            sku: item.sku,
            name: item.name,
            quantity: item.quantity,
            unit_price: item.unit_price,
            discount,
            tax_amount: tax,
            total,
        })
    }

    /// Remove item from order (async)
    pub async fn remove_item_async(&self, _order_id: Uuid, item_id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM order_items WHERE id = $1")
            .bind(item_id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(())
    }

    /// Count orders (async)
    pub async fn count_async(&self, _filter: OrderFilter) -> Result<u64> {
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM orders")
            .fetch_one(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(count.0 as u64)
    }
}

impl OrderRepository for PgOrderRepository {
    fn create(&self, input: CreateOrder) -> Result<Order> {
        tokio::runtime::Handle::current().block_on(self.create_async(input))
    }

    fn get(&self, id: Uuid) -> Result<Option<Order>> {
        tokio::runtime::Handle::current().block_on(self.get_async(id))
    }

    fn get_by_number(&self, order_number: &str) -> Result<Option<Order>> {
        tokio::runtime::Handle::current().block_on(self.get_by_number_async(order_number))
    }

    fn update(&self, id: Uuid, input: UpdateOrder) -> Result<Order> {
        tokio::runtime::Handle::current().block_on(self.update_async(id, input))
    }

    fn list(&self, filter: OrderFilter) -> Result<Vec<Order>> {
        tokio::runtime::Handle::current().block_on(self.list_async(filter))
    }

    fn delete(&self, id: Uuid) -> Result<()> {
        tokio::runtime::Handle::current().block_on(self.delete_async(id))
    }

    fn add_item(&self, order_id: Uuid, item: CreateOrderItem) -> Result<OrderItem> {
        tokio::runtime::Handle::current().block_on(self.add_item_async(order_id, item))
    }

    fn remove_item(&self, order_id: Uuid, item_id: Uuid) -> Result<()> {
        tokio::runtime::Handle::current().block_on(self.remove_item_async(order_id, item_id))
    }

    fn count(&self, filter: OrderFilter) -> Result<u64> {
        tokio::runtime::Handle::current().block_on(self.count_async(filter))
    }
}

fn parse_order_status(s: &str) -> OrderStatus {
    match s {
        "pending" => OrderStatus::Pending,
        "confirmed" => OrderStatus::Confirmed,
        "processing" => OrderStatus::Processing,
        "shipped" => OrderStatus::Shipped,
        "delivered" => OrderStatus::Delivered,
        "cancelled" => OrderStatus::Cancelled,
        "refunded" => OrderStatus::Refunded,
        _ => OrderStatus::Pending,
    }
}

fn parse_payment_status(s: &str) -> PaymentStatus {
    match s {
        "pending" => PaymentStatus::Pending,
        "authorized" => PaymentStatus::Authorized,
        "paid" => PaymentStatus::Paid,
        "partially_paid" => PaymentStatus::PartiallyPaid,
        "refunded" => PaymentStatus::Refunded,
        "partially_refunded" => PaymentStatus::PartiallyRefunded,
        "failed" => PaymentStatus::Failed,
        _ => PaymentStatus::Pending,
    }
}

fn parse_fulfillment_status(s: &str) -> FulfillmentStatus {
    match s {
        "unfulfilled" => FulfillmentStatus::Unfulfilled,
        "partially_fulfilled" => FulfillmentStatus::PartiallyFulfilled,
        "fulfilled" => FulfillmentStatus::Fulfilled,
        "shipped" => FulfillmentStatus::Shipped,
        "delivered" => FulfillmentStatus::Delivered,
        _ => FulfillmentStatus::Unfulfilled,
    }
}
