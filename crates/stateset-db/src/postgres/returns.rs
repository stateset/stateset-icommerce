//! PostgreSQL returns repository implementation.
//!
//! Mirrors the SQLite backend's invariants exactly (see
//! `crate::sqlite::returns` for the list): row locks (`FOR UPDATE`) on the
//! order, its order items and the return replace SQLite's `IMMEDIATE` write
//! lock, and the `idx_returns_idempotency_key` unique index (migration 030)
//! is the backstop for concurrent creates with the same idempotency key.

use super::bins::{
    apply_bin_delta_pg, apply_warehouse_delta_pg, find_disposition_bin_pg, insert_bin_movement_pg,
};
use super::kernel_outbox::append_kernel_event_tx;
use super::payments::{create_refund_pg_tx, open_captures_for_order_pg, refundable_remaining_pg};
use super::{block_on, map_db_error};
use crate::KernelOutboxEvent;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;
use sqlx::{FromRow, PgConnection, QueryBuilder};
use stateset_core::{
    BatchResult, BinMovementType, BinType, CommerceError, CreateReturn, CustomerId, ItemCondition,
    LotTransactionType, OrderId, OrderItemId, OrderStatus, Result, Return, ReturnDisposition,
    ReturnFilter, ReturnId, ReturnItem, ReturnReason, ReturnRepository, ReturnStatus,
    SerialEventType, SerialStatus, SetReturnDisposition, UpdateReturn, validate_batch_size,
};
use uuid::Uuid;

/// Decimal places a line refund is rounded to (matches `order_items.total`).
const MONEY_SCALE: u32 = 2;

/// Unique index that enforces one return per idempotency key (migration 030).
const IDEMPOTENCY_KEY_INDEX: &str = "idx_returns_idempotency_key";

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

/// What a validated return line copies from its order item.
struct ReturnableLine {
    sku: String,
    name: String,
    unit_price: Decimal,
    /// `order_items.total` — unit price × quantity less the line discount plus
    /// line tax, as charged.
    line_total: Decimal,
    ordered_qty: i32,
}

impl ReturnableLine {
    /// The refund for returning `return_qty` units of this line: the
    /// proportional share of what was actually charged for the line, so a
    /// discounted line never refunds more than it collected. Falls back to
    /// `unit_price × qty` for legacy rows with no usable total.
    fn refund_for(&self, return_qty: i32) -> Decimal {
        let qty = Decimal::from(return_qty);
        if self.ordered_qty > 0 && self.line_total >= Decimal::ZERO {
            (self.line_total * qty / Decimal::from(self.ordered_qty)).round_dp(MONEY_SCALE)
        } else {
            (self.unit_price * qty).round_dp(MONEY_SCALE)
        }
    }
}

/// Validate a single return line against its order item, on the given
/// transaction, returning what the return records from the line.
///
/// Rejects the return when:
/// - the order item does not exist,
/// - the order item belongs to a different order than the one being returned, or
/// - returning `return_qty` more units would exceed what was purchased (or, once
///   the order has shipped, what actually shipped), counting units already
///   claimed by non-terminal returns (rejected/cancelled returns release their
///   claim — which is why a return can no longer be rejected or cancelled once
///   its units were restocked, see [`Return::check_transition`]).
///
/// The order item row is locked (`FOR UPDATE OF oi`) for the rest of the
/// transaction, so two concurrent returns of the same line are serialized:
/// the second waits, then sees the first's committed claim in the SUM.
async fn validate_return_item_pg(
    conn: &mut PgConnection,
    order_id: Uuid,
    order_item_id: Uuid,
    return_qty: i32,
) -> Result<ReturnableLine> {
    let (sku, name, unit_price, line_total, oi_order_id, ordered_qty, shipped_qty, order_status): (
        String,
        String,
        Decimal,
        Decimal,
        Uuid,
        i32,
        i32,
        String,
    ) = sqlx::query_as(
        "SELECT oi.sku, oi.name, oi.unit_price, oi.total, oi.order_id, oi.quantity, oi.shipped_quantity, o.status
         FROM order_items oi JOIN orders o ON o.id = oi.order_id
         WHERE oi.id = $1
         FOR UPDATE OF oi",
    )
    .bind(order_item_id)
    .fetch_optional(&mut *conn)
    .await
    .map_err(map_db_error)?
    .ok_or_else(|| CommerceError::ValidationError(format!("Order item {order_item_id} not found")))?;

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

    Ok(ReturnableLine { sku, name, unit_price, line_total, ordered_qty })
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
    lot_id: Option<Uuid>,
    serial_ids: Option<serde_json::Value>,
}

/// Load the items of one return on the caller's connection.
async fn load_items_pg(conn: &mut PgConnection, return_id: Uuid) -> Result<Vec<ReturnItem>> {
    let rows = sqlx::query_as::<_, ReturnItemRow>(
        "SELECT * FROM return_items WHERE return_id = $1 ORDER BY ctid",
    )
    .bind(return_id)
    .fetch_all(&mut *conn)
    .await
    .map_err(map_db_error)?;
    rows.into_iter().map(PgReturnRepository::row_to_item).collect()
}

/// Load a return with its items on the caller's connection, locking the
/// header row when `lock` is set.
async fn load_return_pg(conn: &mut PgConnection, id: Uuid, lock: bool) -> Result<Option<Return>> {
    let sql = if lock {
        "SELECT * FROM returns WHERE id = $1 FOR UPDATE"
    } else {
        "SELECT * FROM returns WHERE id = $1"
    };
    let row = sqlx::query_as::<_, ReturnRow>(sql)
        .bind(id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(map_db_error)?;
    match row {
        Some(row) => {
            let items = load_items_pg(conn, id).await?;
            Ok(Some(PgReturnRepository::row_to_return(row, items)?))
        }
        None => Ok(None),
    }
}

/// Delete a return and its items on the caller's connection, refusing any
/// return outside the early no-effect window (see [`Return::check_deletable`]).
///
/// A missing return is a no-op, matching the previous "delete what is there"
/// contract for batch deletes.
async fn delete_return_pg(conn: &mut PgConnection, id: Uuid) -> Result<()> {
    let Some(existing) = load_return_pg(conn, id, true).await? else {
        return Ok(());
    };
    existing.check_deletable()?;
    sqlx::query("DELETE FROM return_items WHERE return_id = $1")
        .bind(id)
        .execute(&mut *conn)
        .await
        .map_err(map_db_error)?;
    sqlx::query("DELETE FROM returns WHERE id = $1")
        .bind(id)
        .execute(&mut *conn)
        .await
        .map_err(map_db_error)?;
    Ok(())
}

/// Whether `existing` is the return that `input` describes (same order, same
/// lines), so an idempotent replay can be told apart from key reuse.
fn same_return_request(existing: &Return, input: &CreateReturn) -> bool {
    if existing.order_id != input.order_id || existing.items.len() != input.items.len() {
        return false;
    }
    let mut have: Vec<(Uuid, i32)> =
        existing.items.iter().map(|i| (i.order_item_id.into_uuid(), i.quantity)).collect();
    let mut want: Vec<(Uuid, i32)> =
        input.items.iter().map(|i| (i.order_item_id.into_uuid(), i.quantity)).collect();
    have.sort_unstable();
    want.sort_unstable();
    have == want
}

/// Resolve an idempotent replay: the return already stored under `key`, if
/// its payload matches `input` (`Conflict` otherwise).
async fn replay_by_key(
    conn: &mut PgConnection,
    key: &str,
    input: &CreateReturn,
) -> Result<Option<Return>> {
    let existing_id: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM returns WHERE idempotency_key = $1")
            .bind(key)
            .fetch_optional(&mut *conn)
            .await
            .map_err(map_db_error)?;
    let Some(existing_id) = existing_id else {
        return Ok(None);
    };
    let Some(existing) = load_return_pg(conn, existing_id, false).await? else {
        return Ok(None);
    };
    if same_return_request(&existing, input) {
        Ok(Some(existing))
    } else {
        Err(CommerceError::Conflict(format!(
            "Idempotency key {key} was already used for return {existing_id} with a different \
             payload"
        )))
    }
}

/// Insert a return (header + lines + kernel event) on the caller's
/// transaction and return it as stored. Honours `idempotency_key` inside the
/// transaction (see [`replay_by_key`]).
async fn insert_return_pg(
    conn: &mut PgConnection,
    input: &CreateReturn,
    now: DateTime<Utc>,
) -> Result<Return> {
    if let Some(key) = input.idempotency_key.as_deref() {
        if let Some(existing) = replay_by_key(conn, key, input).await? {
            return Ok(existing);
        }
    }
    if input.items.is_empty() {
        return Err(CommerceError::ValidationError("Return must have at least one item".into()));
    }
    for item in &input.items {
        if item.quantity <= 0 {
            return Err(CommerceError::ValidationError(format!(
                "Return item quantity must be positive, got {}",
                item.quantity
            )));
        }
    }

    let id = Uuid::new_v4();
    let order_id = input.order_id.into_uuid();

    // Lock the order for the rest of the transaction (serializes with other
    // creates for the same order and with order status changes), and make
    // sure it is returnable.
    let order_info: Option<(Uuid, String)> =
        sqlx::query_as("SELECT customer_id, status FROM orders WHERE id = $1 FOR UPDATE")
            .bind(order_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(map_db_error)?;
    let (customer_id, order_status) = order_info.ok_or(CommerceError::OrderNotFound(order_id))?;
    ensure_order_returnable(order_id, &order_status)?;

    sqlx::query(
        r#"
        INSERT INTO returns (id, order_id, customer_id, status, reason, reason_details, idempotency_key, notes, created_at, updated_at)
        VALUES ($1, $2, $3, 'requested', $4, $5, $6, $7, $8, $8)
        "#,
    )
    .bind(id)
    .bind(order_id)
    .bind(customer_id)
    .bind(input.reason.to_string())
    .bind(&input.reason_details)
    .bind(&input.idempotency_key)
    .bind(&input.notes)
    .bind(now)
    .execute(&mut *conn)
    .await
    .map_err(map_db_error)?;

    let mut refund_amount = Decimal::ZERO;
    for item_input in &input.items {
        let item_id = Uuid::new_v4();
        let line = validate_return_item_pg(
            conn,
            order_id,
            item_input.order_item_id.into_uuid(),
            item_input.quantity,
        )
        .await?;
        let refund = line.refund_for(item_input.quantity);
        refund_amount += refund;
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
        .bind(&line.sku)
        .bind(&line.name)
        .bind(item_input.quantity)
        .bind(condition.to_string())
        .bind(refund)
        .execute(&mut *conn)
        .await
        .map_err(map_db_error)?;
    }

    sqlx::query("UPDATE returns SET refund_amount = $1 WHERE id = $2")
        .bind(refund_amount)
        .bind(id)
        .execute(&mut *conn)
        .await
        .map_err(map_db_error)?;

    let ret = load_return_pg(conn, id, false).await?.ok_or(CommerceError::ReturnNotFound(id))?;

    append_kernel_event_tx(
        conn,
        &KernelOutboxEvent::domain(
            "returns.created.v1",
            "return",
            id.to_string(),
            serde_json::json!({
                "return_id": id.to_string(),
                "order_id": input.order_id.to_string(),
                "status": ReturnStatus::Requested.to_string(),
                "refund_amount": refund_amount.to_string(),
                "item_count": ret.items.len(),
            }),
            input.idempotency_key.clone(),
        ),
    )
    .await?;

    Ok(ret)
}

/// Settle a completed return's refund through the payments ledger (see the
/// SQLite twin for the rules): `pending` refunds against the order's captured
/// payments, oldest first, until `refund_amount` is covered. Returns the
/// refund ids and any uncovered remainder.
async fn settle_refund_pg(
    conn: &mut PgConnection,
    ret: &Return,
    now: DateTime<Utc>,
) -> Result<(Vec<Uuid>, Decimal)> {
    let amount = ret.refund_amount.unwrap_or(Decimal::ZERO);
    if amount <= Decimal::ZERO
        || !UpdateReturn::refund_method_uses_payments(ret.refund_method.as_deref())
    {
        return Ok((Vec::new(), Decimal::ZERO));
    }
    // Lock the order's payments so the refund reservation cannot race another
    // refund of the same payment.
    sqlx::query("SELECT id FROM payments WHERE order_id = $1 FOR UPDATE")
        .bind(ret.order_id.into_uuid())
        .execute(&mut *conn)
        .await
        .map_err(map_db_error)?;
    let mut remaining = amount;
    let mut refund_ids = Vec::new();
    for payment in open_captures_for_order_pg(conn, ret.order_id.into_uuid()).await? {
        if remaining <= Decimal::ZERO {
            break;
        }
        let capacity = refundable_remaining_pg(conn, &payment).await?;
        let portion = capacity.min(remaining);
        if portion <= Decimal::ZERO {
            continue;
        }
        let key = format!("return:{}:{}", ret.id, payment.id);
        let reason = format!("return {}", ret.id);
        let refund_id = create_refund_pg_tx(
            conn,
            &payment,
            portion,
            Some(&reason),
            Some(&key),
            ret.notes.as_deref(),
            now,
        )
        .await?;
        refund_ids.push(refund_id);
        remaining -= portion;
    }
    Ok((refund_ids, remaining))
}

/// Apply an update (field changes and/or a status transition) on the caller's
/// transaction with every guard in force, returning the stored return. Every
/// status write in this module goes through here.
async fn apply_update_pg(
    conn: &mut PgConnection,
    id: Uuid,
    input: &UpdateReturn,
    now: DateTime<Utc>,
) -> Result<Return> {
    let before = load_return_pg(conn, id, true).await?.ok_or(CommerceError::ReturnNotFound(id))?;
    let status_before = before.status;
    if status_before.is_terminal() {
        return Err(CommerceError::NotPermitted(format!(
            "Return {id} is {status_before}; terminal returns are immutable"
        )));
    }
    let transition = input.status.filter(|next| *next != status_before);
    if let Some(next) = transition {
        before.check_transition(next, input.write_off_undispositioned)?;
    }
    if let Some(amount) = input.refund_amount {
        if amount < Decimal::ZERO {
            return Err(CommerceError::ValidationError(format!(
                "Refund amount must not be negative, got {amount}"
            )));
        }
        let cap = before.max_refund();
        if amount > cap {
            return Err(CommerceError::ValidationError(format!(
                "Refund amount {amount} exceeds the {cap} refundable on return {id} (sum of its \
                 line refund amounts)"
            )));
        }
    }

    let status_after = input.status.unwrap_or(status_before);
    let tracking = input.tracking_number.clone().or(before.tracking_number.clone());
    let refund_amount = input.refund_amount.or(before.refund_amount);
    let refund_method = input.refund_method.clone().or(before.refund_method.clone());
    let notes = input.notes.clone().or(before.notes.clone());

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
    .bind(before.version)
    .fetch_optional(&mut *conn)
    .await
    .map_err(map_db_error)?
    .ok_or_else(|| CommerceError::VersionConflict {
        entity: "return".into(),
        id: id.to_string(),
        expected_version: before.version,
    })?;
    let ret = PgReturnRepository::row_to_return(updated, before.items.clone())?;

    let mut payload = serde_json::json!({
        "return_id": id.to_string(),
        "status_before": status_before.to_string(),
        "status_after": ret.status.to_string(),
        "version_before": before.version,
        "version_after": ret.version,
        "refund_amount": ret.refund_amount.map(|amount| amount.to_string()),
        "refund_method": ret.refund_method,
    });
    if transition == Some(ReturnStatus::Completed) {
        let (refund_ids, uncovered) = settle_refund_pg(conn, &ret, now).await?;
        let written_off: i64 =
            ret.undispositioned_items().map(|item| i64::from(item.quantity)).sum();
        payload["payment_refund_ids"] =
            serde_json::json!(refund_ids.iter().map(ToString::to_string).collect::<Vec<_>>());
        payload["uncovered_refund_amount"] = serde_json::json!(uncovered.to_string());
        payload["undispositioned_units"] = serde_json::json!(written_off);
    }

    append_kernel_event_tx(
        conn,
        &KernelOutboxEvent::domain("returns.updated.v1", "return", id.to_string(), payload, None),
    )
    .await?;
    Ok(ret)
}

/// One guarded serial status hop plus its history row (local SQL: the serial
/// repository exposes no in-transaction transition).
#[allow(clippy::too_many_arguments)]
async fn serial_hop_pg(
    conn: &mut PgConnection,
    serial_id: Uuid,
    serial: &str,
    from: SerialStatus,
    to: SerialStatus,
    event: SerialEventType,
    reference: Option<(&str, Uuid)>,
    to_location_id: Option<i32>,
    from_owner_id: Option<Uuid>,
    performed_by: Option<&str>,
    notes: Option<&str>,
    now: DateTime<Utc>,
) -> Result<()> {
    if !from.can_transition_to(to) {
        return Err(CommerceError::Conflict(format!(
            "Serial {serial} ({serial_id}) cannot move from {from} to {to}"
        )));
    }
    let result = if let Some(location) = to_location_id {
        sqlx::query(
            "UPDATE serial_numbers SET status = $1, updated_at = $2, current_location_id = $3
             WHERE id = $4 AND status = $5",
        )
        .bind(to.to_string())
        .bind(now)
        .bind(location)
        .bind(serial_id)
        .bind(from.to_string())
        .execute(&mut *conn)
        .await
    } else if to == SerialStatus::Returned {
        sqlx::query(
            "UPDATE serial_numbers SET status = $1, updated_at = $2,
                    current_owner_id = NULL, current_owner_type = NULL
             WHERE id = $3 AND status = $4",
        )
        .bind(to.to_string())
        .bind(now)
        .bind(serial_id)
        .bind(from.to_string())
        .execute(&mut *conn)
        .await
    } else {
        sqlx::query(
            "UPDATE serial_numbers SET status = $1, updated_at = $2 WHERE id = $3 AND status = $4",
        )
        .bind(to.to_string())
        .bind(now)
        .bind(serial_id)
        .bind(from.to_string())
        .execute(&mut *conn)
        .await
    }
    .map_err(map_db_error)?;
    if result.rows_affected() != 1 {
        return Err(CommerceError::Conflict(format!(
            "Serial {serial} ({serial_id}) changed concurrently while moving from {from} to {to}"
        )));
    }
    let (reference_type, reference_id) = match reference {
        Some((kind, id)) => (Some(kind), Some(id)),
        None => (None, None),
    };
    sqlx::query(
        "INSERT INTO serial_history (
            id, serial_id, event_type, reference_type, reference_id,
            from_status, to_status, from_location_id, to_location_id,
            from_owner_id, to_owner_id, performed_by, notes, created_at
         ) VALUES ($1, $2, $3, $4, $5, $6, $7, NULL, $8, $9, NULL, $10, $11, $12)",
    )
    .bind(Uuid::new_v4())
    .bind(serial_id)
    .bind(event.to_string())
    .bind(reference_type)
    .bind(reference_id)
    .bind(from.to_string())
    .bind(to.to_string())
    .bind(to_location_id)
    .bind(from_owner_id)
    .bind(performed_by)
    .bind(notes)
    .bind(now)
    .execute(&mut *conn)
    .await
    .map_err(map_db_error)?;
    Ok(())
}

/// Transition every received serial for a return line: `returned` first
/// (owner cleared), then the disposition's target status.
async fn apply_serial_dispositions_pg(
    conn: &mut PgConnection,
    item: &ReturnItem,
    disposition: ReturnDisposition,
    warehouse_id: i32,
    serial_ids: &[Uuid],
    performed_by: Option<&str>,
    now: DateTime<Utc>,
) -> Result<()> {
    if serial_ids.is_empty() {
        return Ok(());
    }
    let mut unique = serial_ids.to_vec();
    unique.sort_unstable();
    unique.dedup();
    if unique.len() != serial_ids.len() {
        return Err(CommerceError::ValidationError(
            "serial_ids must not contain duplicates".into(),
        ));
    }
    if serial_ids.len() != usize::try_from(item.quantity).unwrap_or(0) {
        return Err(CommerceError::ValidationError(format!(
            "Return item {} covers {} unit(s) but {} serial number(s) were given",
            item.id,
            item.quantity,
            serial_ids.len()
        )));
    }
    let reason = format!("return item {} {disposition}", item.id);
    for serial_id in serial_ids {
        let row: Option<(String, String, String, Option<Uuid>)> = sqlx::query_as(
            "SELECT serial, sku, status, current_owner_id FROM serial_numbers WHERE id = $1 FOR UPDATE",
        )
        .bind(serial_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(map_db_error)?;
        let Some((serial, sku, status_raw, owner_id)) = row else {
            return Err(CommerceError::ValidationError(format!("Serial {serial_id} not found")));
        };
        if sku != item.sku {
            return Err(CommerceError::ValidationError(format!(
                "Serial {serial} is SKU {sku}, not {} (return item {})",
                item.sku, item.id
            )));
        }
        let mut status: SerialStatus = status_raw.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid serial.status '{status_raw}': {e}"))
        })?;
        if status != SerialStatus::Returned {
            serial_hop_pg(
                conn,
                *serial_id,
                &serial,
                status,
                SerialStatus::Returned,
                SerialEventType::Returned,
                Some(("return", item.return_id.into_uuid())),
                None,
                owner_id,
                performed_by,
                None,
                now,
            )
            .await?;
            status = SerialStatus::Returned;
        }
        if let Some(target) = disposition.serial_target() {
            let event = match target {
                SerialStatus::Quarantined => SerialEventType::Quarantined,
                SerialStatus::Scrapped => SerialEventType::Scrapped,
                SerialStatus::InService => SerialEventType::Serviced,
                _ => SerialEventType::Received,
            };
            let location = disposition.affects_stock().then_some(warehouse_id);
            serial_hop_pg(
                conn,
                *serial_id,
                &serial,
                status,
                target,
                event,
                Some(("return_item", item.id)),
                location,
                None,
                performed_by,
                Some(&reason),
                now,
            )
            .await?;
        }
    }
    Ok(())
}

/// Restore a return line's units to their lot when the disposition puts them
/// back in stock (see the SQLite twin).
async fn apply_lot_restore_pg(
    conn: &mut PgConnection,
    item: &ReturnItem,
    lot_id: Uuid,
    disposition: ReturnDisposition,
    warehouse_id: i32,
    performed_by: Option<&str>,
    now: DateTime<Utc>,
) -> Result<()> {
    let row: Option<(String, String)> =
        sqlx::query_as("SELECT lot_number, sku FROM lots WHERE id = $1 FOR UPDATE")
            .bind(lot_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(map_db_error)?;
    let Some((lot_number, sku)) = row else {
        return Err(CommerceError::ValidationError(format!("Lot {lot_id} not found")));
    };
    if sku != item.sku {
        return Err(CommerceError::ValidationError(format!(
            "Lot {lot_number} is SKU {sku}, not {} (return item {})",
            item.sku, item.id
        )));
    }
    if !disposition.restores_lot() {
        return Ok(());
    }
    let qty = Decimal::from(item.quantity);
    let quarantined =
        if disposition == ReturnDisposition::Quarantine { qty } else { Decimal::ZERO };
    sqlx::query(
        "UPDATE lots SET quantity_remaining = quantity_remaining + $1,
                quantity_quarantined = quantity_quarantined + $2, updated_at = $3
         WHERE id = $4",
    )
    .bind(qty)
    .bind(quarantined)
    .bind(now)
    .bind(lot_id)
    .execute(&mut *conn)
    .await
    .map_err(map_db_error)?;
    sqlx::query(
        "INSERT INTO lot_locations (lot_id, location_id, quantity, updated_at)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (lot_id, location_id)
         DO UPDATE SET quantity = lot_locations.quantity + EXCLUDED.quantity, updated_at = EXCLUDED.updated_at",
    )
    .bind(lot_id)
    .bind(warehouse_id)
    .bind(qty)
    .bind(now)
    .execute(&mut *conn)
    .await
    .map_err(map_db_error)?;
    sqlx::query(
        "INSERT INTO lot_transactions (id, lot_id, transaction_type, quantity, reference_type,
                                       reference_id, from_location_id, to_location_id, reason,
                                       performed_by, created_at)
         VALUES ($1, $2, $3, $4, 'return_item', $5, NULL, $6, $7, $8, $9)",
    )
    .bind(Uuid::new_v4())
    .bind(lot_id)
    .bind(LotTransactionType::Returned.to_string())
    .bind(qty)
    .bind(item.id)
    .bind(warehouse_id)
    .bind(format!("return {} {disposition}", item.return_id))
    .bind(performed_by)
    .bind(now)
    .execute(&mut *conn)
    .await
    .map_err(map_db_error)?;
    Ok(())
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
            lot_id,
            serial_ids,
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
        let serial_ids: Vec<Uuid> = match serial_ids {
            Some(value) if !value.is_null() => serde_json::from_value(value).map_err(|e| {
                CommerceError::DatabaseError(format!("Invalid return_item.serial_ids: {e}"))
            })?,
            _ => Vec::new(),
        };

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
            lot_id,
            serial_ids,
        })
    }

    /// Record a return item's disposition and apply its stock, serial and lot
    /// effects atomically (async).
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
                // The hold is always recorded at warehouse level (on hand and
                // allocated, so the units are tracked but not sellable); the
                // quarantine bin mirrors it when the warehouse has one.
                let bin = find_disposition_bin_pg(
                    tx.as_mut(),
                    warehouse_id,
                    input.bin_id,
                    &[BinType::Quarantine],
                )
                .await?;
                if let Some(bin) = &bin {
                    apply_bin_delta_pg(tx.as_mut(), bin, &item.sku, qty, qty, now).await?;
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
                    qty,
                    &reason,
                    Some("return_item"),
                    Some(&reference_id),
                    now,
                )
                .await?;
            }
            ReturnDisposition::Refurbish
            | ReturnDisposition::Scrap
            | ReturnDisposition::ReturnToVendor => {
                // No stock effect: the units do not re-enter sellable or held
                // inventory.
            }
            // `ReturnDisposition` is `#[non_exhaustive]`: an unknown variant
            // must fail closed rather than be recorded with no stock effect.
            other => {
                return Err(CommerceError::ValidationError(format!(
                    "Unsupported return disposition {other}"
                )));
            }
        }

        apply_serial_dispositions_pg(
            tx.as_mut(),
            &item,
            input.disposition,
            warehouse_id,
            &input.serial_ids,
            input.disposition_by.as_deref(),
            now,
        )
        .await?;
        if let Some(lot_id) = input.lot_id {
            apply_lot_restore_pg(
                tx.as_mut(),
                &item,
                lot_id,
                input.disposition,
                warehouse_id,
                input.disposition_by.as_deref(),
                now,
            )
            .await?;
        }

        let serial_ids_json = if input.serial_ids.is_empty() {
            None
        } else {
            Some(serde_json::json!(input.serial_ids))
        };
        let updated = sqlx::query_as::<_, ReturnItemRow>(
            "UPDATE return_items SET disposition = $1, disposition_at = $2, disposition_by = $3,
                    lot_id = $4, serial_ids = $5
             WHERE id = $6 RETURNING *",
        )
        .bind(input.disposition.to_string())
        .bind(now)
        .bind(input.disposition_by)
        .bind(input.lot_id)
        .bind(serial_ids_json)
        .bind(item_id)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        tx.commit().await.map_err(map_db_error)?;
        Self::row_to_item(updated)
    }

    /// Create a return (async)
    pub async fn create_async(&self, input: CreateReturn) -> Result<Return> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        match insert_return_pg(tx.as_mut(), &input, Utc::now()).await {
            Ok(ret) => {
                tx.commit().await.map_err(map_db_error)?;
                Ok(ret)
            }
            // Two creates with the same idempotency key raced past the in-tx
            // lookup: the unique index caught the loser, which now replays
            // (or conflicts on) the winner's committed return.
            Err(CommerceError::Conflict(message)) if message.contains(IDEMPOTENCY_KEY_INDEX) => {
                drop(tx);
                let key = input.idempotency_key.as_deref().unwrap_or_default();
                let mut conn = self.pool.acquire().await.map_err(map_db_error)?;
                replay_by_key(&mut conn, key, &input)
                    .await?
                    .ok_or_else(|| CommerceError::Conflict(message))
            }
            Err(error) => Err(error),
        }
    }

    /// Get a return by ID (async)
    pub async fn get_async(&self, id: Uuid) -> Result<Option<Return>> {
        let mut conn = self.pool.acquire().await.map_err(map_db_error)?;
        load_return_pg(&mut conn, id, false).await
    }

    /// Get return items (async)
    pub async fn get_items_async(&self, return_id: Uuid) -> Result<Vec<ReturnItem>> {
        let mut conn = self.pool.acquire().await.map_err(map_db_error)?;
        load_items_pg(&mut conn, return_id).await
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
            "SELECT * FROM return_items WHERE return_id = ANY($1) ORDER BY ctid",
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
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let ret = apply_update_pg(tx.as_mut(), id, &input, Utc::now()).await?;
        tx.commit().await.map_err(map_db_error)?;
        Ok(ret)
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

        builder.push(" ORDER BY created_at DESC, id DESC");

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

    /// Run `precheck` against the locked current return, then apply `update`,
    /// all in one transaction — so a wrong-status error is typed from the
    /// same snapshot the write would have used.
    async fn transition_async(
        &self,
        id: Uuid,
        precheck: impl Fn(&Return) -> Result<()>,
        update: UpdateReturn,
    ) -> Result<Return> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let current = load_return_pg(tx.as_mut(), id, true)
            .await?
            .ok_or(CommerceError::ReturnNotFound(id))?;
        precheck(&current)?;
        let ret = apply_update_pg(tx.as_mut(), id, &update, Utc::now()).await?;
        tx.commit().await.map_err(map_db_error)?;
        Ok(ret)
    }

    /// Approve a return (async)
    pub async fn approve_async(&self, id: Uuid) -> Result<Return> {
        self.transition_async(
            id,
            |current| {
                if current.status == ReturnStatus::Requested {
                    Ok(())
                } else {
                    Err(CommerceError::ReturnCannotBeApproved(current.status.to_string()))
                }
            },
            UpdateReturn { status: Some(ReturnStatus::Approved), ..Default::default() },
        )
        .await
    }

    /// Reject a return (async)
    pub async fn reject_async(&self, id: Uuid, reason: &str) -> Result<Return> {
        self.transition_async(
            id,
            |current| {
                if current.status.can_transition_to(ReturnStatus::Rejected) {
                    Ok(())
                } else {
                    Err(CommerceError::NotPermitted(format!(
                        "Return cannot be rejected in status: {}",
                        current.status
                    )))
                }
            },
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
        self.transition_async(
            id,
            |current| {
                if current.can_complete() {
                    Ok(())
                } else {
                    Err(CommerceError::NotPermitted(format!(
                        "Return cannot be completed in status: {}",
                        current.status
                    )))
                }
            },
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

    /// Delete a return and its items in one transaction (async).
    ///
    /// Guarded by [`Return::check_deletable`] exactly like the SQLite backend:
    /// only a `requested` or `approved` return with no dispositioned item may
    /// be deleted. The header row is locked `FOR UPDATE` before the guard so a
    /// concurrent transition or disposition cannot slip past it.
    pub async fn delete_async(&self, id: Uuid) -> Result<()> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        delete_return_pg(tx.as_mut(), id).await?;
        tx.commit().await.map_err(map_db_error)?;
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
        let now = Utc::now();
        let mut returns = Vec::with_capacity(inputs.len());
        for input in &inputs {
            returns.push(insert_return_pg(tx.as_mut(), input, now).await?);
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

    /// Update multiple returns - atomic (all-or-nothing) (async). Every update
    /// goes through the same guarded path as a single update.
    pub async fn update_batch_atomic_async(
        &self,
        updates: Vec<(ReturnId, UpdateReturn)>,
    ) -> Result<Vec<Return>> {
        validate_batch_size(&updates)?;

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let now = Utc::now();
        let mut returns = Vec::with_capacity(updates.len());
        for (id, input) in &updates {
            returns.push(apply_update_pg(tx.as_mut(), id.into_uuid(), input, now).await?);
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
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        // Every id runs through the same deletability guard as the single
        // delete; one refusal rolls the whole batch back.
        for id in raw_ids {
            delete_return_pg(tx.as_mut(), id).await?;
        }
        tx.commit().await.map_err(map_db_error)?;
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
