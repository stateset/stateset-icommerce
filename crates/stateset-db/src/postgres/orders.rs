//! PostgreSQL order repository implementation

use super::{
    backorder::PgBackorderRepository,
    inventory::{PgInventoryRepository, ReservationConfirmOutcome},
    map_db_error,
};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;
use sqlx::{FromRow, QueryBuilder};
use stateset_core::{
    validate_batch_size, validate_currency_code, validate_postal_code, validate_price,
    validate_required_text, validate_required_uuid, validate_sku, Address, BatchResult,
    CommerceError, CreateBackorder, CreateOrder, CreateOrderItem, FulfillmentStatus, Order, OrderFilter,
    OrderItem, OrderRepository, OrderStatus, PaymentStatus, ReserveInventory, Result, UpdateOrder,
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

    fn validate_order_item_input(item: &CreateOrderItem) -> Result<()> {
        validate_required_uuid("order_item.product_id", item.product_id)?;
        if let Some(variant_id) = item.variant_id {
            validate_required_uuid("order_item.variant_id", variant_id)?;
        }
        validate_sku(&item.sku)?;
        validate_required_text("order_item.name", &item.name, 255)?;

        if item.quantity <= 0 {
            return Err(CommerceError::InvalidInput {
                field: "order_item.quantity".to_string(),
                message: "must be greater than zero".into(),
            });
        }

        validate_price(item.unit_price)?;
        if let Some(discount) = item.discount {
            validate_price(discount)?;
        }
        if let Some(tax) = item.tax_amount {
            validate_price(tax)?;
        }

        let subtotal = item.unit_price * Decimal::from(item.quantity);
        let discount = item.discount.unwrap_or_default();
        let tax = item.tax_amount.unwrap_or_default();
        if discount > subtotal {
            return Err(CommerceError::ValidationError(
                "Order item discount cannot exceed subtotal".into(),
            ));
        }

        let total = subtotal - discount + tax;
        if total < Decimal::ZERO {
            return Err(CommerceError::ValidationError(
                "Order item total cannot be negative".into(),
            ));
        }

        Ok(())
    }

    fn validate_address_input(address: &Address, field_prefix: &str) -> Result<()> {
        validate_required_text(&format!("{field_prefix}.line1"), &address.line1, 255)?;
        validate_required_text(&format!("{field_prefix}.city"), &address.city, 255)?;
        validate_postal_code(&address.postal_code)?;
        validate_required_text(&format!("{field_prefix}.country"), &address.country, 64)?;

        if let Some(line2) = &address.line2 {
            validate_required_text(&format!("{field_prefix}.line2"), line2, 255)?;
        }
        if let Some(state) = &address.state {
            validate_required_text(&format!("{field_prefix}.state"), state, 64)?;
        }

        Ok(())
    }

    fn validate_order_input(input: &CreateOrder) -> Result<()> {
        validate_required_uuid("order.customer_id", input.customer_id)?;

        if let Some(ref currency) = input.currency {
            validate_currency_code(currency)?;
        }

        if input.items.is_empty() {
            return Err(CommerceError::ValidationError(
                "Order must have at least one item".into(),
            ));
        }

        for item in &input.items {
            Self::validate_order_item_input(item)?;
        }

        if let Some(address) = &input.shipping_address {
            Self::validate_address_input(address, "order.shipping_address")?;
        }
        if let Some(address) = &input.billing_address {
            Self::validate_address_input(address, "order.billing_address")?;
        }

        Ok(())
    }

    fn row_to_order(row: OrderRow, items: Vec<OrderItem>) -> Result<Order> {
        let status: OrderStatus = row.status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid order.status '{}': {}",
                row.status, e
            ))
        })?;
        let payment_status: PaymentStatus = row.payment_status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid order.payment_status '{}': {}",
                row.payment_status, e
            ))
        })?;
        let fulfillment_status: FulfillmentStatus = row.fulfillment_status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid order.fulfillment_status '{}': {}",
                row.fulfillment_status, e
            ))
        })?;

        let shipping_address = row
            .shipping_address
            .map(serde_json::from_value)
            .transpose()
            .map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid JSON for order.shipping_address: {}",
                    e
                ))
            })?;
        let billing_address = row
            .billing_address
            .map(serde_json::from_value)
            .transpose()
            .map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid JSON for order.billing_address: {}",
                    e
                ))
            })?;

        Ok(Order {
            id: row.id,
            order_number: row.order_number,
            customer_id: row.customer_id,
            status,
            order_date: row.order_date,
            total_amount: row.total_amount,
            currency: row.currency,
            payment_status,
            fulfillment_status,
            payment_method: row.payment_method,
            shipping_method: row.shipping_method,
            tracking_number: row.tracking_number,
            notes: row.notes,
            shipping_address,
            billing_address,
            items,
            version: row.version,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
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

    async fn update_order_total_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        order_id: Uuid,
    ) -> Result<()> {
        let current_version: i32 = sqlx::query_scalar(
            "SELECT version FROM orders WHERE id = $1 FOR UPDATE",
        )
        .bind(order_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .ok_or(CommerceError::OrderNotFound(order_id))?;

        let total: Decimal = sqlx::query_scalar(
            "SELECT COALESCE(SUM(total), 0) FROM order_items WHERE order_id = $1",
        )
        .bind(order_id)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let result = sqlx::query(
            "UPDATE orders SET total_amount = $1, updated_at = $2, version = version + 1 WHERE id = $3 AND version = $4",
        )
        .bind(total)
        .bind(Utc::now())
        .bind(order_id)
        .bind(current_version)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        if result.rows_affected() == 0 {
            return Err(CommerceError::VersionConflict {
                entity: "order".to_string(),
                id: order_id.to_string(),
                expected_version: current_version,
            });
        }

        Ok(())
    }

    /// Create an order (async)
    pub async fn create_async(&self, input: CreateOrder) -> Result<Order> {
        Self::validate_order_input(&input)?;

        let id = Uuid::new_v4();
        let now = Utc::now();

        let inventory_repo = PgInventoryRepository::new(self.pool.clone());
        let backorder_repo = PgBackorderRepository::new(self.pool.clone());

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        // Get next order number
        let order_number: (i64,) = sqlx::query_as("SELECT nextval('order_number_seq')")
            .fetch_one(tx.as_mut())
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
            .map(|a| {
                serde_json::to_value(a).map_err(|e| {
                    CommerceError::DatabaseError(format!(
                        "Failed to serialize order.shipping_address: {}",
                        e
                    ))
                })
            })
            .transpose()?;
        let billing_address_json = input
            .billing_address
            .as_ref()
            .map(|a| {
                serde_json::to_value(a).map_err(|e| {
                    CommerceError::DatabaseError(format!(
                        "Failed to serialize order.billing_address: {}",
                        e
                    ))
                })
            })
            .transpose()?;

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
        .execute(tx.as_mut())
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
            .execute(tx.as_mut())
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

        let reference_id = id.to_string();
        for item in &items {
            if item.quantity <= 0 {
                continue;
            }

            let item_id: Option<i64> = sqlx::query_scalar(
                "SELECT id FROM inventory_items WHERE sku = $1",
            )
            .bind(&item.sku)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(map_db_error)?;

            let item_id = match item_id {
                Some(item_id) => item_id,
                None => {
                    // Non-inventory item; skip reservations/backorders.
                    continue;
                }
            };

            let available: Decimal = sqlx::query_scalar(
                "SELECT COALESCE(SUM(quantity_available), 0) FROM inventory_balances WHERE item_id = $1",
            )
            .bind(item_id)
            .fetch_one(tx.as_mut())
            .await
            .map_err(map_db_error)?;

            let requested = Decimal::from(item.quantity);
            let reserve_qty = if available > Decimal::ZERO {
                requested.min(available)
            } else {
                Decimal::ZERO
            };

            let mut reserved = Decimal::ZERO;
            if reserve_qty > Decimal::ZERO {
                let reserve_input = ReserveInventory {
                    sku: item.sku.clone(),
                    location_id: None,
                    quantity: reserve_qty,
                    reference_type: "order".to_string(),
                    reference_id: reference_id.clone(),
                    expires_in_seconds: None,
                };

                match inventory_repo.reserve_in_tx(&mut tx, &reserve_input).await {
                    Ok(_) => {
                        reserved = reserve_qty;
                    }
                    Err(err) => {
                        if matches!(err, CommerceError::InsufficientStock { .. }) {
                            reserved = Decimal::ZERO;
                        } else {
                            return Err(err);
                        }
                    }
                }
            }

            let remaining = requested - reserved;
            if remaining > Decimal::ZERO {
                let backorder_input = CreateBackorder {
                    order_id: id,
                    order_line_id: Some(item.id),
                    customer_id: input.customer_id,
                    sku: item.sku.clone(),
                    quantity: remaining,
                    priority: None,
                    expected_date: None,
                    promised_date: None,
                    source_location_id: None,
                    notes: Some("Auto backorder: insufficient stock".to_string()),
                };
                backorder_repo
                    .create_backorder_in_tx(&mut tx, &backorder_input)
                    .await?;
            }
        }

        tx.commit().await.map_err(map_db_error)?;

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
                Ok(Some(Self::row_to_order(order_row, items)?))
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
                Ok(Some(Self::row_to_order(order_row, items)?))
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
        if let Some(address) = &input.shipping_address {
            Self::validate_address_input(address, "order.shipping_address")?;
        }
        if let Some(address) = &input.billing_address {
            Self::validate_address_input(address, "order.billing_address")?;
        }

        let shipping_address_json = input
            .shipping_address
            .as_ref()
            .map(|a| {
                serde_json::to_value(a).map_err(|e| {
                    CommerceError::DatabaseError(format!(
                        "Failed to serialize order.shipping_address: {}",
                        e
                    ))
                })
            })
            .transpose()?;
        let billing_address_json = input
            .billing_address
            .as_ref()
            .map(|a| {
                serde_json::to_value(a).map_err(|e| {
                    CommerceError::DatabaseError(format!(
                        "Failed to serialize order.billing_address: {}",
                        e
                    ))
                })
            })
            .transpose()?;

        let inventory_repo = PgInventoryRepository::new(self.pool.clone());
        let backorder_repo = PgBackorderRepository::new(self.pool.clone());

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let existing_row = sqlx::query_as::<_, OrderRow>(
            "SELECT * FROM orders WHERE id = $1 FOR UPDATE",
        )
        .bind(id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .ok_or(CommerceError::OrderNotFound(id))?;

        let OrderRow {
            status: current_status_raw,
            payment_status: current_payment_status_raw,
            fulfillment_status: current_fulfillment_status_raw,
            tracking_number,
            notes,
            shipping_address,
            billing_address,
            version: expected_version,
            ..
        } = existing_row;

        let current_status: OrderStatus = current_status_raw.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid order.status '{}': {}", current_status_raw, e))
        })?;
        let current_payment_status: PaymentStatus = current_payment_status_raw.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid order.payment_status '{}': {}",
                current_payment_status_raw, e
            ))
        })?;
        let current_fulfillment_status: FulfillmentStatus = current_fulfillment_status_raw.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid order.fulfillment_status '{}': {}",
                current_fulfillment_status_raw, e
            ))
        })?;

        let new_status = input.status.unwrap_or(current_status);
        let new_payment_status = input.payment_status.unwrap_or(current_payment_status);
        let new_fulfillment_status = input.fulfillment_status.unwrap_or(current_fulfillment_status);
        let now = Utc::now();

        if !current_status.can_transition_to(new_status) {
            if new_status == OrderStatus::Cancelled {
                return Err(CommerceError::OrderCannotBeCancelled(current_status.to_string()));
            }

            return Err(CommerceError::InvalidOrderStatusTransition {
                from: current_status.to_string(),
                to: new_status.to_string(),
            });
        }

        if new_status == OrderStatus::Refunded
            && !matches!(
                new_payment_status,
                PaymentStatus::Paid
                    | PaymentStatus::PartiallyPaid
                    | PaymentStatus::Refunded
                    | PaymentStatus::PartiallyRefunded
            )
        {
            return Err(CommerceError::OrderCannotBeRefunded(
                new_payment_status.to_string(),
            ));
        }

        if matches!(input.status, Some(OrderStatus::Shipped)) {
            let reservation_ids = inventory_repo
                .list_reservation_ids_by_reference_in_tx(&mut tx, "order", &id.to_string())
                .await?;

            let mut expired_reservation: Option<Uuid> = None;
            for reservation_id in &reservation_ids {
                if inventory_repo
                    .expire_reservation_if_needed_in_tx(&mut tx, *reservation_id, now)
                    .await?
                {
                    if expired_reservation.is_none() {
                        expired_reservation = Some(*reservation_id);
                    }
                }
            }

            if let Some(expired_id) = expired_reservation {
                tx.commit().await.map_err(map_db_error)?;
                return Err(CommerceError::ReservationExpired(expired_id));
            }

            for reservation_id in reservation_ids {
                match inventory_repo
                    .confirm_reservation_in_tx_with_now(&mut tx, reservation_id, now)
                    .await?
                {
                    ReservationConfirmOutcome::Confirmed => {}
                    ReservationConfirmOutcome::Expired => {
                        if expired_reservation.is_none() {
                            expired_reservation = Some(reservation_id);
                        }
                        break;
                    }
                }
            }

            if let Some(expired_id) = expired_reservation {
                tx.commit().await.map_err(map_db_error)?;
                return Err(CommerceError::ReservationExpired(expired_id));
            }
        }

        let new_tracking = input.tracking_number.or(tracking_number);
        let new_notes = input.notes.or(notes);
        let new_shipping = shipping_address_json.or(shipping_address);
        let new_billing = billing_address_json.or(billing_address);

        let result = sqlx::query(
            r#"
            UPDATE orders
            SET status = $1, payment_status = $2, fulfillment_status = $3,
                tracking_number = $4, notes = $5, shipping_address = $6,
                billing_address = $7, updated_at = $8, version = version + 1
            WHERE id = $9 AND version = $10
            "#,
        )
        .bind(new_status.to_string())
        .bind(new_payment_status.to_string())
        .bind(new_fulfillment_status.to_string())
        .bind(&new_tracking)
        .bind(&new_notes)
        .bind(&new_shipping)
        .bind(&new_billing)
        .bind(now)
        .bind(id)
        .bind(expected_version)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        if result.rows_affected() == 0 {
            return Err(CommerceError::VersionConflict {
                entity: "order".to_string(),
                id: id.to_string(),
                expected_version,
            });
        }

        if matches!(input.status, Some(OrderStatus::Cancelled)) {
            let reservation_ids = inventory_repo
                .list_reservation_ids_by_reference_in_tx(&mut tx, "order", &id.to_string())
                .await?;
            for reservation_id in reservation_ids {
                inventory_repo
                    .release_reservation_in_tx(&mut tx, reservation_id)
                    .await?;
            }
            backorder_repo
                .cancel_backorders_for_order_in_tx(&mut tx, id)
                .await?;
        }

        tx.commit().await.map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::OrderNotFound(id))
    }

    /// List orders (async)
    pub async fn list_async(&self, filter: OrderFilter) -> Result<Vec<Order>> {
        let OrderFilter {
            customer_id,
            status,
            payment_status,
            fulfillment_status,
            from_date,
            to_date,
            limit,
            offset,
        } = filter;

        let mut builder = QueryBuilder::new("SELECT * FROM orders WHERE 1=1");

        if let Some(customer_id) = customer_id {
            builder.push(" AND customer_id = ").push_bind(customer_id);
        }
        if let Some(status) = status {
            builder.push(" AND status = ").push_bind(status.to_string());
        }
        if let Some(payment_status) = payment_status {
            builder
                .push(" AND payment_status = ")
                .push_bind(payment_status.to_string());
        }
        if let Some(fulfillment_status) = fulfillment_status {
            builder
                .push(" AND fulfillment_status = ")
                .push_bind(fulfillment_status.to_string());
        }
        if let Some(from) = from_date {
            builder.push(" AND order_date >= ").push_bind(from);
        }
        if let Some(to) = to_date {
            builder.push(" AND order_date <= ").push_bind(to);
        }

        builder.push(" ORDER BY order_date DESC");

        if let Some(limit) = limit {
            builder.push(" LIMIT ").push_bind(limit as i64);
        }
        if let Some(offset) = offset {
            builder.push(" OFFSET ").push_bind(offset as i64);
        }

        let rows = builder
            .build_query_as::<OrderRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        let mut orders = Vec::new();
        for row in rows {
            let items = self.get_items_async(row.id).await?;
            orders.push(Self::row_to_order(row, items)?);
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
        validate_required_uuid("order.id", order_id)?;
        Self::validate_order_item_input(&item)?;

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
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
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        self.update_order_total_tx(&mut tx, order_id).await?;
        tx.commit().await.map_err(map_db_error)?;

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
    pub async fn remove_item_async(&self, order_id: Uuid, item_id: Uuid) -> Result<()> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        sqlx::query("DELETE FROM order_items WHERE id = $1 AND order_id = $2")
            .bind(item_id)
            .bind(order_id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        self.update_order_total_tx(&mut tx, order_id).await?;
        tx.commit().await.map_err(map_db_error)?;

        Ok(())
    }

    /// Count orders (async)
    pub async fn count_async(&self, filter: OrderFilter) -> Result<u64> {
        let OrderFilter {
            customer_id,
            status,
            payment_status,
            fulfillment_status,
            from_date,
            to_date,
            limit: _,
            offset: _,
        } = filter;

        let mut builder = QueryBuilder::new("SELECT COUNT(*) FROM orders WHERE 1=1");

        if let Some(customer_id) = customer_id {
            builder.push(" AND customer_id = ").push_bind(customer_id);
        }
        if let Some(status) = status {
            builder.push(" AND status = ").push_bind(status.to_string());
        }
        if let Some(payment_status) = payment_status {
            builder
                .push(" AND payment_status = ")
                .push_bind(payment_status.to_string());
        }
        if let Some(fulfillment_status) = fulfillment_status {
            builder
                .push(" AND fulfillment_status = ")
                .push_bind(fulfillment_status.to_string());
        }
        if let Some(from) = from_date {
            builder.push(" AND order_date >= ").push_bind(from);
        }
        if let Some(to) = to_date {
            builder.push(" AND order_date <= ").push_bind(to);
        }

        let count: (i64,) = builder
            .build_query_as()
            .fetch_one(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(count.0 as u64)
    }

    /// Create multiple orders in a batch (async, non-atomic)
    pub async fn create_batch_async(&self, inputs: Vec<CreateOrder>) -> Result<BatchResult<Order>> {
        validate_batch_size(&inputs)?;
        let mut result = BatchResult::with_capacity(inputs.len());

        for (index, input) in inputs.into_iter().enumerate() {
            match self.create_async(input).await {
                Ok(order) => result.record_success(order),
                Err(e) => result.record_failure(index, None, &e),
            }
        }

        Ok(result)
    }

    /// Create multiple orders in a batch atomically (async)
    pub async fn create_batch_atomic_async(&self, inputs: Vec<CreateOrder>) -> Result<Vec<Order>> {
        validate_batch_size(&inputs)?;
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let mut orders = Vec::with_capacity(inputs.len());

        for input in inputs {
            Self::validate_order_input(&input)?;

            let id = Uuid::new_v4();
            let now = Utc::now();

            // Get next order number
            let order_number: (i64,) = sqlx::query_as("SELECT nextval('order_number_seq')")
                .fetch_one(tx.as_mut())
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
                .map(|a| {
                    serde_json::to_value(a).map_err(|e| {
                        CommerceError::DatabaseError(format!(
                            "Failed to serialize order.shipping_address: {}",
                            e
                        ))
                    })
                })
                .transpose()?;
            let billing_address_json = input
                .billing_address
                .as_ref()
                .map(|a| {
                    serde_json::to_value(a).map_err(|e| {
                        CommerceError::DatabaseError(format!(
                            "Failed to serialize order.billing_address: {}",
                            e
                        ))
                    })
                })
                .transpose()?;

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
            .execute(tx.as_mut())
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
                .execute(tx.as_mut())
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

            orders.push(Order {
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
            });
        }

        tx.commit().await.map_err(map_db_error)?;
        Ok(orders)
    }

    /// Update multiple orders in a batch (async, non-atomic)
    pub async fn update_batch_async(&self, updates: Vec<(Uuid, UpdateOrder)>) -> Result<BatchResult<Order>> {
        validate_batch_size(&updates)?;
        let mut result = BatchResult::with_capacity(updates.len());

        for (index, (id, input)) in updates.into_iter().enumerate() {
            match self.update_async(id, input).await {
                Ok(order) => result.record_success(order),
                Err(e) => result.record_failure(index, Some(id.to_string()), &e),
            }
        }

        Ok(result)
    }

    /// Update multiple orders in a batch atomically (async)
    pub async fn update_batch_atomic_async(&self, updates: Vec<(Uuid, UpdateOrder)>) -> Result<Vec<Order>> {
        validate_batch_size(&updates)?;
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let mut orders = Vec::with_capacity(updates.len());
        let now = Utc::now();

        for (id, input) in updates {
            // Get existing order
            let existing_row = sqlx::query_as::<_, OrderRow>("SELECT * FROM orders WHERE id = $1 FOR UPDATE")
                .bind(id)
                .fetch_optional(tx.as_mut())
                .await
                .map_err(map_db_error)?
                .ok_or(CommerceError::OrderNotFound(id))?;

            let existing_items = sqlx::query_as::<_, OrderItemRow>("SELECT * FROM order_items WHERE order_id = $1")
                .bind(id)
                .fetch_all(tx.as_mut())
                .await
                .map_err(map_db_error)?;

            let existing = Self::row_to_order(
                existing_row,
                existing_items.into_iter().map(Self::row_to_item).collect(),
            )?;
            let expected_version = existing.version;

            let new_status = input.status.unwrap_or(existing.status);
            let new_payment_status = input.payment_status.unwrap_or(existing.payment_status);
            let new_fulfillment_status = input.fulfillment_status.unwrap_or(existing.fulfillment_status);
            let new_tracking = input.tracking_number.or(existing.tracking_number);
            let new_notes = input.notes.or(existing.notes);
            let new_shipping = input
                .shipping_address
                .clone()
                .or(existing.shipping_address);
            let new_billing = input
                .billing_address
                .clone()
                .or(existing.billing_address);

            if !existing.status.can_transition_to(new_status) {
                if new_status == OrderStatus::Cancelled {
                    return Err(CommerceError::OrderCannotBeCancelled(existing.status.to_string()));
                }

                return Err(CommerceError::InvalidOrderStatusTransition {
                    from: existing.status.to_string(),
                    to: new_status.to_string(),
                });
            }

            if new_status == OrderStatus::Refunded
                && !matches!(
                    new_payment_status,
                    PaymentStatus::Paid
                        | PaymentStatus::PartiallyPaid
                        | PaymentStatus::Refunded
                        | PaymentStatus::PartiallyRefunded
                )
            {
                return Err(CommerceError::OrderCannotBeRefunded(
                    new_payment_status.to_string(),
                ));
            }

            if let Some(address) = &input.shipping_address {
                Self::validate_address_input(address, "order.shipping_address")?;
            }
            if let Some(address) = &input.billing_address {
                Self::validate_address_input(address, "order.billing_address")?;
            }

            let shipping_json = new_shipping
                .as_ref()
                .map(|a| {
                    serde_json::to_value(a).map_err(|e| {
                        CommerceError::DatabaseError(format!(
                            "Failed to serialize order.shipping_address: {}",
                            e
                        ))
                    })
                })
                .transpose()?;
            let billing_json = new_billing
                .as_ref()
                .map(|a| {
                    serde_json::to_value(a).map_err(|e| {
                        CommerceError::DatabaseError(format!(
                            "Failed to serialize order.billing_address: {}",
                            e
                        ))
                    })
                })
                .transpose()?;

            let result = sqlx::query(
                r#"
                UPDATE orders
                SET status = $1, payment_status = $2, fulfillment_status = $3,
                    tracking_number = $4, notes = $5, shipping_address = $6,
                    billing_address = $7, updated_at = $8, version = version + 1
                WHERE id = $9 AND version = $10
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
            .bind(expected_version)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

            if result.rows_affected() == 0 {
                return Err(CommerceError::VersionConflict {
                    entity: "order".to_string(),
                    id: id.to_string(),
                    expected_version,
                });
            }

            // Fetch updated order
            let updated_row = sqlx::query_as::<_, OrderRow>("SELECT * FROM orders WHERE id = $1")
                .bind(id)
                .fetch_one(tx.as_mut())
                .await
                .map_err(map_db_error)?;

            let items = sqlx::query_as::<_, OrderItemRow>("SELECT * FROM order_items WHERE order_id = $1")
                .bind(id)
                .fetch_all(tx.as_mut())
                .await
                .map_err(map_db_error)?;

            orders.push(Self::row_to_order(
                updated_row,
                items.into_iter().map(Self::row_to_item).collect(),
            )?);
        }

        tx.commit().await.map_err(map_db_error)?;
        Ok(orders)
    }

    /// Delete multiple orders in a batch (async, non-atomic)
    pub async fn delete_batch_async(&self, ids: Vec<Uuid>) -> Result<BatchResult<Uuid>> {
        validate_batch_size(&ids)?;
        let mut result = BatchResult::with_capacity(ids.len());

        for (index, id) in ids.into_iter().enumerate() {
            match self.delete_async(id).await {
                Ok(()) => result.record_success(id),
                Err(e) => result.record_failure(index, Some(id.to_string()), &e),
            }
        }

        Ok(result)
    }

    /// Delete multiple orders in a batch atomically (async)
    pub async fn delete_batch_atomic_async(&self, ids: Vec<Uuid>) -> Result<()> {
        validate_batch_size(&ids)?;
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        // Delete order items first (foreign key constraint)
        sqlx::query("DELETE FROM order_items WHERE order_id = ANY($1)")
            .bind(&ids)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        // Delete orders
        sqlx::query("DELETE FROM orders WHERE id = ANY($1)")
            .bind(&ids)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;
        Ok(())
    }

    /// Get multiple orders by IDs (async)
    pub async fn get_batch_async(&self, ids: Vec<Uuid>) -> Result<Vec<Order>> {
        validate_batch_size(&ids)?;

        let rows = sqlx::query_as::<_, OrderRow>("SELECT * FROM orders WHERE id = ANY($1)")
            .bind(&ids)
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        let mut orders = Vec::with_capacity(rows.len());
        for row in rows {
            let items = self.get_items_async(row.id).await?;
            orders.push(Self::row_to_order(row, items)?);
        }

        Ok(orders)
    }
}

impl OrderRepository for PgOrderRepository {
    fn create(&self, input: CreateOrder) -> Result<Order> {
        super::block_on(self.create_async(input))
    }

    fn get(&self, id: Uuid) -> Result<Option<Order>> {
        super::block_on(self.get_async(id))
    }

    fn get_by_number(&self, order_number: &str) -> Result<Option<Order>> {
        super::block_on(self.get_by_number_async(order_number))
    }

    fn update(&self, id: Uuid, input: UpdateOrder) -> Result<Order> {
        super::block_on(self.update_async(id, input))
    }

    fn list(&self, filter: OrderFilter) -> Result<Vec<Order>> {
        super::block_on(self.list_async(filter))
    }

    fn delete(&self, id: Uuid) -> Result<()> {
        super::block_on(self.delete_async(id))
    }

    fn add_item(&self, order_id: Uuid, item: CreateOrderItem) -> Result<OrderItem> {
        super::block_on(self.add_item_async(order_id, item))
    }

    fn remove_item(&self, order_id: Uuid, item_id: Uuid) -> Result<()> {
        super::block_on(self.remove_item_async(order_id, item_id))
    }

    fn count(&self, filter: OrderFilter) -> Result<u64> {
        super::block_on(self.count_async(filter))
    }

    fn create_batch(&self, inputs: Vec<CreateOrder>) -> Result<BatchResult<Order>> {
        super::block_on(self.create_batch_async(inputs))
    }

    fn create_batch_atomic(&self, inputs: Vec<CreateOrder>) -> Result<Vec<Order>> {
        super::block_on(self.create_batch_atomic_async(inputs))
    }

    fn update_batch(&self, updates: Vec<(Uuid, UpdateOrder)>) -> Result<BatchResult<Order>> {
        super::block_on(self.update_batch_async(updates))
    }

    fn update_batch_atomic(&self, updates: Vec<(Uuid, UpdateOrder)>) -> Result<Vec<Order>> {
        super::block_on(self.update_batch_atomic_async(updates))
    }

    fn delete_batch(&self, ids: Vec<Uuid>) -> Result<BatchResult<Uuid>> {
        super::block_on(self.delete_batch_async(ids))
    }

    fn delete_batch_atomic(&self, ids: Vec<Uuid>) -> Result<()> {
        super::block_on(self.delete_batch_atomic_async(ids))
    }

    fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<Order>> {
        super::block_on(self.get_batch_async(ids))
    }
}
