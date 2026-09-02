//! PostgreSQL order repository implementation

use super::kernel_outbox::append_kernel_event_tx;
use super::{
    backorder::PgBackorderRepository,
    inventory::{PgInventoryRepository, ReservationConfirmOutcome},
    map_db_error,
    payments::{
        open_captures_for_order_pg, order_has_payments_pg, void_in_flight_payments_for_order_pg,
    },
};
use crate::KernelOutboxEvent;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;
use sqlx::{FromRow, QueryBuilder};
use stateset_core::{
    Address, BatchResult, CartId, CommerceError, CreateBackorder, CreateOrder, CreateOrderItem,
    CurrencyCode, CustomerId, FulfillmentStatus, Order, OrderFilter, OrderId, OrderItem,
    OrderItemId, OrderRepository, OrderStatus, PaymentStatus, ProductId, ReserveInventory, Result,
    ShipOrder, ShipmentLineInput, StockPolicy, UpdateOrder, validate_batch_size,
    validate_currency_code, validate_postal_code, validate_price, validate_required_text,
    validate_required_uuid, validate_sku,
};
use uuid::Uuid;

/// PostgreSQL implementation of `OrderRepository`
#[derive(Debug, Clone)]
pub struct PgOrderRepository {
    pool: PgPool,
}

/// How a transition to `Shipped` touches the order lines.
#[derive(Debug, Clone, Copy)]
pub(crate) enum ShipMode<'a> {
    /// Not a shipping update.
    None,
    /// Ship every remaining unit on every line (legacy status flip).
    All,
    /// Ship explicit per-line quantities.
    Lines(&'a [ShipmentLineInput]),
}

/// Units to add to one line's `shipped_quantity`.
#[derive(Debug, Clone)]
pub(crate) struct LineDelta {
    pub(crate) item_id: Uuid,
    pub(crate) sku: String,
    pub(crate) delta: i32,
}

/// What a forced cancel did to the order's payments, for the outbox event.
#[derive(Debug, Default)]
struct CancelMoney {
    voided_payment_ids: Vec<Uuid>,
    outstanding_payment_ids: Vec<Uuid>,
    outstanding_captured: Decimal,
}

/// Result of one in-transaction order update.
///
/// `post_commit_error` carries [`CommerceError::ReservationExpired`] when a
/// shipment found an expired reservation: the expiry bookkeeping must commit,
/// after which the caller surfaces the error (legacy full-ship behaviour).
pub(crate) struct UpdateOutcome {
    pub(crate) order: Order,
    pub(crate) post_commit_error: Option<CommerceError>,
}

/// Refuse a line change on an order whose status no longer allows it.
fn ensure_lines_mutable(id: Uuid, status: OrderStatus) -> Result<()> {
    if status.allows_line_changes() {
        Ok(())
    } else {
        Err(CommerceError::Conflict(format!(
            "order {id} lines cannot be changed while it is {status}; \
             lines may only be added or removed before fulfilment (pending, confirmed, processing)"
        )))
    }
}

/// Refuse deletion of an order that has money against it (see the SQLite
/// twin): its own `payment_status` holds money, or any payment row references
/// it.
fn ensure_no_money_on_delete(
    id: Uuid,
    payment_status: PaymentStatus,
    has_payments: bool,
) -> Result<()> {
    if payment_status.holds_money() {
        return Err(CommerceError::Conflict(format!(
            "order {id} cannot be deleted while its payment status is {payment_status}; \
             refund the order first"
        )));
    }
    if has_payments {
        return Err(CommerceError::Conflict(format!(
            "order {id} cannot be deleted because payments reference it; \
             it is a financial record — cancel or refund instead"
        )));
    }
    Ok(())
}

/// Refuse deletion of an order that is a fulfilment/financial record.
fn ensure_deletable(id: Uuid, status: OrderStatus) -> Result<()> {
    if status.allows_delete() {
        Ok(())
    } else {
        Err(CommerceError::Conflict(format!(
            "order {id} cannot be deleted while it is {status}; \
             shipped, delivered and refunded orders are records — cancel or refund instead"
        )))
    }
}

#[derive(FromRow)]
pub(crate) struct OrderRow {
    id: Uuid,
    order_number: String,
    customer_id: Uuid,
    status: String,
    order_date: DateTime<Utc>,
    total_amount: Decimal,
    tax_amount: Decimal,
    shipping_amount: Decimal,
    discount_amount: Decimal,
    currency: CurrencyCode,
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
pub(crate) struct OrderItemRow {
    id: Uuid,
    order_id: Uuid,
    product_id: Uuid,
    variant_id: Option<Uuid>,
    sku: String,
    name: String,
    quantity: i32,
    shipped_quantity: i32,
    unit_price: Decimal,
    discount: Decimal,
    tax_amount: Decimal,
    total: Decimal,
}

impl PgOrderRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn validate_order_item_input(item: &CreateOrderItem) -> Result<()> {
        validate_required_uuid("order_item.product_id", item.product_id.into_uuid())?;
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

    pub(crate) fn validate_order_input(input: &CreateOrder) -> Result<()> {
        validate_required_uuid("order.customer_id", input.customer_id.into_uuid())?;

        if let Some(currency) = input.currency {
            validate_currency_code(currency.as_str())?;
        }

        if input.items.is_empty() {
            return Err(CommerceError::ValidationError("Order must have at least one item".into()));
        }

        for item in &input.items {
            Self::validate_order_item_input(item)?;
        }

        // Invariant M1 (`commerce.money.scale_exceeds_currency`): no money
        // amount — line or order-level — may carry more decimal places than
        // the order currency allows. Checked here, before the first write, so
        // a rejected order persists nothing.
        input.validate_money_scale()?;
        // Order-level tax/shipping/discount must be non-negative and must not
        // drive `lines + tax + shipping - discount` below zero (refused, never
        // clamped).
        input.validate_order_level_money()?;

        if let Some(address) = &input.shipping_address {
            Self::validate_address_input(address, "order.shipping_address")?;
        }
        if let Some(address) = &input.billing_address {
            Self::validate_address_input(address, "order.billing_address")?;
        }

        Ok(())
    }

    pub(crate) async fn validate_create_order_in_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        input: &CreateOrder,
    ) -> Result<()> {
        Self::validate_order_input(input)?;
        if input.stock_policy.allows_backorder() {
            return Ok(());
        }
        for item in &input.items {
            if item.quantity <= 0 {
                continue;
            }
            let item_id: Option<i64> =
                sqlx::query_scalar("SELECT id FROM inventory_items WHERE sku = $1")
                    .bind(&item.sku)
                    .fetch_optional(tx.as_mut())
                    .await
                    .map_err(map_db_error)?;
            let Some(item_id) = item_id else { continue };
            let available: Decimal = sqlx::query_scalar(
                "SELECT COALESCE(SUM(quantity_available), 0) FROM inventory_balances WHERE item_id = $1",
            )
            .bind(item_id)
            .fetch_one(tx.as_mut())
            .await
            .map_err(map_db_error)?;
            let requested = Decimal::from(item.quantity);
            if available < requested {
                return Err(CommerceError::InsufficientStock {
                    sku: item.sku.clone(),
                    requested: requested.to_string(),
                    available: available.to_string(),
                });
            }
        }
        Ok(())
    }

    pub(crate) fn row_to_order(row: OrderRow, items: Vec<OrderItem>) -> Result<Order> {
        let status: OrderStatus = row.status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid order.status '{}': {}", row.status, e))
        })?;
        let payment_status: PaymentStatus = row.payment_status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid order.payment_status '{}': {}",
                row.payment_status, e
            ))
        })?;
        let fulfillment_status: FulfillmentStatus =
            row.fulfillment_status.parse().map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid order.fulfillment_status '{}': {}",
                    row.fulfillment_status, e
                ))
            })?;

        let shipping_address =
            row.shipping_address.map(serde_json::from_value).transpose().map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid JSON for order.shipping_address: {}",
                    e
                ))
            })?;
        let billing_address =
            row.billing_address.map(serde_json::from_value).transpose().map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid JSON for order.billing_address: {}",
                    e
                ))
            })?;

        Ok(Order {
            id: OrderId::from(row.id),
            order_number: row.order_number,
            customer_id: CustomerId::from(row.customer_id),
            status,
            order_date: row.order_date,
            total_amount: row.total_amount,
            tax_amount: row.tax_amount,
            shipping_amount: row.shipping_amount,
            discount_amount: row.discount_amount,
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

    pub(crate) fn row_to_item(row: OrderItemRow) -> OrderItem {
        OrderItem {
            id: OrderItemId::from(row.id),
            order_id: OrderId::from(row.order_id),
            product_id: ProductId::from(row.product_id),
            variant_id: row.variant_id,
            sku: row.sku,
            name: row.name,
            quantity: row.quantity,
            shipped_quantity: row.shipped_quantity,
            unit_price: row.unit_price,
            discount: row.discount,
            tax_amount: row.tax_amount,
            total: row.total,
        }
    }

    /// Re-derive `total_amount` after a line change.
    ///
    /// The total always foots to `Σ order_items.total + tax_amount +
    /// shipping_amount - discount_amount` — the same rule `create` writes and
    /// [`Order::calculate_total`] reads. It used to be the bare line sum, which
    /// silently dropped the order-level money on the first `add_item`.
    async fn update_order_total_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        order_id: Uuid,
    ) -> Result<()> {
        let (current_version, tax, shipping, discount): (i32, Decimal, Decimal, Decimal) =
            sqlx::query_as(
                "SELECT version, tax_amount, shipping_amount, discount_amount
                 FROM orders WHERE id = $1 FOR UPDATE",
            )
            .bind(order_id)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(map_db_error)?
            .ok_or(CommerceError::OrderNotFound(order_id))?;

        let line_total: Decimal = sqlx::query_scalar(
            "SELECT COALESCE(SUM(total), 0) FROM order_items WHERE order_id = $1",
        )
        .bind(order_id)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        let total = line_total + tax + shipping - discount;

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
        self.create_async_internal(None, false, input).await
    }

    /// Create an order for a cart, using `orders.cart_id` for checkout idempotency.
    pub async fn create_from_cart_async(&self, cart_id: Uuid, input: CreateOrder) -> Result<Order> {
        if let Some(existing) = self.get_by_cart_id_async(cart_id).await? {
            return Ok(existing);
        }

        // If another checkout is racing us, this will return the existing order instead of
        // creating a duplicate.
        self.create_async_internal(Some(cart_id), true, input).await
    }

    async fn create_async_internal(
        &self,
        cart_id: Option<Uuid>,
        idempotent_by_cart_id: bool,
        input: CreateOrder,
    ) -> Result<Order> {
        Self::validate_order_input(&input)?;

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let order = self.create_in_tx(&mut tx, cart_id, idempotent_by_cart_id, input).await?;
        tx.commit().await.map_err(map_db_error)?;
        Ok(order)
    }

    /// Create an order using the caller's transaction. Checkout uses this so
    /// customer resolution, order/items, inventory reservations, cart state,
    /// kernel event, and execution receipt share one commit boundary.
    pub(crate) async fn create_from_cart_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        cart_id: Uuid,
        input: CreateOrder,
    ) -> Result<Order> {
        Self::validate_order_input(&input)?;
        self.create_in_tx(tx, Some(cart_id), true, input).await
    }

    async fn create_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        cart_id: Option<Uuid>,
        idempotent_by_cart_id: bool,
        input: CreateOrder,
    ) -> Result<Order> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        // Get next order number
        let order_number: (i64,) = sqlx::query_as("SELECT nextval('order_number_seq')")
            .fetch_one(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        let order_number = format!("ORD-{}", order_number.0);

        // Order total is the sum of the per-line money totals, each rounded to
        // the currency minor unit by `OrderItem::calculate_total` (the same
        // helper that stores `order_items.total`), so the order foots to its
        // line items and matches the SQLite backend and `update_order_total`.
        let line_total: Decimal = input
            .items
            .iter()
            .map(|i| {
                OrderItem::calculate_total(
                    i.quantity,
                    i.unit_price,
                    i.discount.unwrap_or(Decimal::ZERO),
                    i.tax_amount.unwrap_or(Decimal::ZERO),
                )
            })
            .sum();
        // Order-level money alongside the line sum, so the order records what
        // the customer is actually charged (mirrors the SQLite backend).
        let tax_amount = input.tax_amount.unwrap_or(Decimal::ZERO);
        let shipping_amount = input.shipping_amount.unwrap_or(Decimal::ZERO);
        let discount_amount = input.discount_amount.unwrap_or(Decimal::ZERO);
        let total = line_total + tax_amount + shipping_amount - discount_amount;

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

        // Insert the order row first. When creating from a cart, we ensure at most one order per
        // cart by using `orders.cart_id` and a unique index.
        let (order_id, order_number, inserted) = if idempotent_by_cart_id {
            let cart_id = cart_id.ok_or_else(|| {
                CommerceError::ValidationError("cart_id is required for cart checkout".into())
            })?;

            sqlx::query_as(
                r#"
                INSERT INTO orders (id, order_number, customer_id, status, order_date, total_amount,
                                   tax_amount, shipping_amount, discount_amount,
                                   currency, payment_status, fulfillment_status, payment_method,
                                   shipping_method, notes, shipping_address, billing_address,
                                   cart_id, created_at, updated_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20)
                ON CONFLICT (cart_id) WHERE cart_id IS NOT NULL
                DO UPDATE SET cart_id = EXCLUDED.cart_id
                RETURNING id, order_number, (xmax = 0) AS inserted
                "#,
            )
            .bind(id)
            .bind(&order_number)
            .bind(input.customer_id.into_uuid())
            .bind("pending")
            .bind(now)
            .bind(total)
            .bind(tax_amount)
            .bind(shipping_amount)
            .bind(discount_amount)
            .bind(input.currency.unwrap_or(CurrencyCode::USD).as_str())
            .bind("pending")
            .bind("unfulfilled")
            .bind(&input.payment_method)
            .bind(&input.shipping_method)
            .bind(&input.notes)
            .bind(&shipping_address_json)
            .bind(&billing_address_json)
            .bind(cart_id)
            .bind(now)
            .bind(now)
            .fetch_one(tx.as_mut())
            .await
            .map_err(map_db_error)?
        } else {
            sqlx::query(
                r#"
                INSERT INTO orders (id, order_number, customer_id, status, order_date, total_amount,
                                   tax_amount, shipping_amount, discount_amount,
                                   currency, payment_status, fulfillment_status, payment_method,
                                   shipping_method, notes, shipping_address, billing_address,
                                   created_at, updated_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19)
                "#,
            )
            .bind(id)
            .bind(&order_number)
            .bind(input.customer_id.into_uuid())
            .bind("pending")
            .bind(now)
            .bind(total)
            .bind(tax_amount)
            .bind(shipping_amount)
            .bind(discount_amount)
            .bind(input.currency.unwrap_or(CurrencyCode::USD).as_str())
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

            (id, order_number, true)
        };

        if !inserted {
            let row = sqlx::query_as::<_, OrderRow>("SELECT * FROM orders WHERE id = $1")
                .bind(order_id)
                .fetch_optional(tx.as_mut())
                .await
                .map_err(map_db_error)?
                .ok_or(CommerceError::OrderNotFound(order_id))?;
            let item_rows = sqlx::query_as::<_, OrderItemRow>(
                "SELECT * FROM order_items WHERE order_id = $1 ORDER BY created_at, id",
            )
            .bind(order_id)
            .fetch_all(tx.as_mut())
            .await
            .map_err(map_db_error)?;
            let items = item_rows.into_iter().map(Self::row_to_item).collect();
            return Self::row_to_order(row, items);
        }

        // Insert order items
        let mut items = Vec::new();
        for item_input in &input.items {
            let item_id = Uuid::new_v4();
            let discount = item_input.discount.unwrap_or(Decimal::ZERO);
            let tax = item_input.tax_amount.unwrap_or(Decimal::ZERO);
            let item_total = OrderItem::calculate_total(
                item_input.quantity,
                item_input.unit_price,
                discount,
                tax,
            );

            sqlx::query(
                r#"
                INSERT INTO order_items (id, order_id, product_id, variant_id, sku, name,
                                         quantity, unit_price, discount, tax_amount, total, created_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                "#,
            )
            .bind(item_id)
            .bind(order_id)
            .bind(item_input.product_id.into_uuid())
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
                id: OrderItemId::from(item_id),
                order_id: OrderId::from(order_id),
                product_id: item_input.product_id,
                variant_id: item_input.variant_id,
                sku: item_input.sku.clone(),
                name: item_input.name.clone(),
                quantity: item_input.quantity,
                shipped_quantity: 0,
                unit_price: item_input.unit_price,
                discount,
                tax_amount: tax,
                total: item_total,
            });
        }

        for item in &items {
            self.reserve_line_stock_in_tx(
                tx,
                order_id,
                input.customer_id.into_uuid(),
                item,
                input.stock_policy,
            )
            .await?;
        }

        Ok(Order {
            id: OrderId::from(order_id),
            order_number: order_number.clone(),
            customer_id: input.customer_id,
            status: OrderStatus::Pending,
            order_date: now,
            total_amount: total,
            tax_amount,
            shipping_amount,
            discount_amount,
            currency: input.currency.unwrap_or(CurrencyCode::USD),
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

    /// Reserve stock for one order line under `stock_policy`, backordering any
    /// shortfall. Lines whose SKU is not a tracked inventory item are skipped.
    ///
    /// Shared by order creation (every line) and `add_item` (the new line) so
    /// a line added after the fact is reserved exactly like one present at
    /// creation. Under `RejectIfInsufficient` the error drops the caller's
    /// transaction, rolling everything back.
    pub(crate) async fn reserve_line_stock_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        order_id: Uuid,
        customer_id: Uuid,
        item: &OrderItem,
        stock_policy: StockPolicy,
    ) -> Result<()> {
        if item.quantity <= 0 {
            return Ok(());
        }
        let inventory_repo = PgInventoryRepository::new(self.pool.clone());
        let backorder_repo = PgBackorderRepository::new(self.pool.clone());
        let reference_id = order_id.to_string();

        let item_id: Option<i64> =
            sqlx::query_scalar("SELECT id FROM inventory_items WHERE sku = $1")
                .bind(&item.sku)
                .fetch_optional(tx.as_mut())
                .await
                .map_err(map_db_error)?;
        let Some(item_id) = item_id else {
            // Non-inventory item; skip reservations/backorders.
            return Ok(());
        };

        let available: Decimal = sqlx::query_scalar(
            "SELECT COALESCE(SUM(quantity_available), 0) FROM inventory_balances WHERE item_id = $1",
        )
        .bind(item_id)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let requested = Decimal::from(item.quantity);
        if !stock_policy.allows_backorder() && available < requested {
            return Err(CommerceError::InsufficientStock {
                sku: item.sku.clone(),
                requested: requested.to_string(),
                available: available.to_string(),
            });
        }
        let reserve_qty =
            if available > Decimal::ZERO { requested.min(available) } else { Decimal::ZERO };

        let mut reserved = Decimal::ZERO;
        if reserve_qty > Decimal::ZERO {
            let reserve_input = ReserveInventory {
                sku: item.sku.clone(),
                location_id: None,
                quantity: reserve_qty,
                reference_type: "order".to_string(),
                reference_id,
                expires_in_seconds: None,
            };

            match inventory_repo
                .reserve_for_line_in_tx(tx, &reserve_input, Some(item.id.into_uuid()))
                .await
            {
                Ok(_) => {
                    reserved = reserve_qty;
                }
                Err(err) => {
                    if stock_policy.allows_backorder()
                        && matches!(err, CommerceError::InsufficientStock { .. })
                    {
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
                order_id,
                order_line_id: Some(item.id.into_uuid()),
                customer_id,
                sku: item.sku.clone(),
                quantity: remaining,
                priority: None,
                expected_date: None,
                promised_date: None,
                source_location_id: None,
                notes: Some("Auto backorder: insufficient stock".to_string()),
            };
            backorder_repo.create_backorder_in_tx(tx, &backorder_input).await?;
        }
        Ok(())
    }

    /// Release the open reservations held for `sku` on this order that cover
    /// `quantity` units (whole reservations, oldest first). Used when a line
    /// is removed; reservations are one per line so this releases that
    /// line's hold. Mirrors the SQLite backend.
    /// Release the open reservations held by one order line. Keyed
    /// (`order_item_id`, migration 087) holds are released exactly; legacy
    /// un-keyed rows fall back to the SKU-based path. See the SQLite twin for
    /// the duplicate-SKU failure this fixes.
    async fn release_line_reservations_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        order_id: Uuid,
        item_id: Uuid,
        sku: &str,
        quantity: Decimal,
    ) -> Result<()> {
        let inventory_repo = PgInventoryRepository::new(self.pool.clone());
        let keyed = inventory_repo.list_open_reservations_for_line_in_tx(tx, item_id).await?;
        if !keyed.is_empty() {
            for (reservation_id, _) in keyed {
                inventory_repo.release_reservation_in_tx(tx, reservation_id).await?;
            }
            return Ok(());
        }

        // Legacy (pre-087) reservations: no line key, so release by SKU.
        let mut remaining = quantity;
        let open = inventory_repo
            .list_open_legacy_reservations_for_sku_in_tx(tx, "order", &order_id.to_string(), sku)
            .await?;
        for (reservation_id, reserved_qty) in open {
            if remaining <= Decimal::ZERO {
                break;
            }
            inventory_repo.release_reservation_in_tx(tx, reservation_id).await?;
            remaining -= reserved_qty;
        }
        Ok(())
    }

    /// Open reservations to confirm for one shipped line: the line's own keyed
    /// holds first, then legacy un-keyed holds for the same SKU on the order.
    async fn open_reservations_for_shipped_line_in_tx(
        inventory_repo: &PgInventoryRepository,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        order_id: Uuid,
        delta: &LineDelta,
    ) -> Result<Vec<(Uuid, Decimal)>> {
        let mut open =
            inventory_repo.list_open_reservations_for_line_in_tx(tx, delta.item_id).await?;
        open.extend(
            inventory_repo
                .list_open_legacy_reservations_for_sku_in_tx(
                    tx,
                    "order",
                    &order_id.to_string(),
                    &delta.sku,
                )
                .await?,
        );
        Ok(open)
    }

    /// Kernel outbox event for a line edit (`orders.item_added.v1` /
    /// `orders.item_removed.v1`), on the same transaction as the line change.
    async fn append_line_event_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        kind: &str,
        order_id: Uuid,
        item: &OrderItem,
    ) -> Result<()> {
        let total_amount: Decimal =
            sqlx::query_scalar("SELECT total_amount FROM orders WHERE id = $1")
                .bind(order_id)
                .fetch_one(tx.as_mut())
                .await
                .map_err(map_db_error)?;
        append_kernel_event_tx(
            tx.as_mut(),
            &KernelOutboxEvent::domain(
                kind,
                "order",
                order_id.to_string(),
                serde_json::json!({
                    "order_id": order_id.to_string(),
                    "order_item_id": item.id.to_string(),
                    "sku": item.sku,
                    "quantity": item.quantity,
                    "unit_price": item.unit_price.to_string(),
                    "line_total": item.total.to_string(),
                    "total_amount": total_amount.to_string(),
                }),
                None,
            ),
        )
        .await
    }

    /// Release every reservation and cancel every backorder held by the order.
    async fn release_order_stock_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
    ) -> Result<()> {
        let inventory_repo = PgInventoryRepository::new(self.pool.clone());
        let backorder_repo = PgBackorderRepository::new(self.pool.clone());
        let reservation_ids = inventory_repo
            .list_reservation_ids_by_reference_in_tx(tx, "order", &id.to_string())
            .await?;
        for reservation_id in reservation_ids {
            inventory_repo.release_reservation_in_tx(tx, reservation_id).await?;
        }
        backorder_repo.cancel_backorders_for_order_in_tx(tx, id).await
    }

    /// Current `(status, customer_id)` of an order, row-locked, or
    /// `OrderNotFound`.
    async fn load_status_and_customer_in_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        order_id: Uuid,
    ) -> Result<(OrderStatus, Uuid)> {
        let (status_raw, customer_id): (String, Uuid) =
            sqlx::query_as("SELECT status, customer_id FROM orders WHERE id = $1 FOR UPDATE")
                .bind(order_id)
                .fetch_optional(tx.as_mut())
                .await
                .map_err(map_db_error)?
                .ok_or(CommerceError::OrderNotFound(order_id))?;
        let status: OrderStatus = status_raw.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid order.status '{status_raw}': {e}"))
        })?;
        Ok((status, customer_id))
    }

    /// Load an order with its lines on the caller's transaction.
    async fn load_order_in_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
    ) -> Result<Order> {
        let row = sqlx::query_as::<_, OrderRow>("SELECT * FROM orders WHERE id = $1")
            .bind(id)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(map_db_error)?
            .ok_or(CommerceError::OrderNotFound(id))?;
        let items = sqlx::query_as::<_, OrderItemRow>(
            "SELECT * FROM order_items WHERE order_id = $1 ORDER BY created_at, id",
        )
        .bind(id)
        .fetch_all(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        Self::row_to_order(row, items.into_iter().map(Self::row_to_item).collect())
    }

    /// Delete one order and everything it holds (lines, reservations,
    /// backorders) on the caller's transaction. A missing order is a no-op.
    ///
    /// Decision: orders that have shipped (`PartiallyShipped`/`Shipped`/
    /// `Delivered`) or been refunded are refused with `Conflict` — they are
    /// fulfilment and financial records. Pending/confirmed/processing orders
    /// release their stock before the rows go; cancelled orders already have.
    /// Previously this was a bare pool `DELETE` relying on CASCADE, which
    /// left reservations and backorders (no FK to orders) dangling.
    async fn delete_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
    ) -> Result<()> {
        let row: Option<(String, String)> =
            sqlx::query_as("SELECT status, payment_status FROM orders WHERE id = $1 FOR UPDATE")
                .bind(id)
                .fetch_optional(tx.as_mut())
                .await
                .map_err(map_db_error)?;
        let Some((status_raw, payment_status_raw)) = row else {
            return Ok(());
        };
        let status: OrderStatus = status_raw.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid order.status '{status_raw}': {e}"))
        })?;
        ensure_deletable(id, status)?;
        let payment_status: PaymentStatus = payment_status_raw.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid order.payment_status '{payment_status_raw}': {e}"
            ))
        })?;
        ensure_no_money_on_delete(
            id,
            payment_status,
            order_has_payments_pg(tx.as_mut(), id).await?,
        )?;

        self.release_order_stock_in_tx(tx, id).await?;
        sqlx::query("DELETE FROM order_items WHERE order_id = $1")
            .bind(id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        sqlx::query("DELETE FROM orders WHERE id = $1")
            .bind(id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        Ok(())
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

    /// Get an order by `orders.cart_id` (async)
    pub async fn get_by_cart_id_async(&self, cart_id: Uuid) -> Result<Option<Order>> {
        let row = sqlx::query_as::<_, OrderRow>("SELECT * FROM orders WHERE cart_id = $1")
            .bind(cart_id)
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
        let rows =
            sqlx::query_as::<_, OrderItemRow>("SELECT * FROM order_items WHERE order_id = $1")
                .bind(order_id)
                .fetch_all(&self.pool)
                .await
                .map_err(map_db_error)?;

        Ok(rows.into_iter().map(Self::row_to_item).collect())
    }

    async fn get_items_batch_async(
        &self,
        ids: &[Uuid],
    ) -> Result<std::collections::HashMap<Uuid, Vec<OrderItem>>> {
        let mut map: std::collections::HashMap<Uuid, Vec<OrderItem>> =
            std::collections::HashMap::with_capacity(ids.len());
        if ids.is_empty() {
            return Ok(map);
        }
        let rows =
            sqlx::query_as::<_, OrderItemRow>("SELECT * FROM order_items WHERE order_id = ANY($1)")
                .bind(ids.to_vec())
                .fetch_all(&self.pool)
                .await
                .map_err(map_db_error)?;
        for row in rows {
            let parent = row.order_id;
            map.entry(parent).or_default().push(Self::row_to_item(row));
        }
        Ok(map)
    }

    /// Update an order (async)
    pub async fn update_async(&self, id: Uuid, input: UpdateOrder) -> Result<Order> {
        let mode = Self::ship_mode_for_update(&input)?;
        self.apply_update_async(id, input, mode).await
    }

    /// How a plain status update touches the lines: `Shipped` ships every
    /// remaining unit; `PartiallyShipped` is derived from line quantities and
    /// cannot be set directly (use `ship_async` with lines).
    fn ship_mode_for_update(input: &UpdateOrder) -> Result<ShipMode<'static>> {
        match input.status {
            Some(OrderStatus::Shipped) => Ok(ShipMode::All),
            Some(OrderStatus::PartiallyShipped) => Err(CommerceError::ValidationError(
                "order status partially_shipped is derived from shipped line quantities; \
                 use OrderRepository::ship with explicit lines"
                    .to_string(),
            )),
            _ => Ok(ShipMode::None),
        }
    }

    /// Ship an order (async), fully or per line. See [`OrderRepository::ship`].
    pub async fn ship_async(&self, id: Uuid, input: ShipOrder) -> Result<Order> {
        let ShipOrder { tracking_number, lines } = input;
        let lines = lines.unwrap_or_default();
        let mode = if lines.is_empty() { ShipMode::All } else { ShipMode::Lines(&lines) };
        self.apply_update_async(
            id,
            UpdateOrder {
                status: Some(OrderStatus::Shipped),
                tracking_number,
                ..Default::default()
            },
            mode,
        )
        .await
    }

    /// Load the order's lines and compute how many units each ships now.
    pub(crate) async fn plan_shipment_in_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
        ship: ShipMode<'_>,
    ) -> Result<(OrderStatus, Vec<LineDelta>)> {
        let rows: Vec<(Uuid, String, i32, i32)> = sqlx::query_as(
            "SELECT id, sku, quantity, shipped_quantity FROM order_items WHERE order_id = $1 ORDER BY created_at, id",
        )
        .bind(id)
        .fetch_all(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let mut deltas = Vec::with_capacity(rows.len());
        match ship {
            ShipMode::None => {}
            ShipMode::All => {
                for (item_id, sku, quantity, shipped) in &rows {
                    deltas.push(LineDelta {
                        item_id: *item_id,
                        sku: sku.clone(),
                        delta: (quantity - shipped).max(0),
                    });
                }
            }
            ShipMode::Lines(lines) => {
                let mut requested: std::collections::BTreeMap<Uuid, i64> =
                    std::collections::BTreeMap::new();
                for line in lines {
                    if line.quantity <= 0 {
                        return Err(CommerceError::ValidationError(format!(
                            "shipment line quantity must be positive for order item {}",
                            line.order_item_id
                        )));
                    }
                    *requested.entry(line.order_item_id.into_uuid()).or_insert(0) +=
                        i64::from(line.quantity);
                }
                for (item_id, req) in requested {
                    let Some((_, sku, quantity, shipped)) =
                        rows.iter().find(|(row_id, ..)| *row_id == item_id)
                    else {
                        return Err(CommerceError::ValidationError(format!(
                            "Order item {item_id} does not belong to order {id}"
                        )));
                    };
                    let remaining = (quantity - shipped).max(0);
                    if req > i64::from(remaining) {
                        return Err(CommerceError::ShipmentExceedsOrdered {
                            order_item_id: item_id,
                            requested: i32::try_from(req).unwrap_or(i32::MAX),
                            remaining,
                        });
                    }
                    deltas.push(LineDelta {
                        item_id,
                        sku: sku.clone(),
                        delta: i32::try_from(req).unwrap_or(i32::MAX),
                    });
                }
            }
        }

        let ordered: i64 = rows.iter().map(|r| i64::from(r.2)).sum();
        let shipped_after: i64 = rows.iter().map(|r| i64::from(r.3)).sum::<i64>()
            + deltas.iter().map(|d| i64::from(d.delta)).sum::<i64>();
        let resolved = if shipped_after < ordered {
            OrderStatus::PartiallyShipped
        } else {
            OrderStatus::Shipped
        };
        Ok((resolved, deltas))
    }

    /// Shared implementation of `update_async` and `ship_async`: one
    /// [`Self::apply_update_in_tx`] in its own transaction.
    async fn apply_update_async(
        &self,
        id: Uuid,
        input: UpdateOrder,
        ship: ShipMode<'_>,
    ) -> Result<Order> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let outcome = self.apply_update_in_tx(&mut tx, id, &input, ship).await?;
        tx.commit().await.map_err(map_db_error)?;
        if let Some(err) = outcome.post_commit_error {
            return Err(err);
        }
        Ok(outcome.order)
    }

    /// Apply one order update (status transition with all its side effects,
    /// field patches, version bump, outbox event) on the caller's transaction.
    ///
    /// This is THE transition path: `update_async`, `ship_async` and
    /// `update_batch_atomic_async` all route through it, so a batch of N
    /// updates behaves exactly like N single updates sharing one commit.
    pub(crate) async fn apply_update_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
        input: &UpdateOrder,
        ship: ShipMode<'_>,
    ) -> Result<UpdateOutcome> {
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

        let existing_row =
            sqlx::query_as::<_, OrderRow>("SELECT * FROM orders WHERE id = $1 FOR UPDATE")
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
            total_amount,
            ..
        } = existing_row;

        let current_status: OrderStatus = current_status_raw.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid order.status '{}': {}",
                current_status_raw, e
            ))
        })?;
        let current_payment_status: PaymentStatus =
            current_payment_status_raw.parse().map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid order.payment_status '{}': {}",
                    current_payment_status_raw, e
                ))
            })?;
        let current_fulfillment_status: FulfillmentStatus =
            current_fulfillment_status_raw.parse().map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid order.fulfillment_status '{}': {}",
                    current_fulfillment_status_raw, e
                ))
            })?;

        let is_ship = input.status == Some(OrderStatus::Shipped) && !matches!(ship, ShipMode::None);
        let (new_status, line_deltas) = if is_ship {
            Self::plan_shipment_in_tx(tx, id, ship).await?
        } else {
            (input.status.unwrap_or(current_status), Vec::new())
        };
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
            return Err(CommerceError::OrderCannotBeRefunded(new_payment_status.to_string()));
        }

        if is_ship {
            let reservation_ids = inventory_repo
                .list_reservation_ids_by_reference_in_tx(tx, "order", &id.to_string())
                .await?;

            let mut expired_reservation: Option<Uuid> = None;
            for reservation_id in &reservation_ids {
                if inventory_repo
                    .expire_reservation_if_needed_in_tx(tx, *reservation_id, now)
                    .await?
                    && expired_reservation.is_none()
                {
                    expired_reservation = Some(*reservation_id);
                }
            }

            if let Some(expired_id) = expired_reservation {
                // Commit the expiry bookkeeping, then surface the error.
                return Ok(UpdateOutcome {
                    order: Self::load_order_in_tx(tx, id).await?,
                    post_commit_error: Some(CommerceError::ReservationExpired(expired_id)),
                });
            }

            match ship {
                ShipMode::None => {}
                ShipMode::All => {
                    for reservation_id in reservation_ids {
                        match inventory_repo
                            .confirm_reservation_in_tx_with_now(tx, reservation_id, now)
                            .await?
                        {
                            ReservationConfirmOutcome::Confirmed => {}
                            ReservationConfirmOutcome::Expired => {
                                expired_reservation = Some(reservation_id);
                                break;
                            }
                        }
                    }
                }
                ShipMode::Lines(_) => {
                    'lines: for delta in line_deltas.iter().filter(|d| d.delta > 0) {
                        let mut remaining = Decimal::from(delta.delta);
                        let open = Self::open_reservations_for_shipped_line_in_tx(
                            &inventory_repo,
                            tx,
                            id,
                            delta,
                        )
                        .await?;
                        for (reservation_id, reserved_qty) in open {
                            if remaining <= Decimal::ZERO {
                                break;
                            }
                            let take = remaining.min(reserved_qty);
                            match inventory_repo
                                .confirm_reservation_quantity_in_tx_with_now(
                                    tx,
                                    reservation_id,
                                    take,
                                    now,
                                )
                                .await?
                            {
                                ReservationConfirmOutcome::Confirmed => remaining -= take,
                                ReservationConfirmOutcome::Expired => {
                                    expired_reservation = Some(reservation_id);
                                    break 'lines;
                                }
                            }
                        }
                    }
                }
            }

            if let Some(expired_id) = expired_reservation {
                // Commit the expiry bookkeeping, then surface the error.
                return Ok(UpdateOutcome {
                    order: Self::load_order_in_tx(tx, id).await?,
                    post_commit_error: Some(CommerceError::ReservationExpired(expired_id)),
                });
            }

            for delta in line_deltas.iter().filter(|d| d.delta > 0) {
                sqlx::query(
                    "UPDATE order_items SET shipped_quantity = shipped_quantity + $1 WHERE id = $2",
                )
                .bind(delta.delta)
                .bind(delta.item_id)
                .execute(tx.as_mut())
                .await
                .map_err(map_db_error)?;
            }
        }

        let new_tracking = input.tracking_number.clone().or(tracking_number);
        let new_notes = input.notes.clone().or(notes);
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

        // Money rule for cancel (see `UpdateOrder::void_payments`): an order
        // whose payments still hold money cannot be cancelled unless the
        // caller explicitly voids; even then only in-flight payments are
        // voided here — settled money leaves via a refund.
        let mut cancel_money = CancelMoney::default();
        if matches!(input.status, Some(OrderStatus::Cancelled)) {
            let open = open_captures_for_order_pg(tx.as_mut(), id).await?;
            if !open.is_empty() && !input.void_payments {
                let outstanding: Decimal = open.iter().map(|p| p.amount - p.amount_refunded).sum();
                let currency = open[0].currency;
                return Err(CommerceError::ValidationError(format!(
                    "order {id} cannot be cancelled: {} payment(s) still hold {outstanding} {currency}; \
                     refund them first, or cancel with void_payments = true to void in-flight \
                     payments and leave settled ones for refund",
                    open.len()
                )));
            }
            if input.void_payments {
                cancel_money.voided_payment_ids =
                    void_in_flight_payments_for_order_pg(tx.as_mut(), id, now).await?;
                let voided = &cancel_money.voided_payment_ids;
                let outstanding: Vec<_> =
                    open.iter().filter(|p| !voided.contains(&p.id.into_uuid())).collect();
                cancel_money.outstanding_captured =
                    outstanding.iter().map(|p| p.amount - p.amount_refunded).sum();
                cancel_money.outstanding_payment_ids =
                    outstanding.iter().map(|p| p.id.into_uuid()).collect();
            }

            let reservation_ids = inventory_repo
                .list_reservation_ids_by_reference_in_tx(tx, "order", &id.to_string())
                .await?;
            for reservation_id in reservation_ids {
                inventory_repo.release_reservation_in_tx(tx, reservation_id).await?;
            }
            backorder_repo.cancel_backorders_for_order_in_tx(tx, id).await?;
        }

        append_kernel_event_tx(
            tx.as_mut(),
            &KernelOutboxEvent::domain(
                "orders.updated.v1",
                "order",
                id.to_string(),
                serde_json::json!({
                    "order_id": id.to_string(),
                    "status_before": current_status.to_string(),
                    "status_after": new_status.to_string(),
                    "payment_status_before": current_payment_status.to_string(),
                    "payment_status_after": new_payment_status.to_string(),
                    "fulfillment_status_after": new_fulfillment_status.to_string(),
                    "version_before": expected_version,
                    "version_after": expected_version + 1,
                    "total_amount": total_amount.to_string(),
                    "void_payments": input.void_payments,
                    "voided_payment_ids": cancel_money.voided_payment_ids,
                    "outstanding_payment_ids": cancel_money.outstanding_payment_ids,
                    "outstanding_captured": cancel_money.outstanding_captured.to_string(),
                }),
                None,
            ),
        )
        .await?;

        Ok(UpdateOutcome { order: Self::load_order_in_tx(tx, id).await?, post_commit_error: None })
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
            after_cursor,
        } = filter;

        let mut builder = QueryBuilder::new("SELECT * FROM orders WHERE 1=1");

        if let Some(customer_id) = customer_id {
            builder.push(" AND customer_id = ").push_bind(customer_id.into_uuid());
        }
        if let Some(status) = status {
            builder.push(" AND status = ").push_bind(status.to_string());
        }
        if let Some(payment_status) = payment_status {
            builder.push(" AND payment_status = ").push_bind(payment_status.to_string());
        }
        if let Some(fulfillment_status) = fulfillment_status {
            builder.push(" AND fulfillment_status = ").push_bind(fulfillment_status.to_string());
        }
        if let Some(from) = from_date {
            builder.push(" AND order_date >= ").push_bind(from);
        }
        if let Some(to) = to_date {
            builder.push(" AND order_date <= ").push_bind(to);
        }

        // Keyset cursor: (order_date, id) for stable DESC ordering; matches
        // the SQLite backend's cursor semantics.
        let after_cursor = super::parse_after_cursor(after_cursor.as_ref())?;
        if let Some((cursor_date, cursor_id)) = after_cursor {
            builder
                .push(" AND (order_date < ")
                .push_bind(cursor_date)
                .push(" OR (order_date = ")
                .push_bind(cursor_date)
                .push(" AND id < ")
                .push_bind(cursor_id)
                .push("))");
        }

        builder.push(" ORDER BY order_date DESC, id DESC");

        builder.push(" LIMIT ").push_bind(super::effective_limit(limit));
        // Offset pagination applies only in non-cursor mode.
        if after_cursor.is_none()
            && let Some(offset) = offset
        {
            builder.push(" OFFSET ").push_bind(offset as i64);
        }

        let rows = builder
            .build_query_as::<OrderRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        let ids: Vec<Uuid> = rows.iter().map(|r| r.id).collect();
        let mut items_by_id = self.get_items_batch_async(&ids).await?;
        let mut orders = Vec::new();
        for row in rows {
            let items = items_by_id.remove(&row.id).unwrap_or_default();
            orders.push(Self::row_to_order(row, items)?);
        }

        Ok(orders)
    }

    /// Delete an order (async)
    pub async fn delete_async(&self, id: Uuid) -> Result<()> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        self.delete_in_tx(&mut tx, id).await?;
        tx.commit().await.map_err(map_db_error)
    }

    /// Add item to order (async)
    ///
    /// Only allowed before fulfilment (`pending`/`confirmed`/`processing`).
    /// The line is reserved/backordered through the same stock-policy path as
    /// creation and the total is re-derived, all in one transaction.
    pub async fn add_item_async(&self, order_id: Uuid, item: CreateOrderItem) -> Result<OrderItem> {
        validate_required_uuid("order.id", order_id)?;
        Self::validate_order_item_input(&item)?;

        let id = Uuid::new_v4();
        let now = Utc::now();
        let discount = item.discount.unwrap_or(Decimal::ZERO);
        let tax = item.tax_amount.unwrap_or(Decimal::ZERO);
        let total = OrderItem::calculate_total(item.quantity, item.unit_price, discount, tax);
        let order_item = OrderItem {
            id: OrderItemId::from(id),
            order_id: OrderId::from(order_id),
            product_id: item.product_id,
            variant_id: item.variant_id,
            sku: item.sku,
            name: item.name,
            quantity: item.quantity,
            shipped_quantity: 0,
            unit_price: item.unit_price,
            discount,
            tax_amount: tax,
            total,
        };

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let (status, customer_id) = Self::load_status_and_customer_in_tx(&mut tx, order_id).await?;
        ensure_lines_mutable(order_id, status)?;

        sqlx::query(
            r#"
            INSERT INTO order_items (id, order_id, product_id, variant_id, sku, name,
                                     quantity, unit_price, discount, tax_amount, total, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            "#,
        )
        .bind(id)
        .bind(order_id)
        .bind(order_item.product_id.into_uuid())
        .bind(order_item.variant_id)
        .bind(&order_item.sku)
        .bind(&order_item.name)
        .bind(order_item.quantity)
        .bind(order_item.unit_price)
        .bind(discount)
        .bind(tax)
        .bind(total)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        // Same stock-policy path as creation; a line added later takes the
        // default policy (`AllowBackorder`).
        self.reserve_line_stock_in_tx(
            &mut tx,
            order_id,
            customer_id,
            &order_item,
            StockPolicy::default(),
        )
        .await?;

        Self::update_order_total_tx(&mut tx, order_id).await?;
        Self::append_line_event_tx(&mut tx, "orders.item_added.v1", order_id, &order_item).await?;
        tx.commit().await.map_err(map_db_error)?;

        Ok(order_item)
    }

    /// Remove item from order (async)
    ///
    /// Only allowed before fulfilment. Releases the line's reservation and
    /// cancels its backorder in the same transaction as the delete.
    pub async fn remove_item_async(&self, order_id: Uuid, item_id: Uuid) -> Result<()> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let (status, _customer_id) =
            Self::load_status_and_customer_in_tx(&mut tx, order_id).await?;
        ensure_lines_mutable(order_id, status)?;

        let line = sqlx::query_as::<_, OrderItemRow>(
            "SELECT * FROM order_items WHERE id = $1 AND order_id = $2",
        )
        .bind(item_id)
        .bind(order_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        let Some(line) = line else {
            return Err(CommerceError::ValidationError(format!(
                "Order item {item_id} does not belong to order {order_id}"
            )));
        };
        let removed = Self::row_to_item(line);

        self.release_line_reservations_in_tx(
            &mut tx,
            order_id,
            item_id,
            &removed.sku,
            Decimal::from(removed.quantity),
        )
        .await?;
        PgBackorderRepository::new(self.pool.clone())
            .cancel_backorders_for_order_line_in_tx(&mut tx, order_id, item_id)
            .await?;

        sqlx::query("DELETE FROM order_items WHERE id = $1 AND order_id = $2")
            .bind(item_id)
            .bind(order_id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        Self::update_order_total_tx(&mut tx, order_id).await?;
        Self::append_line_event_tx(&mut tx, "orders.item_removed.v1", order_id, &removed).await?;
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
            after_cursor: _,
        } = filter;

        let mut builder = QueryBuilder::new("SELECT COUNT(*) FROM orders WHERE 1=1");

        if let Some(customer_id) = customer_id {
            builder.push(" AND customer_id = ").push_bind(customer_id.into_uuid());
        }
        if let Some(status) = status {
            builder.push(" AND status = ").push_bind(status.to_string());
        }
        if let Some(payment_status) = payment_status {
            builder.push(" AND payment_status = ").push_bind(payment_status.to_string());
        }
        if let Some(fulfillment_status) = fulfillment_status {
            builder.push(" AND fulfillment_status = ").push_bind(fulfillment_status.to_string());
        }
        if let Some(from) = from_date {
            builder.push(" AND order_date >= ").push_bind(from);
        }
        if let Some(to) = to_date {
            builder.push(" AND order_date <= ").push_bind(to);
        }

        let count: (i64,) =
            builder.build_query_as().fetch_one(&self.pool).await.map_err(map_db_error)?;

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

            // Route batch creation through the SAME guarded path as a single
            // create. The batch loop previously inserted orders and items
            // directly with no stock check, reservation or backorder, so
            // batch-creating 100 units against 5 in stock succeeded and
            // shipping later found zero reservations. `create_in_tx` enforces
            // the stock policy, reserves what is available, backorders the
            // remainder and carries the order-level money — on this
            // transaction, so one rejected line rolls the whole batch back.
            let order = self.create_in_tx(&mut tx, None, false, input).await?;
            orders.push(order);
        }

        tx.commit().await.map_err(map_db_error)?;
        Ok(orders)
    }

    /// Update multiple orders in a batch (async, non-atomic)
    pub async fn update_batch_async(
        &self,
        updates: Vec<(Uuid, UpdateOrder)>,
    ) -> Result<BatchResult<Order>> {
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
    ///
    /// Every row goes through the SAME in-transaction path as a single
    /// `update_async` (shipment planning, reservation confirm/release,
    /// backorder cancel, outbox event, `PartiallyShipped` rejection), sharing
    /// one commit: batch == N single updates, all or nothing.
    pub async fn update_batch_atomic_async(
        &self,
        updates: Vec<(Uuid, UpdateOrder)>,
    ) -> Result<Vec<Order>> {
        validate_batch_size(&updates)?;
        if updates.is_empty() {
            return Ok(vec![]);
        }
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let mut orders = Vec::with_capacity(updates.len());

        for (id, input) in &updates {
            let mode = Self::ship_mode_for_update(input)?;
            let outcome = self.apply_update_in_tx(&mut tx, *id, input, mode).await?;
            if let Some(err) = outcome.post_commit_error {
                // An atomic batch cannot commit part of itself: dropping `tx`
                // rolls every row back; the reservation is still expired by
                // time and is re-expired on retry.
                return Err(err);
            }
            orders.push(outcome.order);
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

        // Same per-order path as a single delete (status predicate,
        // reservation release, backorder cancel), one commit.
        for id in &ids {
            self.delete_in_tx(&mut tx, *id).await?;
        }

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

        let row_ids: Vec<Uuid> = rows.iter().map(|r| r.id).collect();
        let mut items_by_id = self.get_items_batch_async(&row_ids).await?;
        let mut orders = Vec::with_capacity(rows.len());
        for row in rows {
            let items = items_by_id.remove(&row.id).unwrap_or_default();
            orders.push(Self::row_to_order(row, items)?);
        }

        Ok(orders)
    }
}

impl OrderRepository for PgOrderRepository {
    fn create(&self, input: CreateOrder) -> Result<Order> {
        super::block_on(self.create_async(input))
    }

    fn create_from_cart(&self, cart_id: CartId, input: CreateOrder) -> Result<Order> {
        super::block_on(self.create_from_cart_async(cart_id.into_uuid(), input))
    }

    fn get(&self, id: OrderId) -> Result<Option<Order>> {
        super::block_on(self.get_async(id.into_uuid()))
    }

    fn get_by_number(&self, order_number: &str) -> Result<Option<Order>> {
        super::block_on(self.get_by_number_async(order_number))
    }

    fn update(&self, id: OrderId, input: UpdateOrder) -> Result<Order> {
        super::block_on(self.update_async(id.into_uuid(), input))
    }

    fn ship(&self, id: OrderId, input: ShipOrder) -> Result<Order> {
        super::block_on(self.ship_async(id.into_uuid(), input))
    }

    fn list(&self, filter: OrderFilter) -> Result<Vec<Order>> {
        super::block_on(self.list_async(filter))
    }

    fn delete(&self, id: OrderId) -> Result<()> {
        super::block_on(self.delete_async(id.into_uuid()))
    }

    fn add_item(&self, order_id: OrderId, item: CreateOrderItem) -> Result<OrderItem> {
        super::block_on(self.add_item_async(order_id.into_uuid(), item))
    }

    fn remove_item(&self, order_id: OrderId, item_id: OrderItemId) -> Result<()> {
        super::block_on(self.remove_item_async(order_id.into_uuid(), item_id.into_uuid()))
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

    fn update_batch(&self, updates: Vec<(OrderId, UpdateOrder)>) -> Result<BatchResult<Order>> {
        let raw_updates: Vec<(Uuid, UpdateOrder)> =
            updates.into_iter().map(|(id, u)| (id.into_uuid(), u)).collect();
        super::block_on(self.update_batch_async(raw_updates))
    }

    fn update_batch_atomic(&self, updates: Vec<(OrderId, UpdateOrder)>) -> Result<Vec<Order>> {
        let raw_updates: Vec<(Uuid, UpdateOrder)> =
            updates.into_iter().map(|(id, u)| (id.into_uuid(), u)).collect();
        super::block_on(self.update_batch_atomic_async(raw_updates))
    }

    fn delete_batch(&self, ids: Vec<OrderId>) -> Result<BatchResult<OrderId>> {
        let raw_ids: Vec<Uuid> = ids.iter().map(|id| id.into_uuid()).collect();
        let result = super::block_on(self.delete_batch_async(raw_ids))?;
        // Convert BatchResult<Uuid> to BatchResult<OrderId>
        Ok(BatchResult {
            succeeded: result.succeeded.into_iter().map(OrderId::from).collect(),
            failed: result.failed,
            total_attempted: result.total_attempted,
            success_count: result.success_count,
            failure_count: result.failure_count,
        })
    }

    fn delete_batch_atomic(&self, ids: Vec<OrderId>) -> Result<()> {
        let raw_ids: Vec<Uuid> = ids.into_iter().map(|id| id.into_uuid()).collect();
        super::block_on(self.delete_batch_atomic_async(raw_ids))
    }

    fn get_batch(&self, ids: Vec<OrderId>) -> Result<Vec<Order>> {
        let raw_ids: Vec<Uuid> = ids.into_iter().map(|id| id.into_uuid()).collect();
        super::block_on(self.get_batch_async(raw_ids))
    }
}
