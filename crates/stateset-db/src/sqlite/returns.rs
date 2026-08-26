//! SQLite return repository implementation

use super::bins::{
    apply_bin_delta_tx, apply_warehouse_delta_tx, find_disposition_bin_tx, insert_bin_movement_tx,
    smuggle,
};
use super::parse_helpers::{parse_decimal, parse_enum, parse_uuid};
use super::{
    build_in_clause, map_db_error, params_refs, parse_datetime_opt_row, parse_datetime_row,
    parse_decimal_opt_row, parse_decimal_row, parse_enum_row, parse_uuid_row, sum_decimal_query,
    uuid_params, with_immediate_transaction,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::OptionalExtension;
use rust_decimal::Decimal;
use stateset_core::{
    BatchResult, BinMovementType, BinType, CommerceError, CreateReturn, CustomerId, OrderId,
    OrderItemId, OrderStatus, Result, Return, ReturnDisposition, ReturnFilter, ReturnId,
    ReturnItem, ReturnRepository, ReturnStatus, SetReturnDisposition, UpdateReturn,
    validate_batch_size,
};
use uuid::Uuid;

/// Parse the nullable `disposition` column of a `return_items` row.
fn parse_disposition_row(
    row: &rusqlite::Row<'_>,
) -> std::result::Result<Option<ReturnDisposition>, rusqlite::Error> {
    match row.get::<_, Option<String>>("disposition")? {
        Some(raw) if !raw.is_empty() => {
            Ok(Some(parse_enum_row(&raw, "return_item", "disposition")?))
        }
        _ => Ok(None),
    }
}

const RETURN_ITEM_COLUMNS: &str = "id, return_id, order_item_id, sku, name, quantity, condition, \
                                   refund_amount, disposition, disposition_at, disposition_by";

fn row_to_return_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<ReturnItem> {
    Ok(ReturnItem {
        id: parse_uuid_row(&row.get::<_, String>("id")?, "return_item", "id")?,
        return_id: ReturnId::from(parse_uuid_row(
            &row.get::<_, String>("return_id")?,
            "return_item",
            "return_id",
        )?),
        order_item_id: OrderItemId::from(parse_uuid_row(
            &row.get::<_, String>("order_item_id")?,
            "return_item",
            "order_item_id",
        )?),
        sku: row.get("sku")?,
        name: row.get("name")?,
        quantity: row.get("quantity")?,
        condition: parse_enum_row(&row.get::<_, String>("condition")?, "return_item", "condition")?,
        refund_amount: parse_decimal_row(
            &row.get::<_, String>("refund_amount")?,
            "return_item",
            "refund_amount",
        )?,
        disposition: parse_disposition_row(row)?,
        disposition_at: parse_datetime_opt_row(
            row.get::<_, Option<String>>("disposition_at")?,
            "return_item",
            "disposition_at",
        )?,
        disposition_by: row.get("disposition_by")?,
    })
}

/// SQLite implementation of `ReturnRepository`
#[derive(Debug)]
pub struct SqliteReturnRepository {
    pool: Pool<SqliteConnectionManager>,
}

/// Returns may only be requested against orders whose goods have left the
/// building: `PartiallyShipped`, `Shipped` or `Delivered`. Anything earlier has
/// nothing to send back (and would let a return + refund be opened against
/// goods never fulfilled); cancelled/refunded orders are closed. A partially
/// shipped order is returnable for the units that actually shipped — the
/// per-line `shipped_quantity` cap enforces that.
fn ensure_order_returnable(order_id: &str, raw_status: &str) -> Result<()> {
    let status: OrderStatus = parse_enum(raw_status, "order", "status")?;
    if matches!(
        status,
        OrderStatus::PartiallyShipped | OrderStatus::Shipped | OrderStatus::Delivered
    ) {
        return Ok(());
    }
    Err(CommerceError::ReturnOrderNotShipped {
        order_id: parse_uuid(order_id, "order", "id")?,
        status: status.to_string(),
    })
}

/// Validate a single return line against its order item, inside a write
/// transaction, returning the item's `(sku, name, unit_price)` for the caller
/// to record on the return.
///
/// Rejects the return when:
/// - the order item does not exist,
/// - the order item belongs to a different order than the one being returned, or
/// - returning `return_qty` more units would exceed what was purchased (or, once
///   the order has shipped, what was actually shipped on that line), counting
///   units already claimed by non-terminal returns (rejected/cancelled returns
///   release their claim).
///
/// This guards against over-returning (and thus over-refunding) more units than
/// were ordered, and against returning another order's items.
fn validate_return_item_tx(
    tx: &rusqlite::Transaction<'_>,
    order_id: &str,
    order_item_id: &str,
    return_qty: i64,
) -> Result<(String, String, String)> {
    let (sku, name, unit_price, oi_order_id, ordered_qty, shipped_qty, order_status): (
        String,
        String,
        String,
        String,
        i64,
        i64,
        String,
    ) = tx
        .query_row(
            "SELECT oi.sku, oi.name, oi.unit_price, oi.order_id, oi.quantity, oi.shipped_quantity, o.status
             FROM order_items oi JOIN orders o ON o.id = oi.order_id
             WHERE oi.id = ?",
            [order_item_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                ))
            },
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                CommerceError::ValidationError(format!("Order item {order_item_id} not found"))
            }
            other => map_db_error(other),
        })?;

    if oi_order_id != order_id {
        return Err(CommerceError::ValidationError(format!(
            "Order item {order_item_id} does not belong to order {order_id}"
        )));
    }

    // Units already returned for this order item, excluding rejected/cancelled
    // returns (which release their claim on the ordered quantity).
    let already_returned: i64 = tx
        .query_row(
            "SELECT COALESCE(SUM(ri.quantity), 0) FROM return_items ri
             JOIN returns r ON ri.return_id = r.id
             WHERE ri.order_item_id = ? AND r.status NOT IN ('rejected', 'cancelled')",
            [order_item_id],
            |row| row.get(0),
        )
        .map_err(map_db_error)?;

    // Once the order has shipped (fully or partially), only units that actually
    // left the warehouse are returnable. Before shipment the cap stays at the
    // ordered quantity (legacy behaviour).
    let (cap, cap_label) = if crate::error_helpers::order_status_has_shipped(&order_status) {
        (shipped_qty, "shipped")
    } else {
        (ordered_qty, "ordered")
    };

    if return_qty + already_returned > cap {
        return Err(CommerceError::ReturnExceedsReturnable {
            order_item_id: parse_uuid(order_item_id, "order_item", "id")?,
            basis: cap_label,
            returnable: cap,
            already_returned,
            requested: return_qty,
        });
    }

    Ok((sku, name, unit_price))
}

impl SqliteReturnRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_return(row: &rusqlite::Row<'_>) -> rusqlite::Result<Return> {
        Ok(Return {
            id: ReturnId::from(parse_uuid_row(&row.get::<_, String>("id")?, "return", "id")?),
            order_id: OrderId::from(parse_uuid_row(
                &row.get::<_, String>("order_id")?,
                "return",
                "order_id",
            )?),
            customer_id: CustomerId::from(parse_uuid_row(
                &row.get::<_, String>("customer_id")?,
                "return",
                "customer_id",
            )?),
            status: parse_enum_row(&row.get::<_, String>("status")?, "return", "status")?,
            reason: parse_enum_row(&row.get::<_, String>("reason")?, "return", "reason")?,
            reason_details: row.get("reason_details")?,
            idempotency_key: row.get("idempotency_key")?,
            refund_amount: parse_decimal_opt_row(
                row.get::<_, Option<String>>("refund_amount")?,
                "return",
                "refund_amount",
            )?,
            refund_method: row.get("refund_method")?,
            tracking_number: row.get("tracking_number")?,
            items: vec![], // Loaded separately
            notes: row.get("notes")?,
            version: row.get::<_, Option<i32>>("version")?.unwrap_or(1),
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "return",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "return",
                "updated_at",
            )?,
        })
    }

    /// Load items for many returns in one batched `IN (...)` query, keyed by
    /// the return id's string form.
    fn load_return_items_batch(
        conn: &rusqlite::Connection,
        ids: &[ReturnId],
    ) -> Result<std::collections::HashMap<String, Vec<ReturnItem>>> {
        let mut items_by_id: std::collections::HashMap<String, Vec<ReturnItem>> =
            std::collections::HashMap::with_capacity(ids.len());
        if ids.is_empty() {
            return Ok(items_by_id);
        }
        let placeholders = vec!["?"; ids.len()].join(", ");
        let sql = format!(
            "SELECT id, return_id, order_item_id, sku, name, quantity, condition, refund_amount, disposition, disposition_at, disposition_by
             FROM return_items WHERE return_id IN ({placeholders})"
        );
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let params = ids.iter().map(ToString::to_string).collect::<Vec<_>>();
        let items = stmt
            .query_map(rusqlite::params_from_iter(params.iter()), |row| {
                Ok(ReturnItem {
                    id: parse_uuid_row(&row.get::<_, String>("id")?, "return_item", "id")?,
                    return_id: ReturnId::from(parse_uuid_row(
                        &row.get::<_, String>("return_id")?,
                        "return_item",
                        "return_id",
                    )?),
                    order_item_id: OrderItemId::from(parse_uuid_row(
                        &row.get::<_, String>("order_item_id")?,
                        "return_item",
                        "order_item_id",
                    )?),
                    sku: row.get("sku")?,
                    name: row.get("name")?,
                    quantity: row.get("quantity")?,
                    condition: parse_enum_row(
                        &row.get::<_, String>("condition")?,
                        "return_item",
                        "condition",
                    )?,
                    refund_amount: parse_decimal_row(
                        &row.get::<_, String>("refund_amount")?,
                        "return_item",
                        "refund_amount",
                    )?,
                    disposition: parse_disposition_row(row)?,
                    disposition_at: parse_datetime_opt_row(
                        row.get::<_, Option<String>>("disposition_at")?,
                        "return_item",
                        "disposition_at",
                    )?,
                    disposition_by: row.get("disposition_by")?,
                })
            })
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;
        for item in items {
            items_by_id.entry(item.return_id.to_string()).or_default().push(item);
        }
        Ok(items_by_id)
    }

    #[allow(dead_code)]
    fn load_return_items(&self, return_id: Uuid) -> Result<Vec<ReturnItem>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, return_id, order_item_id, sku, name, quantity, condition, refund_amount, disposition, disposition_at, disposition_by
                 FROM return_items WHERE return_id = ?",
            )
            .map_err(map_db_error)?;

        let items = stmt
            .query_map([return_id.to_string()], |row| {
                Ok(ReturnItem {
                    id: parse_uuid_row(&row.get::<_, String>("id")?, "return_item", "id")?,
                    return_id: ReturnId::from(parse_uuid_row(
                        &row.get::<_, String>("return_id")?,
                        "return_item",
                        "return_id",
                    )?),
                    order_item_id: OrderItemId::from(parse_uuid_row(
                        &row.get::<_, String>("order_item_id")?,
                        "return_item",
                        "order_item_id",
                    )?),
                    sku: row.get("sku")?,
                    name: row.get("name")?,
                    quantity: row.get("quantity")?,
                    condition: parse_enum_row(
                        &row.get::<_, String>("condition")?,
                        "return_item",
                        "condition",
                    )?,
                    refund_amount: parse_decimal_row(
                        &row.get::<_, String>("refund_amount")?,
                        "return_item",
                        "refund_amount",
                    )?,
                    disposition: parse_disposition_row(row)?,
                    disposition_at: parse_datetime_opt_row(
                        row.get::<_, Option<String>>("disposition_at")?,
                        "return_item",
                        "disposition_at",
                    )?,
                    disposition_by: row.get("disposition_by")?,
                })
            })
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        Ok(items)
    }

    /// Delete a return and its items
    fn delete(&self, id: Uuid) -> Result<()> {
        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;

        tx.execute("DELETE FROM return_items WHERE return_id = ?", [id.to_string()])
            .map_err(map_db_error)?;
        tx.execute("DELETE FROM returns WHERE id = ?", [id.to_string()]).map_err(map_db_error)?;
        tx.commit().map_err(map_db_error)?;
        Ok(())
    }

    fn get_by_idempotency_key(&self, key: &str) -> Result<Option<Return>> {
        let conn = self.conn()?;
        let result = conn.query_row(
            "SELECT * FROM returns WHERE idempotency_key = ?",
            [key],
            Self::row_to_return,
        );

        match result {
            Ok(mut ret) => {
                let mut stmt = conn
                    .prepare(
                        "SELECT id, return_id, order_item_id, sku, name, quantity, condition, refund_amount, disposition, disposition_at, disposition_by
                         FROM return_items WHERE return_id = ?",
                    )
                    .map_err(map_db_error)?;

                ret.items = stmt
                    .query_map([ret.id.to_string()], |row| {
                        Ok(ReturnItem {
                            id: parse_uuid_row(&row.get::<_, String>("id")?, "return_item", "id")?,
                            return_id: ReturnId::from(parse_uuid_row(
                                &row.get::<_, String>("return_id")?,
                                "return_item",
                                "return_id",
                            )?),
                            order_item_id: OrderItemId::from(parse_uuid_row(
                                &row.get::<_, String>("order_item_id")?,
                                "return_item",
                                "order_item_id",
                            )?),
                            sku: row.get("sku")?,
                            name: row.get("name")?,
                            quantity: row.get("quantity")?,
                            condition: parse_enum_row(
                                &row.get::<_, String>("condition")?,
                                "return_item",
                                "condition",
                            )?,
                            refund_amount: parse_decimal_row(
                                &row.get::<_, String>("refund_amount")?,
                                "return_item",
                                "refund_amount",
                            )?,
                            disposition: parse_disposition_row(row)?,
                            disposition_at: parse_datetime_opt_row(
                                row.get::<_, Option<String>>("disposition_at")?,
                                "return_item",
                                "disposition_at",
                            )?,
                            disposition_by: row.get("disposition_by")?,
                        })
                    })
                    .map_err(map_db_error)?
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .map_err(map_db_error)?;

                Ok(Some(ret))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }
}

impl ReturnRepository for SqliteReturnRepository {
    fn create(&self, input: CreateReturn) -> Result<Return> {
        if let Some(key) = input.idempotency_key.as_deref() {
            if let Some(existing) = self.get_by_idempotency_key(key)? {
                return Ok(existing);
            }
        }

        // Validate return has at least one item
        if input.items.is_empty() {
            return Err(CommerceError::ValidationError(
                "Return must have at least one item".into(),
            ));
        }

        // Validate item quantities
        for item in &input.items {
            if item.quantity <= 0 {
                return Err(CommerceError::ValidationError(format!(
                    "Return item quantity must be positive, got {}",
                    item.quantity
                )));
            }
        }

        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let id = Uuid::new_v4();
        let now = Utc::now();

        // Get order to get customer_id, and make sure it is returnable.
        let (customer_id, order_status): (String, String) = tx
            .query_row(
                "SELECT customer_id, status FROM orders WHERE id = ?",
                [input.order_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|_| CommerceError::OrderNotFound(input.order_id.into()))?;
        ensure_order_returnable(&input.order_id.to_string(), &order_status)?;

        tx.execute(
            "INSERT INTO returns (id, order_id, customer_id, status, reason, reason_details, idempotency_key, notes, created_at, updated_at)
             VALUES (?, ?, ?, 'requested', ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                id.to_string(),
                input.order_id.to_string(),
                customer_id,
                input.reason.to_string(),
                input.reason_details,
                input.idempotency_key,
                input.notes,
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;

        // Insert return items
        for item in &input.items {
            let item_id = Uuid::new_v4();

            // Validate the item belongs to this order and the return quantity
            // does not exceed what remains returnable, then get its details.
            let (sku, name, unit_price) = validate_return_item_tx(
                &tx,
                &input.order_id.to_string(),
                &item.order_item_id.to_string(),
                i64::from(item.quantity),
            )?;

            let refund_amount = parse_decimal(&unit_price, "order_item", "unit_price")?
                * Decimal::from(item.quantity);

            tx.execute(
                "INSERT INTO return_items (id, return_id, order_item_id, sku, name, quantity, condition, refund_amount)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    item_id.to_string(),
                    id.to_string(),
                    item.order_item_id.to_string(),
                    sku,
                    name,
                    item.quantity,
                    item.condition.unwrap_or_default().to_string(),
                    refund_amount.to_string(),
                ],
            )
            .map_err(map_db_error)?;
        }

        // Calculate total refund amount
        let return_id_param = id.to_string();
        let return_params: [&dyn rusqlite::ToSql; 1] = [&return_id_param];
        let total_refund = sum_decimal_query(
            &tx,
            "SELECT refund_amount FROM return_items WHERE return_id = ?",
            &return_params,
            "return_item",
            "refund_amount",
        )?;

        tx.execute(
            "UPDATE returns SET refund_amount = ? WHERE id = ?",
            rusqlite::params![total_refund.to_string(), return_id_param],
        )
        .map_err(map_db_error)?;

        // Build the return with items using the same transaction.
        let mut ret = tx
            .query_row("SELECT * FROM returns WHERE id = ?", [id.to_string()], Self::row_to_return)
            .map_err(map_db_error)?;

        {
            let mut stmt = tx
                .prepare(
                    "SELECT id, return_id, order_item_id, sku, name, quantity, condition, refund_amount, disposition, disposition_at, disposition_by
                     FROM return_items WHERE return_id = ?",
                )
                .map_err(map_db_error)?;

            ret.items = stmt
                .query_map([id.to_string()], |row| {
                    Ok(ReturnItem {
                        id: parse_uuid_row(&row.get::<_, String>("id")?, "return_item", "id")?,
                        return_id: ReturnId::from(parse_uuid_row(
                            &row.get::<_, String>("return_id")?,
                            "return_item",
                            "return_id",
                        )?),
                        order_item_id: OrderItemId::from(parse_uuid_row(
                            &row.get::<_, String>("order_item_id")?,
                            "return_item",
                            "order_item_id",
                        )?),
                        sku: row.get("sku")?,
                        name: row.get("name")?,
                        quantity: row.get("quantity")?,
                        condition: parse_enum_row(
                            &row.get::<_, String>("condition")?,
                            "return_item",
                            "condition",
                        )?,
                        refund_amount: parse_decimal_row(
                            &row.get::<_, String>("refund_amount")?,
                            "return_item",
                            "refund_amount",
                        )?,
                        disposition: parse_disposition_row(row)?,
                        disposition_at: parse_datetime_opt_row(
                            row.get::<_, Option<String>>("disposition_at")?,
                            "return_item",
                            "disposition_at",
                        )?,
                        disposition_by: row.get("disposition_by")?,
                    })
                })
                .map_err(map_db_error)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(map_db_error)?;
        }

        tx.commit().map_err(map_db_error)?;

        Ok(ret)
    }

    fn get(&self, id: ReturnId) -> Result<Option<Return>> {
        let conn = self.conn()?;
        let result = conn.query_row(
            "SELECT * FROM returns WHERE id = ?",
            [id.to_string()],
            Self::row_to_return,
        );

        match result {
            Ok(mut ret) => {
                // Inline load_return_items to use same connection
                let mut stmt = conn
                    .prepare(
                        "SELECT id, return_id, order_item_id, sku, name, quantity, condition, refund_amount, disposition, disposition_at, disposition_by
                         FROM return_items WHERE return_id = ?",
                    )
                    .map_err(map_db_error)?;

                ret.items = stmt
                    .query_map([id.to_string()], |row| {
                        Ok(ReturnItem {
                            id: parse_uuid_row(&row.get::<_, String>("id")?, "return_item", "id")?,
                            return_id: ReturnId::from(parse_uuid_row(
                                &row.get::<_, String>("return_id")?,
                                "return_item",
                                "return_id",
                            )?),
                            order_item_id: OrderItemId::from(parse_uuid_row(
                                &row.get::<_, String>("order_item_id")?,
                                "return_item",
                                "order_item_id",
                            )?),
                            sku: row.get("sku")?,
                            name: row.get("name")?,
                            quantity: row.get("quantity")?,
                            condition: parse_enum_row(
                                &row.get::<_, String>("condition")?,
                                "return_item",
                                "condition",
                            )?,
                            refund_amount: parse_decimal_row(
                                &row.get::<_, String>("refund_amount")?,
                                "return_item",
                                "refund_amount",
                            )?,
                            disposition: parse_disposition_row(row)?,
                            disposition_at: parse_datetime_opt_row(
                                row.get::<_, Option<String>>("disposition_at")?,
                                "return_item",
                                "disposition_at",
                            )?,
                            disposition_by: row.get("disposition_by")?,
                        })
                    })
                    .map_err(map_db_error)?
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .map_err(map_db_error)?;

                Ok(Some(ret))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn update(&self, id: ReturnId, input: UpdateReturn) -> Result<Return> {
        let conn = self.conn()?;
        let now = Utc::now();

        let mut updates = vec!["updated_at = ?", "version = version + 1"];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now.to_rfc3339())];

        if let Some(status) = &input.status {
            updates.push("status = ?");
            params.push(Box::new(status.to_string()));
        }
        if let Some(tracking) = &input.tracking_number {
            updates.push("tracking_number = ?");
            params.push(Box::new(tracking.clone()));
        }
        if let Some(amount) = &input.refund_amount {
            updates.push("refund_amount = ?");
            params.push(Box::new(amount.to_string()));
        }
        if let Some(method) = &input.refund_method {
            updates.push("refund_method = ?");
            params.push(Box::new(method.clone()));
        }
        if let Some(notes) = &input.notes {
            updates.push("notes = ?");
            params.push(Box::new(notes.clone()));
        }

        params.push(Box::new(id.to_string()));

        let sql = format!("UPDATE returns SET {} WHERE id = ?", updates.join(", "));
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();

        conn.execute(&sql, params_refs.as_slice()).map_err(map_db_error)?;

        // Inline the get logic to avoid connection pool deadlock
        let result = conn.query_row(
            "SELECT * FROM returns WHERE id = ?",
            [id.to_string()],
            Self::row_to_return,
        );

        let raw_id: Uuid = id.into();
        match result {
            Ok(mut ret) => {
                // Inline load_return_items to use same connection
                let mut stmt = conn
                    .prepare(
                        "SELECT id, return_id, order_item_id, sku, name, quantity, condition, refund_amount, disposition, disposition_at, disposition_by
                         FROM return_items WHERE return_id = ?",
                    )
                    .map_err(map_db_error)?;

                ret.items = stmt
                    .query_map([id.to_string()], |row| {
                        Ok(ReturnItem {
                            id: parse_uuid_row(&row.get::<_, String>("id")?, "return_item", "id")?,
                            return_id: ReturnId::from(parse_uuid_row(
                                &row.get::<_, String>("return_id")?,
                                "return_item",
                                "return_id",
                            )?),
                            order_item_id: OrderItemId::from(parse_uuid_row(
                                &row.get::<_, String>("order_item_id")?,
                                "return_item",
                                "order_item_id",
                            )?),
                            sku: row.get("sku")?,
                            name: row.get("name")?,
                            quantity: row.get("quantity")?,
                            condition: parse_enum_row(
                                &row.get::<_, String>("condition")?,
                                "return_item",
                                "condition",
                            )?,
                            refund_amount: parse_decimal_row(
                                &row.get::<_, String>("refund_amount")?,
                                "return_item",
                                "refund_amount",
                            )?,
                            disposition: parse_disposition_row(row)?,
                            disposition_at: parse_datetime_opt_row(
                                row.get::<_, Option<String>>("disposition_at")?,
                                "return_item",
                                "disposition_at",
                            )?,
                            disposition_by: row.get("disposition_by")?,
                        })
                    })
                    .map_err(map_db_error)?
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .map_err(map_db_error)?;

                Ok(ret)
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Err(CommerceError::ReturnNotFound(raw_id)),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn list(&self, filter: ReturnFilter) -> Result<Vec<Return>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM returns WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(order_id) = &filter.order_id {
            sql.push_str(" AND order_id = ?");
            params.push(Box::new(order_id.to_string()));
        }
        if let Some(customer_id) = &filter.customer_id {
            sql.push_str(" AND customer_id = ?");
            params.push(Box::new(customer_id.to_string()));
        }
        if let Some(status) = &filter.status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(status.to_string()));
        }
        if let Some(reason) = &filter.reason {
            sql.push_str(" AND reason = ?");
            params.push(Box::new(reason.to_string()));
        }
        if let Some(from) = &filter.from_date {
            sql.push_str(" AND created_at >= ?");
            params.push(Box::new(from.to_rfc3339()));
        }
        if let Some(to) = &filter.to_date {
            sql.push_str(" AND created_at <= ?");
            params.push(Box::new(to.to_rfc3339()));
        }

        // Keyset cursor: (created_at, id) for stable DESC ordering
        if let Some((cursor_date, cursor_id)) = &filter.after_cursor {
            sql.push_str(" AND (created_at < ? OR (created_at = ? AND id < ?))");
            params.push(Box::new(cursor_date.clone()));
            params.push(Box::new(cursor_date.clone()));
            params.push(Box::new(cursor_id.clone()));
        }

        sql.push_str(" ORDER BY created_at DESC, id DESC");

        // Offset pagination applies only in non-cursor mode; the helper emits
        // `LIMIT -1 OFFSET n` when an offset is set without a limit (SQLite rejects
        // a bare OFFSET).
        let offset = if filter.after_cursor.is_none() { filter.offset } else { None };
        crate::sqlite::append_limit_offset(&mut sql, filter.limit, offset);

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;

        let returns = stmt
            .query_map(params_refs.as_slice(), Self::row_to_return)
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        // Load items for each return using same connection
        let ids: Vec<ReturnId> = returns.iter().map(|r| r.id).collect();
        let mut items_by_id = Self::load_return_items_batch(&conn, &ids)?;
        let mut result = vec![];
        for mut ret in returns {
            ret.items = items_by_id.remove(&ret.id.to_string()).unwrap_or_default();
            result.push(ret);
        }

        Ok(result)
    }

    fn approve(&self, id: ReturnId) -> Result<Return> {
        let ret = self.get(id)?.ok_or(CommerceError::ReturnNotFound(id.into()))?;

        if ret.status != ReturnStatus::Requested {
            return Err(CommerceError::ReturnCannotBeApproved(ret.status.to_string()));
        }

        self.update(id, UpdateReturn { status: Some(ReturnStatus::Approved), ..Default::default() })
    }

    fn reject(&self, id: ReturnId, reason: &str) -> Result<Return> {
        let ret = self.get(id)?.ok_or(CommerceError::ReturnNotFound(id.into()))?;

        if ret.status != ReturnStatus::Requested {
            return Err(CommerceError::ReturnCannotBeApproved(ret.status.to_string()));
        }

        self.update(
            id,
            UpdateReturn {
                status: Some(ReturnStatus::Rejected),
                notes: Some(reason.to_string()),
                ..Default::default()
            },
        )
    }

    fn complete(&self, id: ReturnId) -> Result<Return> {
        let ret = self.get(id)?.ok_or(CommerceError::ReturnNotFound(id.into()))?;

        if !ret.can_complete() {
            return Err(CommerceError::NotPermitted(format!(
                "Return cannot be completed in status: {}",
                ret.status
            )));
        }

        self.update(
            id,
            UpdateReturn { status: Some(ReturnStatus::Completed), ..Default::default() },
        )
    }

    fn cancel(&self, id: ReturnId) -> Result<Return> {
        self.update(
            id,
            UpdateReturn { status: Some(ReturnStatus::Cancelled), ..Default::default() },
        )
    }

    fn set_item_disposition(
        &self,
        return_id: ReturnId,
        item_id: Uuid,
        input: SetReturnDisposition,
    ) -> Result<ReturnItem> {
        let warehouse_id = input.warehouse_id.unwrap_or(1);
        with_immediate_transaction(&self.pool, |tx| {
            let now = Utc::now().to_rfc3339();
            let status_raw: Option<String> = tx
                .query_row(
                    "SELECT status FROM returns WHERE id = ?1",
                    [return_id.to_string()],
                    |r| r.get(0),
                )
                .optional()?;
            let status: ReturnStatus = match status_raw {
                Some(raw) => parse_enum_row(&raw, "return", "status")?,
                None => return Err(smuggle(CommerceError::ReturnNotFound(return_id.into()))),
            };
            if !matches!(status, ReturnStatus::Received | ReturnStatus::Inspecting) {
                return Err(smuggle(CommerceError::NotPermitted(format!(
                    "Return items can only be dispositioned once received (status: {status})"
                ))));
            }
            let item = tx
                .query_row(
                    &format!(
                        "SELECT {RETURN_ITEM_COLUMNS} FROM return_items WHERE id = ?1 AND return_id = ?2"
                    ),
                    [item_id.to_string(), return_id.to_string()],
                    row_to_return_item,
                )
                .optional()?
                .ok_or_else(|| {
                    smuggle(CommerceError::ValidationError(format!(
                        "Return item {item_id} not found on return {return_id}"
                    )))
                })?;
            if let Some(existing) = item.disposition {
                return Err(smuggle(CommerceError::Conflict(format!(
                    "Return item {item_id} already dispositioned as {existing}"
                ))));
            }

            let qty = Decimal::from(item.quantity);
            let reference_id = item_id.to_string();
            let reason = format!("return {return_id} {}", input.disposition);
            match input.disposition {
                ReturnDisposition::Restock => {
                    let bin = find_disposition_bin_tx(
                        tx,
                        warehouse_id,
                        input.bin_id,
                        &[BinType::Returns, BinType::Quarantine],
                    )?;
                    if let Some(bin) = &bin {
                        apply_bin_delta_tx(tx, bin, &item.sku, qty, Decimal::ZERO, &now)?;
                        insert_bin_movement_tx(
                            tx,
                            BinMovementType::ReturnDisposition,
                            None,
                            Some(bin.id),
                            &item.sku,
                            qty,
                            Some(&reason),
                            Some("return_item"),
                            Some(&reference_id),
                            input.disposition_by.as_deref(),
                            &now,
                        )?;
                    }
                    apply_warehouse_delta_tx(
                        tx,
                        warehouse_id,
                        &item.sku,
                        qty,
                        Decimal::ZERO,
                        &reason,
                        Some("return_item"),
                        Some(&reference_id),
                        &now,
                    )?;
                }
                ReturnDisposition::Quarantine => {
                    if let Some(bin) = find_disposition_bin_tx(
                        tx,
                        warehouse_id,
                        input.bin_id,
                        &[BinType::Quarantine],
                    )? {
                        apply_bin_delta_tx(tx, &bin, &item.sku, qty, qty, &now)?;
                        apply_warehouse_delta_tx(
                            tx,
                            warehouse_id,
                            &item.sku,
                            qty,
                            qty,
                            &reason,
                            Some("return_item"),
                            Some(&reference_id),
                            &now,
                        )?;
                        insert_bin_movement_tx(
                            tx,
                            BinMovementType::ReturnDisposition,
                            None,
                            Some(bin.id),
                            &item.sku,
                            qty,
                            Some(&reason),
                            Some("return_item"),
                            Some(&reference_id),
                            input.disposition_by.as_deref(),
                            &now,
                        )?;
                    }
                }
                _ => {}
            }

            tx.execute(
                "UPDATE return_items SET disposition = ?1, disposition_at = ?2, disposition_by = ?3
                 WHERE id = ?4",
                rusqlite::params![
                    input.disposition.to_string(),
                    now,
                    input.disposition_by,
                    item_id.to_string()
                ],
            )?;
            tx.query_row(
                &format!("SELECT {RETURN_ITEM_COLUMNS} FROM return_items WHERE id = ?1"),
                [item_id.to_string()],
                row_to_return_item,
            )
        })
    }

    fn count(&self, filter: ReturnFilter) -> Result<u64> {
        let conn = self.conn()?;
        let mut sql = "SELECT COUNT(*) FROM returns WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(status) = &filter.status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(status.to_string()));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();
        let count: i64 =
            conn.query_row(&sql, params_refs.as_slice(), |row| row.get(0)).map_err(map_db_error)?;

        Ok(count as u64)
    }

    // === Batch Operations ===

    fn create_batch(&self, inputs: Vec<CreateReturn>) -> Result<BatchResult<Return>> {
        validate_batch_size(&inputs)?;
        let mut result = BatchResult::with_capacity(inputs.len());

        for (index, input) in inputs.into_iter().enumerate() {
            match self.create(input) {
                Ok(ret) => result.record_success(ret),
                Err(e) => result.record_failure(index, None, &e),
            }
        }

        Ok(result)
    }

    fn create_batch_atomic(&self, inputs: Vec<CreateReturn>) -> Result<Vec<Return>> {
        validate_batch_size(&inputs)?;
        if inputs.is_empty() {
            return Ok(vec![]);
        }

        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let mut results = Vec::with_capacity(inputs.len());

        for input in inputs {
            let id = Uuid::new_v4();
            let now = Utc::now();

            // Get order to get customer_id, and make sure it is returnable.
            let (customer_id, order_status): (String, String) = tx
                .query_row(
                    "SELECT customer_id, status FROM orders WHERE id = ?",
                    [input.order_id.to_string()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .map_err(|_| CommerceError::OrderNotFound(input.order_id.into()))?;
            ensure_order_returnable(&input.order_id.to_string(), &order_status)?;

            tx.execute(
                "INSERT INTO returns (id, order_id, customer_id, status, reason, reason_details, idempotency_key, notes, created_at, updated_at)
                 VALUES (?, ?, ?, 'requested', ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    id.to_string(),
                    input.order_id.to_string(),
                    customer_id,
                    input.reason.to_string(),
                    input.reason_details,
                    input.idempotency_key.clone(),
                    input.notes,
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                ],
            )
            .map_err(map_db_error)?;

            // Insert return items
            let mut items = Vec::with_capacity(input.items.len());
            for item in &input.items {
                let item_id = Uuid::new_v4();

                // Validate the item belongs to this order and the return
                // quantity does not exceed what remains returnable, then get its
                // details.
                let (sku, name, unit_price) = validate_return_item_tx(
                    &tx,
                    &input.order_id.to_string(),
                    &item.order_item_id.to_string(),
                    i64::from(item.quantity),
                )?;

                let refund_amount = parse_decimal(&unit_price, "order_item", "unit_price")?
                    * Decimal::from(item.quantity);

                tx.execute(
                    "INSERT INTO return_items (id, return_id, order_item_id, sku, name, quantity, condition, refund_amount)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                    rusqlite::params![
                        item_id.to_string(),
                        id.to_string(),
                        item.order_item_id.to_string(),
                        sku.clone(),
                        name.clone(),
                        item.quantity,
                        item.condition.unwrap_or_default().to_string(),
                        refund_amount.to_string(),
                    ],
                )
                .map_err(map_db_error)?;

                items.push(ReturnItem {
                    id: item_id,
                    return_id: ReturnId::from(id),
                    order_item_id: item.order_item_id,
                    sku,
                    name,
                    quantity: item.quantity,
                    condition: item.condition.unwrap_or_default(),
                    refund_amount,
                    disposition: None,
                    disposition_at: None,
                    disposition_by: None,
                });
            }

            // Calculate total refund amount
            let return_id_param = id.to_string();
            let return_params: [&dyn rusqlite::ToSql; 1] = [&return_id_param];
            let total_refund = sum_decimal_query(
                &tx,
                "SELECT refund_amount FROM return_items WHERE return_id = ?",
                &return_params,
                "return_item",
                "refund_amount",
            )?;

            tx.execute(
                "UPDATE returns SET refund_amount = ? WHERE id = ?",
                rusqlite::params![total_refund.to_string(), return_id_param],
            )
            .map_err(map_db_error)?;

            results.push(Return {
                id: ReturnId::from(id),
                order_id: input.order_id,
                customer_id: CustomerId::from(parse_uuid(&customer_id, "return", "customer_id")?),
                status: ReturnStatus::Requested,
                reason: input.reason,
                reason_details: input.reason_details,
                idempotency_key: input.idempotency_key,
                refund_amount: Some(total_refund),
                refund_method: None,
                tracking_number: None,
                items,
                notes: input.notes,
                version: 1,
                created_at: now,
                updated_at: now,
            });
        }

        tx.commit().map_err(map_db_error)?;
        Ok(results)
    }

    fn update_batch(&self, updates: Vec<(ReturnId, UpdateReturn)>) -> Result<BatchResult<Return>> {
        validate_batch_size(&updates)?;
        let mut result = BatchResult::with_capacity(updates.len());

        for (index, (id, input)) in updates.into_iter().enumerate() {
            match self.update(id, input) {
                Ok(ret) => result.record_success(ret),
                Err(e) => result.record_failure(index, Some(id.to_string()), &e),
            }
        }

        Ok(result)
    }

    fn update_batch_atomic(&self, updates: Vec<(ReturnId, UpdateReturn)>) -> Result<Vec<Return>> {
        validate_batch_size(&updates)?;
        if updates.is_empty() {
            return Ok(vec![]);
        }

        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let mut results = Vec::with_capacity(updates.len());

        for (id, input) in updates {
            let now = Utc::now();

            let mut update_parts = vec!["updated_at = ?"];
            let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now.to_rfc3339())];

            if let Some(status) = &input.status {
                update_parts.push("status = ?");
                params.push(Box::new(status.to_string()));
            }
            if let Some(tracking) = &input.tracking_number {
                update_parts.push("tracking_number = ?");
                params.push(Box::new(tracking.clone()));
            }
            if let Some(amount) = &input.refund_amount {
                update_parts.push("refund_amount = ?");
                params.push(Box::new(amount.to_string()));
            }
            if let Some(method) = &input.refund_method {
                update_parts.push("refund_method = ?");
                params.push(Box::new(method.clone()));
            }
            if let Some(notes) = &input.notes {
                update_parts.push("notes = ?");
                params.push(Box::new(notes.clone()));
            }

            params.push(Box::new(id.to_string()));

            let sql = format!("UPDATE returns SET {} WHERE id = ?", update_parts.join(", "));
            let params_refs: Vec<&dyn rusqlite::ToSql> =
                params.iter().map(std::convert::AsRef::as_ref).collect();

            let rows_affected = tx.execute(&sql, params_refs.as_slice()).map_err(map_db_error)?;
            if rows_affected == 0 {
                return Err(CommerceError::ReturnNotFound(id.into()));
            }

            // Fetch the updated return
            let ret = tx
                .query_row(
                    "SELECT * FROM returns WHERE id = ?",
                    [id.to_string()],
                    Self::row_to_return,
                )
                .map_err(map_db_error)?;

            results.push(ret);
        }

        tx.commit().map_err(map_db_error)?;

        // Load items for all returns in one batched query
        let conn = self.conn()?;
        let ids: Vec<ReturnId> = results.iter().map(|r| r.id).collect();
        let mut items_by_id = Self::load_return_items_batch(&conn, &ids)?;
        for ret in &mut results {
            ret.items = items_by_id.remove(&ret.id.to_string()).unwrap_or_default();
        }

        Ok(results)
    }

    fn delete_batch(&self, ids: Vec<ReturnId>) -> Result<BatchResult<Uuid>> {
        validate_batch_size(&ids)?;
        let mut result = BatchResult::with_capacity(ids.len());

        for (index, id) in ids.into_iter().enumerate() {
            let raw_id: Uuid = id.into();
            match self.delete(raw_id) {
                Ok(()) => result.record_success(raw_id),
                Err(e) => result.record_failure(index, Some(id.to_string()), &e),
            }
        }

        Ok(result)
    }

    fn delete_batch_atomic(&self, ids: Vec<ReturnId>) -> Result<()> {
        validate_batch_size(&ids)?;
        if ids.is_empty() {
            return Ok(());
        }

        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;

        let raw_ids: Vec<Uuid> = ids.iter().map(|id| (*id).into()).collect();
        let placeholders = build_in_clause(ids.len());
        let params = uuid_params(&raw_ids);
        let params_refs = params_refs(&params);

        // Delete return items first
        let sql = format!("DELETE FROM return_items WHERE return_id IN ({placeholders})");
        tx.execute(&sql, params_refs.as_slice()).map_err(map_db_error)?;

        // Delete returns
        let sql = format!("DELETE FROM returns WHERE id IN ({placeholders})");
        tx.execute(&sql, params_refs.as_slice()).map_err(map_db_error)?;

        tx.commit().map_err(map_db_error)?;
        Ok(())
    }

    fn get_batch(&self, ids: Vec<ReturnId>) -> Result<Vec<Return>> {
        validate_batch_size(&ids)?;
        if ids.is_empty() {
            return Ok(vec![]);
        }

        let conn = self.conn()?;
        let raw_ids: Vec<Uuid> = ids.iter().map(|id| (*id).into()).collect();
        let placeholders = build_in_clause(ids.len());
        let sql = format!("SELECT * FROM returns WHERE id IN ({placeholders})");

        let params = uuid_params(&raw_ids);
        let params_refs = params_refs(&params);

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let returns = stmt
            .query_map(params_refs.as_slice(), Self::row_to_return)
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        // Load items for all returns in one batched query
        let ids: Vec<ReturnId> = returns.iter().map(|r| r.id).collect();
        let mut items_by_id = Self::load_return_items_batch(&conn, &ids)?;
        let mut result = vec![];
        for mut ret in returns {
            ret.items = items_by_id.remove(&ret.id.to_string()).unwrap_or_default();
            result.push(ret);
        }

        Ok(result)
    }
}
