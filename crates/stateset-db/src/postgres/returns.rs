//! PostgreSQL returns repository implementation

use super::bins::{
    apply_bin_delta_pg, apply_warehouse_delta_pg, find_disposition_bin_pg, insert_bin_movement_pg,
};
use super::kernel_outbox::append_kernel_event_tx;
use super::{block_on, map_db_error};
use crate::KernelOutboxEvent;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;
use sqlx::{FromRow, QueryBuilder};
use stateset_core::{
    BatchResult, BinMovementType, BinType, CommerceError, CreateReturn, CustomerId, ItemCondition,
    OrderId, OrderItemId, OrderStatus, Result, Return, ReturnDisposition, ReturnFilter, ReturnId,
    ReturnItem, ReturnReason, ReturnRepository, ReturnStatus, SetReturnDisposition, UpdateReturn,
    validate_batch_size,
};
use uuid::Uuid;

/// PostgreSQL implementation of `ReturnRepository`
#[derive(Debug, Clone)]
pub struct PgReturnRepository {
    pool: PgPool,
}

/// Returns may only be requested against `Shipped` or `Delivered` orders (see
/// the SQLite backend for the rationale).
fn ensure_order_returnable(order_id: Uuid, raw_status: &str) -> Result<()> {
    let status: OrderStatus = raw_status.parse().map_err(|e| {
        CommerceError::DatabaseError(format!("Invalid order.status '{raw_status}': {e}"))
    })?;
    if matches!(
        status,
        OrderStatus::PartiallyShipped | OrderStatus::Shipped | OrderStatus::Delivered
    ) {
        return Ok(());
    }
    Err(CommerceError::ReturnOrderNotShipped { order_id, status: status.to_string() })
}

/// Validate a single return line against its order item, on the given
/// connection (typically a transaction), returning the item's
/// `(sku, name, unit_price)` for the caller to record on the return.
///
/// Rejects the return when:
/// - the order item does not exist,
/// - the order item belongs to a different order than the one being returned, or
/// - returning `return_qty` more units would exceed what was purchased, counting
///   units already claimed by non-terminal returns (rejected/cancelled returns
///   release their claim).
///
/// This guards against over-returning (and thus over-refunding) more units than
/// were ordered, and against returning another order's items.
async fn validate_return_item_pg(
    conn: &mut sqlx::PgConnection,
    order_id: Uuid,
    order_item_id: Uuid,
    return_qty: i32,
) -> Result<(String, String, Decimal)> {
    let (sku, name, unit_price, oi_order_id, ordered_qty, shipped_qty, order_status): (
        String,
        String,
        Decimal,
        Uuid,
        i32,
        i32,
        String,
    ) = sqlx::query_as(
        "SELECT oi.sku, oi.name, oi.unit_price, oi.order_id, oi.quantity, oi.shipped_quantity, o.status
         FROM order_items oi JOIN orders o ON o.id = oi.order_id
         WHERE oi.id = $1",
    )
        .bind(order_item_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(map_db_error)?
        .ok_or_else(|| {
            CommerceError::ValidationError(format!("Order item {order_item_id} not found"))
        })?;

    if oi_order_id != order_id {
        return Err(CommerceError::ValidationError(format!(
            "Order item {order_item_id} does not belong to order {order_id}"
        )));
    }

    // Units already returned for this order item, excluding rejected/cancelled
    // returns (which release their claim on the ordered quantity).
    let (already_returned,): (i64,) = sqlx::query_as(
        "SELECT COALESCE(SUM(ri.quantity), 0) FROM return_items ri
         JOIN returns r ON ri.return_id = r.id
         WHERE ri.order_item_id = $1 AND r.status NOT IN ('rejected', 'cancelled')",
    )
    .bind(order_item_id)
    .fetch_one(&mut *conn)
    .await
    .map_err(map_db_error)?;

    // Once the order has shipped (fully or partially), only units that actually
    // left the warehouse are returnable. Before shipment the cap stays at the
    // ordered quantity (legacy behaviour).
    let (cap, cap_label) = if crate::error_helpers::order_status_has_shipped(&order_status) {
        (i64::from(shipped_qty), "shipped")
    } else {
        (i64::from(ordered_qty), "ordered")
    };

    if i64::from(return_qty) + already_returned > cap {
        return Err(CommerceError::ReturnExceedsReturnable {
            order_item_id,
            basis: cap_label,
            returnable: cap,
            already_returned,
            requested: i64::from(return_qty),
        });
    }

    Ok((sku, name, unit_price))
}

#[derive(FromRow)]
pub(crate) struct ReturnRow {
    id: Uuid,
    order_id: Uuid,
    customer_id: Uuid,
    status: String,
    reason: String,
    reason_details: Option<String>,
    idempotency_key: Option<String>,
    refund_amount: Option<Decimal>,
    refund_method: Option<String>,
    tracking_number: Option<String>,
    notes: Option<String>,
    version: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
pub(crate) struct ReturnItemRow {
    id: Uuid,
    return_id: Uuid,
    order_item_id: Uuid,
    sku: String,
    name: String,
    quantity: i32,
    condition: String,
    refund_amount: Decimal,
    disposition: Option<String>,
    disposition_at: Option<DateTime<Utc>>,
    disposition_by: Option<String>,
}

impl PgReturnRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub(crate) fn row_to_return(row: ReturnRow, items: Vec<ReturnItem>) -> Result<Return> {
        let ReturnRow {
            id,
            order_id,
            customer_id,
            status,
            reason,
            reason_details,
            idempotency_key,
            refund_amount,
            refund_method,
            tracking_number,
            notes,
            version,
            created_at,
            updated_at,
        } = row;

        let status: ReturnStatus = status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid return.status '{}': {}", status, e))
        })?;
        let reason: ReturnReason = reason.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid return.reason '{}': {}", reason, e))
        })?;

        Ok(Return {
            id: ReturnId::from(id),
            order_id: OrderId::from(order_id),
            customer_id: CustomerId::from(customer_id),
            status,
            reason,
            reason_details,
            idempotency_key,
            refund_amount,
            refund_method,
            tracking_number,
            items,
            notes,
            version,
            created_at,
            updated_at,
        })
    }

    pub(crate) fn row_to_item(row: ReturnItemRow) -> Result<ReturnItem> {
        let ReturnItemRow {
            id,
            return_id,
            order_item_id,
            sku,
            name,
            quantity,
            condition,
            refund_amount,
            disposition,
            disposition_at,
            disposition_by,
        } = row;

        let condition: ItemCondition = condition.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid return_item.condition '{}': {}",
                condition, e
            ))
        })?;
        let disposition: Option<ReturnDisposition> = disposition
            .filter(|d| !d.is_empty())
            .map(|d| {
                d.parse().map_err(|e| {
                    CommerceError::DatabaseError(format!(
                        "Invalid return_item.disposition '{d}': {e}"
                    ))
                })
            })
            .transpose()?;

        Ok(ReturnItem {
            id,
            return_id: ReturnId::from(return_id),
            order_item_id: OrderItemId::from(order_item_id),
            sku,
            name,
            quantity,
            condition,
            refund_amount,
            disposition,
            disposition_at,
            disposition_by,
        })
    }

    /// Record a return item's disposition and apply its stock effect (async).
    pub async fn set_item_disposition_async(
        &self,
        return_id: ReturnId,
        item_id: Uuid,
        input: SetReturnDisposition,
    ) -> Result<ReturnItem> {
        let warehouse_id = input.warehouse_id.unwrap_or(1);
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let now = Utc::now();
        let status_raw: Option<String> =
            sqlx::query_scalar("SELECT status FROM returns WHERE id = $1 FOR UPDATE")
                .bind(return_id.into_uuid())
                .fetch_optional(tx.as_mut())
                .await
                .map_err(map_db_error)?;
        let status: ReturnStatus = match status_raw {
            Some(raw) => raw.parse().map_err(|e| {
                CommerceError::DatabaseError(format!("Invalid return.status '{raw}': {e}"))
            })?,
            None => return Err(CommerceError::ReturnNotFound(return_id.into())),
        };
        if !matches!(status, ReturnStatus::Received | ReturnStatus::Inspecting) {
            return Err(CommerceError::NotPermitted(format!(
                "Return items can only be dispositioned once received (status: {status})"
            )));
        }
        let item = sqlx::query_as::<_, ReturnItemRow>(
            "SELECT * FROM return_items WHERE id = $1 AND return_id = $2 FOR UPDATE",
        )
        .bind(item_id)
        .bind(return_id.into_uuid())
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .ok_or_else(|| {
            CommerceError::ValidationError(format!(
                "Return item {item_id} not found on return {return_id}"
            ))
        })?;
        let item = Self::row_to_item(item)?;
        if let Some(existing) = item.disposition {
            return Err(CommerceError::Conflict(format!(
                "Return item {item_id} already dispositioned as {existing}"
            )));
        }

        let qty = Decimal::from(item.quantity);
        let reference_id = item_id.to_string();
        let reason = format!("return {return_id} {}", input.disposition);
        match input.disposition {
            ReturnDisposition::Restock => {
                let bin = find_disposition_bin_pg(
                    tx.as_mut(),
                    warehouse_id,
                    input.bin_id,
                    &[BinType::Returns, BinType::Quarantine],
                )
                .await?;
                if let Some(bin) = &bin {
                    apply_bin_delta_pg(tx.as_mut(), bin, &item.sku, qty, Decimal::ZERO, now)
                        .await?;
                    insert_bin_movement_pg(
                        tx.as_mut(),
                        BinMovementType::ReturnDisposition,
                        None,
                        Some(bin.id),
                        &item.sku,
                        qty,
                        Some(&reason),
                        Some("return_item"),
                        Some(&reference_id),
                        input.disposition_by.as_deref(),
                        now,
                    )
                    .await?;
                }
                apply_warehouse_delta_pg(
                    tx.as_mut(),
                    warehouse_id,
                    &item.sku,
                    qty,
                    Decimal::ZERO,
                    &reason,
                    Some("return_item"),
                    Some(&reference_id),
                    now,
                )
                .await?;
            }
            ReturnDisposition::Quarantine => {
                if let Some(bin) = find_disposition_bin_pg(
                    tx.as_mut(),
                    warehouse_id,
                    input.bin_id,
                    &[BinType::Quarantine],
                )
                .await?
                {
                    apply_bin_delta_pg(tx.as_mut(), &bin, &item.sku, qty, qty, now).await?;
                    apply_warehouse_delta_pg(
                        tx.as_mut(),
                        warehouse_id,
                        &item.sku,
                        qty,
                        qty,
                        &reason,
                        Some("return_item"),
                        Some(&reference_id),
                        now,
                    )
                    .await?;
                    insert_bin_movement_pg(
                        tx.as_mut(),
                        BinMovementType::ReturnDisposition,
                        None,
                        Some(bin.id),
                        &item.sku,
                        qty,
                        Some(&reason),
                        Some("return_item"),
                        Some(&reference_id),
                        input.disposition_by.as_deref(),
                        now,
                    )
                    .await?;
                }
            }
            _ => {}
        }

        let updated = sqlx::query_as::<_, ReturnItemRow>(
            "UPDATE return_items SET disposition = $1, disposition_at = $2, disposition_by = $3
             WHERE id = $4 RETURNING *",
        )
        .bind(input.disposition.to_string())
        .bind(now)
        .bind(input.disposition_by)
        .bind(item_id)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        tx.commit().await.map_err(map_db_error)?;
        Self::row_to_item(updated)
    }

    /// Create a return (async)
    pub async fn create_async(&self, input: CreateReturn) -> Result<Return> {
        if let Some(key) = input.idempotency_key.as_deref() {
            if let Some(existing) = self.get_by_idempotency_key_async(key).await? {
                return Ok(existing);
            }
        }

        let id = Uuid::new_v4();
        let now = Utc::now();

        // Run header + item inserts in a single transaction so a rejected item
        // (e.g. an over-return) rolls back the whole return rather than leaving
        // a partially-created return behind.
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        // Get customer_id from order, and make sure it is returnable.
        let order_info: (Uuid, String) =
            sqlx::query_as("SELECT customer_id, status FROM orders WHERE id = $1")
                .bind(input.order_id.into_uuid())
                .fetch_one(tx.as_mut())
                .await
                .map_err(|_| CommerceError::OrderNotFound(input.order_id.into_uuid()))?;
        ensure_order_returnable(input.order_id.into_uuid(), &order_info.1)?;

        let customer_id = order_info.0;

        sqlx::query(
            r#"
            INSERT INTO returns (id, order_id, customer_id, status, reason, reason_details, idempotency_key, notes, created_at, updated_at)
            VALUES ($1, $2, $3, 'requested', $4, $5, $6, $7, $8, $9)
            "#,
        )
        .bind(id)
        .bind(input.order_id.into_uuid())
        .bind(customer_id)
        .bind(input.reason.to_string())
        .bind(&input.reason_details)
        .bind(&input.idempotency_key)
        .bind(&input.notes)
        .bind(now)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        // Create return items
        let mut items = Vec::with_capacity(input.items.len());
        for item_input in &input.items {
            let item_id = Uuid::new_v4();

            // Validate the item belongs to this order and the return quantity
            // does not exceed what remains returnable, then get its details.
            let (sku, name, unit_price) = validate_return_item_pg(
                tx.as_mut(),
                input.order_id.into_uuid(),
                item_input.order_item_id.into_uuid(),
                item_input.quantity,
            )
            .await?;

            let refund = unit_price * Decimal::from(item_input.quantity);
            let condition = item_input.condition.unwrap_or(ItemCondition::New);

            sqlx::query(
                r#"
                INSERT INTO return_items (id, return_id, order_item_id, sku, name, quantity, condition, refund_amount)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                "#,
            )
            .bind(item_id)
            .bind(id)
            .bind(item_input.order_item_id.into_uuid())
            .bind(&sku)
            .bind(&name)
            .bind(item_input.quantity)
            .bind(condition.to_string())
            .bind(refund)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

            items.push(ReturnItem {
                id: item_id,
                return_id: ReturnId::from(id),
                order_item_id: item_input.order_item_id,
                sku,
                name,
                quantity: item_input.quantity,
                condition,
                refund_amount: refund,
                disposition: None,
                disposition_at: None,
                disposition_by: None,
            });
        }

        let refund_amount: Decimal = items.iter().map(|item| item.refund_amount).sum();
        sqlx::query("UPDATE returns SET refund_amount = $1 WHERE id = $2")
            .bind(refund_amount)
            .bind(id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        append_kernel_event_tx(
            tx.as_mut(),
            &KernelOutboxEvent::domain(
                "returns.created.v1",
                "return",
                id.to_string(),
                serde_json::json!({
                    "return_id": id.to_string(),
                    "order_id": input.order_id.to_string(),
                    "status": ReturnStatus::Requested.to_string(),
                    "refund_amount": refund_amount.to_string(),
                    "item_count": items.len(),
                }),
                input.idempotency_key.clone(),
            ),
        )
        .await?;

        tx.commit().await.map_err(map_db_error)?;

        Ok(Return {
            id: ReturnId::from(id),
            order_id: input.order_id,
            customer_id: CustomerId::from(customer_id),
            status: ReturnStatus::Requested,
            reason: input.reason,
            reason_details: input.reason_details,
            idempotency_key: input.idempotency_key,
            refund_amount: Some(refund_amount),
            refund_method: None,
            tracking_number: None,
            items,
            notes: input.notes,
            version: 1,
            created_at: now,
            updated_at: now,
        })
    }

    async fn get_by_idempotency_key_async(&self, key: &str) -> Result<Option<Return>> {
        let row =
            sqlx::query_as::<_, ReturnRow>("SELECT * FROM returns WHERE idempotency_key = $1")
                .bind(key)
                .fetch_optional(&self.pool)
                .await
                .map_err(map_db_error)?;

        let Some(row) = row else {
            return Ok(None);
        };

        let items = self.get_items_async(row.id).await?;
        Ok(Some(Self::row_to_return(row, items)?))
    }

    /// Get a return by ID (async)
    pub async fn get_async(&self, id: Uuid) -> Result<Option<Return>> {
        let row = sqlx::query_as::<_, ReturnRow>("SELECT * FROM returns WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        match row {
            Some(return_row) => {
                let items = self.get_items_async(id).await?;
                Ok(Some(Self::row_to_return(return_row, items)?))
            }
            None => Ok(None),
        }
    }

    /// Get return items (async)
    pub async fn get_items_async(&self, return_id: Uuid) -> Result<Vec<ReturnItem>> {
        let rows =
            sqlx::query_as::<_, ReturnItemRow>("SELECT * FROM return_items WHERE return_id = $1")
                .bind(return_id)
                .fetch_all(&self.pool)
                .await
                .map_err(map_db_error)?;

        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            items.push(Self::row_to_item(row)?);
        }
        Ok(items)
    }

    async fn get_items_batch_async(
        &self,
        ids: &[Uuid],
    ) -> Result<std::collections::HashMap<Uuid, Vec<ReturnItem>>> {
        let mut map: std::collections::HashMap<Uuid, Vec<ReturnItem>> =
            std::collections::HashMap::with_capacity(ids.len());
        if ids.is_empty() {
            return Ok(map);
        }
        let rows = sqlx::query_as::<_, ReturnItemRow>(
            "SELECT * FROM return_items WHERE return_id = ANY($1)",
        )
        .bind(ids.to_vec())
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;
        for row in rows {
            let parent = row.return_id;
            map.entry(parent).or_default().push(Self::row_to_item(row)?);
        }
        Ok(map)
    }

    /// Update a return (async)
    pub async fn update_async(&self, id: Uuid, input: UpdateReturn) -> Result<Return> {
        let now = Utc::now();
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let existing =
            sqlx::query_as::<_, ReturnRow>("SELECT * FROM returns WHERE id = $1 FOR UPDATE")
                .bind(id)
                .fetch_optional(tx.as_mut())
                .await
                .map_err(map_db_error)?
                .ok_or(CommerceError::ReturnNotFound(id))?;
        let status_before: ReturnStatus = existing.status.parse().map_err(|error| {
            CommerceError::DatabaseError(format!(
                "Invalid return.status '{}': {error}",
                existing.status
            ))
        })?;
        let status_after = input.status.unwrap_or(status_before);
        if !status_before.can_transition_to(status_after) {
            return Err(CommerceError::ValidationError(format!(
                "Invalid return status transition from {status_before} to {status_after}"
            )));
        }
        let version_before = existing.version;
        let tracking = input.tracking_number.or(existing.tracking_number);
        let refund_amount = input.refund_amount.or(existing.refund_amount);
        let refund_method = input.refund_method.or(existing.refund_method);
        let notes = input.notes.or(existing.notes);

        let updated = sqlx::query_as::<_, ReturnRow>(
            r#"
            UPDATE returns
            SET status = $1, tracking_number = $2, refund_amount = $3,
                refund_method = $4, notes = $5, updated_at = $6,
                version = version + 1
            WHERE id = $7 AND version = $8
            RETURNING *
            "#,
        )
        .bind(status_after.to_string())
        .bind(&tracking)
        .bind(refund_amount)
        .bind(&refund_method)
        .bind(&notes)
        .bind(now)
        .bind(id)
        .bind(version_before)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .ok_or_else(|| CommerceError::VersionConflict {
            entity: "return".into(),
            id: id.to_string(),
            expected_version: version_before,
        })?;

        append_kernel_event_tx(
            tx.as_mut(),
            &KernelOutboxEvent::domain(
                "returns.updated.v1",
                "return",
                id.to_string(),
                serde_json::json!({
                    "return_id": id.to_string(),
                    "status_before": status_before.to_string(),
                    "status_after": status_after.to_string(),
                    "version_before": version_before,
                    "version_after": updated.version,
                    "refund_amount": refund_amount.map(|amount| amount.to_string()),
                }),
                None,
            ),
        )
        .await?;
        tx.commit().await.map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::ReturnNotFound(id))
    }

    /// List returns (async)
    pub async fn list_async(&self, filter: ReturnFilter) -> Result<Vec<Return>> {
        let ReturnFilter {
            order_id,
            customer_id,
            status,
            reason,
            from_date,
            to_date,
            limit,
            offset,
            after_cursor: _,
        } = filter;

        let mut builder = QueryBuilder::new("SELECT * FROM returns WHERE 1=1");

        if let Some(order_id) = order_id {
            builder.push(" AND order_id = ").push_bind(order_id.into_uuid());
        }
        if let Some(customer_id) = customer_id {
            builder.push(" AND customer_id = ").push_bind(customer_id.into_uuid());
        }
        if let Some(status) = status {
            builder.push(" AND status = ").push_bind(status.to_string());
        }
        if let Some(reason) = reason {
            builder.push(" AND reason = ").push_bind(reason.to_string());
        }
        if let Some(from) = from_date {
            builder.push(" AND created_at >= ").push_bind(from);
        }
        if let Some(to) = to_date {
            builder.push(" AND created_at <= ").push_bind(to);
        }

        builder.push(" ORDER BY created_at DESC");

        builder.push(" LIMIT ").push_bind(super::effective_limit(limit));
        if let Some(offset) = offset {
            builder.push(" OFFSET ").push_bind(offset as i64);
        }

        let rows = builder
            .build_query_as::<ReturnRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        let ids: Vec<Uuid> = rows.iter().map(|r| r.id).collect();
        let mut items_by_id = self.get_items_batch_async(&ids).await?;
        let mut returns = Vec::new();
        for row in rows {
            let items = items_by_id.remove(&row.id).unwrap_or_default();
            returns.push(Self::row_to_return(row, items)?);
        }

        Ok(returns)
    }

    /// Approve a return (async)
    pub async fn approve_async(&self, id: Uuid) -> Result<Return> {
        self.update_async(
            id,
            UpdateReturn { status: Some(ReturnStatus::Approved), ..Default::default() },
        )
        .await
    }

    /// Reject a return (async)
    pub async fn reject_async(&self, id: Uuid, reason: &str) -> Result<Return> {
        self.update_async(
            id,
            UpdateReturn {
                status: Some(ReturnStatus::Rejected),
                notes: Some(reason.into()),
                ..Default::default()
            },
        )
        .await
    }

    /// Complete a return (async)
    pub async fn complete_async(&self, id: Uuid) -> Result<Return> {
        self.update_async(
            id,
            UpdateReturn { status: Some(ReturnStatus::Completed), ..Default::default() },
        )
        .await
    }

    /// Cancel a return (async)
    pub async fn cancel_async(&self, id: Uuid) -> Result<Return> {
        self.update_async(
            id,
            UpdateReturn { status: Some(ReturnStatus::Cancelled), ..Default::default() },
        )
        .await
    }

    /// Count returns (async)
    pub async fn count_async(&self, filter: ReturnFilter) -> Result<u64> {
        let ReturnFilter {
            order_id,
            customer_id,
            status,
            reason,
            from_date,
            to_date,
            limit: _,
            offset: _,
            after_cursor: _,
        } = filter;

        let mut builder = QueryBuilder::new("SELECT COUNT(*) FROM returns WHERE 1=1");

        if let Some(order_id) = order_id {
            builder.push(" AND order_id = ").push_bind(order_id.into_uuid());
        }
        if let Some(customer_id) = customer_id {
            builder.push(" AND customer_id = ").push_bind(customer_id.into_uuid());
        }
        if let Some(status) = status {
            builder.push(" AND status = ").push_bind(status.to_string());
        }
        if let Some(reason) = reason {
            builder.push(" AND reason = ").push_bind(reason.to_string());
        }
        if let Some(from) = from_date {
            builder.push(" AND created_at >= ").push_bind(from);
        }
        if let Some(to) = to_date {
            builder.push(" AND created_at <= ").push_bind(to);
        }

        let count: (i64,) =
            builder.build_query_as().fetch_one(&self.pool).await.map_err(map_db_error)?;

        Ok(count.0 as u64)
    }

    /// Delete a return (async)
    pub async fn delete_async(&self, id: Uuid) -> Result<()> {
        // First delete return items
        sqlx::query("DELETE FROM return_items WHERE return_id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        // Then delete the return
        sqlx::query("DELETE FROM returns WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(())
    }

    // =========================================================================
    // Batch Operations (async)
    // =========================================================================

    /// Create multiple returns - partial success allowed (async)
    pub async fn create_batch_async(
        &self,
        inputs: Vec<CreateReturn>,
    ) -> Result<BatchResult<Return>> {
        validate_batch_size(&inputs)?;

        let mut result = BatchResult::with_capacity(inputs.len());

        for (index, input) in inputs.into_iter().enumerate() {
            match self.create_async(input).await {
                Ok(ret) => result.record_success(ret),
                Err(e) => result.record_failure(index, None, &e),
            }
        }

        Ok(result)
    }

    /// Create multiple returns - atomic (all-or-nothing) (async)
    pub async fn create_batch_atomic_async(
        &self,
        inputs: Vec<CreateReturn>,
    ) -> Result<Vec<Return>> {
        validate_batch_size(&inputs)?;

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let mut returns = Vec::with_capacity(inputs.len());

        for input in inputs {
            let id = Uuid::new_v4();
            let now = Utc::now();

            // Get customer_id from order, and make sure it is returnable.
            let order_info: (Uuid, String) =
                sqlx::query_as("SELECT customer_id, status FROM orders WHERE id = $1")
                    .bind(input.order_id.into_uuid())
                    .fetch_one(tx.as_mut())
                    .await
                    .map_err(|_| CommerceError::OrderNotFound(input.order_id.into_uuid()))?;
            ensure_order_returnable(input.order_id.into_uuid(), &order_info.1)?;

            let customer_id = order_info.0;

            sqlx::query(
                r#"
                INSERT INTO returns (id, order_id, customer_id, status, reason, reason_details, idempotency_key, notes, created_at, updated_at)
                VALUES ($1, $2, $3, 'requested', $4, $5, $6, $7, $8, $9)
                "#,
            )
            .bind(id)
            .bind(input.order_id.into_uuid())
            .bind(customer_id)
            .bind(input.reason.to_string())
            .bind(&input.reason_details)
            .bind(&input.idempotency_key)
            .bind(&input.notes)
            .bind(now)
            .bind(now)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

            // Create return items within transaction
            let mut items = Vec::new();
            for item_input in input.items.clone() {
                let item_id = Uuid::new_v4();

                // Validate the item belongs to this order and the return
                // quantity does not exceed what remains returnable, then get its
                // details.
                let (sku, name, unit_price) = validate_return_item_pg(
                    tx.as_mut(),
                    input.order_id.into_uuid(),
                    item_input.order_item_id.into_uuid(),
                    item_input.quantity,
                )
                .await?;

                let refund = unit_price * Decimal::from(item_input.quantity);
                let condition = item_input.condition.unwrap_or(ItemCondition::New);

                sqlx::query(
                    r#"
                    INSERT INTO return_items (id, return_id, order_item_id, sku, name, quantity, condition, refund_amount)
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                    "#,
                )
                .bind(item_id)
                .bind(id)
                .bind(item_input.order_item_id.into_uuid())
                .bind(&sku)
                .bind(&name)
                .bind(item_input.quantity)
                .bind(condition.to_string())
                .bind(refund)
                .execute(tx.as_mut())
                .await
                .map_err(map_db_error)?;

                items.push(ReturnItem {
                    id: item_id,
                    return_id: ReturnId::from(id),
                    order_item_id: item_input.order_item_id,
                    sku,
                    name,
                    quantity: item_input.quantity,
                    condition,
                    refund_amount: refund,
                    disposition: None,
                    disposition_at: None,
                    disposition_by: None,
                });
            }

            returns.push(Return {
                id: ReturnId::from(id),
                order_id: input.order_id,
                customer_id: CustomerId::from(customer_id),
                status: ReturnStatus::Requested,
                reason: input.reason,
                reason_details: input.reason_details,
                idempotency_key: input.idempotency_key,
                refund_amount: None,
                refund_method: None,
                tracking_number: None,
                items,
                notes: input.notes,
                version: 1,
                created_at: now,
                updated_at: now,
            });
        }

        tx.commit().await.map_err(map_db_error)?;
        Ok(returns)
    }

    /// Update multiple returns - partial success allowed (async)
    pub async fn update_batch_async(
        &self,
        updates: Vec<(ReturnId, UpdateReturn)>,
    ) -> Result<BatchResult<Return>> {
        validate_batch_size(&updates)?;

        let mut result = BatchResult::with_capacity(updates.len());

        for (index, (id, input)) in updates.into_iter().enumerate() {
            let raw_id = id.into_uuid();
            match self.update_async(raw_id, input).await {
                Ok(ret) => result.record_success(ret),
                Err(e) => result.record_failure(index, Some(raw_id.to_string()), &e),
            }
        }

        Ok(result)
    }

    /// Update multiple returns - atomic (all-or-nothing) (async)
    pub async fn update_batch_atomic_async(
        &self,
        updates: Vec<(ReturnId, UpdateReturn)>,
    ) -> Result<Vec<Return>> {
        validate_batch_size(&updates)?;

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let mut returns = Vec::with_capacity(updates.len());

        for (id, input) in updates {
            let raw_id = id.into_uuid();
            let now = Utc::now();

            let existing_row =
                sqlx::query_as::<_, ReturnRow>("SELECT * FROM returns WHERE id = $1")
                    .bind(raw_id)
                    .fetch_optional(tx.as_mut())
                    .await
                    .map_err(map_db_error)?
                    .ok_or(CommerceError::ReturnNotFound(raw_id))?;

            let items = sqlx::query_as::<_, ReturnItemRow>(
                "SELECT * FROM return_items WHERE return_id = $1",
            )
            .bind(raw_id)
            .fetch_all(tx.as_mut())
            .await
            .map_err(map_db_error)?;

            let mut existing_items = Vec::with_capacity(items.len());
            for item in items {
                existing_items.push(Self::row_to_item(item)?);
            }
            let existing = Self::row_to_return(existing_row, existing_items.clone())?;

            let new_status = input.status.unwrap_or(existing.status);
            let new_tracking = input.tracking_number.or(existing.tracking_number);
            let new_refund_amount = input.refund_amount.or(existing.refund_amount);
            let new_refund_method = input.refund_method.or(existing.refund_method);
            let new_notes = input.notes.or(existing.notes);

            sqlx::query(
                r#"
                UPDATE returns
                SET status = $1, tracking_number = $2, refund_amount = $3,
                    refund_method = $4, notes = $5, updated_at = $6,
                    version = version + 1
                WHERE id = $7
                "#,
            )
            .bind(new_status.to_string())
            .bind(&new_tracking)
            .bind(new_refund_amount)
            .bind(&new_refund_method)
            .bind(&new_notes)
            .bind(now)
            .bind(raw_id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

            // Fetch the updated return
            let updated_row = sqlx::query_as::<_, ReturnRow>("SELECT * FROM returns WHERE id = $1")
                .bind(raw_id)
                .fetch_one(tx.as_mut())
                .await
                .map_err(map_db_error)?;

            returns.push(Self::row_to_return(updated_row, existing_items)?);
        }

        tx.commit().await.map_err(map_db_error)?;
        Ok(returns)
    }

    /// Delete multiple returns - partial success allowed (async)
    pub async fn delete_batch_async(&self, ids: Vec<ReturnId>) -> Result<BatchResult<Uuid>> {
        validate_batch_size(&ids)?;

        let mut result = BatchResult::with_capacity(ids.len());

        for (index, id) in ids.into_iter().enumerate() {
            let raw_id = id.into_uuid();
            match self.delete_async(raw_id).await {
                Ok(()) => result.record_success(raw_id),
                Err(e) => result.record_failure(index, Some(raw_id.to_string()), &e),
            }
        }

        Ok(result)
    }

    /// Delete multiple returns - atomic (all-or-nothing) (async)
    pub async fn delete_batch_atomic_async(&self, ids: Vec<ReturnId>) -> Result<()> {
        validate_batch_size(&ids)?;

        if ids.is_empty() {
            return Ok(());
        }

        let raw_ids: Vec<Uuid> = ids.into_iter().map(|id| id.into_uuid()).collect();

        // First delete return items for all returns
        sqlx::query("DELETE FROM return_items WHERE return_id = ANY($1)")
            .bind(&raw_ids)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        // Then delete the returns
        sqlx::query("DELETE FROM returns WHERE id = ANY($1)")
            .bind(&raw_ids)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(())
    }

    /// Get multiple returns by ID (async)
    pub async fn get_batch_async(&self, ids: Vec<ReturnId>) -> Result<Vec<Return>> {
        validate_batch_size(&ids)?;

        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let raw_ids: Vec<Uuid> = ids.into_iter().map(|id| id.into_uuid()).collect();

        let rows = sqlx::query_as::<_, ReturnRow>("SELECT * FROM returns WHERE id = ANY($1)")
            .bind(&raw_ids)
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        let ids: Vec<Uuid> = rows.iter().map(|r| r.id).collect();
        let mut items_by_id = self.get_items_batch_async(&ids).await?;
        let mut returns = Vec::new();
        for row in rows {
            let items = items_by_id.remove(&row.id).unwrap_or_default();
            returns.push(Self::row_to_return(row, items)?);
        }

        Ok(returns)
    }
}

impl ReturnRepository for PgReturnRepository {
    fn create(&self, input: CreateReturn) -> Result<Return> {
        super::block_on(self.create_async(input))
    }

    fn get(&self, id: ReturnId) -> Result<Option<Return>> {
        super::block_on(self.get_async(id.into_uuid()))
    }

    fn update(&self, id: ReturnId, input: UpdateReturn) -> Result<Return> {
        super::block_on(self.update_async(id.into_uuid(), input))
    }

    fn list(&self, filter: ReturnFilter) -> Result<Vec<Return>> {
        super::block_on(self.list_async(filter))
    }

    fn approve(&self, id: ReturnId) -> Result<Return> {
        super::block_on(self.approve_async(id.into_uuid()))
    }

    fn reject(&self, id: ReturnId, reason: &str) -> Result<Return> {
        super::block_on(self.reject_async(id.into_uuid(), reason))
    }

    fn complete(&self, id: ReturnId) -> Result<Return> {
        super::block_on(self.complete_async(id.into_uuid()))
    }

    fn cancel(&self, id: ReturnId) -> Result<Return> {
        super::block_on(self.cancel_async(id.into_uuid()))
    }

    fn set_item_disposition(
        &self,
        return_id: ReturnId,
        item_id: Uuid,
        input: SetReturnDisposition,
    ) -> Result<ReturnItem> {
        block_on(self.set_item_disposition_async(return_id, item_id, input))
    }

    fn count(&self, filter: ReturnFilter) -> Result<u64> {
        super::block_on(self.count_async(filter))
    }

    // =========================================================================
    // Batch Operations
    // =========================================================================

    fn create_batch(&self, inputs: Vec<CreateReturn>) -> Result<BatchResult<Return>> {
        super::block_on(self.create_batch_async(inputs))
    }

    fn create_batch_atomic(&self, inputs: Vec<CreateReturn>) -> Result<Vec<Return>> {
        super::block_on(self.create_batch_atomic_async(inputs))
    }

    fn update_batch(&self, updates: Vec<(ReturnId, UpdateReturn)>) -> Result<BatchResult<Return>> {
        super::block_on(self.update_batch_async(updates))
    }

    fn update_batch_atomic(&self, updates: Vec<(ReturnId, UpdateReturn)>) -> Result<Vec<Return>> {
        super::block_on(self.update_batch_atomic_async(updates))
    }

    fn delete_batch(&self, ids: Vec<ReturnId>) -> Result<BatchResult<Uuid>> {
        super::block_on(self.delete_batch_async(ids))
    }

    fn delete_batch_atomic(&self, ids: Vec<ReturnId>) -> Result<()> {
        super::block_on(self.delete_batch_atomic_async(ids))
    }

    fn get_batch(&self, ids: Vec<ReturnId>) -> Result<Vec<Return>> {
        super::block_on(self.get_batch_async(ids))
    }
}
