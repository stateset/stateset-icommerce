//! SQLite return repository implementation.
//!
//! Invariants enforced here (mirrored exactly by the Postgres backend):
//!
//! - **Over-return**: a return line never claims more units than remain
//!   returnable on its order line (shipped units once the order has shipped,
//!   ordered units before that), counting every non-terminal return. The check
//!   and the insert share one `IMMEDIATE` transaction so concurrent creates
//!   are serialized.
//! - **Idempotency**: `idempotency_key` is unique at the database
//!   (`idx_returns_idempotency_key`); the lookup runs inside the write
//!   transaction, a replay with the same key returns the original return and
//!   a replay with a different payload is a `Conflict`.
//! - **Refund bounds**: `refund_amount` is never negative, never above the sum
//!   of the line refund amounts, and immutable once the return is terminal.
//!   Line refunds are the proportional share of `order_items.total` (so line
//!   discounts and tax are honoured), not `unit_price * quantity`.
//! - **Status guards**: every status write goes through
//!   [`Return::check_transition`]: no rejecting/cancelling after units were
//!   restocked or quarantined, no completing with undispositioned items unless
//!   explicitly written off.
//! - **Completion settles**: completing a return creates `pending` payment
//!   refunds against the order's captured payments in the same transaction
//!   (unless `refund_method` settles out of band).
//! - **Traceability**: dispositions transition the received serials and
//!   restore lot on-hand in the same transaction as the stock effect.

use super::bins::{
    apply_bin_delta_tx, apply_warehouse_delta_tx, find_disposition_bin_tx, insert_bin_movement_tx,
    smuggle,
};
use super::kernel_outbox::append_kernel_event_tx;
use super::parse_helpers::{parse_decimal, parse_enum, parse_uuid};
use super::payments::{
    create_refund_in_tx, open_captures_for_order_conn, refundable_remaining_in_tx,
};
use super::{
    build_in_clause, map_db_error, params_refs, parse_datetime_opt_row, parse_datetime_row,
    parse_decimal_opt_row, parse_decimal_row, parse_enum_row, parse_uuid_row, uuid_params,
    with_immediate_transaction,
};
use crate::KernelOutboxEvent;
use chrono::{DateTime, Utc};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::OptionalExtension;
use rust_decimal::Decimal;
use stateset_core::{
    BatchResult, BinMovementType, BinType, CommerceError, CreateReturn, CustomerId,
    LotTransactionType, OrderId, OrderItemId, OrderStatus, Result, Return, ReturnDisposition,
    ReturnFilter, ReturnId, ReturnItem, ReturnRepository, ReturnStatus, SerialEventType,
    SerialStatus, SetReturnDisposition, UpdateReturn, validate_batch_size,
};
use uuid::Uuid;

/// Decimal places a line refund is rounded to (matches `order_items.total`).
const MONEY_SCALE: u32 = 2;

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

/// Parse the nullable JSON `serial_ids` column of a `return_items` row.
fn parse_serial_ids(raw: Option<String>) -> rusqlite::Result<Vec<Uuid>> {
    match raw {
        Some(json) if !json.trim().is_empty() => {
            serde_json::from_str::<Vec<Uuid>>(&json).map_err(|e| {
                smuggle(CommerceError::DatabaseError(format!(
                    "Invalid return_item.serial_ids '{json}': {e}"
                )))
            })
        }
        _ => Ok(Vec::new()),
    }
}

const RETURN_ITEM_COLUMNS: &str = "id, return_id, order_item_id, sku, name, quantity, condition, \
                                   refund_amount, disposition, disposition_at, disposition_by, \
                                   lot_id, serial_ids";

pub(crate) fn row_to_return_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<ReturnItem> {
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
        lot_id: match row.get::<_, Option<String>>("lot_id")? {
            Some(raw) if !raw.is_empty() => Some(parse_uuid_row(&raw, "return_item", "lot_id")?),
            _ => None,
        },
        serial_ids: parse_serial_ids(row.get::<_, Option<String>>("serial_ids")?)?,
    })
}

/// Load the items of one return on the caller's connection/transaction.
pub(crate) fn load_return_items_conn(
    conn: &rusqlite::Connection,
    return_id: &str,
) -> rusqlite::Result<Vec<ReturnItem>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {RETURN_ITEM_COLUMNS} FROM return_items WHERE return_id = ?1 ORDER BY rowid"
    ))?;
    let items =
        stmt.query_map([return_id], row_to_return_item)?.collect::<rusqlite::Result<_>>()?;
    Ok(items)
}

/// Load a return with its items on the caller's connection/transaction.
pub(crate) fn load_return_conn(
    conn: &rusqlite::Connection,
    id: &str,
) -> rusqlite::Result<Option<Return>> {
    let header = conn
        .query_row(
            "SELECT * FROM returns WHERE id = ?1",
            [id],
            SqliteReturnRepository::row_to_return,
        )
        .optional()?;
    match header {
        Some(mut ret) => {
            ret.items = load_return_items_conn(conn, id)?;
            Ok(Some(ret))
        }
        None => Ok(None),
    }
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

/// What a validated return line copies from its order item.
struct ReturnableLine {
    sku: String,
    name: String,
    unit_price: Decimal,
    /// `order_items.total` — unit price × quantity less the line discount plus
    /// line tax, as charged.
    line_total: Decimal,
    ordered_qty: i64,
}

impl ReturnableLine {
    /// The refund for returning `return_qty` units of this line: the
    /// proportional share of what was actually charged for the line, so a
    /// discounted line never refunds more than it collected. Falls back to
    /// `unit_price × qty` for legacy rows with no usable total.
    fn refund_for(&self, return_qty: i64) -> Decimal {
        let qty = Decimal::from(return_qty);
        if self.ordered_qty > 0 && self.line_total >= Decimal::ZERO {
            (self.line_total * qty / Decimal::from(self.ordered_qty)).round_dp(MONEY_SCALE)
        } else {
            (self.unit_price * qty).round_dp(MONEY_SCALE)
        }
    }
}

/// Validate a single return line against its order item, inside a write
/// transaction, returning what the return records from the line.
///
/// Rejects the return when:
/// - the order item does not exist,
/// - the order item belongs to a different order than the one being returned, or
/// - returning `return_qty` more units would exceed what was purchased (or, once
///   the order has shipped, what was actually shipped on that line), counting
///   units already claimed by non-terminal returns (rejected/cancelled returns
///   release their claim — which is why a return can no longer be rejected or
///   cancelled once its units were restocked, see [`Return::check_transition`]).
///
/// This guards against over-returning (and thus over-refunding) more units than
/// were ordered, and against returning another order's items. The caller holds
/// the `IMMEDIATE` write lock, so the read and the insert cannot interleave
/// with another create.
fn validate_return_item_tx(
    tx: &rusqlite::Transaction<'_>,
    order_id: &str,
    order_item_id: &str,
    return_qty: i64,
) -> Result<ReturnableLine> {
    let (sku, name, unit_price, line_total, oi_order_id, ordered_qty, shipped_qty, order_status): (
        String,
        String,
        String,
        String,
        String,
        i64,
        i64,
        String,
    ) = tx
        .query_row(
            "SELECT oi.sku, oi.name, oi.unit_price, oi.total, oi.order_id, oi.quantity, oi.shipped_quantity, o.status
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
                    row.get(7)?,
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

    Ok(ReturnableLine {
        sku,
        name,
        unit_price: parse_decimal(&unit_price, "order_item", "unit_price")?,
        line_total: parse_decimal(&line_total, "order_item", "total")?,
        ordered_qty,
    })
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

/// Insert a return (header + lines + kernel event) on the caller's
/// transaction and return it as stored. Honours `idempotency_key` inside the
/// transaction: an existing return with the key is returned as-is when the
/// request matches it, and a `Conflict` is raised when it does not.
fn insert_return_tx(
    tx: &rusqlite::Transaction<'_>,
    input: &CreateReturn,
    now: DateTime<Utc>,
) -> rusqlite::Result<Return> {
    if let Some(key) = input.idempotency_key.as_deref() {
        let existing_id: Option<String> = tx
            .query_row("SELECT id FROM returns WHERE idempotency_key = ?1", [key], |row| row.get(0))
            .optional()?;
        if let Some(existing_id) = existing_id {
            if let Some(existing) = load_return_conn(tx, &existing_id)? {
                if same_return_request(&existing, input) {
                    return Ok(existing);
                }
                return Err(smuggle(CommerceError::Conflict(format!(
                    "Idempotency key {key} was already used for return {existing_id} with a \
                     different payload"
                ))));
            }
        }
    }

    if input.items.is_empty() {
        return Err(smuggle(CommerceError::ValidationError(
            "Return must have at least one item".into(),
        )));
    }
    for item in &input.items {
        if item.quantity <= 0 {
            return Err(smuggle(CommerceError::ValidationError(format!(
                "Return item quantity must be positive, got {}",
                item.quantity
            ))));
        }
    }

    let id = Uuid::new_v4();
    let order_id = input.order_id.to_string();

    // Get order to get customer_id, and make sure it is returnable.
    let (customer_id, order_status): (String, String) = tx
        .query_row("SELECT customer_id, status FROM orders WHERE id = ?", [&order_id], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .optional()?
        .ok_or_else(|| smuggle(CommerceError::OrderNotFound(input.order_id.into())))?;
    ensure_order_returnable(&order_id, &order_status).map_err(smuggle)?;

    tx.execute(
        "INSERT INTO returns (id, order_id, customer_id, status, reason, reason_details, idempotency_key, notes, created_at, updated_at)
         VALUES (?, ?, ?, 'requested', ?, ?, ?, ?, ?, ?)",
        rusqlite::params![
            id.to_string(),
            order_id,
            customer_id,
            input.reason.to_string(),
            input.reason_details,
            input.idempotency_key,
            input.notes,
            now.to_rfc3339(),
            now.to_rfc3339(),
        ],
    )?;

    let mut total_refund = Decimal::ZERO;
    for item in &input.items {
        let item_id = Uuid::new_v4();
        let line = validate_return_item_tx(
            tx,
            &order_id,
            &item.order_item_id.to_string(),
            i64::from(item.quantity),
        )
        .map_err(smuggle)?;
        let refund_amount = line.refund_for(i64::from(item.quantity));
        total_refund += refund_amount;

        tx.execute(
            "INSERT INTO return_items (id, return_id, order_item_id, sku, name, quantity, condition, refund_amount)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                item_id.to_string(),
                id.to_string(),
                item.order_item_id.to_string(),
                line.sku,
                line.name,
                item.quantity,
                item.condition.unwrap_or_default().to_string(),
                refund_amount.to_string(),
            ],
        )?;
    }

    tx.execute(
        "UPDATE returns SET refund_amount = ? WHERE id = ?",
        rusqlite::params![total_refund.to_string(), id.to_string()],
    )?;

    let ret = load_return_conn(tx, &id.to_string())?
        .ok_or_else(|| smuggle(CommerceError::ReturnNotFound(id)))?;

    append_kernel_event_tx(
        tx,
        &KernelOutboxEvent::domain(
            "returns.created.v1",
            "return",
            id.to_string(),
            serde_json::json!({
                "return_id": id.to_string(),
                "order_id": input.order_id.to_string(),
                "status": ReturnStatus::Requested.to_string(),
                "refund_amount": ret.refund_amount.map(|amount| amount.to_string()),
                "item_count": ret.items.len(),
            }),
            input.idempotency_key.clone(),
        ),
    )?;

    Ok(ret)
}

/// Settle a completed return's refund through the payments ledger: create a
/// `pending` refund against each of the order's captured payments (oldest
/// first) until `refund_amount` is covered. Returns the refund ids and any
/// amount no captured payment could cover (recorded on the completion event
/// for follow-up; the return still completes). Nothing is created when the
/// refund is zero or `refund_method` settles out of band (store credit,
/// exchange, ...).
fn settle_refund_tx(
    tx: &rusqlite::Transaction<'_>,
    ret: &Return,
    now: DateTime<Utc>,
) -> rusqlite::Result<(Vec<Uuid>, Decimal)> {
    let amount = ret.refund_amount.unwrap_or(Decimal::ZERO);
    if amount <= Decimal::ZERO
        || !UpdateReturn::refund_method_uses_payments(ret.refund_method.as_deref())
    {
        return Ok((Vec::new(), Decimal::ZERO));
    }
    let mut remaining = amount;
    let mut refund_ids = Vec::new();
    for payment in open_captures_for_order_conn(tx, &ret.order_id.to_string())? {
        if remaining <= Decimal::ZERO {
            break;
        }
        let capacity = refundable_remaining_in_tx(tx, &payment)?;
        let portion = capacity.min(remaining);
        if portion <= Decimal::ZERO {
            continue;
        }
        let key = format!("return:{}:{}", ret.id, payment.id);
        let reason = format!("return {}", ret.id);
        let refund_id = create_refund_in_tx(
            tx,
            &payment,
            portion,
            Some(&reason),
            Some(&key),
            ret.notes.as_deref(),
            now,
        )?;
        refund_ids.push(refund_id);
        remaining -= portion;
    }
    Ok((refund_ids, remaining))
}

/// Apply an update (field changes and/or a status transition) on the caller's
/// transaction with every guard in force, returning the stored return. Every
/// status write in this module goes through here.
fn apply_update_tx(
    tx: &rusqlite::Transaction<'_>,
    id: ReturnId,
    input: &UpdateReturn,
    now: DateTime<Utc>,
) -> rusqlite::Result<Return> {
    let before = load_return_conn(tx, &id.to_string())?
        .ok_or_else(|| smuggle(CommerceError::ReturnNotFound(id.into())))?;
    let status_before = before.status;
    if status_before.is_terminal() {
        return Err(smuggle(CommerceError::NotPermitted(format!(
            "Return {id} is {status_before}; terminal returns are immutable"
        ))));
    }
    let transition = input.status.filter(|next| *next != status_before);
    if let Some(next) = transition {
        before.check_transition(next, input.write_off_undispositioned).map_err(smuggle)?;
    }
    if let Some(amount) = input.refund_amount {
        if amount < Decimal::ZERO {
            return Err(smuggle(CommerceError::ValidationError(format!(
                "Refund amount must not be negative, got {amount}"
            ))));
        }
        let cap = before.max_refund();
        if amount > cap {
            return Err(smuggle(CommerceError::ValidationError(format!(
                "Refund amount {amount} exceeds the {cap} refundable on return {id} \
                 (sum of its line refund amounts)"
            ))));
        }
    }

    let mut updates = vec!["updated_at = ?", "version = version + 1"];
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now.to_rfc3339())];
    if let Some(status) = input.status {
        updates.push("status = ?");
        params.push(Box::new(status.to_string()));
    }
    if let Some(tracking) = &input.tracking_number {
        updates.push("tracking_number = ?");
        params.push(Box::new(tracking.clone()));
    }
    if let Some(amount) = input.refund_amount {
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
    params.push(Box::new(before.version));

    let sql = format!("UPDATE returns SET {} WHERE id = ? AND version = ?", updates.join(", "));
    let refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(std::convert::AsRef::as_ref).collect();
    if tx.execute(&sql, refs.as_slice())? == 0 {
        return Err(smuggle(CommerceError::VersionConflict {
            entity: "return".into(),
            id: id.to_string(),
            expected_version: before.version,
        }));
    }

    let ret = load_return_conn(tx, &id.to_string())?
        .ok_or_else(|| smuggle(CommerceError::ReturnNotFound(id.into())))?;

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
        let (refund_ids, uncovered) = settle_refund_tx(tx, &ret, now)?;
        let written_off: i64 =
            ret.undispositioned_items().map(|item| i64::from(item.quantity)).sum();
        payload["payment_refund_ids"] =
            serde_json::json!(refund_ids.iter().map(ToString::to_string).collect::<Vec<_>>());
        payload["uncovered_refund_amount"] = serde_json::json!(uncovered.to_string());
        payload["undispositioned_units"] = serde_json::json!(written_off);
    }

    append_kernel_event_tx(
        tx,
        &KernelOutboxEvent::domain("returns.updated.v1", "return", id.to_string(), payload, None),
    )?;
    Ok(ret)
}

/// Transition every received serial for a return line: `returned` first
/// (owner cleared), then the disposition's target status, with a history row
/// for each hop. Runs on the caller's transaction; SQL is local to the returns
/// module because the serial repository exposes no in-transaction transition.
fn apply_serial_dispositions_tx(
    tx: &rusqlite::Transaction<'_>,
    item: &ReturnItem,
    disposition: ReturnDisposition,
    warehouse_id: i32,
    serial_ids: &[Uuid],
    performed_by: Option<&str>,
    now: &str,
) -> rusqlite::Result<()> {
    if serial_ids.is_empty() {
        return Ok(());
    }
    let mut unique = serial_ids.to_vec();
    unique.sort_unstable();
    unique.dedup();
    if unique.len() != serial_ids.len() {
        return Err(smuggle(CommerceError::ValidationError(
            "serial_ids must not contain duplicates".into(),
        )));
    }
    if serial_ids.len() != usize::try_from(item.quantity).unwrap_or(0) {
        return Err(smuggle(CommerceError::ValidationError(format!(
            "Return item {} covers {} unit(s) but {} serial number(s) were given",
            item.id,
            item.quantity,
            serial_ids.len()
        ))));
    }
    let reason = format!("return item {} {disposition}", item.id);
    for serial_id in serial_ids {
        let row: Option<(String, String, String, Option<String>)> = tx
            .query_row(
                "SELECT serial, sku, status, current_owner_id FROM serial_numbers WHERE id = ?1",
                [serial_id.to_string()],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()?;
        let Some((serial, sku, status_raw, owner_id)) = row else {
            return Err(smuggle(CommerceError::ValidationError(format!(
                "Serial {serial_id} not found"
            ))));
        };
        if sku != item.sku {
            return Err(smuggle(CommerceError::ValidationError(format!(
                "Serial {serial} is SKU {sku}, not {} (return item {})",
                item.sku, item.id
            ))));
        }
        let mut status: SerialStatus = parse_enum_row(&status_raw, "serial_number", "status")?;
        if status != SerialStatus::Returned {
            serial_hop(
                tx,
                *serial_id,
                &serial,
                status,
                SerialStatus::Returned,
                SerialEventType::Returned,
                Some(("return", item.return_id.to_string())),
                None,
                owner_id.as_deref(),
                performed_by,
                None,
                now,
            )?;
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
            serial_hop(
                tx,
                *serial_id,
                &serial,
                status,
                target,
                event,
                Some(("return_item", item.id.to_string())),
                location,
                None,
                performed_by,
                Some(&reason),
                now,
            )?;
        }
    }
    Ok(())
}

/// One guarded serial status hop plus its history row.
#[allow(clippy::too_many_arguments)]
fn serial_hop(
    tx: &rusqlite::Transaction<'_>,
    serial_id: Uuid,
    serial: &str,
    from: SerialStatus,
    to: SerialStatus,
    event: SerialEventType,
    reference: Option<(&str, String)>,
    to_location_id: Option<i32>,
    from_owner_id: Option<&str>,
    performed_by: Option<&str>,
    notes: Option<&str>,
    now: &str,
) -> rusqlite::Result<()> {
    if !from.can_transition_to(to) {
        return Err(smuggle(CommerceError::Conflict(format!(
            "Serial {serial} ({serial_id}) cannot move from {from} to {to}"
        ))));
    }
    let rows = match to_location_id {
        Some(location) => tx.execute(
            "UPDATE serial_numbers SET status = ?1, updated_at = ?2, current_location_id = ?3
             WHERE id = ?4 AND status = ?5",
            rusqlite::params![
                to.to_string(),
                now,
                location,
                serial_id.to_string(),
                from.to_string()
            ],
        )?,
        None if to == SerialStatus::Returned => tx.execute(
            "UPDATE serial_numbers SET status = ?1, updated_at = ?2,
                    current_owner_id = NULL, current_owner_type = NULL
             WHERE id = ?3 AND status = ?4",
            rusqlite::params![to.to_string(), now, serial_id.to_string(), from.to_string()],
        )?,
        None => tx.execute(
            "UPDATE serial_numbers SET status = ?1, updated_at = ?2 WHERE id = ?3 AND status = ?4",
            rusqlite::params![to.to_string(), now, serial_id.to_string(), from.to_string()],
        )?,
    };
    if rows != 1 {
        return Err(smuggle(CommerceError::Conflict(format!(
            "Serial {serial} ({serial_id}) changed concurrently while moving from {from} to {to}"
        ))));
    }
    let (reference_type, reference_id) = match &reference {
        Some((kind, id)) => (Some(*kind), Some(id.as_str())),
        None => (None, None),
    };
    tx.execute(
        "INSERT INTO serial_history (
            id, serial_id, event_type, reference_type, reference_id,
            from_status, to_status, from_location_id, to_location_id,
            from_owner_id, to_owner_id, performed_by, notes, created_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, NULL, ?, ?, NULL, ?, ?, ?)",
        rusqlite::params![
            Uuid::new_v4().to_string(),
            serial_id.to_string(),
            event.to_string(),
            reference_type,
            reference_id,
            from.to_string(),
            to.to_string(),
            to_location_id,
            from_owner_id,
            performed_by,
            notes,
            now,
        ],
    )?;
    Ok(())
}

/// Restore a return line's units to their lot when the disposition puts them
/// back in stock (`quantity_remaining`, plus `quantity_quarantined` for a
/// quarantine hold), place them at the warehouse in `lot_locations`, and
/// record a `returned` lot transaction. Other dispositions only validate the
/// lot (it must exist and carry the item's SKU). Local SQL: the lot
/// repository exposes no in-transaction adjustment.
fn apply_lot_restore_tx(
    tx: &rusqlite::Transaction<'_>,
    item: &ReturnItem,
    lot_id: Uuid,
    disposition: ReturnDisposition,
    warehouse_id: i32,
    performed_by: Option<&str>,
    now: &str,
) -> rusqlite::Result<()> {
    let row: Option<(String, String, String, String)> = tx
        .query_row(
            "SELECT lot_number, sku, quantity_remaining, quantity_quarantined FROM lots WHERE id = ?1",
            [lot_id.to_string()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;
    let Some((lot_number, sku, remaining_raw, quarantined_raw)) = row else {
        return Err(smuggle(CommerceError::ValidationError(format!("Lot {lot_id} not found"))));
    };
    if sku != item.sku {
        return Err(smuggle(CommerceError::ValidationError(format!(
            "Lot {lot_number} is SKU {sku}, not {} (return item {})",
            item.sku, item.id
        ))));
    }
    if !disposition.restores_lot() {
        return Ok(());
    }
    let qty = Decimal::from(item.quantity);
    let remaining = parse_decimal_row(&remaining_raw, "lot", "quantity_remaining")? + qty;
    let mut quarantined = parse_decimal_row(&quarantined_raw, "lot", "quantity_quarantined")?;
    if disposition == ReturnDisposition::Quarantine {
        quarantined += qty;
    }
    tx.execute(
        "UPDATE lots SET quantity_remaining = ?1, quantity_quarantined = ?2, updated_at = ?3 WHERE id = ?4",
        rusqlite::params![remaining.to_string(), quarantined.to_string(), now, lot_id.to_string()],
    )?;
    let placed: Option<String> = tx
        .query_row(
            "SELECT quantity FROM lot_locations WHERE lot_id = ?1 AND location_id = ?2",
            rusqlite::params![lot_id.to_string(), warehouse_id],
            |r| r.get(0),
        )
        .optional()?;
    match placed {
        Some(current) => {
            let total = parse_decimal_row(&current, "lot_location", "quantity")? + qty;
            tx.execute(
                "UPDATE lot_locations SET quantity = ?1, updated_at = ?2 WHERE lot_id = ?3 AND location_id = ?4",
                rusqlite::params![total.to_string(), now, lot_id.to_string(), warehouse_id],
            )?;
        }
        None => {
            tx.execute(
                "INSERT INTO lot_locations (lot_id, location_id, quantity, updated_at) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![lot_id.to_string(), warehouse_id, qty.to_string(), now],
            )?;
        }
    }
    tx.execute(
        "INSERT INTO lot_transactions (id, lot_id, transaction_type, quantity, reference_type,
                                       reference_id, from_location_id, to_location_id, reason,
                                       performed_by, created_at)
         VALUES (?, ?, ?, ?, 'return_item', ?, NULL, ?, ?, ?, ?)",
        rusqlite::params![
            Uuid::new_v4().to_string(),
            lot_id.to_string(),
            LotTransactionType::Returned.to_string(),
            qty.to_string(),
            item.id.to_string(),
            warehouse_id,
            format!("return {} {disposition}", item.return_id),
            performed_by,
            now,
        ],
    )?;
    Ok(())
}

impl SqliteReturnRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    pub(crate) fn row_to_return(row: &rusqlite::Row<'_>) -> rusqlite::Result<Return> {
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
            "SELECT {RETURN_ITEM_COLUMNS} FROM return_items WHERE return_id IN ({placeholders})
             ORDER BY rowid"
        );
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let params = ids.iter().map(ToString::to_string).collect::<Vec<_>>();
        let items = stmt
            .query_map(rusqlite::params_from_iter(params.iter()), row_to_return_item)
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;
        for item in items {
            items_by_id.entry(item.return_id.to_string()).or_default().push(item);
        }
        Ok(items_by_id)
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

    /// Append the `ReturnFilter` predicates shared by `list` and `count`.
    fn push_filter_predicates(
        filter: &ReturnFilter,
        sql: &mut String,
        params: &mut Vec<Box<dyn rusqlite::ToSql>>,
    ) {
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
    }
}

impl ReturnRepository for SqliteReturnRepository {
    fn create(&self, input: CreateReturn) -> Result<Return> {
        with_immediate_transaction(&self.pool, |tx| insert_return_tx(tx, &input, Utc::now()))
    }

    fn get(&self, id: ReturnId) -> Result<Option<Return>> {
        let conn = self.conn()?;
        load_return_conn(&conn, &id.to_string()).map_err(map_db_error)
    }

    fn update(&self, id: ReturnId, input: UpdateReturn) -> Result<Return> {
        with_immediate_transaction(&self.pool, |tx| apply_update_tx(tx, id, &input, Utc::now()))
    }

    fn list(&self, filter: ReturnFilter) -> Result<Vec<Return>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM returns WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];
        Self::push_filter_predicates(&filter, &mut sql, &mut params);

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
        with_immediate_transaction(&self.pool, |tx| {
            let current = load_return_conn(tx, &id.to_string())?
                .ok_or_else(|| smuggle(CommerceError::ReturnNotFound(id.into())))?;
            if current.status != ReturnStatus::Requested {
                return Err(smuggle(CommerceError::ReturnCannotBeApproved(
                    current.status.to_string(),
                )));
            }
            apply_update_tx(
                tx,
                id,
                &UpdateReturn { status: Some(ReturnStatus::Approved), ..Default::default() },
                Utc::now(),
            )
        })
    }

    fn reject(&self, id: ReturnId, reason: &str) -> Result<Return> {
        with_immediate_transaction(&self.pool, |tx| {
            let current = load_return_conn(tx, &id.to_string())?
                .ok_or_else(|| smuggle(CommerceError::ReturnNotFound(id.into())))?;
            if !current.status.can_transition_to(ReturnStatus::Rejected) {
                return Err(smuggle(CommerceError::NotPermitted(format!(
                    "Return cannot be rejected in status: {}",
                    current.status
                ))));
            }
            apply_update_tx(
                tx,
                id,
                &UpdateReturn {
                    status: Some(ReturnStatus::Rejected),
                    notes: Some(reason.to_string()),
                    ..Default::default()
                },
                Utc::now(),
            )
        })
    }

    fn complete(&self, id: ReturnId) -> Result<Return> {
        with_immediate_transaction(&self.pool, |tx| {
            let current = load_return_conn(tx, &id.to_string())?
                .ok_or_else(|| smuggle(CommerceError::ReturnNotFound(id.into())))?;
            if !current.can_complete() {
                return Err(smuggle(CommerceError::NotPermitted(format!(
                    "Return cannot be completed in status: {}",
                    current.status
                ))));
            }
            apply_update_tx(
                tx,
                id,
                &UpdateReturn { status: Some(ReturnStatus::Completed), ..Default::default() },
                Utc::now(),
            )
        })
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
                    // The hold is always recorded at warehouse level (on hand and
                    // allocated, so the units are tracked but not sellable); the
                    // quarantine bin mirrors it when the warehouse has one.
                    let bin = find_disposition_bin_tx(
                        tx,
                        warehouse_id,
                        input.bin_id,
                        &[BinType::Quarantine],
                    )?;
                    if let Some(bin) = &bin {
                        apply_bin_delta_tx(tx, bin, &item.sku, qty, qty, &now)?;
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
                        qty,
                        &reason,
                        Some("return_item"),
                        Some(&reference_id),
                        &now,
                    )?;
                }
                ReturnDisposition::Refurbish
                | ReturnDisposition::Scrap
                | ReturnDisposition::ReturnToVendor => {
                    // No stock effect: the units do not re-enter sellable or held
                    // inventory.
                }
                // `ReturnDisposition` is `#[non_exhaustive]`: a variant this
                // backend does not know how to apply must fail closed rather
                // than be recorded with no stock effect.
                other => {
                    return Err(smuggle(CommerceError::ValidationError(format!(
                        "Unsupported return disposition {other}"
                    ))));
                }
            }

            apply_serial_dispositions_tx(
                tx,
                &item,
                input.disposition,
                warehouse_id,
                &input.serial_ids,
                input.disposition_by.as_deref(),
                &now,
            )?;
            if let Some(lot_id) = input.lot_id {
                apply_lot_restore_tx(
                    tx,
                    &item,
                    lot_id,
                    input.disposition,
                    warehouse_id,
                    input.disposition_by.as_deref(),
                    &now,
                )?;
            }

            let serial_ids_json = if input.serial_ids.is_empty() {
                None
            } else {
                Some(serde_json::to_string(&input.serial_ids).map_err(|e| {
                    smuggle(CommerceError::DatabaseError(format!("Cannot encode serial_ids: {e}")))
                })?)
            };
            tx.execute(
                "UPDATE return_items SET disposition = ?1, disposition_at = ?2, disposition_by = ?3,
                        lot_id = ?4, serial_ids = ?5
                 WHERE id = ?6",
                rusqlite::params![
                    input.disposition.to_string(),
                    now,
                    input.disposition_by,
                    input.lot_id.map(|id| id.to_string()),
                    serial_ids_json,
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
        Self::push_filter_predicates(&filter, &mut sql, &mut params);

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
        with_immediate_transaction(&self.pool, |tx| {
            let now = Utc::now();
            inputs.iter().map(|input| insert_return_tx(tx, input, now)).collect()
        })
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
        with_immediate_transaction(&self.pool, |tx| {
            let now = Utc::now();
            updates.iter().map(|(id, input)| apply_update_tx(tx, *id, input, now)).collect()
        })
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
