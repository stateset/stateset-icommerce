//! SQLite order repository implementation

use super::kernel_outbox::append_kernel_event_tx;
use super::{
    backorder::{
        cancel_backorders_for_order_in_tx, cancel_backorders_for_order_line_in_tx,
        create_backorder_in_tx,
    },
    build_in_clause,
    inventory::{ReservationConfirmOutcome, SqliteInventoryRepository},
    map_db_error, params_refs, parse_datetime_row, parse_decimal_row, parse_enum, parse_enum_row,
    parse_json_opt_row, parse_uuid_row,
    payments::{
        open_captures_for_order_conn, order_has_payments_conn,
        void_in_flight_payments_for_order_conn,
    },
    sum_decimal_query, uuid_params, with_immediate_transaction,
};
use crate::KernelOutboxEvent;
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rust_decimal::Decimal;
use stateset_core::{
    Address, BatchResult, CartId, CommerceError, CreateBackorder, CreateOrder, CreateOrderItem,
    CustomerId, FulfillmentStatus, Order, OrderFilter, OrderId, OrderItem, OrderItemId,
    OrderRepository, OrderStatus, PaymentStatus, ProductId, ReserveInventory, Result, ShipOrder,
    ShipmentLineInput, StockPolicy, UpdateOrder, validate_batch_size, validate_currency_code,
    validate_postal_code, validate_price, validate_required_text, validate_required_uuid,
    validate_sku,
};
use uuid::Uuid;

/// SQLite implementation of `OrderRepository`
#[derive(Debug)]
pub struct SqliteOrderRepository {
    pool: Pool<SqliteConnectionManager>,
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
    pub(crate) item_id: String,
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

fn to_sql_err(err: CommerceError) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(err))
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
fn ensure_lines_mutable(id: OrderId, status: OrderStatus) -> Result<()> {
    if status.allows_line_changes() {
        Ok(())
    } else {
        Err(CommerceError::Conflict(format!(
            "order {id} lines cannot be changed while it is {status}; \
             lines may only be added or removed before fulfilment (pending, confirmed, processing)"
        )))
    }
}

/// Refuse deletion of an order that has money against it.
///
/// `allows_delete` only looks at the order status, so a `Pending`/`Cancelled`
/// order with `payment_status = paid` could be erased while its payment rows
/// survived, pointing at nothing. Both signals are checked: the order's own
/// `payment_status` (`PaymentStatus::holds_money`) and the existence of ANY
/// payment row for the order (queried in the delete transaction), so even a
/// pending or failed payment attempt keeps the order as a record.
fn ensure_no_money_on_delete(
    id: OrderId,
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
fn ensure_deletable(id: OrderId, status: OrderStatus) -> Result<()> {
    if status.allows_delete() {
        Ok(())
    } else {
        Err(CommerceError::Conflict(format!(
            "order {id} cannot be deleted while it is {status}; \
             shipped, delivered and refunded orders are records — cancel or refund instead"
        )))
    }
}

impl SqliteOrderRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn generate_order_number() -> String {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let now = Utc::now();
        let timestamp = now.timestamp();
        let nanos = now.timestamp_subsec_nanos();
        // Monotonic counter is cheaper than Uuid::new_v4() and still unique per process
        let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
        format!("ORD-{}-{:06}-{:08X}", timestamp, nanos / 1000, seq as u32)
    }

    pub(crate) fn row_to_order(row: &rusqlite::Row<'_>) -> rusqlite::Result<Order> {
        let shipping_addr: Option<Address> = parse_json_opt_row(
            row.get::<_, Option<String>>("shipping_address")?,
            "order",
            "shipping_address",
        )?;
        let billing_addr: Option<Address> = parse_json_opt_row(
            row.get::<_, Option<String>>("billing_address")?,
            "order",
            "billing_address",
        )?;

        Ok(Order {
            id: OrderId::from(parse_uuid_row(&row.get::<_, String>("id")?, "order", "id")?),
            order_number: row.get("order_number")?,
            customer_id: CustomerId::from(parse_uuid_row(
                &row.get::<_, String>("customer_id")?,
                "order",
                "customer_id",
            )?),
            status: parse_enum_row(&row.get::<_, String>("status")?, "order", "status")?,
            order_date: parse_datetime_row(
                &row.get::<_, String>("order_date")?,
                "order",
                "order_date",
            )?,
            total_amount: parse_decimal_row(
                &row.get::<_, String>("total_amount")?,
                "order",
                "total_amount",
            )?,
            tax_amount: parse_decimal_row(
                &row.get::<_, String>("tax_amount")?,
                "order",
                "tax_amount",
            )?,
            shipping_amount: parse_decimal_row(
                &row.get::<_, String>("shipping_amount")?,
                "order",
                "shipping_amount",
            )?,
            discount_amount: parse_decimal_row(
                &row.get::<_, String>("discount_amount")?,
                "order",
                "discount_amount",
            )?,
            currency: row.get("currency")?,
            payment_status: parse_enum_row(
                &row.get::<_, String>("payment_status")?,
                "order",
                "payment_status",
            )?,
            fulfillment_status: parse_enum_row(
                &row.get::<_, String>("fulfillment_status")?,
                "order",
                "fulfillment_status",
            )?,
            payment_method: row.get("payment_method")?,
            shipping_method: row.get("shipping_method")?,
            tracking_number: row.get("tracking_number")?,
            notes: row.get("notes")?,
            shipping_address: shipping_addr,
            billing_address: billing_addr,
            items: vec![], // Loaded separately
            version: row.get::<_, Option<i32>>("version")?.unwrap_or(1),
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "order",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "order",
                "updated_at",
            )?,
        })
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

        if let Some(ref currency) = input.currency {
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

    pub(crate) fn validate_create_order_in_tx(
        tx: &rusqlite::Transaction<'_>,
        input: &CreateOrder,
    ) -> std::result::Result<(), rusqlite::Error> {
        Self::validate_order_input(input)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        if input.stock_policy.allows_backorder() {
            return Ok(());
        }
        for item in &input.items {
            if item.quantity <= 0 {
                continue;
            }
            let item_id = match tx.query_row(
                "SELECT id FROM inventory_items WHERE sku = ?",
                [&item.sku],
                |row| row.get::<_, i64>(0),
            ) {
                Ok(id) => id,
                Err(rusqlite::Error::QueryReturnedNoRows) => continue,
                Err(error) => return Err(error),
            };
            let available = sum_decimal_query(
                tx,
                "SELECT quantity_available FROM inventory_balances WHERE item_id = ?",
                &[&item_id],
                "inventory_balance",
                "quantity_available",
            )
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
            let requested = Decimal::from(item.quantity);
            if available < requested {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    CommerceError::InsufficientStock {
                        sku: item.sku.clone(),
                        requested: requested.to_string(),
                        available: available.to_string(),
                    },
                )));
            }
        }
        Ok(())
    }

    pub(crate) fn load_order_items_with_conn(
        conn: &rusqlite::Connection,
        order_id: OrderId,
    ) -> Result<Vec<OrderItem>> {
        let mut stmt = conn
            .prepare(
                "SELECT id, order_id, product_id, variant_id, sku, name, quantity,
                        shipped_quantity, unit_price, discount, tax_amount, total
                 FROM order_items WHERE order_id = ?",
            )
            .map_err(map_db_error)?;

        let items = stmt
            .query_map([order_id.to_string()], Self::row_to_order_item)
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        Ok(items)
    }

    fn row_to_order_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<OrderItem> {
        Ok(OrderItem {
            id: OrderItemId::from(parse_uuid_row(
                &row.get::<_, String>("id")?,
                "order_item",
                "id",
            )?),
            order_id: OrderId::from(parse_uuid_row(
                &row.get::<_, String>("order_id")?,
                "order_item",
                "order_id",
            )?),
            product_id: ProductId::from(parse_uuid_row(
                &row.get::<_, String>("product_id")?,
                "order_item",
                "product_id",
            )?),
            variant_id: row.get::<_, Option<String>>("variant_id")?.and_then(|s| s.parse().ok()),
            sku: row.get("sku")?,
            name: row.get("name")?,
            quantity: row.get("quantity")?,
            shipped_quantity: row.get("shipped_quantity")?,
            unit_price: parse_decimal_row(
                &row.get::<_, String>("unit_price")?,
                "order_item",
                "unit_price",
            )?,
            discount: parse_decimal_row(
                &row.get::<_, String>("discount")?,
                "order_item",
                "discount",
            )?,
            tax_amount: parse_decimal_row(
                &row.get::<_, String>("tax_amount")?,
                "order_item",
                "tax_amount",
            )?,
            total: parse_decimal_row(&row.get::<_, String>("total")?, "order_item", "total")?,
        })
    }

    fn load_order_items_batch(
        conn: &rusqlite::Connection,
        ids: &[OrderId],
    ) -> Result<std::collections::HashMap<OrderId, Vec<OrderItem>>> {
        let mut map: std::collections::HashMap<OrderId, Vec<OrderItem>> =
            std::collections::HashMap::with_capacity(ids.len());
        for chunk in ids.chunks(500) {
            let placeholders = build_in_clause(chunk.len());
            let sql = format!(
                "SELECT id, order_id, product_id, variant_id, sku, name, quantity,
                        shipped_quantity, unit_price, discount, tax_amount, total
                 FROM order_items WHERE order_id IN ({placeholders})"
            );
            let id_strs: Vec<String> = chunk.iter().map(ToString::to_string).collect();
            let param_refs: Vec<&dyn rusqlite::ToSql> =
                id_strs.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
            let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
            let rows = stmt
                .query_map(param_refs.as_slice(), Self::row_to_order_item)
                .map_err(map_db_error)?;
            for row in rows {
                let item = row.map_err(map_db_error)?;
                map.entry(item.order_id).or_default().push(item);
            }
        }
        Ok(map)
    }

    fn get_by_cart_id_in_conn(
        conn: &rusqlite::Connection,
        cart_id: Uuid,
    ) -> std::result::Result<Option<Order>, rusqlite::Error> {
        let result = conn.query_row(
            "SELECT * FROM orders WHERE cart_id = ?",
            [cart_id.to_string()],
            Self::row_to_order,
        );

        match result {
            Ok(mut order) => {
                order.items = Self::load_order_items_with_conn(conn, order.id)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
                Ok(Some(order))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn get_by_cart_id(&self, cart_id: Uuid) -> Result<Option<Order>> {
        let conn = self.conn()?;
        Self::get_by_cart_id_in_conn(&conn, cart_id).map_err(map_db_error)
    }

    fn create_internal_in_tx(
        tx: &rusqlite::Transaction<'_>,
        cart_id: Option<Uuid>,
        idempotent_by_cart_id: bool,
        input: &CreateOrder,
    ) -> std::result::Result<Order, rusqlite::Error> {
        let id = OrderId::new();
        let order_number = Self::generate_order_number();
        let now = Utc::now();
        let currency = input.currency.unwrap_or_default();

        // Pre-compute strings used multiple times to avoid repeated allocation
        let id_str = id.to_string();
        let customer_id_str = input.customer_id.to_string();
        let now_str = now.to_rfc3339();

        // Order total is the sum of the per-line money totals. Each line is
        // rounded to the currency minor unit by `OrderItem::calculate_total`
        // (the same helper used to store `order_items.total`), so the order
        // total foots exactly to its line items and matches `update_order_total`
        // and the Postgres backend.
        let line_total: Decimal = input
            .items
            .iter()
            .map(|item| {
                OrderItem::calculate_total(
                    item.quantity,
                    item.unit_price,
                    item.discount.unwrap_or_default(),
                    item.tax_amount.unwrap_or_default(),
                )
            })
            .sum();
        // Order-level money sits alongside the line sum, so the order records
        // what the customer is actually charged. Checkout carries the cart's
        // tax, shipping and discount here; a plain `create` leaves them zero
        // and the total is the line sum exactly as before.
        let tax_amount = input.tax_amount.unwrap_or_default();
        let shipping_amount = input.shipping_amount.unwrap_or_default();
        let discount_amount = input.discount_amount.unwrap_or_default();
        let total = line_total + tax_amount + shipping_amount - discount_amount;
        let total_str = total.to_string();
        let tax_str = tax_amount.to_string();
        let shipping_str = shipping_amount.to_string();
        let discount_str = discount_amount.to_string();

        let shipping_address_json = input
            .shipping_address
            .as_ref()
            .map(|address| {
                serde_json::to_string(address).map_err(|error| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(CommerceError::DatabaseError(
                        format!("Failed to serialize order.shipping_address: {error}"),
                    )))
                })
            })
            .transpose()?;
        let billing_address_json = input
            .billing_address
            .as_ref()
            .map(|address| {
                serde_json::to_string(address).map_err(|error| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(CommerceError::DatabaseError(
                        format!("Failed to serialize order.billing_address: {error}"),
                    )))
                })
            })
            .transpose()?;

        let cart_id_str = if idempotent_by_cart_id {
            Some(
                cart_id
                    .ok_or_else(|| {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(
                            CommerceError::ValidationError(
                                "cart_id is required for cart checkout".into(),
                            ),
                        ))
                    })?
                    .to_string(),
            )
        } else {
            None
        };

        let inserted = if idempotent_by_cart_id {
            let cart_id_str = cart_id_str.as_deref().ok_or_else(|| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(CommerceError::DatabaseError(
                    "cart_id was required but missing (internal error)".into(),
                )))
            })?;
            let rows_affected = tx.execute(
                "INSERT OR IGNORE INTO orders (id, order_number, customer_id, status, order_date, total_amount,
                                 tax_amount, shipping_amount, discount_amount,
                                 currency, payment_status, fulfillment_status, payment_method,
                                 shipping_method, notes, shipping_address, billing_address,
                                 cart_id, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    &id_str,
                    &order_number,
                    &customer_id_str,
                    "pending",
                    &now_str,
                    &total_str,
                    &tax_str,
                    &shipping_str,
                    &discount_str,
                    &currency,
                    "pending",
                    "unfulfilled",
                    &input.payment_method,
                    &input.shipping_method,
                    &input.notes,
                    &shipping_address_json,
                    &billing_address_json,
                    cart_id_str,
                    &now_str,
                    &now_str,
                ],
            )?;

            rows_affected > 0
        } else {
            tx.prepare_cached(
                "INSERT INTO orders (id, order_number, customer_id, status, order_date, total_amount,
                                 tax_amount, shipping_amount, discount_amount,
                                 currency, payment_status, fulfillment_status, payment_method,
                                 shipping_method, notes, shipping_address, billing_address,
                                 created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )?.execute(
                rusqlite::params![
                    &id_str,
                    &order_number,
                    &customer_id_str,
                    "pending",
                    &now_str,
                    &total_str,
                    &tax_str,
                    &shipping_str,
                    &discount_str,
                    &currency,
                    "pending",
                    "unfulfilled",
                    &input.payment_method,
                    &input.shipping_method,
                    &input.notes,
                    &shipping_address_json,
                    &billing_address_json,
                    &now_str,
                    &now_str,
                ],
            )?;

            true
        };

        if !inserted {
            let cart_id = cart_id.ok_or_else(|| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(CommerceError::DatabaseError(
                    "cart_id was required but missing (internal error)".into(),
                )))
            })?;
            let existing = Self::get_by_cart_id_in_conn(tx, cart_id)?;
            return existing.ok_or_else(|| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(CommerceError::DatabaseError(
                    "Order exists for cart_id but could not be loaded".into(),
                )))
            });
        }

        let mut items = Vec::with_capacity(input.items.len());
        {
            let mut stmt = tx.prepare_cached(
                "INSERT INTO order_items (id, order_id, product_id, variant_id, sku, name,
                                          quantity, unit_price, discount, tax_amount, total)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )?;
            for item in &input.items {
                let item_id = OrderItemId::new();
                let item_total = OrderItem::calculate_total(
                    item.quantity,
                    item.unit_price,
                    item.discount.unwrap_or_default(),
                    item.tax_amount.unwrap_or_default(),
                );

                stmt.execute(rusqlite::params![
                    item_id.to_string(),
                    &id_str,
                    item.product_id.to_string(),
                    item.variant_id.map(|variant_id| variant_id.to_string()),
                    &item.sku,
                    &item.name,
                    item.quantity,
                    item.unit_price.to_string(),
                    item.discount.unwrap_or_default().to_string(),
                    item.tax_amount.unwrap_or_default().to_string(),
                    item_total.to_string(),
                ])?;

                items.push(OrderItem {
                    id: item_id,
                    order_id: id,
                    product_id: item.product_id,
                    variant_id: item.variant_id,
                    sku: item.sku.clone(),
                    name: item.name.clone(),
                    quantity: item.quantity,
                    shipped_quantity: 0,
                    unit_price: item.unit_price,
                    discount: item.discount.unwrap_or_default(),
                    tax_amount: item.tax_amount.unwrap_or_default(),
                    total: item_total,
                });
            }
        }

        for item in &items {
            Self::reserve_line_stock_in_tx(tx, id, input.customer_id, item, input.stock_policy)?;
        }

        Ok(Order {
            id,
            order_number,
            customer_id: input.customer_id,
            status: OrderStatus::Pending,
            order_date: now,
            total_amount: total,
            tax_amount,
            shipping_amount,
            discount_amount,
            currency,
            payment_status: PaymentStatus::Pending,
            fulfillment_status: FulfillmentStatus::Unfulfilled,
            payment_method: input.payment_method.clone(),
            shipping_method: input.shipping_method.clone(),
            tracking_number: None,
            notes: input.notes.clone(),
            shipping_address: input.shipping_address.clone(),
            billing_address: input.billing_address.clone(),
            items,
            version: 1,
            created_at: now,
            updated_at: now,
        })
    }

    /// Reserve stock for one order line under `stock_policy`, backordering any
    /// shortfall. Lines whose SKU is not a tracked inventory item are skipped.
    ///
    /// Shared by order creation (every line) and [`OrderRepository::add_item`]
    /// (the new line) so a line added after the fact is reserved exactly like
    /// one present at creation. Under `RejectIfInsufficient` the error rolls
    /// the caller's transaction back.
    pub(crate) fn reserve_line_stock_in_tx(
        tx: &rusqlite::Transaction<'_>,
        order_id: OrderId,
        customer_id: CustomerId,
        item: &OrderItem,
        stock_policy: StockPolicy,
    ) -> std::result::Result<(), rusqlite::Error> {
        if item.quantity <= 0 {
            return Ok(());
        }
        let reference_id = order_id.to_string();

        let item_id =
            match tx.query_row("SELECT id FROM inventory_items WHERE sku = ?", [&item.sku], |row| {
                row.get::<_, i64>(0)
            }) {
                Ok(item_id) => item_id,
                Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(()),
                Err(e) => return Err(e),
            };

        let available = sum_decimal_query(
            tx,
            "SELECT quantity_available FROM inventory_balances WHERE item_id = ?",
            &[&item_id],
            "inventory_balance",
            "quantity_available",
        )
        .map_err(to_sql_err)?;

        let requested = Decimal::from(item.quantity);
        if !stock_policy.allows_backorder() && available < requested {
            // Fail the whole transaction: no order row, no reservations,
            // no backorders survive.
            return Err(to_sql_err(CommerceError::InsufficientStock {
                sku: item.sku.clone(),
                requested: requested.to_string(),
                available: available.to_string(),
            }));
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

            match SqliteInventoryRepository::reserve_for_line_in_tx(
                tx,
                &reserve_input,
                Some(item.id.into_uuid()),
            ) {
                Ok(_reservation) => {
                    reserved = reserve_qty;
                }
                Err(err) => {
                    let commerce_err = map_db_error(err);
                    if stock_policy.allows_backorder()
                        && matches!(commerce_err, CommerceError::InsufficientStock { .. })
                    {
                        reserved = Decimal::ZERO;
                    } else {
                        return Err(to_sql_err(commerce_err));
                    }
                }
            }
        }

        let remaining = requested - reserved;
        if remaining > Decimal::ZERO {
            let backorder_input = CreateBackorder {
                order_id: order_id.into_uuid(),
                order_line_id: Some(item.id.into_uuid()),
                customer_id: customer_id.into_uuid(),
                sku: item.sku.clone(),
                quantity: remaining,
                priority: None,
                expected_date: None,
                promised_date: None,
                source_location_id: None,
                notes: Some("Auto backorder: insufficient stock".to_string()),
            };
            create_backorder_in_tx(tx, &backorder_input)?;
        }
        Ok(())
    }

    /// Release the open reservations held by one order line, returning stock
    /// to available. Used when a line is removed.
    ///
    /// Reservations created since migration 080 carry the line's
    /// `order_item_id`, so the line's own holds are released — never a
    /// sibling line's hold for the same SKU (removing a 1-unit line B used to
    /// free line A's 5-unit reservation because both shared a SKU and A was
    /// older). Legacy rows without a line key fall back to the historical
    /// SKU-based path: whole un-keyed reservations for the SKU, oldest first,
    /// until the removed line's quantity is covered.
    fn release_line_reservations_in_tx(
        tx: &rusqlite::Transaction<'_>,
        order_id: OrderId,
        item_id: OrderItemId,
        sku: &str,
        quantity: Decimal,
    ) -> std::result::Result<(), rusqlite::Error> {
        let keyed = SqliteInventoryRepository::list_open_reservations_for_line_in_tx(
            tx,
            item_id.into_uuid(),
        )?;
        if !keyed.is_empty() {
            for (reservation_id, _) in keyed {
                SqliteInventoryRepository::release_reservation_in_tx(tx, reservation_id)?;
            }
            return Ok(());
        }

        // Legacy (pre-080) reservations: no line key, so release by SKU.
        let mut remaining = quantity;
        let open = SqliteInventoryRepository::list_open_legacy_reservations_for_sku_in_tx(
            tx,
            "order",
            &order_id.to_string(),
            sku,
        )?;
        for (reservation_id, reserved_qty) in open {
            if remaining <= Decimal::ZERO {
                break;
            }
            SqliteInventoryRepository::release_reservation_in_tx(tx, reservation_id)?;
            remaining -= reserved_qty;
        }
        Ok(())
    }

    /// Open reservations to confirm for one shipped line: the line's own keyed
    /// holds first, then (legacy, pre-080 rows only) un-keyed holds for the
    /// same SKU on the order.
    fn open_reservations_for_shipped_line_in_tx(
        tx: &rusqlite::Transaction<'_>,
        reference_id: &str,
        delta: &LineDelta,
    ) -> std::result::Result<Vec<(Uuid, Decimal)>, rusqlite::Error> {
        let item_id = parse_uuid_row(&delta.item_id, "order_item", "id")?;
        let mut open =
            SqliteInventoryRepository::list_open_reservations_for_line_in_tx(tx, item_id)?;
        open.extend(SqliteInventoryRepository::list_open_legacy_reservations_for_sku_in_tx(
            tx,
            "order",
            reference_id,
            &delta.sku,
        )?);
        Ok(open)
    }

    /// Release every reservation and cancel every backorder held by the order.
    fn release_order_stock_in_tx(
        tx: &rusqlite::Transaction<'_>,
        id: OrderId,
    ) -> std::result::Result<(), rusqlite::Error> {
        let reservation_ids = SqliteInventoryRepository::list_reservation_ids_by_reference_in_tx(
            tx,
            "order",
            &id.to_string(),
        )?;
        for reservation_id in reservation_ids {
            SqliteInventoryRepository::release_reservation_in_tx(tx, reservation_id)?;
        }
        cancel_backorders_for_order_in_tx(tx, id.into_uuid())
    }

    /// Delete one order and everything it holds (lines, reservations,
    /// backorders) on the caller's transaction. A missing order is a no-op.
    ///
    /// Decision: orders that have shipped (`PartiallyShipped`/`Shipped`/
    /// `Delivered`) or been refunded are refused with `Conflict` — they are
    /// fulfilment and financial records. Pending/confirmed/processing orders
    /// release their stock before the rows go; cancelled orders already have.
    fn delete_in_tx(
        tx: &rusqlite::Transaction<'_>,
        id: OrderId,
    ) -> std::result::Result<(), rusqlite::Error> {
        let (status_raw, payment_status_raw) = match tx.query_row(
            "SELECT status, payment_status FROM orders WHERE id = ?",
            [id.to_string()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        ) {
            Ok(row) => row,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(()),
            Err(e) => return Err(e),
        };
        let status: OrderStatus = parse_enum(&status_raw, "order", "status").map_err(to_sql_err)?;
        ensure_deletable(id, status).map_err(to_sql_err)?;
        let payment_status: PaymentStatus =
            parse_enum(&payment_status_raw, "order", "payment_status").map_err(to_sql_err)?;
        ensure_no_money_on_delete(
            id,
            payment_status,
            order_has_payments_conn(tx, &id.to_string())?,
        )
        .map_err(to_sql_err)?;

        Self::release_order_stock_in_tx(tx, id)?;
        tx.execute("DELETE FROM order_items WHERE order_id = ?", [id.to_string()])?;
        tx.execute("DELETE FROM orders WHERE id = ?", [id.to_string()])?;
        Ok(())
    }

    pub(crate) fn create_from_cart_in_tx(
        tx: &rusqlite::Transaction<'_>,
        cart_id: Uuid,
        input: &CreateOrder,
    ) -> std::result::Result<Order, rusqlite::Error> {
        Self::validate_order_input(input)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

        if let Some(existing) = Self::get_by_cart_id_in_conn(tx, cart_id)? {
            return Ok(existing);
        }

        Self::create_internal_in_tx(tx, Some(cart_id), true, input)
    }

    pub fn create_from_cart(&self, cart_id: Uuid, input: CreateOrder) -> Result<Order> {
        if let Some(existing) = self.get_by_cart_id(cart_id)? {
            return Ok(existing);
        }

        self.create_internal(Some(cart_id), true, input)
    }

    fn create_internal(
        &self,
        cart_id: Option<Uuid>,
        idempotent_by_cart_id: bool,
        input: CreateOrder,
    ) -> Result<Order> {
        Self::validate_order_input(&input)?;

        with_immediate_transaction(&self.pool, |tx| {
            Self::create_internal_in_tx(tx, cart_id, idempotent_by_cart_id, &input)
        })
    }
}

impl SqliteOrderRepository {
    /// Load the order's lines and compute how many units each ships now.
    ///
    /// Returns the resolved order status (`PartiallyShipped` while
    /// Σ shipped < Σ ordered, else `Shipped`) and the per-line increments.
    pub(crate) fn plan_shipment_in_tx(
        tx: &rusqlite::Transaction<'_>,
        id: OrderId,
        ship: &ShipMode<'_>,
    ) -> std::result::Result<(OrderStatus, Vec<LineDelta>), rusqlite::Error> {
        let mut stmt = tx.prepare(
            "SELECT id, sku, quantity, shipped_quantity FROM order_items WHERE order_id = ? ORDER BY rowid",
        )?;
        let rows = stmt
            .query_map([id.to_string()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i32>(2)?,
                    row.get::<_, i32>(3)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut deltas = Vec::with_capacity(rows.len());
        match ship {
            ShipMode::None => {}
            ShipMode::All => {
                for (item_id, sku, quantity, shipped) in &rows {
                    deltas.push(LineDelta {
                        item_id: item_id.clone(),
                        sku: sku.clone(),
                        delta: (quantity - shipped).max(0),
                    });
                }
            }
            ShipMode::Lines(lines) => {
                let mut requested: std::collections::BTreeMap<String, i64> =
                    std::collections::BTreeMap::new();
                for line in *lines {
                    if line.quantity <= 0 {
                        return Err(to_sql_err(CommerceError::ValidationError(format!(
                            "shipment line quantity must be positive for order item {}",
                            line.order_item_id
                        ))));
                    }
                    *requested.entry(line.order_item_id.to_string()).or_insert(0) +=
                        i64::from(line.quantity);
                }
                for (item_id, req) in requested {
                    let Some((_, sku, quantity, shipped)) =
                        rows.iter().find(|(row_id, ..)| *row_id == item_id)
                    else {
                        return Err(to_sql_err(CommerceError::ValidationError(format!(
                            "Order item {item_id} does not belong to order {id}"
                        ))));
                    };
                    let remaining = (quantity - shipped).max(0);
                    if req > i64::from(remaining) {
                        return Err(to_sql_err(CommerceError::ShipmentExceedsOrdered {
                            order_item_id: parse_uuid_row(&item_id, "order_item", "id")?,
                            requested: i32::try_from(req).unwrap_or(i32::MAX),
                            remaining,
                        }));
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

    /// Confirm the shipped portion of the order's inventory reservations.
    ///
    /// Returns the first expired reservation, if any; the caller then surfaces
    /// [`CommerceError::ReservationExpired`] after committing the expiry
    /// bookkeeping (matching the legacy full-ship behaviour).
    pub(crate) fn confirm_shipped_reservations_in_tx(
        tx: &rusqlite::Transaction<'_>,
        id: OrderId,
        ship: &ShipMode<'_>,
        deltas: &[LineDelta],
        now: chrono::DateTime<Utc>,
    ) -> std::result::Result<Option<Uuid>, rusqlite::Error> {
        let reference_id = id.to_string();
        let reservation_ids = SqliteInventoryRepository::list_reservation_ids_by_reference_in_tx(
            tx,
            "order",
            &reference_id,
        )?;
        for reservation_id in &reservation_ids {
            if SqliteInventoryRepository::expire_reservation_if_needed_in_tx(
                tx,
                *reservation_id,
                now,
            )? {
                return Ok(Some(*reservation_id));
            }
        }

        match ship {
            ShipMode::None => {}
            ShipMode::All => {
                for reservation_id in reservation_ids {
                    match SqliteInventoryRepository::confirm_reservation_in_tx_with_now(
                        tx,
                        reservation_id,
                        now,
                    )? {
                        ReservationConfirmOutcome::Confirmed => {}
                        ReservationConfirmOutcome::Expired => return Ok(Some(reservation_id)),
                    }
                }
            }
            ShipMode::Lines(_) => {
                for delta in deltas.iter().filter(|d| d.delta > 0) {
                    let mut remaining = Decimal::from(delta.delta);
                    let open =
                        Self::open_reservations_for_shipped_line_in_tx(tx, &reference_id, delta)?;
                    for (reservation_id, reserved_qty) in open {
                        if remaining <= Decimal::ZERO {
                            break;
                        }
                        let take = remaining.min(reserved_qty);
                        match SqliteInventoryRepository::confirm_reservation_quantity_in_tx_with_now(
                            tx,
                            reservation_id,
                            take,
                            now,
                        )? {
                            ReservationConfirmOutcome::Confirmed => remaining -= take,
                            ReservationConfirmOutcome::Expired => return Ok(Some(reservation_id)),
                        }
                    }
                }
            }
        }
        Ok(None)
    }

    /// Apply one order update (status transition with all its side effects,
    /// field patches, version bump, outbox event) on the caller's transaction.
    ///
    /// This is THE transition path: [`OrderRepository::update`],
    /// [`OrderRepository::ship`] and [`OrderRepository::update_batch_atomic`]
    /// all route through it, so a batch of N updates behaves exactly like N
    /// single updates sharing one commit. `ship` selects how a transition to
    /// `Shipped` touches the order lines: [`ShipMode::All`] ships every
    /// remaining unit (legacy status flip), [`ShipMode::Lines`] ships explicit
    /// per-line quantities and resolves the status to
    /// `PartiallyShipped`/`Shipped` from the line totals.
    pub(crate) fn apply_update_in_tx(
        tx: &rusqlite::Transaction<'_>,
        id: OrderId,
        input: &UpdateOrder,
        ship: &ShipMode<'_>,
    ) -> std::result::Result<UpdateOutcome, rusqlite::Error> {
        if let Some(address) = &input.shipping_address {
            Self::validate_address_input(address, "order.shipping_address").map_err(to_sql_err)?;
        }
        if let Some(address) = &input.billing_address {
            Self::validate_address_input(address, "order.billing_address").map_err(to_sql_err)?;
        }

        let shipping_address_json = input
            .shipping_address
            .as_ref()
            .map(|a| {
                serde_json::to_string(a).map_err(|e| {
                    to_sql_err(CommerceError::DatabaseError(format!(
                        "Failed to serialize order.shipping_address: {e}"
                    )))
                })
            })
            .transpose()?;
        let billing_address_json = input
            .billing_address
            .as_ref()
            .map(|a| {
                serde_json::to_string(a).map_err(|e| {
                    to_sql_err(CommerceError::DatabaseError(format!(
                        "Failed to serialize order.billing_address: {e}"
                    )))
                })
            })
            .transpose()?;

        {
            let now = Utc::now();
            let (current_version, current_status_raw, current_payment_status_raw): (
                i32,
                String,
                String,
            ) = tx
                .query_row(
                    "SELECT version, status, payment_status FROM orders WHERE id = ?",
                    [id.to_string()],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(
                            CommerceError::OrderNotFound(id.into_uuid()),
                        ))
                    }
                    e => e,
                })?;

            let current_status: OrderStatus = parse_enum(&current_status_raw, "order", "status")
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            let current_payment_status: PaymentStatus =
                parse_enum(&current_payment_status_raw, "order", "payment_status")
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

            let mut reservation_expired: Option<Uuid> = None;
            let mut effective_status = input.status;
            let mut line_deltas: Vec<LineDelta> = Vec::new();

            if let Some(status) = input.status {
                let is_ship = status == OrderStatus::Shipped && !matches!(ship, ShipMode::None);
                if is_ship {
                    let (resolved, deltas) = Self::plan_shipment_in_tx(tx, id, ship)?;
                    effective_status = Some(resolved);
                    line_deltas = deltas;
                }
                let target = effective_status.unwrap_or(status);

                if !current_status.can_transition_to(target) {
                    if target == OrderStatus::Cancelled {
                        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                            CommerceError::OrderCannotBeCancelled(current_status.to_string()),
                        )));
                    }

                    return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                        CommerceError::InvalidOrderStatusTransition {
                            from: current_status.to_string(),
                            to: target.to_string(),
                        },
                    )));
                }

                if status == OrderStatus::Refunded {
                    let effective_payment_status =
                        input.payment_status.unwrap_or(current_payment_status);
                    if !matches!(
                        effective_payment_status,
                        PaymentStatus::Paid
                            | PaymentStatus::PartiallyPaid
                            | PaymentStatus::Refunded
                            | PaymentStatus::PartiallyRefunded
                    ) {
                        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                            CommerceError::OrderCannotBeRefunded(
                                effective_payment_status.to_string(),
                            ),
                        )));
                    }
                }

                if is_ship {
                    reservation_expired =
                        Self::confirm_shipped_reservations_in_tx(tx, id, ship, &line_deltas, now)?;
                }
            }

            if let Some(expired_id) = reservation_expired {
                let result = tx.query_row(
                    "SELECT * FROM orders WHERE id = ?",
                    [id.to_string()],
                    Self::row_to_order,
                );

                let mut order = match result {
                    Ok(order) => order,
                    Err(rusqlite::Error::QueryReturnedNoRows) => {
                        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                            CommerceError::OrderNotFound(id.into_uuid()),
                        )));
                    }
                    Err(e) => return Err(e),
                };

                order.items = Self::load_order_items_with_conn(tx, id)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

                return Ok(UpdateOutcome {
                    order,
                    post_commit_error: Some(CommerceError::ReservationExpired(expired_id)),
                });
            }

            for delta in line_deltas.iter().filter(|d| d.delta > 0) {
                tx.execute(
                    "UPDATE order_items SET shipped_quantity = shipped_quantity + ? WHERE id = ?",
                    rusqlite::params![delta.delta, delta.item_id],
                )?;
            }

            // Money rule for cancel (see `UpdateOrder::void_payments`): an
            // order whose payments still hold money cannot be cancelled
            // unless the caller explicitly voids; even then only in-flight
            // payments are voided here — settled money leaves via a refund.
            let mut cancel_money = CancelMoney::default();
            if matches!(input.status, Some(OrderStatus::Cancelled)) {
                let open = open_captures_for_order_conn(tx, &id.to_string())?;
                if !open.is_empty() && !input.void_payments {
                    let outstanding: Decimal =
                        open.iter().map(|p| p.amount - p.amount_refunded).sum();
                    let currency = open[0].currency;
                    return Err(to_sql_err(CommerceError::ValidationError(format!(
                        "order {id} cannot be cancelled: {} payment(s) still hold {outstanding} {currency}; \
                         refund them first, or cancel with void_payments = true to void in-flight \
                         payments and leave settled ones for refund",
                        open.len()
                    ))));
                }
                if input.void_payments {
                    cancel_money.voided_payment_ids =
                        void_in_flight_payments_for_order_conn(tx, &id.to_string(), now)?;
                    let voided = &cancel_money.voided_payment_ids;
                    let outstanding: Vec<_> =
                        open.iter().filter(|p| !voided.contains(&p.id.into_uuid())).collect();
                    cancel_money.outstanding_captured =
                        outstanding.iter().map(|p| p.amount - p.amount_refunded).sum();
                    cancel_money.outstanding_payment_ids =
                        outstanding.iter().map(|p| p.id.into_uuid()).collect();
                }
            }

            // Build dynamic update
            let mut updates = vec!["updated_at = ?"];
            let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now.to_rfc3339())];

            if let Some(status) = &effective_status {
                updates.push("status = ?");
                params.push(Box::new(status.to_string()));
            }
            if let Some(payment_status) = &input.payment_status {
                updates.push("payment_status = ?");
                params.push(Box::new(payment_status.to_string()));
            }
            if let Some(fulfillment_status) = &input.fulfillment_status {
                updates.push("fulfillment_status = ?");
                params.push(Box::new(fulfillment_status.to_string()));
            }
            if let Some(tracking) = &input.tracking_number {
                updates.push("tracking_number = ?");
                params.push(Box::new(tracking.clone()));
            }
            if let Some(notes) = &input.notes {
                updates.push("notes = ?");
                params.push(Box::new(notes.clone()));
            }
            if let Some(addr_json) = &shipping_address_json {
                updates.push("shipping_address = ?");
                params.push(Box::new(addr_json.clone()));
            }
            if let Some(addr_json) = &billing_address_json {
                updates.push("billing_address = ?");
                params.push(Box::new(addr_json.clone()));
            }

            updates.push("version = version + 1");
            params.push(Box::new(id.to_string()));
            params.push(Box::new(current_version));

            let sql =
                format!("UPDATE orders SET {} WHERE id = ? AND version = ?", updates.join(", "));

            let params_refs: Vec<&dyn rusqlite::ToSql> =
                params.iter().map(std::convert::AsRef::as_ref).collect();
            let rows_affected = tx.execute(&sql, params_refs.as_slice())?;
            if rows_affected == 0 {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    CommerceError::VersionConflict {
                        entity: "order".to_string(),
                        id: id.to_string(),
                        expected_version: current_version,
                    },
                )));
            }

            if matches!(input.status, Some(OrderStatus::Cancelled)) {
                let reservation_ids =
                    SqliteInventoryRepository::list_reservation_ids_by_reference_in_tx(
                        tx,
                        "order",
                        &id.to_string(),
                    )?;
                for reservation_id in reservation_ids {
                    SqliteInventoryRepository::release_reservation_in_tx(tx, reservation_id)?;
                }
                cancel_backorders_for_order_in_tx(tx, id.into_uuid())?;
            }

            let result = tx.query_row(
                "SELECT * FROM orders WHERE id = ?",
                [id.to_string()],
                Self::row_to_order,
            );

            let mut order = match result {
                Ok(order) => order,
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                        CommerceError::OrderNotFound(id.into_uuid()),
                    )));
                }
                Err(e) => return Err(e),
            };

            order.items = Self::load_order_items_with_conn(tx, id)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

            append_kernel_event_tx(
                tx,
                &KernelOutboxEvent::domain(
                    "orders.updated.v1",
                    "order",
                    id.to_string(),
                    serde_json::json!({
                        "order_id": id.to_string(),
                        "status_before": current_status.to_string(),
                        "status_after": order.status.to_string(),
                        "payment_status_before": current_payment_status.to_string(),
                        "payment_status_after": order.payment_status.to_string(),
                        "fulfillment_status_after": order.fulfillment_status.to_string(),
                        "version_before": current_version,
                        "version_after": order.version,
                        "total_amount": order.total_amount.to_string(),
                        "void_payments": input.void_payments,
                        "voided_payment_ids": cancel_money.voided_payment_ids,
                        "outstanding_payment_ids": cancel_money.outstanding_payment_ids,
                        "outstanding_captured": cancel_money.outstanding_captured.to_string(),
                    }),
                    None,
                ),
            )?;

            Ok(UpdateOutcome { order, post_commit_error: None })
        }
    }

    /// Shared implementation of [`OrderRepository::update`] and [`OrderRepository::ship`]:
    /// one [`Self::apply_update_in_tx`] in its own immediate transaction.
    fn apply_update(&self, id: OrderId, input: UpdateOrder, ship: ShipMode<'_>) -> Result<Order> {
        let outcome = with_immediate_transaction(&self.pool, |tx| {
            Self::apply_update_in_tx(tx, id, &input, &ship)
        })?;

        if let Some(err) = outcome.post_commit_error {
            return Err(err);
        }

        Ok(outcome.order)
    }

    /// How a plain status update touches the lines: `Shipped` ships every
    /// remaining unit; `PartiallyShipped` is derived from line quantities and
    /// cannot be set directly (use [`OrderRepository::ship`] with lines).
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
}

impl OrderRepository for SqliteOrderRepository {
    fn create(&self, input: CreateOrder) -> Result<Order> {
        self.create_internal(None, false, input)
    }

    fn create_from_cart(&self, cart_id: CartId, input: CreateOrder) -> Result<Order> {
        Self::create_from_cart(self, cart_id.into_uuid(), input)
    }

    fn get(&self, id: OrderId) -> Result<Option<Order>> {
        let conn = self.conn()?;
        let result = conn.query_row(
            "SELECT * FROM orders WHERE id = ?",
            [id.to_string()],
            Self::row_to_order,
        );

        match result {
            Ok(mut order) => {
                order.items = Self::load_order_items_with_conn(&conn, id)?;
                Ok(Some(order))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn get_by_number(&self, order_number: &str) -> Result<Option<Order>> {
        let conn = self.conn()?;
        let result = conn.query_row(
            "SELECT * FROM orders WHERE order_number = ?",
            [order_number],
            Self::row_to_order,
        );

        match result {
            Ok(mut order) => {
                order.items = Self::load_order_items_with_conn(&conn, order.id)?;
                Ok(Some(order))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn update(&self, id: OrderId, input: UpdateOrder) -> Result<Order> {
        let mode = Self::ship_mode_for_update(&input)?;
        self.apply_update(id, input, mode)
    }

    fn ship(&self, id: OrderId, input: ShipOrder) -> Result<Order> {
        let ShipOrder { tracking_number, lines } = input;
        let lines = lines.unwrap_or_default();
        let mode = if lines.is_empty() { ShipMode::All } else { ShipMode::Lines(&lines) };
        self.apply_update(
            id,
            UpdateOrder {
                status: Some(OrderStatus::Shipped),
                tracking_number,
                ..Default::default()
            },
            mode,
        )
    }

    fn list(&self, filter: OrderFilter) -> Result<Vec<Order>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM orders WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(customer_id) = &filter.customer_id {
            sql.push_str(" AND customer_id = ?");
            params.push(Box::new(customer_id.to_string()));
        }
        if let Some(status) = &filter.status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(status.to_string()));
        }
        if let Some(payment_status) = &filter.payment_status {
            sql.push_str(" AND payment_status = ?");
            params.push(Box::new(payment_status.to_string()));
        }
        if let Some(fulfillment_status) = &filter.fulfillment_status {
            sql.push_str(" AND fulfillment_status = ?");
            params.push(Box::new(fulfillment_status.to_string()));
        }
        if let Some(from) = &filter.from_date {
            sql.push_str(" AND order_date >= ?");
            params.push(Box::new(from.to_rfc3339()));
        }
        if let Some(to) = &filter.to_date {
            sql.push_str(" AND order_date <= ?");
            params.push(Box::new(to.to_rfc3339()));
        }

        // Keyset cursor: (order_date, id) for stable DESC ordering
        if let Some((cursor_date, cursor_id)) = &filter.after_cursor {
            sql.push_str(" AND (order_date < ? OR (order_date = ? AND id < ?))");
            params.push(Box::new(cursor_date.clone()));
            params.push(Box::new(cursor_date.clone()));
            params.push(Box::new(cursor_id.clone()));
        }

        sql.push_str(" ORDER BY order_date DESC, id DESC");

        // Offset pagination applies only in non-cursor mode; the helper emits
        // `LIMIT -1 OFFSET n` when an offset is set without a limit (SQLite rejects
        // a bare OFFSET).
        let offset = if filter.after_cursor.is_none() { filter.offset } else { None };
        crate::sqlite::append_limit_offset(&mut sql, filter.limit, offset);

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;

        let orders = stmt
            .query_map(params_refs.as_slice(), Self::row_to_order)
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        // Load items for all orders in one batched query on the same connection
        let ids: Vec<OrderId> = orders.iter().map(|o| o.id).collect();
        let mut items_by_id = Self::load_order_items_batch(&conn, &ids)?;
        let mut result = vec![];
        for mut order in orders {
            order.items = items_by_id.remove(&order.id).unwrap_or_default();
            result.push(order);
        }

        Ok(result)
    }

    fn delete(&self, id: OrderId) -> Result<()> {
        with_immediate_transaction(&self.pool, |tx| Self::delete_in_tx(tx, id))
    }

    fn add_item(&self, order_id: OrderId, item: CreateOrderItem) -> Result<OrderItem> {
        validate_required_uuid("order.id", order_id.into_uuid())?;
        Self::validate_order_item_input(&item)?;

        let item_id = OrderItemId::new();
        let item_total = OrderItem::calculate_total(
            item.quantity,
            item.unit_price,
            item.discount.unwrap_or_default(),
            item.tax_amount.unwrap_or_default(),
        );
        let order_item = OrderItem {
            id: item_id,
            order_id,
            product_id: item.product_id,
            variant_id: item.variant_id,
            sku: item.sku,
            name: item.name,
            quantity: item.quantity,
            shipped_quantity: 0,
            unit_price: item.unit_price,
            discount: item.discount.unwrap_or_default(),
            tax_amount: item.tax_amount.unwrap_or_default(),
            total: item_total,
        };

        with_immediate_transaction(&self.pool, |tx| {
            // Status predicate, line insert, stock reservation/backorder and
            // total recompute share this transaction: a refused or failed add
            // leaves no line and no reservation behind.
            let (status, customer_id) = Self::load_status_and_customer_in_tx(tx, order_id)?;
            ensure_lines_mutable(order_id, status).map_err(to_sql_err)?;

            tx.execute(
                "INSERT INTO order_items (id, order_id, product_id, variant_id, sku, name,
                                          quantity, unit_price, discount, tax_amount, total)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    order_item.id.to_string(),
                    order_id.to_string(),
                    order_item.product_id.to_string(),
                    order_item.variant_id.map(|v| v.to_string()),
                    &order_item.sku,
                    &order_item.name,
                    order_item.quantity,
                    order_item.unit_price.to_string(),
                    order_item.discount.to_string(),
                    order_item.tax_amount.to_string(),
                    order_item.total.to_string(),
                ],
            )?;

            // Same stock-policy path as creation. A line added later has no
            // per-request policy, so it takes the default (`AllowBackorder`):
            // reserve what is available, backorder the rest.
            Self::reserve_line_stock_in_tx(
                tx,
                order_id,
                customer_id,
                &order_item,
                StockPolicy::default(),
            )?;

            Self::update_order_total(tx, order_id).map_err(to_sql_err)?;
            Self::append_line_event_tx(tx, "orders.item_added.v1", order_id, &order_item)?;
            Ok(())
        })?;

        Ok(order_item)
    }

    fn remove_item(&self, order_id: OrderId, item_id: OrderItemId) -> Result<()> {
        with_immediate_transaction(&self.pool, |tx| {
            let (status, _customer_id) = Self::load_status_and_customer_in_tx(tx, order_id)?;
            ensure_lines_mutable(order_id, status).map_err(to_sql_err)?;

            let removed = match tx.query_row(
                "SELECT * FROM order_items WHERE id = ? AND order_id = ?",
                [item_id.to_string(), order_id.to_string()],
                Self::row_to_order_item,
            ) {
                Ok(line) => line,
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    return Err(to_sql_err(CommerceError::ValidationError(format!(
                        "Order item {item_id} does not belong to order {order_id}"
                    ))));
                }
                Err(e) => return Err(e),
            };

            // Give the line's stock back and drop its backorder in the same
            // transaction as the delete, so removal never leaks a hold.
            Self::release_line_reservations_in_tx(
                tx,
                order_id,
                item_id,
                &removed.sku,
                Decimal::from(removed.quantity),
            )?;
            cancel_backorders_for_order_line_in_tx(tx, order_id.into_uuid(), item_id.into_uuid())?;

            tx.execute(
                "DELETE FROM order_items WHERE id = ? AND order_id = ?",
                [item_id.to_string(), order_id.to_string()],
            )?;

            Self::update_order_total(tx, order_id).map_err(to_sql_err)?;
            Self::append_line_event_tx(tx, "orders.item_removed.v1", order_id, &removed)?;
            Ok(())
        })
    }

    fn count(&self, filter: OrderFilter) -> Result<u64> {
        let conn = self.conn()?;
        let mut sql = "SELECT COUNT(*) FROM orders WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(customer_id) = &filter.customer_id {
            sql.push_str(" AND customer_id = ?");
            params.push(Box::new(customer_id.to_string()));
        }
        if let Some(status) = &filter.status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(status.to_string()));
        }
        if let Some(payment_status) = &filter.payment_status {
            sql.push_str(" AND payment_status = ?");
            params.push(Box::new(payment_status.to_string()));
        }
        if let Some(fulfillment_status) = &filter.fulfillment_status {
            sql.push_str(" AND fulfillment_status = ?");
            params.push(Box::new(fulfillment_status.to_string()));
        }
        if let Some(from) = &filter.from_date {
            sql.push_str(" AND order_date >= ?");
            params.push(Box::new(from.to_rfc3339()));
        }
        if let Some(to) = &filter.to_date {
            sql.push_str(" AND order_date <= ?");
            params.push(Box::new(to.to_rfc3339()));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();
        let count: i64 =
            conn.query_row(&sql, params_refs.as_slice(), |row| row.get(0)).map_err(map_db_error)?;

        Ok(count as u64)
    }

    // === Batch Operations ===

    fn create_batch(&self, inputs: Vec<CreateOrder>) -> Result<BatchResult<Order>> {
        validate_batch_size(&inputs)?;
        let mut result = BatchResult::with_capacity(inputs.len());

        // Use a single connection for the entire batch to avoid pool churn.
        // Each order still gets its own transaction for partial-success semantics.
        let mut conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        for (index, input) in inputs.into_iter().enumerate() {
            Self::validate_order_input(&input)?;
            let tx_result =
                conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate);
            match tx_result {
                Ok(tx) => {
                    match Self::create_internal_in_tx(&tx, None, false, &input) {
                        Ok(order) => {
                            if let Err(e) = tx.commit() {
                                result.record_failure(index, None, &map_db_error(e));
                            } else {
                                result.record_success(order);
                            }
                        }
                        Err(e) => {
                            // Transaction auto-rolls back on drop
                            result.record_failure(index, None, &map_db_error(e));
                        }
                    }
                }
                Err(e) => result.record_failure(index, None, &map_db_error(e)),
            }
        }

        Ok(result)
    }

    fn create_batch_atomic(&self, inputs: Vec<CreateOrder>) -> Result<Vec<Order>> {
        validate_batch_size(&inputs)?;
        if inputs.is_empty() {
            return Ok(vec![]);
        }

        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let mut results = Vec::with_capacity(inputs.len());

        for input in inputs {
            Self::validate_order_input(&input)?;

            // Route batch creation through the SAME guarded path as a single
            // create. The batch loop previously inserted orders and items
            // directly, performing no stock check, no reservation and no
            // backorder: batch-creating 100 units against 5 in stock succeeded,
            // and shipping later found zero reservations, so inventory was
            // never decremented. `create_internal_in_tx` enforces the stock
            // policy, reserves what is available, backorders the remainder, and
            // carries the order-level money — all on this transaction, so one
            // rejected line still rolls the whole batch back.
            let order =
                Self::create_internal_in_tx(&tx, None, false, &input).map_err(map_db_error)?;
            results.push(order);
        }

        tx.commit().map_err(map_db_error)?;
        Ok(results)
    }

    fn update_batch(&self, updates: Vec<(OrderId, UpdateOrder)>) -> Result<BatchResult<Order>> {
        validate_batch_size(&updates)?;
        let mut result = BatchResult::with_capacity(updates.len());

        for (index, (id, input)) in updates.into_iter().enumerate() {
            match self.update(id, input) {
                Ok(order) => result.record_success(order),
                Err(e) => result.record_failure(index, Some(id.to_string()), &e),
            }
        }

        Ok(result)
    }

    fn update_batch_atomic(&self, updates: Vec<(OrderId, UpdateOrder)>) -> Result<Vec<Order>> {
        validate_batch_size(&updates)?;
        if updates.is_empty() {
            return Ok(vec![]);
        }

        // Every row goes through the SAME in-transaction path as a single
        // `update` (shipment planning, reservation confirm/release, backorder
        // cancel, outbox event, `PartiallyShipped` rejection), sharing one
        // commit: batch == N single updates, all or nothing.
        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let mut results = Vec::with_capacity(updates.len());

        for (id, input) in &updates {
            let mode = Self::ship_mode_for_update(input)?;
            let outcome = Self::apply_update_in_tx(&tx, *id, input, &mode).map_err(map_db_error)?;
            if let Some(err) = outcome.post_commit_error {
                // A single update commits the expiry bookkeeping before
                // surfacing `ReservationExpired`; an atomic batch cannot
                // commit part of itself, so the whole batch rolls back (the
                // reservation is still expired by time and is re-expired on
                // retry).
                return Err(err);
            }
            results.push(outcome.order);
        }

        tx.commit().map_err(map_db_error)?;
        Ok(results)
    }

    fn delete_batch(&self, ids: Vec<OrderId>) -> Result<BatchResult<OrderId>> {
        validate_batch_size(&ids)?;
        let mut result = BatchResult::with_capacity(ids.len());

        for (index, id) in ids.into_iter().enumerate() {
            match self.delete(id) {
                Ok(()) => result.record_success(id),
                Err(e) => result.record_failure(index, Some(id.to_string()), &e),
            }
        }

        Ok(result)
    }

    fn delete_batch_atomic(&self, ids: Vec<OrderId>) -> Result<()> {
        validate_batch_size(&ids)?;
        if ids.is_empty() {
            return Ok(());
        }

        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;

        // Same per-order path as a single delete (status predicate, reservation
        // release, backorder cancel), one commit.
        for id in &ids {
            Self::delete_in_tx(&tx, *id).map_err(map_db_error)?;
        }

        tx.commit().map_err(map_db_error)?;
        Ok(())
    }

    fn get_batch(&self, ids: Vec<OrderId>) -> Result<Vec<Order>> {
        validate_batch_size(&ids)?;
        if ids.is_empty() {
            return Ok(vec![]);
        }

        let conn = self.conn()?;
        let placeholders = build_in_clause(ids.len());
        let sql = format!("SELECT * FROM orders WHERE id IN ({placeholders})");

        let raw_ids: Vec<Uuid> = ids.iter().map(|id| id.into_uuid()).collect();
        let params = uuid_params(&raw_ids);
        let params_refs = params_refs(&params);

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let orders = stmt
            .query_map(params_refs.as_slice(), Self::row_to_order)
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        // Load items for all orders in one batched query
        let order_ids: Vec<OrderId> = orders.iter().map(|o| o.id).collect();
        let mut items_by_id = Self::load_order_items_batch(&conn, &order_ids)?;
        let mut result = vec![];
        for mut order in orders {
            order.items = items_by_id.remove(&order.id).unwrap_or_default();
            result.push(order);
        }

        Ok(result)
    }
}

impl SqliteOrderRepository {
    /// Current `(status, customer_id)` of an order, or `OrderNotFound`.
    fn load_status_and_customer_in_tx(
        tx: &rusqlite::Transaction<'_>,
        order_id: OrderId,
    ) -> std::result::Result<(OrderStatus, CustomerId), rusqlite::Error> {
        let (status_raw, customer_raw) = match tx.query_row(
            "SELECT status, customer_id FROM orders WHERE id = ?",
            [order_id.to_string()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        ) {
            Ok(row) => row,
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                return Err(to_sql_err(CommerceError::OrderNotFound(order_id.into_uuid())));
            }
            Err(e) => return Err(e),
        };
        let status: OrderStatus = parse_enum(&status_raw, "order", "status").map_err(to_sql_err)?;
        let customer_id = CustomerId::from(parse_uuid_row(&customer_raw, "order", "customer_id")?);
        Ok((status, customer_id))
    }

    /// Re-derive `total_amount` after a line change.
    ///
    /// The total always foots to `Σ order_items.total + tax_amount +
    /// shipping_amount - discount_amount` — the same rule `create` writes and
    /// [`Order::calculate_total`] reads. It used to be the bare line sum, which
    /// silently dropped the order-level money on the first `add_item`.
    /// Kernel outbox event for a line edit (`orders.item_added.v1` /
    /// `orders.item_removed.v1`), written on the same transaction as the
    /// line change so consumers see every mutation of the order's money and
    /// stock, not only status transitions.
    fn append_line_event_tx(
        tx: &rusqlite::Transaction<'_>,
        kind: &str,
        order_id: OrderId,
        item: &OrderItem,
    ) -> std::result::Result<(), rusqlite::Error> {
        let total_amount: String = tx.query_row(
            "SELECT total_amount FROM orders WHERE id = ?",
            [order_id.to_string()],
            |row| row.get(0),
        )?;
        append_kernel_event_tx(
            tx,
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
                    "total_amount": total_amount,
                }),
                None,
            ),
        )
    }

    fn update_order_total(conn: &rusqlite::Connection, order_id: OrderId) -> Result<()> {
        let (current_version, tax_raw, shipping_raw, discount_raw): (i32, String, String, String) =
            conn.query_row(
                "SELECT version, tax_amount, shipping_amount, discount_amount FROM orders WHERE id = ?",
                [order_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    CommerceError::OrderNotFound(order_id.into_uuid())
                }
                e => map_db_error(e),
            })?;
        let tax = parse_decimal_row(&tax_raw, "order", "tax_amount").map_err(map_db_error)?;
        let shipping =
            parse_decimal_row(&shipping_raw, "order", "shipping_amount").map_err(map_db_error)?;
        let discount =
            parse_decimal_row(&discount_raw, "order", "discount_amount").map_err(map_db_error)?;

        let order_id_param = order_id.to_string();
        let order_params: [&dyn rusqlite::ToSql; 1] = [&order_id_param];
        let line_total = sum_decimal_query(
            conn,
            "SELECT total FROM order_items WHERE order_id = ?",
            &order_params,
            "order_item",
            "total",
        )?;
        let total = (line_total + tax + shipping - discount).to_string();

        let rows_affected = conn
            .execute(
                "UPDATE orders SET total_amount = ?, updated_at = ?, version = version + 1 WHERE id = ? AND version = ?",
                rusqlite::params![
                    total,
                    Utc::now().to_rfc3339(),
                    order_id.to_string(),
                    current_version
                ],
            )
            .map_err(map_db_error)?;
        if rows_affected == 0 {
            return Err(CommerceError::VersionConflict {
                entity: "order".to_string(),
                id: order_id.to_string(),
                expected_version: current_version,
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn parses_payment_status_snake_case_and_legacy() {
        assert_eq!(
            PaymentStatus::from_str("partially_paid").unwrap(),
            PaymentStatus::PartiallyPaid
        );
        assert_eq!(PaymentStatus::from_str("partiallypaid").unwrap(), PaymentStatus::PartiallyPaid);
        assert_eq!(
            PaymentStatus::from_str("partially_refunded").unwrap(),
            PaymentStatus::PartiallyRefunded
        );
        assert_eq!(
            PaymentStatus::from_str("partiallyrefunded").unwrap(),
            PaymentStatus::PartiallyRefunded
        );
    }

    #[test]
    fn parses_fulfillment_status_snake_case_and_legacy() {
        assert_eq!(
            FulfillmentStatus::from_str("partially_fulfilled").unwrap(),
            FulfillmentStatus::PartiallyFulfilled
        );
        assert_eq!(
            FulfillmentStatus::from_str("partiallyfulfilled").unwrap(),
            FulfillmentStatus::PartiallyFulfilled
        );
    }
}
