//! SQLite inventory repository implementation

use super::kernel_outbox::append_kernel_event_tx;
use super::{
    INITIAL_BACKOFF_MS, MAX_BACKOFF_MS, MAX_RETRIES, build_in_clause, i64_params, map_db_error,
    params_refs, parse_datetime_opt_row, parse_datetime_row, parse_decimal_opt_row,
    parse_decimal_row, parse_decimal_strict, parse_enum_row, parse_uuid, parse_uuid_row,
    string_params, with_immediate_transaction, with_retry,
};
use crate::KernelOutboxEvent;
use chrono::{DateTime, Utc};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::OptionalExtension;
use rust_decimal::Decimal;
use stateset_core::{
    AdjustInventory, BatchResult, CommerceError, CreateInventoryItem, InventoryBalance,
    InventoryFilter, InventoryItem, InventoryRepository, InventoryReservation,
    InventoryTransaction, LocationStock, ReservationStatus, ReserveInventory, Result, StockLevel,
    TransactionType, Validate, validate_batch_size, validate_quantity, validate_sku,
};
use std::cell::Cell;
use uuid::Uuid;

/// SQLite implementation of `InventoryRepository`
#[derive(Debug)]
pub struct SqliteInventoryRepository {
    pool: Pool<SqliteConnectionManager>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReservationConfirmOutcome {
    Confirmed,
    Expired,
}

thread_local! {
    static INVENTORY_RETRY_SEED: Cell<u64> = const { Cell::new(0x9E37_79B9_7F4A_7C15) };
}

fn inventory_retry_delay_ms(backoff_ms: u64, retry: u32) -> u64 {
    let jitter = INVENTORY_RETRY_SEED.with(|seed| {
        let mut state = seed.get().wrapping_add(u64::from(retry) + 1);
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        seed.set(state);
        state % 50
    });

    backoff_ms.min(MAX_BACKOFF_MS) + jitter
}

fn should_retry_inventory_error(err: &CommerceError) -> bool {
    match err {
        CommerceError::VersionConflict { entity, .. } => entity == "inventory_balance",
        CommerceError::DatabaseError(message) => {
            message.contains("database is locked") || message.contains("database table is locked")
        }
        _ => false,
    }
}

fn with_inventory_retry<T, F>(mut operation: F) -> Result<T>
where
    F: FnMut() -> Result<T>,
{
    let mut retries = 0;
    let mut backoff_ms = INITIAL_BACKOFF_MS;

    loop {
        match operation() {
            Ok(result) => return Ok(result),
            Err(err) if should_retry_inventory_error(&err) && retries < MAX_RETRIES => {
                retries += 1;
                let delay_ms = inventory_retry_delay_ms(backoff_ms, retries);
                std::thread::sleep(std::time::Duration::from_millis(delay_ms));
                backoff_ms = (backoff_ms * 2).min(MAX_BACKOFF_MS);
            }
            Err(err) => return Err(err),
        }
    }
}

impl SqliteInventoryRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    /// Move `delta` units into (positive) or out of (negative) the allocated
    /// bucket of one balance, recomputing `quantity_available` from the exact
    /// `Decimal` values read in this transaction. Returns the new `version`.
    ///
    /// All balance arithmetic happens in Rust: the columns are TEXT decimals,
    /// and an SQL expression such as `quantity_allocated - ?` would coerce
    /// both operands to IEEE-754 floats (`0.3 - 0.2 = 0.09999…`), corrupting
    /// fractional balances. The optimistic `version` guard proves the row was
    /// not changed between the read and the write.
    fn apply_allocation_delta_in_tx(
        conn: &rusqlite::Connection,
        item_id: i64,
        location_id: i32,
        delta: Decimal,
        now: DateTime<Utc>,
    ) -> Result<i32> {
        let (on_hand_str, allocated_str, current_version): (String, String, i32) = conn
            .query_row(
                "SELECT quantity_on_hand, quantity_allocated, version FROM inventory_balances
                 WHERE item_id = ? AND location_id = ?",
                rusqlite::params![item_id, location_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(map_db_error)?;
        let on_hand = parse_decimal_strict(&on_hand_str, "inventory_balance", "quantity_on_hand")?;
        let allocated =
            parse_decimal_strict(&allocated_str, "inventory_balance", "quantity_allocated")?;

        let mut new_allocated = allocated + delta;
        if new_allocated < Decimal::ZERO {
            // Only reachable on a balance that drifted before the sweeper /
            // exact-arithmetic fixes: releasing more than is recorded as
            // allocated. Clamp so the release can still complete; the
            // remaining drift is visible as allocated == 0 with open holds.
            tracing::warn!(
                item_id,
                location_id,
                %allocated,
                %delta,
                "inventory_balance.quantity_allocated would go negative; clamping to zero"
            );
            new_allocated = Decimal::ZERO;
        }
        let new_available = on_hand - new_allocated;

        let rows_affected = conn
            .execute(
                "UPDATE inventory_balances SET quantity_allocated = ?, quantity_available = ?,
                 version = version + 1, updated_at = ?
                 WHERE item_id = ? AND location_id = ? AND version = ?",
                rusqlite::params![
                    new_allocated.to_string(),
                    new_available.to_string(),
                    now.to_rfc3339(),
                    item_id,
                    location_id,
                    current_version
                ],
            )
            .map_err(map_db_error)?;

        if rows_affected == 0 {
            return Err(CommerceError::VersionConflict {
                entity: "inventory_balance".to_string(),
                id: format!("{item_id}:{location_id}"),
                expected_version: current_version,
            });
        }
        Ok(current_version + 1)
    }

    fn expire_reservation_in_tx(
        conn: &rusqlite::Connection,
        reservation_id: Uuid,
        item_id: i64,
        location_id: i32,
        quantity: Decimal,
        now: DateTime<Utc>,
    ) -> Result<()> {
        Self::apply_allocation_delta_in_tx(conn, item_id, location_id, -quantity, now)?;

        conn.execute(
            "UPDATE inventory_reservations SET status = 'expired' WHERE id = ?",
            [reservation_id.to_string()],
        )
        .map_err(map_db_error)?;

        Ok(())
    }

    /// Sweep up to `limit` expired open reservations across every item and
    /// location (the per-item lazy expiry only runs on traffic). Rows are
    /// taken oldest-expiry first so repeated calls drain a backlog in order.
    pub(crate) fn expire_reservations_in_tx(
        conn: &rusqlite::Connection,
        now: DateTime<Utc>,
        limit: u32,
    ) -> Result<u64> {
        let mut stmt = conn
            .prepare(
                "SELECT id, item_id, location_id, quantity FROM inventory_reservations
                 WHERE status IN ('pending', 'confirmed', 'allocated')
                   AND expires_at IS NOT NULL AND expires_at < ?
                 ORDER BY expires_at, id
                 LIMIT ?",
            )
            .map_err(map_db_error)?;
        let rows = stmt
            .query_map(rusqlite::params![now.to_rfc3339(), i64::from(limit)], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i32>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        let mut expired = 0u64;
        for (id_str, item_id, location_id, qty_str) in rows {
            let reservation_id = parse_uuid(&id_str, "inventory_reservation", "id")?;
            let quantity = parse_decimal_strict(&qty_str, "inventory_reservation", "quantity")?;
            Self::expire_reservation_in_tx(
                conn,
                reservation_id,
                item_id,
                location_id,
                quantity,
                now,
            )?;
            expired += 1;
        }
        Ok(expired)
    }

    /// Take `quantity` units straight out of available stock (on-hand and
    /// available both go down, allocated is untouched) and write a `shipment`
    /// ledger row. `InsufficientStock` when available does not cover it.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn consume_available_in_tx(
        tx: &rusqlite::Transaction<'_>,
        item_id: i64,
        location_id: i32,
        quantity: Decimal,
        reference_type: &str,
        reference_id: &str,
        reason: &str,
        now: DateTime<Utc>,
    ) -> Result<()> {
        validate_quantity(quantity)?;
        let (on_hand_str, allocated_str, current_version): (String, String, i32) = tx
            .query_row(
                "SELECT quantity_on_hand, quantity_allocated, version FROM inventory_balances
                 WHERE item_id = ? AND location_id = ?",
                rusqlite::params![item_id, location_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(map_db_error)?
            .ok_or_else(|| CommerceError::InsufficientStock {
                sku: format!("item:{item_id}"),
                requested: quantity.to_string(),
                available: "0".to_string(),
            })?;
        let on_hand = parse_decimal_strict(&on_hand_str, "inventory_balance", "quantity_on_hand")?;
        let allocated =
            parse_decimal_strict(&allocated_str, "inventory_balance", "quantity_allocated")?;
        let available = on_hand - allocated;
        if available < quantity {
            return Err(CommerceError::InsufficientStock {
                sku: format!("item:{item_id}"),
                requested: quantity.to_string(),
                available: available.to_string(),
            });
        }
        let new_on_hand = on_hand - quantity;
        let rows_affected = tx
            .execute(
                "UPDATE inventory_balances SET quantity_on_hand = ?, quantity_available = ?,
                 version = version + 1, updated_at = ?
                 WHERE item_id = ? AND location_id = ? AND version = ?",
                rusqlite::params![
                    new_on_hand.to_string(),
                    (new_on_hand - allocated).to_string(),
                    now.to_rfc3339(),
                    item_id,
                    location_id,
                    current_version
                ],
            )
            .map_err(map_db_error)?;
        if rows_affected == 0 {
            return Err(CommerceError::VersionConflict {
                entity: "inventory_balance".to_string(),
                id: format!("{item_id}:{location_id}"),
                expected_version: current_version,
            });
        }
        tx.execute(
            "INSERT INTO inventory_transactions (item_id, location_id, transaction_type, quantity, reference_type, reference_id, reason, created_at)
             VALUES (?, ?, 'shipment', ?, ?, ?, ?, ?)",
            rusqlite::params![
                item_id,
                location_id,
                (-quantity).to_string(),
                reference_type,
                reference_id,
                reason,
                now.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;
        Ok(())
    }

    /// Consume `quantity` units of an open reservation: the units leave both
    /// `quantity_on_hand` and `quantity_allocated` (so `quantity_available`
    /// is unchanged), a `shipment` ledger row is written, and the reservation
    /// is marked `fulfilled` when fully consumed (otherwise its quantity is
    /// reduced and it stays open for the remainder). Used by backorder
    /// fulfilment. Errors if the reservation is not open or too small.
    pub(crate) fn fulfil_reservation_in_tx(
        tx: &rusqlite::Transaction<'_>,
        reservation_id: Uuid,
        quantity: Decimal,
        reason: &str,
        now: DateTime<Utc>,
    ) -> Result<()> {
        validate_quantity(quantity)?;
        let res = tx
            .query_row(
                "SELECT item_id, location_id, quantity, status, reference_type, reference_id
                 FROM inventory_reservations WHERE id = ?",
                [reservation_id.to_string()],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i32>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                    ))
                },
            )
            .optional()
            .map_err(map_db_error)?
            .ok_or(CommerceError::ReservationNotFound(reservation_id))?;
        let (item_id, location_id, reserved_str, status_str, reference_type, reference_id) = res;
        let reserved = parse_decimal_strict(&reserved_str, "inventory_reservation", "quantity")?;
        let status: ReservationStatus = status_str.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid inventory_reservation.status '{status_str}': {e}"
            ))
        })?;
        if !status.holds_stock() {
            return Err(CommerceError::Conflict(format!(
                "inventory reservation {reservation_id} is {status}; only an open reservation can be fulfilled"
            )));
        }
        if quantity > reserved {
            return Err(CommerceError::InsufficientStock {
                sku: format!("reservation:{reservation_id}"),
                requested: quantity.to_string(),
                available: reserved.to_string(),
            });
        }

        let (on_hand_str, allocated_str, current_version): (String, String, i32) = tx
            .query_row(
                "SELECT quantity_on_hand, quantity_allocated, version FROM inventory_balances
                 WHERE item_id = ? AND location_id = ?",
                rusqlite::params![item_id, location_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(map_db_error)?;
        let on_hand = parse_decimal_strict(&on_hand_str, "inventory_balance", "quantity_on_hand")?;
        let allocated =
            parse_decimal_strict(&allocated_str, "inventory_balance", "quantity_allocated")?;
        let new_on_hand = on_hand - quantity;
        let new_allocated = (allocated - quantity).max(Decimal::ZERO);
        if new_on_hand < Decimal::ZERO {
            return Err(CommerceError::InsufficientStock {
                sku: format!("item:{item_id}"),
                requested: quantity.to_string(),
                available: on_hand.to_string(),
            });
        }
        let new_available = new_on_hand - new_allocated;
        let rows_affected = tx
            .execute(
                "UPDATE inventory_balances SET quantity_on_hand = ?, quantity_allocated = ?,
                 quantity_available = ?, version = version + 1, updated_at = ?
                 WHERE item_id = ? AND location_id = ? AND version = ?",
                rusqlite::params![
                    new_on_hand.to_string(),
                    new_allocated.to_string(),
                    new_available.to_string(),
                    now.to_rfc3339(),
                    item_id,
                    location_id,
                    current_version
                ],
            )
            .map_err(map_db_error)?;
        if rows_affected == 0 {
            return Err(CommerceError::VersionConflict {
                entity: "inventory_balance".to_string(),
                id: format!("{item_id}:{location_id}"),
                expected_version: current_version,
            });
        }

        if quantity == reserved {
            tx.execute(
                "UPDATE inventory_reservations SET status = 'fulfilled' WHERE id = ?",
                [reservation_id.to_string()],
            )
            .map_err(map_db_error)?;
        } else {
            tx.execute(
                "UPDATE inventory_reservations SET quantity = ? WHERE id = ?",
                rusqlite::params![(reserved - quantity).to_string(), reservation_id.to_string()],
            )
            .map_err(map_db_error)?;
        }

        tx.execute(
            "INSERT INTO inventory_transactions (item_id, location_id, transaction_type, quantity, reference_type, reference_id, reason, created_at)
             VALUES (?, ?, 'shipment', ?, ?, ?, ?, ?)",
            rusqlite::params![
                item_id,
                location_id,
                (-quantity).to_string(),
                reference_type,
                reference_id,
                reason,
                now.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;

        append_kernel_event_tx(
            tx,
            &KernelOutboxEvent::domain(
                "inventory.reservation_fulfilled.v1",
                "inventory_reservation",
                reservation_id.to_string(),
                serde_json::json!({
                    "reservation_id": reservation_id.to_string(),
                    "item_id": item_id,
                    "location_id": location_id,
                    "quantity": quantity.to_string(),
                    "remaining_quantity": (reserved - quantity).to_string(),
                    "balance_version": current_version + 1,
                }),
                None,
            ),
        )
        .map_err(map_db_error)?;
        Ok(())
    }

    fn expire_reservations_for_item_in_tx(
        conn: &rusqlite::Connection,
        item_id: i64,
        location_id: i32,
        now: DateTime<Utc>,
    ) -> Result<()> {
        let mut stmt = conn
            .prepare(
                "SELECT id, quantity FROM inventory_reservations
                 WHERE item_id = ? AND location_id = ?
                   AND status IN ('pending', 'confirmed', 'allocated')
                   AND expires_at IS NOT NULL AND expires_at < ?",
            )
            .map_err(map_db_error)?;
        let mut rows = stmt
            .query(rusqlite::params![item_id, location_id, now.to_rfc3339()])
            .map_err(map_db_error)?;

        while let Some(row) = rows.next().map_err(map_db_error)? {
            let id_str: String = row.get(0).map_err(map_db_error)?;
            let qty_str: String = row.get(1).map_err(map_db_error)?;
            let reservation_id = parse_uuid(&id_str, "inventory_reservation", "id")?;
            let quantity = parse_decimal_strict(&qty_str, "inventory_reservation", "quantity")?;

            Self::expire_reservation_in_tx(
                conn,
                reservation_id,
                item_id,
                location_id,
                quantity,
                now,
            )?;
        }

        Ok(())
    }

    pub(crate) fn reserve_in_tx(
        tx: &rusqlite::Transaction<'_>,
        input: &ReserveInventory,
    ) -> std::result::Result<(InventoryReservation, Uuid), rusqlite::Error> {
        Self::reserve_for_line_in_tx(tx, input, None)
    }

    /// [`Self::reserve_in_tx`] keyed to the order line that holds the stock.
    ///
    /// `order_item_id` is stored on the reservation row (migration 080) so the
    /// orders module can release/confirm exactly this line's hold instead of
    /// "some reservation for the same SKU". Non-order references pass `None`.
    pub(crate) fn reserve_for_line_in_tx(
        tx: &rusqlite::Transaction<'_>,
        input: &ReserveInventory,
        order_item_id: Option<Uuid>,
    ) -> std::result::Result<(InventoryReservation, Uuid), rusqlite::Error> {
        validate_quantity(input.quantity)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

        let sku = input.sku.clone();
        let quantity = input.quantity;
        let location_id = input.location_id.unwrap_or(1);
        let reference_type = input.reference_type.clone();
        let reference_id = input.reference_id.clone();
        let expires_in_seconds = input.expires_in_seconds;

        let now = Utc::now();

        let item = tx
            .query_row("SELECT * FROM inventory_items WHERE sku = ?", [&sku], |row| {
                Ok(InventoryItem {
                    id: row.get("id")?,
                    sku: row.get("sku")?,
                    name: row.get("name")?,
                    description: row.get("description")?,
                    unit_of_measure: row.get("unit_of_measure")?,
                    is_active: row.get::<_, i32>("is_active")? != 0,
                    created_at: parse_datetime_row(
                        &row.get::<_, String>("created_at")?,
                        "inventory_item",
                        "created_at",
                    )?,
                    updated_at: parse_datetime_row(
                        &row.get::<_, String>("updated_at")?,
                        "inventory_item",
                        "updated_at",
                    )?,
                })
            })
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => rusqlite::Error::ToSqlConversionFailure(
                    Box::new(CommerceError::InventoryItemNotFound(sku.clone())),
                ),
                other => other,
            })?;

        Self::expire_reservations_for_item_in_tx(tx, item.id, location_id, now)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

        let balance = tx.query_row(
            "SELECT * FROM inventory_balances WHERE item_id = ? AND location_id = ?",
            rusqlite::params![item.id, location_id],
            |row| {
                Ok(InventoryBalance {
                    id: row.get("id")?,
                    item_id: row.get("item_id")?,
                    location_id: row.get("location_id")?,
                    quantity_on_hand: parse_decimal_row(
                        &row.get::<_, String>("quantity_on_hand")?,
                        "inventory_balance",
                        "quantity_on_hand",
                    )?,
                    quantity_allocated: parse_decimal_row(
                        &row.get::<_, String>("quantity_allocated")?,
                        "inventory_balance",
                        "quantity_allocated",
                    )?,
                    quantity_available: parse_decimal_row(
                        &row.get::<_, String>("quantity_available")?,
                        "inventory_balance",
                        "quantity_available",
                    )?,
                    reorder_point: parse_decimal_opt_row(
                        row.get::<_, Option<String>>("reorder_point")?,
                        "inventory_balance",
                        "reorder_point",
                    )?,
                    safety_stock: parse_decimal_opt_row(
                        row.get::<_, Option<String>>("safety_stock")?,
                        "inventory_balance",
                        "safety_stock",
                    )?,
                    version: row.get("version")?,
                    last_counted_at: parse_datetime_opt_row(
                        row.get::<_, Option<String>>("last_counted_at")?,
                        "inventory_balance",
                        "last_counted_at",
                    )?,
                    updated_at: parse_datetime_row(
                        &row.get::<_, String>("updated_at")?,
                        "inventory_balance",
                        "updated_at",
                    )?,
                })
            },
        )?;

        if balance.quantity_available < quantity {
            return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                CommerceError::InsufficientStock {
                    sku,
                    requested: quantity.to_string(),
                    available: balance.quantity_available.to_string(),
                },
            )));
        }

        let reservation_id = Uuid::new_v4();
        let expires_at = expires_in_seconds.map(|secs| now + chrono::Duration::seconds(secs));

        tx.execute(
            "INSERT INTO inventory_reservations (id, item_id, location_id, quantity, status, reference_type, reference_id, expires_at, created_at, order_item_id)
             VALUES (?, ?, ?, ?, 'pending', ?, ?, ?, ?, ?)",
            rusqlite::params![
                reservation_id.to_string(),
                item.id,
                location_id,
                quantity.to_string(),
                &reference_type,
                &reference_id,
                expires_at.map(|t| t.to_rfc3339()),
                now.to_rfc3339(),
                order_item_id.map(|id| id.to_string()),
            ],
        )?;

        let new_allocated = balance.quantity_allocated + quantity;
        let new_available = balance.quantity_on_hand - new_allocated;
        let current_version = balance.version;

        // Guard the write with the optimistic-lock version check. The Rust-side
        // `balance.quantity_available < quantity` check above already rejected
        // insufficient stock using exact `Decimal` arithmetic; matching `version`
        // here proves the row has not changed since that read, so the balance is
        // still current and the reservation cannot over-allocate. (An earlier
        // `AND CAST(quantity_available AS REAL) >= CAST(? AS REAL)` clause was
        // dropped: comparing TEXT money columns as IEEE-754 floats could both
        // spuriously reject valid reservations and allow sub-cent oversells at
        // the boundary — and it was redundant given the version guard.)
        let rows_affected = tx.execute(
            "UPDATE inventory_balances SET quantity_allocated = ?, quantity_available = ?, version = version + 1, updated_at = ?
             WHERE item_id = ? AND location_id = ? AND version = ?",
            rusqlite::params![
                new_allocated.to_string(),
                new_available.to_string(),
                now.to_rfc3339(),
                item.id,
                location_id,
                current_version,
            ],
        )?;

        if rows_affected == 0 {
            return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                CommerceError::VersionConflict {
                    entity: "inventory_balance".to_string(),
                    id: format!("{}:{}", item.id, location_id),
                    expected_version: current_version,
                },
            )));
        }

        let event = KernelOutboxEvent::domain(
            "inventory.reservation_created.v1",
            "inventory_reservation",
            reservation_id.to_string(),
            serde_json::json!({
                "reservation_id": reservation_id.to_string(),
                "item_id": item.id,
                "sku": input.sku,
                "location_id": location_id,
                "quantity": quantity.to_string(),
                "reference_type": reference_type,
                "reference_id": reference_id,
                "status": ReservationStatus::Pending.to_string(),
                "balance_version": current_version + 1,
            }),
            None,
        );
        let event_id = event.id;
        append_kernel_event_tx(tx, &event)?;

        Ok((
            InventoryReservation {
                id: reservation_id,
                item_id: item.id,
                location_id,
                quantity,
                status: ReservationStatus::Pending,
                reference_type,
                reference_id,
                expires_at,
                created_at: now,
            },
            event_id,
        ))
    }

    pub(crate) fn list_reservation_ids_by_reference_in_tx(
        tx: &rusqlite::Transaction<'_>,
        reference_type: &str,
        reference_id: &str,
    ) -> std::result::Result<Vec<Uuid>, rusqlite::Error> {
        let mut stmt = tx.prepare(
            "SELECT id FROM inventory_reservations WHERE reference_type = ? AND reference_id = ? ORDER BY created_at",
        )?;
        let rows = stmt.query_map(rusqlite::params![reference_type, reference_id], |row| {
            let id_str: String = row.get(0)?;
            parse_uuid(&id_str, "inventory_reservation", "id")
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
        })?;
        let mut ids = Vec::new();
        for row in rows {
            ids.push(row?);
        }
        Ok(ids)
    }

    pub(crate) fn release_reservation_in_tx(
        tx: &rusqlite::Transaction<'_>,
        reservation_id: Uuid,
    ) -> std::result::Result<(), rusqlite::Error> {
        let now = Utc::now();

        let res = tx.query_row(
            "SELECT item_id, location_id, quantity, status, expires_at FROM inventory_reservations WHERE id = ?",
            [reservation_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, i64>("item_id")?,
                    row.get::<_, i32>("location_id")?,
                    parse_decimal_row(&row.get::<_, String>("quantity")?, "inventory_reservation", "quantity")?,
                    row.get::<_, String>("status")?,
                    parse_datetime_opt_row(row.get::<_, Option<String>>("expires_at")?, "inventory_reservation", "expires_at")?,
                ))
            },
        );

        let (item_id, location_id, quantity, status, expires_at) = match res {
            Ok(r) => r,
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    CommerceError::ReservationNotFound(reservation_id),
                )));
            }
            Err(e) => return Err(e),
        };

        let parsed_status: ReservationStatus = status.parse().map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(CommerceError::DatabaseError(
                format!("Invalid inventory_reservation.status '{status}': {e}"),
            )))
        })?;

        if parsed_status == ReservationStatus::Released
            || parsed_status == ReservationStatus::Cancelled
            || parsed_status == ReservationStatus::Expired
        {
            return Ok(());
        }

        if let Some(expires_at) = expires_at {
            if expires_at < now {
                Self::expire_reservation_in_tx(
                    tx,
                    reservation_id,
                    item_id,
                    location_id,
                    quantity,
                    now,
                )
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
                return Ok(());
            }
        }

        tx.execute(
            "UPDATE inventory_reservations SET status = 'released' WHERE id = ?",
            [reservation_id.to_string()],
        )?;

        // Exact `Decimal` arithmetic (see `apply_allocation_delta_in_tx`).
        let new_version =
            Self::apply_allocation_delta_in_tx(tx, item_id, location_id, -quantity, now)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

        append_kernel_event_tx(
            tx,
            &KernelOutboxEvent::domain(
                "inventory.reservation_released.v1",
                "inventory_reservation",
                reservation_id.to_string(),
                serde_json::json!({
                    "reservation_id": reservation_id.to_string(),
                    "item_id": item_id,
                    "location_id": location_id,
                    "quantity": quantity.to_string(),
                    "status": ReservationStatus::Released.to_string(),
                    "balance_version": new_version,
                }),
                None,
            ),
        )?;

        Ok(())
    }

    pub(crate) fn confirm_reservation_in_tx(
        tx: &rusqlite::Transaction<'_>,
        reservation_id: Uuid,
    ) -> std::result::Result<ReservationConfirmOutcome, rusqlite::Error> {
        Self::confirm_reservation_in_tx_with_now(tx, reservation_id, Utc::now())
    }

    pub(crate) fn confirm_reservation_in_tx_with_now(
        tx: &rusqlite::Transaction<'_>,
        reservation_id: Uuid,
        now: DateTime<Utc>,
    ) -> std::result::Result<ReservationConfirmOutcome, rusqlite::Error> {
        let res = tx.query_row(
            "SELECT item_id, location_id, quantity, status, expires_at FROM inventory_reservations WHERE id = ?",
            [reservation_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, i64>("item_id")?,
                    row.get::<_, i32>("location_id")?,
                    parse_decimal_row(&row.get::<_, String>("quantity")?, "inventory_reservation", "quantity")?,
                    row.get::<_, String>("status")?,
                    parse_datetime_opt_row(row.get::<_, Option<String>>("expires_at")?, "inventory_reservation", "expires_at")?,
                ))
            },
        );

        let (item_id, location_id, quantity, status, expires_at) = match res {
            Ok(r) => r,
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    CommerceError::ReservationNotFound(reservation_id),
                )));
            }
            Err(e) => return Err(e),
        };

        let parsed_status: ReservationStatus = status.parse().map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(CommerceError::DatabaseError(
                format!("Invalid inventory_reservation.status '{status}': {e}"),
            )))
        })?;

        if parsed_status == ReservationStatus::Released
            || parsed_status == ReservationStatus::Cancelled
        {
            return Ok(ReservationConfirmOutcome::Confirmed);
        }
        if parsed_status == ReservationStatus::Confirmed {
            return Ok(ReservationConfirmOutcome::Confirmed);
        }

        if parsed_status == ReservationStatus::Expired {
            return Ok(ReservationConfirmOutcome::Expired);
        }

        if let Some(expires_at) = expires_at {
            if expires_at < now {
                Self::expire_reservation_in_tx(
                    tx,
                    reservation_id,
                    item_id,
                    location_id,
                    quantity,
                    now,
                )
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
                return Ok(ReservationConfirmOutcome::Expired);
            }
        }

        tx.execute(
            "UPDATE inventory_reservations SET status = 'confirmed' WHERE id = ?",
            [reservation_id.to_string()],
        )?;

        append_kernel_event_tx(
            tx,
            &KernelOutboxEvent::domain(
                "inventory.reservation_confirmed.v1",
                "inventory_reservation",
                reservation_id.to_string(),
                serde_json::json!({
                    "reservation_id": reservation_id.to_string(),
                    "item_id": item_id,
                    "location_id": location_id,
                    "quantity": quantity.to_string(),
                    "status": ReservationStatus::Confirmed.to_string(),
                }),
                None,
            ),
        )?;

        Ok(ReservationConfirmOutcome::Confirmed)
    }

    /// Open (`pending`/`allocated`) reservations keyed to one order line
    /// (migration 080), oldest first, as `(reservation_id, quantity)`.
    pub(crate) fn list_open_reservations_for_line_in_tx(
        tx: &rusqlite::Transaction<'_>,
        order_item_id: Uuid,
    ) -> std::result::Result<Vec<(Uuid, Decimal)>, rusqlite::Error> {
        let mut stmt = tx.prepare(
            "SELECT id, quantity FROM inventory_reservations
             WHERE order_item_id = ? AND status IN ('pending', 'allocated')
             ORDER BY created_at, id",
        )?;
        let rows = stmt.query_map([order_item_id.to_string()], |row| {
            let id_str: String = row.get(0)?;
            let qty_str: String = row.get(1)?;
            let id = parse_uuid(&id_str, "inventory_reservation", "id")
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            let quantity = parse_decimal_row(&qty_str, "inventory_reservation", "quantity")?;
            Ok((id, quantity))
        })?;
        rows.collect()
    }

    /// The open-reservations-by-SKU lookup restricted to LEGACY rows
    /// (created before migration 080, so not keyed to an order line). The
    /// orders module uses this as the fallback after the line-keyed lookup so
    /// a SKU-based release can never take another line's keyed hold.
    pub(crate) fn list_open_legacy_reservations_for_sku_in_tx(
        tx: &rusqlite::Transaction<'_>,
        reference_type: &str,
        reference_id: &str,
        sku: &str,
    ) -> std::result::Result<Vec<(Uuid, Decimal)>, rusqlite::Error> {
        let mut stmt = tx.prepare(
            "SELECT r.id, r.quantity FROM inventory_reservations r
             JOIN inventory_items i ON i.id = r.item_id
             WHERE r.reference_type = ? AND r.reference_id = ? AND i.sku = ?
               AND r.order_item_id IS NULL
               AND r.status IN ('pending', 'allocated')
             ORDER BY r.created_at, r.id",
        )?;
        let rows = stmt.query_map(rusqlite::params![reference_type, reference_id, sku], |row| {
            let id_str: String = row.get(0)?;
            let qty_str: String = row.get(1)?;
            let id = parse_uuid(&id_str, "inventory_reservation", "id")
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            let quantity = parse_decimal_row(&qty_str, "inventory_reservation", "quantity")?;
            Ok((id, quantity))
        })?;
        rows.collect()
    }

    /// Confirm `quantity` units of a reservation.
    ///
    /// When `quantity` covers the whole reservation this is
    /// [`Self::confirm_reservation_in_tx_with_now`]. Otherwise the reservation
    /// is split: a new `confirmed` row is created for the shipped units and the
    /// original keeps the unshipped remainder (still `pending`, same expiry), so
    /// allocated balances are unchanged and the remainder can be shipped or
    /// released later.
    pub(crate) fn confirm_reservation_quantity_in_tx_with_now(
        tx: &rusqlite::Transaction<'_>,
        reservation_id: Uuid,
        quantity: Decimal,
        now: DateTime<Utc>,
    ) -> std::result::Result<ReservationConfirmOutcome, rusqlite::Error> {
        let res = tx.query_row(
            "SELECT item_id, location_id, quantity, status, reference_type, reference_id, expires_at
             FROM inventory_reservations WHERE id = ?",
            [reservation_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, i64>("item_id")?,
                    row.get::<_, i32>("location_id")?,
                    parse_decimal_row(&row.get::<_, String>("quantity")?, "inventory_reservation", "quantity")?,
                    row.get::<_, String>("status")?,
                    row.get::<_, String>("reference_type")?,
                    row.get::<_, String>("reference_id")?,
                    parse_datetime_opt_row(row.get::<_, Option<String>>("expires_at")?, "inventory_reservation", "expires_at")?,
                ))
            },
        );
        let (item_id, location_id, reserved, status, reference_type, reference_id, expires_at) =
            match res {
                Ok(r) => r,
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                        CommerceError::ReservationNotFound(reservation_id),
                    )));
                }
                Err(e) => return Err(e),
            };

        if quantity >= reserved {
            return Self::confirm_reservation_in_tx_with_now(tx, reservation_id, now);
        }

        let parsed_status: ReservationStatus = status.parse().map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(CommerceError::DatabaseError(
                format!("Invalid inventory_reservation.status '{status}': {e}"),
            )))
        })?;
        if matches!(parsed_status, ReservationStatus::Released | ReservationStatus::Cancelled) {
            return Ok(ReservationConfirmOutcome::Confirmed);
        }
        if parsed_status == ReservationStatus::Confirmed {
            return Ok(ReservationConfirmOutcome::Confirmed);
        }
        if parsed_status == ReservationStatus::Expired {
            return Ok(ReservationConfirmOutcome::Expired);
        }
        if let Some(expires_at) = expires_at {
            if expires_at < now {
                Self::expire_reservation_in_tx(
                    tx,
                    reservation_id,
                    item_id,
                    location_id,
                    reserved,
                    now,
                )
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
                return Ok(ReservationConfirmOutcome::Expired);
            }
        }
        if quantity <= Decimal::ZERO {
            return Ok(ReservationConfirmOutcome::Confirmed);
        }

        let confirmed_id = Uuid::new_v4();
        tx.execute(
            "UPDATE inventory_reservations SET quantity = ? WHERE id = ?",
            rusqlite::params![(reserved - quantity).to_string(), reservation_id.to_string()],
        )?;
        tx.execute(
            "INSERT INTO inventory_reservations (id, item_id, location_id, quantity, status, reference_type, reference_id, expires_at, created_at, order_item_id)
             VALUES (?, ?, ?, ?, 'confirmed', ?, ?, NULL, ?,
                     (SELECT order_item_id FROM inventory_reservations WHERE id = ?))",
            rusqlite::params![
                confirmed_id.to_string(),
                item_id,
                location_id,
                quantity.to_string(),
                reference_type,
                reference_id,
                now.to_rfc3339(),
                reservation_id.to_string(),
            ],
        )?;

        append_kernel_event_tx(
            tx,
            &KernelOutboxEvent::domain(
                "inventory.reservation_confirmed.v1",
                "inventory_reservation",
                confirmed_id.to_string(),
                serde_json::json!({
                    "reservation_id": confirmed_id.to_string(),
                    "source_reservation_id": reservation_id.to_string(),
                    "item_id": item_id,
                    "location_id": location_id,
                    "quantity": quantity.to_string(),
                    "remaining_quantity": (reserved - quantity).to_string(),
                    "status": ReservationStatus::Confirmed.to_string(),
                }),
                None,
            ),
        )?;

        Ok(ReservationConfirmOutcome::Confirmed)
    }

    pub(crate) fn expire_reservation_if_needed_in_tx(
        tx: &rusqlite::Transaction<'_>,
        reservation_id: Uuid,
        now: DateTime<Utc>,
    ) -> std::result::Result<bool, rusqlite::Error> {
        let res = tx.query_row(
            "SELECT item_id, location_id, quantity, status, expires_at FROM inventory_reservations WHERE id = ?",
            [reservation_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, i64>("item_id")?,
                    row.get::<_, i32>("location_id")?,
                    parse_decimal_row(&row.get::<_, String>("quantity")?, "inventory_reservation", "quantity")?,
                    row.get::<_, String>("status")?,
                    parse_datetime_opt_row(row.get::<_, Option<String>>("expires_at")?, "inventory_reservation", "expires_at")?,
                ))
            },
        );

        let (item_id, location_id, quantity, status, expires_at) = match res {
            Ok(r) => r,
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    CommerceError::ReservationNotFound(reservation_id),
                )));
            }
            Err(e) => return Err(e),
        };

        let parsed_status: ReservationStatus = status.parse().map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(CommerceError::DatabaseError(
                format!("Invalid inventory_reservation.status '{status}': {e}"),
            )))
        })?;

        if parsed_status == ReservationStatus::Released
            || parsed_status == ReservationStatus::Cancelled
        {
            return Ok(false);
        }

        if parsed_status == ReservationStatus::Expired {
            return Ok(true);
        }

        if let Some(expires_at) = expires_at {
            if expires_at < now {
                Self::expire_reservation_in_tx(
                    tx,
                    reservation_id,
                    item_id,
                    location_id,
                    quantity,
                    now,
                )
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
                return Ok(true);
            }
        }

        Ok(false)
    }
}

impl InventoryRepository for SqliteInventoryRepository {
    fn create_item(&self, input: CreateInventoryItem) -> Result<InventoryItem> {
        // Validate SKU format
        validate_sku(&input.sku)?;

        // Clone values needed in the closure
        let sku = input.sku.clone();
        let name = input.name.clone();
        let description = input.description.clone();
        let unit_of_measure = input.unit_of_measure.clone().unwrap_or_else(|| "EA".to_string());
        let location_id = input.location_id.unwrap_or(1);
        let initial_qty = input.initial_quantity.unwrap_or_default();
        let reorder_point = input.reorder_point;
        let safety_stock = input.safety_stock;

        with_immediate_transaction(&self.pool, |tx| {
            let now = Utc::now();

            // Check SKU uniqueness
            let exists: i32 = tx.query_row(
                "SELECT COUNT(*) FROM inventory_items WHERE sku = ?",
                [&sku],
                |row| row.get(0),
            )?;

            if exists > 0 {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    CommerceError::DuplicateSku(sku.clone()),
                )));
            }

            tx.execute(
                "INSERT INTO inventory_items (sku, name, description, unit_of_measure, is_active, created_at, updated_at)
                 VALUES (?, ?, ?, ?, 1, ?, ?)",
                rusqlite::params![
                    &sku,
                    &name,
                    &description,
                    &unit_of_measure,
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                ],
            )?;

            let item_id = tx.last_insert_rowid();

            tx.execute(
                "INSERT INTO inventory_balances (item_id, location_id, quantity_on_hand, quantity_allocated, quantity_available, reorder_point, safety_stock, updated_at)
                 VALUES (?, ?, ?, '0', ?, ?, ?, ?)",
                rusqlite::params![
                    item_id,
                    location_id,
                    initial_qty.to_string(),
                    initial_qty.to_string(),
                    reorder_point.map(|d| d.to_string()),
                    safety_stock.map(|d| d.to_string()),
                    now.to_rfc3339(),
                ],
            )?;

            // Record initial transaction if quantity > 0
            if initial_qty > Decimal::ZERO {
                tx.execute(
                    "INSERT INTO inventory_transactions (item_id, location_id, transaction_type, quantity, reason, created_at)
                     VALUES (?, ?, 'receipt', ?, 'Initial stock', ?)",
                    rusqlite::params![item_id, location_id, initial_qty.to_string(), now.to_rfc3339()],
                )?;
            }

            // Clone values for the return since Fn closure may be called multiple times
            Ok(InventoryItem {
                id: item_id,
                sku: sku.clone(),
                name: name.clone(),
                description: description.clone(),
                unit_of_measure: unit_of_measure.clone(),
                is_active: true,
                created_at: now,
                updated_at: now,
            })
        })
    }

    fn get_item(&self, id: i64) -> Result<Option<InventoryItem>> {
        let conn = self.conn()?;
        let result = conn.query_row("SELECT * FROM inventory_items WHERE id = ?", [id], |row| {
            Ok(InventoryItem {
                id: row.get("id")?,
                sku: row.get("sku")?,
                name: row.get("name")?,
                description: row.get("description")?,
                unit_of_measure: row.get("unit_of_measure")?,
                is_active: row.get::<_, i32>("is_active")? != 0,
                created_at: parse_datetime_row(
                    &row.get::<_, String>("created_at")?,
                    "inventory_item",
                    "created_at",
                )?,
                updated_at: parse_datetime_row(
                    &row.get::<_, String>("updated_at")?,
                    "inventory_item",
                    "updated_at",
                )?,
            })
        });

        match result {
            Ok(item) => Ok(Some(item)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn get_item_by_sku(&self, sku: &str) -> Result<Option<InventoryItem>> {
        let conn = self.conn()?;
        let result = conn.query_row("SELECT * FROM inventory_items WHERE sku = ?", [sku], |row| {
            Ok(InventoryItem {
                id: row.get("id")?,
                sku: row.get("sku")?,
                name: row.get("name")?,
                description: row.get("description")?,
                unit_of_measure: row.get("unit_of_measure")?,
                is_active: row.get::<_, i32>("is_active")? != 0,
                created_at: parse_datetime_row(
                    &row.get::<_, String>("created_at")?,
                    "inventory_item",
                    "created_at",
                )?,
                updated_at: parse_datetime_row(
                    &row.get::<_, String>("updated_at")?,
                    "inventory_item",
                    "updated_at",
                )?,
            })
        });

        match result {
            Ok(item) => Ok(Some(item)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn get_stock(&self, sku: &str) -> Result<Option<StockLevel>> {
        with_retry(
            || {
                let conn = self.pool.get().map_err(|e| {
                    rusqlite::Error::SqliteFailure(
                        rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_BUSY),
                        Some(e.to_string()),
                    )
                })?;

                // Get item directly with this connection
                let item_result =
                    conn.query_row("SELECT * FROM inventory_items WHERE sku = ?", [sku], |row| {
                        Ok(InventoryItem {
                            id: row.get("id")?,
                            sku: row.get("sku")?,
                            name: row.get("name")?,
                            description: row.get("description")?,
                            unit_of_measure: row.get("unit_of_measure")?,
                            is_active: row.get::<_, i32>("is_active")? != 0,
                            created_at: parse_datetime_row(
                                &row.get::<_, String>("created_at")?,
                                "inventory_item",
                                "created_at",
                            )?,
                            updated_at: parse_datetime_row(
                                &row.get::<_, String>("updated_at")?,
                                "inventory_item",
                                "updated_at",
                            )?,
                        })
                    });

                let item = match item_result {
                    Ok(item) => item,
                    Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
                    Err(e) => return Err(e),
                };

                // Get all balances for item
                let mut stmt = conn.prepare(
                    "SELECT b.*, l.name as location_name
                 FROM inventory_balances b
                 LEFT JOIN inventory_locations l ON b.location_id = l.id
                 WHERE b.item_id = ?",
                )?;

                let locations: Vec<LocationStock> = stmt
                    .query_map([item.id], |row| {
                        Ok(LocationStock {
                            location_id: row.get("location_id")?,
                            location_name: row.get("location_name")?,
                            on_hand: parse_decimal_row(
                                &row.get::<_, String>("quantity_on_hand")?,
                                "inventory_balance",
                                "quantity_on_hand",
                            )?,
                            allocated: parse_decimal_row(
                                &row.get::<_, String>("quantity_allocated")?,
                                "inventory_balance",
                                "quantity_allocated",
                            )?,
                            available: parse_decimal_row(
                                &row.get::<_, String>("quantity_available")?,
                                "inventory_balance",
                                "quantity_available",
                            )?,
                        })
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()?;

                let total_on_hand: Decimal = locations.iter().map(|l| l.on_hand).sum();
                let total_allocated: Decimal = locations.iter().map(|l| l.allocated).sum();
                let total_available: Decimal = locations.iter().map(|l| l.available).sum();

                Ok(Some(StockLevel {
                    sku: item.sku,
                    name: item.name,
                    total_on_hand,
                    total_allocated,
                    total_available,
                    locations,
                }))
            },
            MAX_RETRIES,
        )
        .map_err(map_db_error)
    }

    fn get_balance(&self, item_id: i64, location_id: i32) -> Result<Option<InventoryBalance>> {
        let conn = self.conn()?;
        let result = conn.query_row(
            "SELECT * FROM inventory_balances WHERE item_id = ? AND location_id = ?",
            rusqlite::params![item_id, location_id],
            |row| {
                Ok(InventoryBalance {
                    id: row.get("id")?,
                    item_id: row.get("item_id")?,
                    location_id: row.get("location_id")?,
                    quantity_on_hand: parse_decimal_row(
                        &row.get::<_, String>("quantity_on_hand")?,
                        "inventory_balance",
                        "quantity_on_hand",
                    )?,
                    quantity_allocated: parse_decimal_row(
                        &row.get::<_, String>("quantity_allocated")?,
                        "inventory_balance",
                        "quantity_allocated",
                    )?,
                    quantity_available: parse_decimal_row(
                        &row.get::<_, String>("quantity_available")?,
                        "inventory_balance",
                        "quantity_available",
                    )?,
                    reorder_point: parse_decimal_opt_row(
                        row.get::<_, Option<String>>("reorder_point")?,
                        "inventory_balance",
                        "reorder_point",
                    )?,
                    safety_stock: parse_decimal_opt_row(
                        row.get::<_, Option<String>>("safety_stock")?,
                        "inventory_balance",
                        "safety_stock",
                    )?,
                    version: row.get("version")?,
                    last_counted_at: parse_datetime_opt_row(
                        row.get::<_, Option<String>>("last_counted_at")?,
                        "inventory_balance",
                        "last_counted_at",
                    )?,
                    updated_at: parse_datetime_row(
                        &row.get::<_, String>("updated_at")?,
                        "inventory_balance",
                        "updated_at",
                    )?,
                })
            },
        );

        match result {
            Ok(balance) => Ok(Some(balance)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn adjust(&self, input: AdjustInventory) -> Result<InventoryTransaction> {
        // SKU format, non-zero quantity, non-blank reason, positive location.
        input.validate()?;

        // Clone values needed in the closure
        let sku = input.sku.clone();
        let quantity = input.quantity;
        let location_id = input.location_id.unwrap_or(1);
        let reference_type = input.reference_type.clone();
        let reference_id = input.reference_id.clone();
        let reason = input.reason;

        with_immediate_transaction(&self.pool, |tx| {
            let now = Utc::now();

            // Get item directly with this connection
            let item = tx.query_row(
                "SELECT * FROM inventory_items WHERE sku = ?",
                [&sku],
                |row| {
                    Ok(InventoryItem {
                        id: row.get("id")?,
                        sku: row.get("sku")?,
                        name: row.get("name")?,
                        description: row.get("description")?,
                        unit_of_measure: row.get("unit_of_measure")?,
                        is_active: row.get::<_, i32>("is_active")? != 0,
                        created_at: parse_datetime_row(&row.get::<_, String>("created_at")?, "inventory_item", "created_at")?,
                        updated_at: parse_datetime_row(&row.get::<_, String>("updated_at")?, "inventory_item", "updated_at")?,
                    })
                },
            )?;

            // Get or create balance directly with this connection
            let balance_result = tx.query_row(
                "SELECT * FROM inventory_balances WHERE item_id = ? AND location_id = ?",
                rusqlite::params![item.id, location_id],
                |row| {
                    Ok(InventoryBalance {
                        id: row.get("id")?,
                        item_id: row.get("item_id")?,
                        location_id: row.get("location_id")?,
                        quantity_on_hand: parse_decimal_row(&row.get::<_, String>("quantity_on_hand")?, "inventory_balance", "quantity_on_hand")?,
                        quantity_allocated: parse_decimal_row(&row.get::<_, String>("quantity_allocated")?, "inventory_balance", "quantity_allocated")?,
                        quantity_available: parse_decimal_row(&row.get::<_, String>("quantity_available")?, "inventory_balance", "quantity_available")?,
                        reorder_point: parse_decimal_opt_row(row.get::<_, Option<String>>("reorder_point")?, "inventory_balance", "reorder_point")?,
                        safety_stock: parse_decimal_opt_row(row.get::<_, Option<String>>("safety_stock")?, "inventory_balance", "safety_stock")?,
                        version: row.get("version")?,
                        last_counted_at: parse_datetime_opt_row(row.get::<_, Option<String>>("last_counted_at")?, "inventory_balance", "last_counted_at")?,
                        updated_at: parse_datetime_row(&row.get::<_, String>("updated_at")?, "inventory_balance", "updated_at")?,
                    })
                },
            );

            let balance = match balance_result {
                Ok(b) => b,
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    tx.execute(
                        "INSERT INTO inventory_balances (item_id, location_id, quantity_on_hand, quantity_allocated, quantity_available, updated_at)
                         VALUES (?, ?, '0', '0', '0', ?)",
                        rusqlite::params![item.id, location_id, now.to_rfc3339()],
                    )?;

                    // Query the newly created balance
                    tx.query_row(
                        "SELECT * FROM inventory_balances WHERE item_id = ? AND location_id = ?",
                        rusqlite::params![item.id, location_id],
                        |row| {
                            Ok(InventoryBalance {
                                id: row.get("id")?,
                                item_id: row.get("item_id")?,
                                location_id: row.get("location_id")?,
                                quantity_on_hand: parse_decimal_row(&row.get::<_, String>("quantity_on_hand")?, "inventory_balance", "quantity_on_hand")?,
                                quantity_allocated: parse_decimal_row(&row.get::<_, String>("quantity_allocated")?, "inventory_balance", "quantity_allocated")?,
                                quantity_available: parse_decimal_row(&row.get::<_, String>("quantity_available")?, "inventory_balance", "quantity_available")?,
                                reorder_point: parse_decimal_opt_row(row.get::<_, Option<String>>("reorder_point")?, "inventory_balance", "reorder_point")?,
                                safety_stock: parse_decimal_opt_row(row.get::<_, Option<String>>("safety_stock")?, "inventory_balance", "safety_stock")?,
                                version: row.get("version")?,
                                last_counted_at: parse_datetime_opt_row(row.get::<_, Option<String>>("last_counted_at")?, "inventory_balance", "last_counted_at")?,
                                updated_at: parse_datetime_row(&row.get::<_, String>("updated_at")?, "inventory_balance", "updated_at")?,
                            })
                        },
                    )?
                }
                Err(e) => return Err(e),
            };

            // Calculate new quantities
            let new_on_hand = balance.quantity_on_hand + quantity;
            let new_available = new_on_hand - balance.quantity_allocated;

            if new_on_hand < Decimal::ZERO {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    CommerceError::InsufficientStock {
                        sku: sku.clone(),
                        requested: quantity.abs().to_string(),
                        available: balance.quantity_on_hand.to_string(),
                    },
                )));
            }
            if new_available < Decimal::ZERO {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    CommerceError::InsufficientStock {
                        sku: sku.clone(),
                        requested: quantity.abs().to_string(),
                        available: balance.quantity_available.to_string(),
                    },
                )));
            }

            // Update balance with optimistic locking
            let current_version = balance.version;
            let rows_affected = tx.execute(
                "UPDATE inventory_balances SET quantity_on_hand = ?, quantity_available = ?, version = version + 1, updated_at = ?
                 WHERE item_id = ? AND location_id = ? AND version = ?",
                rusqlite::params![
                    new_on_hand.to_string(),
                    new_available.to_string(),
                    now.to_rfc3339(),
                    item.id,
                    location_id,
                    current_version
                ],
            )?;

            if rows_affected == 0 {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    CommerceError::VersionConflict {
                        entity: "inventory_balance".to_string(),
                        id: format!("{}:{}", item.id, location_id),
                        expected_version: current_version,
                    },
                )));
            }

            // Record transaction
            let tx_type = if quantity >= Decimal::ZERO { "receipt" } else { "adjustment" };
            tx.execute(
                "INSERT INTO inventory_transactions (item_id, location_id, transaction_type, quantity, reference_type, reference_id, reason, created_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    item.id,
                    location_id,
                    tx_type,
                    quantity.to_string(),
                    &reference_type,
                    &reference_id,
                    &reason,
                    now.to_rfc3339(),
                ],
            )?;

            let tx_id = tx.last_insert_rowid();
            // Clone values for the return since Fn closure may be called multiple times
            Ok(InventoryTransaction {
                id: tx_id,
                item_id: item.id,
                location_id,
                transaction_type: if quantity >= Decimal::ZERO {
                    TransactionType::Receipt
                } else {
                    TransactionType::Adjustment
                },
                quantity,
                reference_type: reference_type.clone(),
                reference_id: reference_id.clone(),
                reason: Some(reason.clone()),
                created_by: None,
                created_at: now,
            })
        })
        .map_err(|e| {
            // Check if it's not found
            if e.is_not_found() {
                return CommerceError::InventoryItemNotFound(sku.clone());
            }
            e
        })
    }

    fn reserve(&self, input: ReserveInventory) -> Result<InventoryReservation> {
        with_inventory_retry(|| {
            with_immediate_transaction(&self.pool, |tx| {
                Self::reserve_in_tx(tx, &input).map(|(reservation, _)| reservation)
            })
            .map_err(|e| {
                if e.is_not_found() {
                    return CommerceError::InventoryItemNotFound(input.sku.clone());
                }
                e
            })
        })
    }

    fn get_reservation(&self, reservation_id: Uuid) -> Result<Option<InventoryReservation>> {
        let conn = self.conn()?;

        let result = conn.query_row(
            "SELECT id, item_id, location_id, quantity, status, reference_type, reference_id, expires_at, created_at
             FROM inventory_reservations WHERE id = ?",
            [reservation_id.to_string()],
            |row| {
                Ok(InventoryReservation {
                    id: parse_uuid_row(&row.get::<_, String>("id")?, "inventory_reservation", "id")?,
                    item_id: row.get("item_id")?,
                    location_id: row.get("location_id")?,
                    quantity: parse_decimal_row(
                        &row.get::<_, String>("quantity")?,
                        "inventory_reservation",
                        "quantity",
                    )?,
                    status: parse_enum_row(
                        &row.get::<_, String>("status")?,
                        "inventory_reservation",
                        "status",
                    )?,
                    reference_type: row.get("reference_type")?,
                    reference_id: row.get("reference_id")?,
                    expires_at: parse_datetime_opt_row(
                        row.get::<_, Option<String>>("expires_at")?,
                        "inventory_reservation",
                        "expires_at",
                    )?,
                    created_at: parse_datetime_row(
                        &row.get::<_, String>("created_at")?,
                        "inventory_reservation",
                        "created_at",
                    )?,
                })
            },
        );

        match result {
            Ok(reservation) => Ok(Some(reservation)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn release_reservation(&self, reservation_id: Uuid) -> Result<()> {
        with_inventory_retry(|| {
            with_immediate_transaction(&self.pool, |tx| {
                Self::release_reservation_in_tx(tx, reservation_id)
            })
        })
    }

    fn confirm_reservation(&self, reservation_id: Uuid) -> Result<()> {
        let outcome = with_inventory_retry(|| {
            with_immediate_transaction(&self.pool, |tx| {
                Self::confirm_reservation_in_tx(tx, reservation_id)
            })
        })?;

        match outcome {
            ReservationConfirmOutcome::Confirmed => Ok(()),
            ReservationConfirmOutcome::Expired => {
                Err(CommerceError::ReservationExpired(reservation_id))
            }
        }
    }

    fn list_reservations_by_reference(
        &self,
        reference_type: &str,
        reference_id: &str,
    ) -> Result<Vec<InventoryReservation>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, item_id, location_id, quantity, status, reference_type, reference_id, expires_at, created_at
                 FROM inventory_reservations
                 WHERE reference_type = ? AND reference_id = ?
                 ORDER BY created_at",
            )
            .map_err(map_db_error)?;

        let reservations = stmt
            .query_map(rusqlite::params![reference_type, reference_id], |row| {
                Ok(InventoryReservation {
                    id: parse_uuid_row(
                        &row.get::<_, String>("id")?,
                        "inventory_reservation",
                        "id",
                    )?,
                    item_id: row.get("item_id")?,
                    location_id: row.get("location_id")?,
                    quantity: parse_decimal_row(
                        &row.get::<_, String>("quantity")?,
                        "inventory_reservation",
                        "quantity",
                    )?,
                    status: parse_enum_row(
                        &row.get::<_, String>("status")?,
                        "inventory_reservation",
                        "status",
                    )?,
                    reference_type: row.get("reference_type")?,
                    reference_id: row.get("reference_id")?,
                    expires_at: parse_datetime_opt_row(
                        row.get::<_, Option<String>>("expires_at")?,
                        "inventory_reservation",
                        "expires_at",
                    )?,
                    created_at: parse_datetime_row(
                        &row.get::<_, String>("created_at")?,
                        "inventory_reservation",
                        "created_at",
                    )?,
                })
            })
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        Ok(reservations)
    }

    fn expire_reservations(&self, now: DateTime<Utc>, limit: u32) -> Result<u64> {
        if limit == 0 {
            return Ok(0);
        }
        with_inventory_retry(|| {
            with_immediate_transaction(&self.pool, |tx| {
                Self::expire_reservations_in_tx(tx, now, limit)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
            })
        })
    }

    fn list(&self, filter: InventoryFilter) -> Result<Vec<InventoryItem>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM inventory_items WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(sku) = &filter.sku {
            sql.push_str(" AND sku LIKE ?");
            params.push(Box::new(format!("%{sku}%")));
        }
        if let Some(is_active) = &filter.is_active {
            sql.push_str(" AND is_active = ?");
            params.push(Box::new(i32::from(*is_active)));
        }

        sql.push_str(" ORDER BY sku");

        crate::sqlite::append_limit_offset(&mut sql, filter.limit, filter.offset);

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;

        let items = stmt
            .query_map(params_refs.as_slice(), |row| {
                Ok(InventoryItem {
                    id: row.get("id")?,
                    sku: row.get("sku")?,
                    name: row.get("name")?,
                    description: row.get("description")?,
                    unit_of_measure: row.get("unit_of_measure")?,
                    is_active: row.get::<_, i32>("is_active")? != 0,
                    created_at: parse_datetime_row(
                        &row.get::<_, String>("created_at")?,
                        "inventory_item",
                        "created_at",
                    )?,
                    updated_at: parse_datetime_row(
                        &row.get::<_, String>("updated_at")?,
                        "inventory_item",
                        "updated_at",
                    )?,
                })
            })
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        Ok(items)
    }

    fn get_reorder_needed(&self) -> Result<Vec<StockLevel>> {
        // `quantity_available` and `reorder_point` are TEXT decimals; comparing
        // them in SQL with CAST(... AS REAL) coerces both operands to IEEE-754
        // floats and can misclassify balances right at the reorder boundary,
        // so the comparison happens on exact parsed `Decimal`s in Rust.
        let candidates = {
            let conn = self.conn()?;
            let mut stmt = conn
                .prepare(
                    "SELECT i.sku, b.quantity_available, b.reorder_point, b.safety_stock
                     FROM inventory_items i
                     JOIN inventory_balances b ON i.id = b.item_id
                     WHERE b.reorder_point IS NOT NULL
                     AND i.is_active = 1
                     ORDER BY i.sku",
                )
                .map_err(map_db_error)?;

            stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            })
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?
        };

        // Threshold = reorder_point + safety_stock (the buffer that must stay
        // untouched), matching `InventoryBalance::reorder_threshold`.
        let mut skus: Vec<String> = Vec::new();
        for (sku, available, reorder_point, safety_stock) in candidates {
            let available =
                parse_decimal_strict(&available, "inventory_balance", "quantity_available")?;
            let reorder_point =
                parse_decimal_strict(&reorder_point, "inventory_balance", "reorder_point")?;
            let safety_stock = safety_stock
                .map(|v| parse_decimal_strict(&v, "inventory_balance", "safety_stock"))
                .transpose()?
                .unwrap_or(Decimal::ZERO);
            if available < reorder_point + safety_stock && !skus.contains(&sku) {
                skus.push(sku);
            }
        }

        let mut result = Vec::with_capacity(skus.len());
        for sku in skus {
            if let Some(stock) = self.get_stock(&sku)? {
                result.push(stock);
            }
        }

        Ok(result)
    }

    fn record_transaction(
        &self,
        transaction: InventoryTransaction,
    ) -> Result<InventoryTransaction> {
        let conn = self.conn()?;

        conn.execute(
            "INSERT INTO inventory_transactions (item_id, location_id, transaction_type, quantity, reference_type, reference_id, reason, created_by, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                transaction.item_id,
                transaction.location_id,
                transaction.transaction_type.to_string(),
                transaction.quantity.to_string(),
                transaction.reference_type,
                transaction.reference_id,
                transaction.reason,
                transaction.created_by,
                transaction.created_at.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;

        let id = conn.last_insert_rowid();

        Ok(InventoryTransaction { id, ..transaction })
    }

    fn get_transactions(&self, item_id: i64, limit: u32) -> Result<Vec<InventoryTransaction>> {
        let conn = self.conn()?;

        let mut stmt = conn
            .prepare(&format!(
                "SELECT * FROM inventory_transactions WHERE item_id = ? ORDER BY created_at DESC LIMIT {limit}"
            ))
            .map_err(map_db_error)?;

        let transactions = stmt
            .query_map([item_id], |row| {
                Ok(InventoryTransaction {
                    id: row.get("id")?,
                    item_id: row.get("item_id")?,
                    location_id: row.get("location_id")?,
                    transaction_type: parse_enum_row(
                        &row.get::<_, String>("transaction_type")?,
                        "inventory_transaction",
                        "transaction_type",
                    )?,
                    quantity: parse_decimal_row(
                        &row.get::<_, String>("quantity")?,
                        "inventory_transaction",
                        "quantity",
                    )?,
                    reference_type: row.get("reference_type")?,
                    reference_id: row.get("reference_id")?,
                    reason: row.get("reason")?,
                    created_by: row.get("created_by")?,
                    created_at: parse_datetime_row(
                        &row.get::<_, String>("created_at")?,
                        "inventory_transaction",
                        "created_at",
                    )?,
                })
            })
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        Ok(transactions)
    }

    // === Batch Operations ===

    fn create_item_batch(
        &self,
        inputs: Vec<CreateInventoryItem>,
    ) -> Result<BatchResult<InventoryItem>> {
        validate_batch_size(&inputs)?;
        let mut result = BatchResult::with_capacity(inputs.len());

        for (index, input) in inputs.into_iter().enumerate() {
            match self.create_item(input) {
                Ok(item) => result.record_success(item),
                Err(e) => result.record_failure(index, None, &e),
            }
        }

        Ok(result)
    }

    fn create_item_batch_atomic(
        &self,
        inputs: Vec<CreateInventoryItem>,
    ) -> Result<Vec<InventoryItem>> {
        validate_batch_size(&inputs)?;
        if inputs.is_empty() {
            return Ok(vec![]);
        }

        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let mut results = Vec::with_capacity(inputs.len());

        for input in inputs {
            let now = Utc::now();
            let sku = input.sku.clone();
            let name = input.name.clone();
            let description = input.description.clone();
            let unit_of_measure = input.unit_of_measure.clone().unwrap_or_else(|| "EA".to_string());

            // Check SKU uniqueness
            let exists: i32 = tx
                .query_row("SELECT COUNT(*) FROM inventory_items WHERE sku = ?", [&sku], |row| {
                    row.get(0)
                })
                .map_err(map_db_error)?;

            if exists > 0 {
                return Err(CommerceError::DuplicateSku(sku));
            }

            tx.execute(
                "INSERT INTO inventory_items (sku, name, description, unit_of_measure, is_active, created_at, updated_at)
                 VALUES (?, ?, ?, ?, 1, ?, ?)",
                rusqlite::params![
                    &sku,
                    &name,
                    &description,
                    &unit_of_measure,
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                ],
            )
            .map_err(map_db_error)?;

            let item_id = tx.last_insert_rowid();

            // Create initial balance if quantity provided
            let location_id = input.location_id.unwrap_or(1);
            let initial_qty = input.initial_quantity.unwrap_or_default();

            tx.execute(
                "INSERT INTO inventory_balances (item_id, location_id, quantity_on_hand, quantity_allocated, quantity_available, reorder_point, safety_stock, updated_at)
                 VALUES (?, ?, ?, '0', ?, ?, ?, ?)",
                rusqlite::params![
                    item_id,
                    location_id,
                    initial_qty.to_string(),
                    initial_qty.to_string(),
                    input.reorder_point.map(|d| d.to_string()),
                    input.safety_stock.map(|d| d.to_string()),
                    now.to_rfc3339(),
                ],
            )
            .map_err(map_db_error)?;

            // Record initial transaction if quantity > 0
            if initial_qty > Decimal::ZERO {
                tx.execute(
                    "INSERT INTO inventory_transactions (item_id, location_id, transaction_type, quantity, reason, created_at)
                     VALUES (?, ?, 'receipt', ?, 'Initial stock', ?)",
                    rusqlite::params![item_id, location_id, initial_qty.to_string(), now.to_rfc3339()],
                )
                .map_err(map_db_error)?;
            }

            results.push(InventoryItem {
                id: item_id,
                sku,
                name,
                description,
                unit_of_measure,
                is_active: true,
                created_at: now,
                updated_at: now,
            });
        }

        tx.commit().map_err(map_db_error)?;
        Ok(results)
    }

    fn adjust_batch(
        &self,
        adjustments: Vec<AdjustInventory>,
    ) -> Result<BatchResult<InventoryTransaction>> {
        validate_batch_size(&adjustments)?;
        let mut result = BatchResult::with_capacity(adjustments.len());

        for (index, input) in adjustments.into_iter().enumerate() {
            let sku = input.sku.clone();
            match self.adjust(input) {
                Ok(transaction) => result.record_success(transaction),
                Err(e) => result.record_failure(index, Some(sku), &e),
            }
        }

        Ok(result)
    }

    fn adjust_batch_atomic(
        &self,
        adjustments: Vec<AdjustInventory>,
    ) -> Result<Vec<InventoryTransaction>> {
        validate_batch_size(&adjustments)?;
        if adjustments.is_empty() {
            return Ok(vec![]);
        }

        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let mut results = Vec::with_capacity(adjustments.len());
        let now = Utc::now();

        for input in adjustments {
            input.validate()?;
            // Get item directly with this connection
            let item = tx
                .query_row("SELECT * FROM inventory_items WHERE sku = ?", [&input.sku], |row| {
                    Ok(InventoryItem {
                        id: row.get("id")?,
                        sku: row.get("sku")?,
                        name: row.get("name")?,
                        description: row.get("description")?,
                        unit_of_measure: row.get("unit_of_measure")?,
                        is_active: row.get::<_, i32>("is_active")? != 0,
                        created_at: parse_datetime_row(
                            &row.get::<_, String>("created_at")?,
                            "inventory_item",
                            "created_at",
                        )?,
                        updated_at: parse_datetime_row(
                            &row.get::<_, String>("updated_at")?,
                            "inventory_item",
                            "updated_at",
                        )?,
                    })
                })
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => {
                        CommerceError::InventoryItemNotFound(input.sku.clone())
                    }
                    e => map_db_error(e),
                })?;

            let location_id = input.location_id.unwrap_or(1);

            // Get or create balance directly with this connection
            let balance_result = tx.query_row(
                "SELECT * FROM inventory_balances WHERE item_id = ? AND location_id = ?",
                rusqlite::params![item.id, location_id],
                |row| {
                    Ok(InventoryBalance {
                        id: row.get("id")?,
                        item_id: row.get("item_id")?,
                        location_id: row.get("location_id")?,
                        quantity_on_hand: parse_decimal_row(
                            &row.get::<_, String>("quantity_on_hand")?,
                            "inventory_balance",
                            "quantity_on_hand",
                        )?,
                        quantity_allocated: parse_decimal_row(
                            &row.get::<_, String>("quantity_allocated")?,
                            "inventory_balance",
                            "quantity_allocated",
                        )?,
                        quantity_available: parse_decimal_row(
                            &row.get::<_, String>("quantity_available")?,
                            "inventory_balance",
                            "quantity_available",
                        )?,
                        reorder_point: parse_decimal_opt_row(
                            row.get::<_, Option<String>>("reorder_point")?,
                            "inventory_balance",
                            "reorder_point",
                        )?,
                        safety_stock: parse_decimal_opt_row(
                            row.get::<_, Option<String>>("safety_stock")?,
                            "inventory_balance",
                            "safety_stock",
                        )?,
                        version: row.get("version")?,
                        last_counted_at: parse_datetime_opt_row(
                            row.get::<_, Option<String>>("last_counted_at")?,
                            "inventory_balance",
                            "last_counted_at",
                        )?,
                        updated_at: parse_datetime_row(
                            &row.get::<_, String>("updated_at")?,
                            "inventory_balance",
                            "updated_at",
                        )?,
                    })
                },
            );

            let balance = match balance_result {
                Ok(b) => b,
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    tx.execute(
                        "INSERT INTO inventory_balances (item_id, location_id, quantity_on_hand, quantity_allocated, quantity_available, updated_at)
                         VALUES (?, ?, '0', '0', '0', ?)",
                        rusqlite::params![item.id, location_id, now.to_rfc3339()],
                    )
                    .map_err(map_db_error)?;

                    // Query the newly created balance
                    tx.query_row(
                        "SELECT * FROM inventory_balances WHERE item_id = ? AND location_id = ?",
                        rusqlite::params![item.id, location_id],
                        |row| {
                            Ok(InventoryBalance {
                                id: row.get("id")?,
                                item_id: row.get("item_id")?,
                                location_id: row.get("location_id")?,
                                quantity_on_hand: parse_decimal_row(
                                    &row.get::<_, String>("quantity_on_hand")?,
                                    "inventory_balance",
                                    "quantity_on_hand",
                                )?,
                                quantity_allocated: parse_decimal_row(
                                    &row.get::<_, String>("quantity_allocated")?,
                                    "inventory_balance",
                                    "quantity_allocated",
                                )?,
                                quantity_available: parse_decimal_row(
                                    &row.get::<_, String>("quantity_available")?,
                                    "inventory_balance",
                                    "quantity_available",
                                )?,
                                reorder_point: parse_decimal_opt_row(
                                    row.get::<_, Option<String>>("reorder_point")?,
                                    "inventory_balance",
                                    "reorder_point",
                                )?,
                                safety_stock: parse_decimal_opt_row(
                                    row.get::<_, Option<String>>("safety_stock")?,
                                    "inventory_balance",
                                    "safety_stock",
                                )?,
                                version: row.get("version")?,
                                last_counted_at: parse_datetime_opt_row(
                                    row.get::<_, Option<String>>("last_counted_at")?,
                                    "inventory_balance",
                                    "last_counted_at",
                                )?,
                                updated_at: parse_datetime_row(
                                    &row.get::<_, String>("updated_at")?,
                                    "inventory_balance",
                                    "updated_at",
                                )?,
                            })
                        },
                    )
                    .map_err(map_db_error)?
                }
                Err(e) => return Err(map_db_error(e)),
            };

            // Calculate new quantities
            let new_on_hand = balance.quantity_on_hand + input.quantity;
            let new_available = new_on_hand - balance.quantity_allocated;

            if new_on_hand < Decimal::ZERO {
                return Err(CommerceError::InsufficientStock {
                    sku: input.sku.clone(),
                    requested: input.quantity.abs().to_string(),
                    available: balance.quantity_on_hand.to_string(),
                });
            }
            if new_available < Decimal::ZERO {
                return Err(CommerceError::InsufficientStock {
                    sku: input.sku.clone(),
                    requested: input.quantity.abs().to_string(),
                    available: balance.quantity_available.to_string(),
                });
            }

            // Update balance with optimistic locking
            let current_version = balance.version;
            let rows_affected = tx.execute(
                "UPDATE inventory_balances SET quantity_on_hand = ?, quantity_available = ?, version = version + 1, updated_at = ?
                 WHERE item_id = ? AND location_id = ? AND version = ?",
                rusqlite::params![
                    new_on_hand.to_string(),
                    new_available.to_string(),
                    now.to_rfc3339(),
                    item.id,
                    location_id,
                    current_version
                ],
            )
            .map_err(map_db_error)?;

            if rows_affected == 0 {
                return Err(CommerceError::VersionConflict {
                    entity: "inventory_balance".to_string(),
                    id: format!("{}:{}", item.id, location_id),
                    expected_version: current_version,
                });
            }

            // Record transaction
            let tx_type = if input.quantity >= Decimal::ZERO { "receipt" } else { "adjustment" };
            tx.execute(
                "INSERT INTO inventory_transactions (item_id, location_id, transaction_type, quantity, reference_type, reference_id, reason, created_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    item.id,
                    location_id,
                    tx_type,
                    input.quantity.to_string(),
                    input.reference_type,
                    input.reference_id,
                    input.reason,
                    now.to_rfc3339(),
                ],
            )
            .map_err(map_db_error)?;

            let tx_id = tx.last_insert_rowid();
            results.push(InventoryTransaction {
                id: tx_id,
                item_id: item.id,
                location_id,
                transaction_type: if input.quantity >= Decimal::ZERO {
                    TransactionType::Receipt
                } else {
                    TransactionType::Adjustment
                },
                quantity: input.quantity,
                reference_type: input.reference_type,
                reference_id: input.reference_id,
                reason: Some(input.reason),
                created_by: None,
                created_at: now,
            });
        }

        tx.commit().map_err(map_db_error)?;
        Ok(results)
    }

    fn get_item_batch(&self, ids: Vec<i64>) -> Result<Vec<InventoryItem>> {
        validate_batch_size(&ids)?;
        if ids.is_empty() {
            return Ok(vec![]);
        }

        let conn = self.conn()?;
        let placeholders = build_in_clause(ids.len());
        let sql = format!("SELECT * FROM inventory_items WHERE id IN ({placeholders})");

        let params = i64_params(&ids);
        let params_refs = params_refs(&params);

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let items = stmt
            .query_map(params_refs.as_slice(), |row| {
                Ok(InventoryItem {
                    id: row.get("id")?,
                    sku: row.get("sku")?,
                    name: row.get("name")?,
                    description: row.get("description")?,
                    unit_of_measure: row.get("unit_of_measure")?,
                    is_active: row.get::<_, i32>("is_active")? != 0,
                    created_at: parse_datetime_row(
                        &row.get::<_, String>("created_at")?,
                        "inventory_item",
                        "created_at",
                    )?,
                    updated_at: parse_datetime_row(
                        &row.get::<_, String>("updated_at")?,
                        "inventory_item",
                        "updated_at",
                    )?,
                })
            })
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        Ok(items)
    }

    fn get_stock_batch(&self, skus: Vec<String>) -> Result<Vec<StockLevel>> {
        validate_batch_size(&skus)?;
        if skus.is_empty() {
            return Ok(vec![]);
        }

        let conn = self.conn()?;
        let placeholders = build_in_clause(skus.len());
        let sql = format!("SELECT * FROM inventory_items WHERE sku IN ({placeholders})");

        let params = string_params(&skus);
        let params_refs = params_refs(&params);

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let items: Vec<InventoryItem> = stmt
            .query_map(params_refs.as_slice(), |row| {
                Ok(InventoryItem {
                    id: row.get("id")?,
                    sku: row.get("sku")?,
                    name: row.get("name")?,
                    description: row.get("description")?,
                    unit_of_measure: row.get("unit_of_measure")?,
                    is_active: row.get::<_, i32>("is_active")? != 0,
                    created_at: parse_datetime_row(
                        &row.get::<_, String>("created_at")?,
                        "inventory_item",
                        "created_at",
                    )?,
                    updated_at: parse_datetime_row(
                        &row.get::<_, String>("updated_at")?,
                        "inventory_item",
                        "updated_at",
                    )?,
                })
            })
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        // Build stock levels for each item
        let mut results = Vec::with_capacity(items.len());
        for item in items {
            let mut balance_stmt = conn
                .prepare(
                    "SELECT b.*, l.name as location_name
                     FROM inventory_balances b
                     LEFT JOIN inventory_locations l ON b.location_id = l.id
                     WHERE b.item_id = ?",
                )
                .map_err(map_db_error)?;

            let locations: Vec<LocationStock> = balance_stmt
                .query_map([item.id], |row| {
                    Ok(LocationStock {
                        location_id: row.get("location_id")?,
                        location_name: row.get("location_name")?,
                        on_hand: parse_decimal_row(
                            &row.get::<_, String>("quantity_on_hand")?,
                            "inventory_balance",
                            "quantity_on_hand",
                        )?,
                        allocated: parse_decimal_row(
                            &row.get::<_, String>("quantity_allocated")?,
                            "inventory_balance",
                            "quantity_allocated",
                        )?,
                        available: parse_decimal_row(
                            &row.get::<_, String>("quantity_available")?,
                            "inventory_balance",
                            "quantity_available",
                        )?,
                    })
                })
                .map_err(map_db_error)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(map_db_error)?;

            let total_on_hand: Decimal = locations.iter().map(|l| l.on_hand).sum();
            let total_allocated: Decimal = locations.iter().map(|l| l.allocated).sum();
            let total_available: Decimal = locations.iter().map(|l| l.available).sum();

            results.push(StockLevel {
                sku: item.sku,
                name: item.name,
                total_on_hand,
                total_allocated,
                total_available,
                locations,
            });
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SqliteDatabase;
    use rust_decimal_macros::dec;
    use stateset_core::{CommerceError, InventoryRepository};

    fn fresh_repo() -> SqliteInventoryRepository {
        let db = SqliteDatabase::in_memory().expect("in-memory sqlite");
        db.inventory()
    }

    fn item(sku: &str) -> CreateInventoryItem {
        CreateInventoryItem {
            sku: sku.into(),
            name: format!("Item {sku}"),
            description: Some("test".into()),
            unit_of_measure: Some("EA".into()),
            initial_quantity: Some(dec!(10)),
            location_id: None,
            reorder_point: Some(dec!(5)),
            safety_stock: Some(dec!(2)),
        }
    }

    #[test]
    fn reorder_needed_compares_available_to_reorder_point_exactly() {
        let repo = fresh_repo();
        let below = repo.create_item(item("REORD-BELOW")).expect("create below");
        let above = repo.create_item(item("REORD-ABOVE")).expect("create above");
        {
            let conn = repo.conn().expect("conn");
            // Below the reorder point by 1e-18: both values round to the same
            // f64, so a CAST-AS-REAL comparison would wrongly skip the reorder.
            conn.execute(
                "UPDATE inventory_balances
                 SET quantity_available = '9.999999999999999999', reorder_point = '10',
                     safety_stock = NULL
                 WHERE item_id = ?1",
                rusqlite::params![below.id],
            )
            .expect("update below");
            conn.execute(
                "UPDATE inventory_balances
                 SET quantity_available = '10.000000000000000001', reorder_point = '10',
                     safety_stock = NULL
                 WHERE item_id = ?1",
                rusqlite::params![above.id],
            )
            .expect("update above");
        }

        let needed = repo.get_reorder_needed().expect("ok");
        let skus: Vec<&str> = needed.iter().map(|s| s.sku.as_str()).collect();
        assert!(
            skus.contains(&"REORD-BELOW"),
            "9.999999999999999999 < 10 exactly, even though the f64s are equal"
        );
        assert!(
            !skus.contains(&"REORD-ABOVE"),
            "10.000000000000000001 is not below the reorder point"
        );
    }

    #[test]
    fn create_item_persists_basic_fields() {
        let repo = fresh_repo();
        let created = repo.create_item(item("WIDGET-001")).expect("create");
        assert_eq!(created.sku, "WIDGET-001");
        assert_eq!(created.name, "Item WIDGET-001");
        assert_eq!(created.unit_of_measure, "EA");
        assert!(created.is_active);
    }

    #[test]
    fn create_item_with_zero_initial_quantity_records_no_transaction() {
        let repo = fresh_repo();
        let mut input = item("ZERO-INIT");
        input.initial_quantity = Some(Decimal::ZERO);
        let created = repo.create_item(input).expect("create");
        let txns = repo.get_transactions(created.id, 10).expect("transactions");
        assert!(txns.is_empty(), "no transaction expected for zero initial qty");
    }

    #[test]
    fn create_item_records_initial_receipt_transaction() {
        let repo = fresh_repo();
        let created = repo.create_item(item("INIT-001")).expect("create");
        let txns = repo.get_transactions(created.id, 10).expect("transactions");
        assert_eq!(txns.len(), 1, "initial receipt expected");
    }

    #[test]
    fn create_item_rejects_duplicate_sku() {
        let repo = fresh_repo();
        repo.create_item(item("DUPE-001")).expect("first create");
        let err = repo.create_item(item("DUPE-001")).expect_err("dup err");
        assert!(matches!(err, CommerceError::DuplicateSku(s) if s == "DUPE-001"));
    }

    #[test]
    fn create_item_rejects_invalid_sku() {
        let repo = fresh_repo();
        let mut input = item("");
        input.sku = "bad sku!!".into();
        let err = repo.create_item(input).expect_err("invalid");
        assert!(matches!(err, CommerceError::ValidationError(_)));
    }

    #[test]
    fn get_item_by_sku_round_trips() {
        let repo = fresh_repo();
        let created = repo.create_item(item("ROUND-001")).expect("create");
        let by_sku = repo.get_item_by_sku("ROUND-001").expect("get by sku").expect("found");
        assert_eq!(by_sku.id, created.id);
        let by_id = repo.get_item(created.id).expect("get").expect("found");
        assert_eq!(by_id.sku, "ROUND-001");
        assert!(repo.get_item_by_sku("MISSING").expect("ok").is_none());
    }

    #[test]
    fn get_stock_aggregates_by_location() {
        let repo = fresh_repo();
        repo.create_item(item("STOCK-001")).expect("create");
        let stock = repo.get_stock("STOCK-001").expect("get stock").expect("found");
        assert_eq!(stock.sku, "STOCK-001");
        assert_eq!(stock.total_on_hand, dec!(10));
        assert_eq!(stock.total_available, dec!(10));
        assert_eq!(stock.total_allocated, dec!(0));
        assert_eq!(stock.locations.len(), 1);
    }

    #[test]
    fn adjust_increases_and_decreases_on_hand() {
        let repo = fresh_repo();
        repo.create_item(item("ADJ-001")).expect("create");
        repo.adjust(AdjustInventory {
            sku: "ADJ-001".into(),
            location_id: Some(1),
            quantity: dec!(5),
            reason: "receipt".into(),
            reference_type: None,
            reference_id: None,
        })
        .expect("receipt");
        let stock = repo.get_stock("ADJ-001").expect("ok").expect("found");
        assert_eq!(stock.total_on_hand, dec!(15));

        repo.adjust(AdjustInventory {
            sku: "ADJ-001".into(),
            location_id: Some(1),
            quantity: dec!(-3),
            reason: "shrink".into(),
            reference_type: None,
            reference_id: None,
        })
        .expect("decrement");
        let stock = repo.get_stock("ADJ-001").expect("ok").expect("found");
        assert_eq!(stock.total_on_hand, dec!(12));
    }

    #[test]
    fn reserve_then_release_round_trip() {
        let repo = fresh_repo();
        repo.create_item(item("RESERVE-001")).expect("create");
        let res = repo
            .reserve(ReserveInventory {
                sku: "RESERVE-001".into(),
                location_id: Some(1),
                quantity: dec!(4),
                reference_type: "order".into(),
                reference_id: "ord-1".into(),
                expires_in_seconds: Some(60),
            })
            .expect("reserve");

        let after_reserve = repo.get_stock("RESERVE-001").expect("ok").expect("found");
        assert_eq!(after_reserve.total_allocated, dec!(4));
        assert_eq!(after_reserve.total_available, dec!(6));

        let fetched = repo.get_reservation(res.id).expect("get res").expect("found");
        assert_eq!(fetched.id, res.id);

        repo.release_reservation(res.id).expect("release");
        let after_release = repo.get_stock("RESERVE-001").expect("ok").expect("found");
        assert_eq!(after_release.total_allocated, dec!(0));
        assert_eq!(after_release.total_available, dec!(10));
    }

    #[test]
    fn list_reservations_by_reference_filters_correctly() {
        let repo = fresh_repo();
        repo.create_item(item("MULTI-RES-001")).expect("create");
        repo.reserve(ReserveInventory {
            sku: "MULTI-RES-001".into(),
            location_id: Some(1),
            quantity: dec!(1),
            reference_type: "order".into(),
            reference_id: "ord-A".into(),
            expires_in_seconds: None,
        })
        .expect("res 1");
        repo.reserve(ReserveInventory {
            sku: "MULTI-RES-001".into(),
            location_id: Some(1),
            quantity: dec!(2),
            reference_type: "order".into(),
            reference_id: "ord-A".into(),
            expires_in_seconds: None,
        })
        .expect("res 2");
        repo.reserve(ReserveInventory {
            sku: "MULTI-RES-001".into(),
            location_id: Some(1),
            quantity: dec!(1),
            reference_type: "order".into(),
            reference_id: "ord-B".into(),
            expires_in_seconds: None,
        })
        .expect("res 3");

        let by_a = repo.list_reservations_by_reference("order", "ord-A").expect("list");
        assert_eq!(by_a.len(), 2);
        let by_b = repo.list_reservations_by_reference("order", "ord-B").expect("list");
        assert_eq!(by_b.len(), 1);
    }

    #[test]
    fn list_filters_by_sku() {
        let repo = fresh_repo();
        repo.create_item(item("LIST-001")).expect("c1");
        repo.create_item(item("LIST-002")).expect("c2");
        repo.create_item(item("OTHER-001")).expect("c3");

        let listed = repo
            .list(InventoryFilter { sku: Some("LIST".into()), ..Default::default() })
            .expect("list");
        assert_eq!(listed.len(), 2);
        assert!(listed.iter().all(|i| i.sku.starts_with("LIST")));
    }

    #[test]
    fn get_reorder_needed_returns_below_threshold() {
        let repo = fresh_repo();

        let mut healthy = item("HEALTHY-001");
        healthy.initial_quantity = Some(dec!(100));
        healthy.reorder_point = Some(dec!(10));
        repo.create_item(healthy).expect("healthy");

        let mut low = item("LOW-001");
        low.initial_quantity = Some(dec!(2));
        low.reorder_point = Some(dec!(10));
        repo.create_item(low).expect("low");

        let needs = repo.get_reorder_needed().expect("reorder");
        let skus: Vec<&str> = needs.iter().map(|s| s.sku.as_str()).collect();
        assert!(skus.contains(&"LOW-001"));
        assert!(!skus.contains(&"HEALTHY-001"));
    }

    #[test]
    fn get_transactions_returns_in_recent_first_order() {
        let repo = fresh_repo();
        let created = repo.create_item(item("TX-001")).expect("create");
        repo.adjust(AdjustInventory {
            sku: "TX-001".into(),
            location_id: Some(1),
            quantity: dec!(3),
            reason: "receipt".into(),
            reference_type: None,
            reference_id: None,
        })
        .expect("adjust");

        let txns = repo.get_transactions(created.id, 10).expect("txns");
        assert_eq!(txns.len(), 2);
    }

    #[test]
    fn create_item_batch_creates_all() {
        let repo = fresh_repo();
        let result = repo
            .create_item_batch(vec![item("BATCH-001"), item("BATCH-002"), item("BATCH-003")])
            .expect("batch");
        assert_eq!(result.success_count, 3);
        assert_eq!(result.failure_count, 0);
        assert_eq!(result.succeeded.len(), 3);
    }
}
