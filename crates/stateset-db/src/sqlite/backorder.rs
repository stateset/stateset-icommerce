//! SQLite implementation of backorder repository

use chrono::{DateTime, Utc};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::OptionalExtension;
use rust_decimal::Decimal;
use stateset_core::{
    AllocateBackorder, AllocationStatus, Backorder, BackorderAllocation, BackorderFilter,
    BackorderFulfillment, BackorderRepository, BackorderStatus, BackorderSummary, CommerceError,
    CreateBackorder, FulfillBackorder, FulfillmentSourceType, ReserveInventory, Result,
    SkuBackorderSummary, UpdateBackorder, generate_backorder_number,
};
use uuid::Uuid;

use super::inventory::{ReservationConfirmOutcome, SqliteInventoryRepository};
use super::{
    map_db_error, parse_datetime_opt, parse_datetime_opt_row, parse_datetime_row,
    parse_decimal_row, parse_decimal_strict, parse_enum_row, parse_uuid, parse_uuid_opt_row,
    parse_uuid_row, sum_decimal_query, with_immediate_transaction,
};

/// `reference_type` of the inventory reservation that backs an allocation.
pub(crate) const BACKORDER_RESERVATION_REFERENCE: &str = "backorder";

const ALLOCATION_COLUMNS: &str = "id, backorder_id, sku, quantity, location_id, lot_id, status, allocated_at, expires_at, reservation_id";

fn to_sql_err(e: CommerceError) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(e))
}

/// An open allocation row as needed by the release/consume paths.
struct OpenAllocation {
    id: Uuid,
    quantity: Decimal,
    reservation_id: Option<Uuid>,
}

fn open_allocations_in_tx(
    tx: &rusqlite::Transaction<'_>,
    backorder_id: Uuid,
) -> rusqlite::Result<Vec<OpenAllocation>> {
    let mut stmt = tx.prepare(
        "SELECT id, quantity, reservation_id FROM backorder_allocations
         WHERE backorder_id = ? AND status IN ('reserved', 'confirmed')
         ORDER BY allocated_at, id",
    )?;
    let rows = stmt.query_map([backorder_id.to_string()], |row| {
        Ok(OpenAllocation {
            id: parse_uuid_row(&row.get::<_, String>(0)?, "backorder_allocation", "id")?,
            quantity: parse_decimal_row(
                &row.get::<_, String>(1)?,
                "backorder_allocation",
                "quantity",
            )?,
            reservation_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>(2)?,
                "backorder_allocation",
                "reservation_id",
            )?,
        })
    })?;
    rows.collect()
}

/// Release the inventory reservations behind every open allocation of the
/// given backorders and mark the allocations `released`. Shared by cancel
/// (single, per order, per order line).
fn release_open_allocations_in_tx(
    tx: &rusqlite::Transaction<'_>,
    backorder_ids: &[Uuid],
) -> rusqlite::Result<()> {
    for backorder_id in backorder_ids {
        for allocation in open_allocations_in_tx(tx, *backorder_id)? {
            if let Some(reservation_id) = allocation.reservation_id {
                SqliteInventoryRepository::release_reservation_in_tx(tx, reservation_id)?;
            }
            tx.execute(
                "UPDATE backorder_allocations SET status = 'released' WHERE id = ?",
                [allocation.id.to_string()],
            )?;
        }
    }
    Ok(())
}

fn backorder_ids_for_order_in_tx(
    tx: &rusqlite::Transaction<'_>,
    order_id: Uuid,
    order_line_id: Option<Uuid>,
) -> rusqlite::Result<Vec<Uuid>> {
    let mut ids = Vec::new();
    let mut collect = |row: &rusqlite::Row<'_>| -> rusqlite::Result<()> {
        let id: String = row.get(0)?;
        ids.push(parse_uuid(&id, "backorder", "id").map_err(to_sql_err)?);
        Ok(())
    };
    match order_line_id {
        Some(line) => {
            let mut stmt = tx.prepare(
                "SELECT id FROM backorders WHERE order_id = ? AND order_line_id = ?
                   AND status NOT IN ('fulfilled', 'cancelled')",
            )?;
            let mut rows = stmt.query([order_id.to_string(), line.to_string()])?;
            while let Some(row) = rows.next()? {
                collect(row)?;
            }
        }
        None => {
            let mut stmt = tx.prepare(
                "SELECT id FROM backorders WHERE order_id = ?
                   AND status NOT IN ('fulfilled', 'cancelled')",
            )?;
            let mut rows = stmt.query([order_id.to_string()])?;
            while let Some(row) = rows.next()? {
                collect(row)?;
            }
        }
    }
    Ok(ids)
}

/// Units of a backorder already covered by open allocations.
fn open_allocated_quantity_in_tx(
    tx: &rusqlite::Transaction<'_>,
    backorder_id: Uuid,
) -> rusqlite::Result<Decimal> {
    Ok(open_allocations_in_tx(tx, backorder_id)?.iter().map(|a| a.quantity).sum())
}

/// Reserve `quantity` units of `sku` for a backorder and record the
/// allocation. The reservation is keyed `backorder:<id>` so it can never be
/// confused with an order/cart hold; its expiry mirrors the allocation's.
#[allow(clippy::too_many_arguments)]
fn allocate_in_tx(
    tx: &rusqlite::Transaction<'_>,
    backorder_id: Uuid,
    sku: &str,
    quantity: Decimal,
    location_id: i32,
    lot_id: Option<Uuid>,
    expires_at: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> rusqlite::Result<BackorderAllocation> {
    let expires_in_seconds = expires_at.map(|at| (at - now).num_seconds().max(1));
    let (reservation, _) = SqliteInventoryRepository::reserve_in_tx(
        tx,
        &ReserveInventory {
            sku: sku.to_string(),
            location_id: Some(location_id),
            quantity,
            reference_type: BACKORDER_RESERVATION_REFERENCE.to_string(),
            reference_id: backorder_id.to_string(),
            expires_in_seconds,
        },
    )?;

    let id = Uuid::new_v4();
    tx.execute(
        "INSERT INTO backorder_allocations (id, backorder_id, sku, quantity, location_id,
            lot_id, status, allocated_at, expires_at, reservation_id)
         VALUES (?, ?, ?, ?, ?, ?, 'reserved', ?, ?, ?)",
        rusqlite::params![
            id.to_string(),
            backorder_id.to_string(),
            sku,
            quantity.to_string(),
            location_id,
            lot_id.map(|id| id.to_string()),
            now.to_rfc3339(),
            expires_at.map(|d| d.to_rfc3339()),
            reservation.id.to_string(),
        ],
    )?;
    tx.execute(
        "UPDATE backorders SET status = 'allocated', updated_at = ?
         WHERE id = ? AND status = 'pending'",
        [now.to_rfc3339(), backorder_id.to_string()],
    )?;

    Ok(BackorderAllocation {
        id,
        backorder_id,
        sku: sku.to_string(),
        quantity,
        location_id: Some(location_id),
        lot_id,
        status: AllocationStatus::Reserved,
        allocated_at: now,
        expires_at,
        reservation_id: Some(reservation.id),
    })
}

/// If a backorder was flagged `allocated` and no open allocation remains,
/// drop it back to `pending` so it is picked up again by auto-allocation.
fn settle_backorder_allocation_status_in_tx(
    tx: &rusqlite::Transaction<'_>,
    backorder_id: Uuid,
    now: DateTime<Utc>,
) -> rusqlite::Result<()> {
    if open_allocations_in_tx(tx, backorder_id)?.is_empty() {
        tx.execute(
            "UPDATE backorders SET status = 'pending', updated_at = ?
             WHERE id = ? AND status = 'allocated'",
            [now.to_rfc3339(), backorder_id.to_string()],
        )?;
    }
    Ok(())
}

fn row_to_backorder_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Backorder> {
    Ok(Backorder {
        id: parse_uuid_row(&row.get::<_, String>(0)?, "backorder", "id")?,
        backorder_number: row.get(1)?,
        order_id: parse_uuid_row(&row.get::<_, String>(2)?, "backorder", "order_id")?,
        order_line_id: parse_uuid_opt_row(
            row.get::<_, Option<String>>(3)?,
            "backorder",
            "order_line_id",
        )?,
        customer_id: parse_uuid_row(&row.get::<_, String>(4)?, "backorder", "customer_id")?,
        sku: row.get(5)?,
        quantity_ordered: parse_decimal_row(
            &row.get::<_, String>(6)?,
            "backorder",
            "quantity_ordered",
        )?,
        quantity_fulfilled: parse_decimal_row(
            &row.get::<_, String>(7)?,
            "backorder",
            "quantity_fulfilled",
        )?,
        quantity_remaining: parse_decimal_row(
            &row.get::<_, String>(8)?,
            "backorder",
            "quantity_remaining",
        )?,
        status: parse_enum_row(&row.get::<_, String>(9)?, "backorder", "status")?,
        priority: parse_enum_row(&row.get::<_, String>(10)?, "backorder", "priority")?,
        expected_date: parse_datetime_opt_row(
            row.get::<_, Option<String>>(11)?,
            "backorder",
            "expected_date",
        )?,
        promised_date: parse_datetime_opt_row(
            row.get::<_, Option<String>>(12)?,
            "backorder",
            "promised_date",
        )?,
        source_location_id: row.get(13)?,
        notes: row.get(14)?,
        created_at: parse_datetime_row(&row.get::<_, String>(15)?, "backorder", "created_at")?,
        updated_at: parse_datetime_row(&row.get::<_, String>(16)?, "backorder", "updated_at")?,
    })
}

#[derive(Debug)]
pub struct SqliteBackorderRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteBackorderRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn row_to_backorder(&self, row: &rusqlite::Row<'_>) -> rusqlite::Result<Backorder> {
        row_to_backorder_row(row)
    }

    fn row_to_fulfillment(
        &self,
        row: &rusqlite::Row<'_>,
    ) -> rusqlite::Result<BackorderFulfillment> {
        Ok(BackorderFulfillment {
            id: parse_uuid_row(&row.get::<_, String>(0)?, "backorder_fulfillment", "id")?,
            backorder_id: parse_uuid_row(
                &row.get::<_, String>(1)?,
                "backorder_fulfillment",
                "backorder_id",
            )?,
            quantity: parse_decimal_row(
                &row.get::<_, String>(2)?,
                "backorder_fulfillment",
                "quantity",
            )?,
            source_type: parse_enum_row(
                &row.get::<_, String>(3)?,
                "backorder_fulfillment",
                "source_type",
            )?,
            source_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>(4)?,
                "backorder_fulfillment",
                "source_id",
            )?,
            notes: row.get(5)?,
            fulfilled_at: parse_datetime_row(
                &row.get::<_, String>(6)?,
                "backorder_fulfillment",
                "fulfilled_at",
            )?,
            fulfilled_by: row.get(7)?,
        })
    }

    fn row_to_allocation(&self, row: &rusqlite::Row<'_>) -> rusqlite::Result<BackorderAllocation> {
        Ok(BackorderAllocation {
            id: parse_uuid_row(&row.get::<_, String>(0)?, "backorder_allocation", "id")?,
            backorder_id: parse_uuid_row(
                &row.get::<_, String>(1)?,
                "backorder_allocation",
                "backorder_id",
            )?,
            sku: row.get(2)?,
            quantity: parse_decimal_row(
                &row.get::<_, String>(3)?,
                "backorder_allocation",
                "quantity",
            )?,
            location_id: row.get(4)?,
            lot_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>(5)?,
                "backorder_allocation",
                "lot_id",
            )?,
            status: parse_enum_row(&row.get::<_, String>(6)?, "backorder_allocation", "status")?,
            allocated_at: parse_datetime_row(
                &row.get::<_, String>(7)?,
                "backorder_allocation",
                "allocated_at",
            )?,
            expires_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>(8)?,
                "backorder_allocation",
                "expires_at",
            )?,
            reservation_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>(9)?,
                "backorder_allocation",
                "reservation_id",
            )?,
        })
    }

    fn get_allocation_in_tx(
        &self,
        tx: &rusqlite::Transaction<'_>,
        allocation_id: Uuid,
    ) -> rusqlite::Result<BackorderAllocation> {
        tx.query_row(
            &format!("SELECT {ALLOCATION_COLUMNS} FROM backorder_allocations WHERE id = ?"),
            [allocation_id.to_string()],
            |row| self.row_to_allocation(row),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => to_sql_err(CommerceError::NotFound),
            other => other,
        })
    }
}

pub(crate) fn create_backorder_in_tx(
    tx: &rusqlite::Transaction<'_>,
    input: &CreateBackorder,
) -> std::result::Result<Backorder, rusqlite::Error> {
    let id = Uuid::new_v4();
    let now = Utc::now();
    let backorder_number = generate_backorder_number();
    let priority = input.priority.unwrap_or_default();

    tx.execute(
        "INSERT INTO backorders (id, backorder_number, order_id, order_line_id, customer_id,
            sku, quantity_ordered, quantity_fulfilled, quantity_remaining, status, priority,
            expected_date, promised_date, source_location_id, notes, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, '0', ?, 'pending', ?, ?, ?, ?, ?, ?, ?)",
        rusqlite::params![
            id.to_string(),
            &backorder_number,
            input.order_id.to_string(),
            input.order_line_id.map(|id| id.to_string()),
            input.customer_id.to_string(),
            &input.sku,
            input.quantity.to_string(),
            input.quantity.to_string(),
            priority.to_string(),
            input.expected_date.map(|d| d.to_rfc3339()),
            input.promised_date.map(|d| d.to_rfc3339()),
            input.source_location_id,
            input.notes,
            now.to_rfc3339(),
            now.to_rfc3339(),
        ],
    )?;

    let row = tx.query_row(
        "SELECT id, backorder_number, order_id, order_line_id, customer_id, sku,
                quantity_ordered, quantity_fulfilled, quantity_remaining, status, priority,
                expected_date, promised_date, source_location_id, notes, created_at, updated_at
         FROM backorders WHERE id = ?",
        [id.to_string()],
        row_to_backorder_row,
    );

    match row {
        Ok(bo) => Ok(bo),
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            Err(rusqlite::Error::ToSqlConversionFailure(Box::new(CommerceError::NotFound)))
        }
        Err(e) => Err(e),
    }
}

pub(crate) fn cancel_backorders_for_order_in_tx(
    tx: &rusqlite::Transaction<'_>,
    order_id: Uuid,
) -> std::result::Result<(), rusqlite::Error> {
    let now = Utc::now();

    // Hand the allocated stock back before flipping status: every open
    // allocation is backed by an inventory reservation.
    let ids = backorder_ids_for_order_in_tx(tx, order_id, None)?;
    release_open_allocations_in_tx(tx, &ids)?;

    tx.execute(
        "UPDATE backorders SET status = 'cancelled', updated_at = ?
         WHERE order_id = ? AND status NOT IN ('fulfilled', 'cancelled')",
        [now.to_rfc3339(), order_id.to_string()],
    )?;

    Ok(())
}

/// Cancel every open backorder raised for one order line (used when the line
/// is removed from its order) and release any stock allocated against it.
pub(crate) fn cancel_backorders_for_order_line_in_tx(
    tx: &rusqlite::Transaction<'_>,
    order_id: Uuid,
    order_line_id: Uuid,
) -> std::result::Result<(), rusqlite::Error> {
    let now = Utc::now();

    let ids = backorder_ids_for_order_in_tx(tx, order_id, Some(order_line_id))?;
    release_open_allocations_in_tx(tx, &ids)?;

    tx.execute(
        "UPDATE backorders SET status = 'cancelled', updated_at = ?
         WHERE order_id = ? AND order_line_id = ?
           AND status NOT IN ('fulfilled', 'cancelled')",
        [now.to_rfc3339(), order_id.to_string(), order_line_id.to_string()],
    )?;

    Ok(())
}

impl SqliteBackorderRepository {
    /// Take `input.quantity` units out of stock for a fulfilment.
    ///
    /// Open allocations are consumed first (oldest first): their reservations
    /// are fulfilled, which decrements on-hand and allocated together and
    /// writes a `shipment` ledger row. Any remainder is then taken straight
    /// from available stock when `source_type` is `Inventory` and the SKU
    /// has an inventory master (`InsufficientStock` if it does not cover the
    /// remainder). For other sources (purchase order, transfer, production)
    /// the remainder is stock that arrives and ships through without ever
    /// being on hand, so no balance is touched; the same holds for SKUs
    /// without an inventory item.
    fn consume_stock_for_fulfilment_in_tx(
        tx: &rusqlite::Transaction<'_>,
        input: &FulfillBackorder,
        sku: &str,
        source_location_id: Option<i32>,
        now: DateTime<Utc>,
    ) -> rusqlite::Result<()> {
        let reason = format!("Backorder {} fulfilment", input.backorder_id);
        let mut remaining = input.quantity;
        for allocation in open_allocations_in_tx(tx, input.backorder_id)? {
            if remaining <= Decimal::ZERO {
                break;
            }
            let take = remaining.min(allocation.quantity);
            if let Some(reservation_id) = allocation.reservation_id {
                SqliteInventoryRepository::fulfil_reservation_in_tx(
                    tx,
                    reservation_id,
                    take,
                    &reason,
                    now,
                )
                .map_err(to_sql_err)?;
            }
            if take == allocation.quantity {
                tx.execute(
                    "UPDATE backorder_allocations SET status = 'fulfilled' WHERE id = ?",
                    [allocation.id.to_string()],
                )?;
            } else {
                tx.execute(
                    "UPDATE backorder_allocations SET quantity = ? WHERE id = ?",
                    rusqlite::params![
                        (allocation.quantity - take).to_string(),
                        allocation.id.to_string()
                    ],
                )?;
            }
            remaining -= take;
        }

        if remaining > Decimal::ZERO && input.source_type == FulfillmentSourceType::Inventory {
            let item_id: Option<i64> = tx
                .query_row("SELECT id FROM inventory_items WHERE sku = ?", [sku], |row| row.get(0))
                .optional()?;
            if let Some(item_id) = item_id {
                SqliteInventoryRepository::consume_available_in_tx(
                    tx,
                    item_id,
                    source_location_id.unwrap_or(1),
                    remaining,
                    BACKORDER_RESERVATION_REFERENCE,
                    &input.backorder_id.to_string(),
                    &reason,
                    now,
                )
                .map_err(to_sql_err)?;
            }
        }
        Ok(())
    }
}

impl BackorderRepository for SqliteBackorderRepository {
    fn create_backorder(&self, input: CreateBackorder) -> Result<Backorder> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let backorder_number = generate_backorder_number();
        let priority = input.priority.unwrap_or_default();

        {
            let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            conn.execute(
                "INSERT INTO backorders (id, backorder_number, order_id, order_line_id, customer_id,
                    sku, quantity_ordered, quantity_fulfilled, quantity_remaining, status, priority,
                    expected_date, promised_date, source_location_id, notes, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, '0', ?, 'pending', ?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    id.to_string(),
                    &backorder_number,
                    input.order_id.to_string(),
                    input.order_line_id.map(|id| id.to_string()),
                    input.customer_id.to_string(),
                    &input.sku,
                    input.quantity.to_string(),
                    input.quantity.to_string(), // quantity_remaining starts same as ordered
                    priority.to_string(),
                    input.expected_date.map(|d| d.to_rfc3339()),
                    input.promised_date.map(|d| d.to_rfc3339()),
                    input.source_location_id,
                    input.notes,
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                ],
            ).map_err(map_db_error)?;
        }

        self.get_backorder(id)?.ok_or(CommerceError::NotFound)
    }

    fn get_backorder(&self, id: Uuid) -> Result<Option<Backorder>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let result = conn.query_row(
            "SELECT id, backorder_number, order_id, order_line_id, customer_id, sku,
                    quantity_ordered, quantity_fulfilled, quantity_remaining, status, priority,
                    expected_date, promised_date, source_location_id, notes, created_at, updated_at
             FROM backorders WHERE id = ?",
            [id.to_string()],
            |row| self.row_to_backorder(row),
        );

        match result {
            Ok(bo) => Ok(Some(bo)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn get_backorder_by_number(&self, number: &str) -> Result<Option<Backorder>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let result = conn.query_row(
            "SELECT id, backorder_number, order_id, order_line_id, customer_id, sku,
                    quantity_ordered, quantity_fulfilled, quantity_remaining, status, priority,
                    expected_date, promised_date, source_location_id, notes, created_at, updated_at
             FROM backorders WHERE backorder_number = ?",
            [number],
            |row| self.row_to_backorder(row),
        );

        match result {
            Ok(bo) => Ok(Some(bo)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn update_backorder(&self, id: Uuid, input: UpdateBackorder) -> Result<Backorder> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let now = Utc::now();

        conn.execute(
            "UPDATE backorders SET
                priority = COALESCE(?, priority),
                expected_date = COALESCE(?, expected_date),
                promised_date = COALESCE(?, promised_date),
                source_location_id = COALESCE(?, source_location_id),
                notes = COALESCE(?, notes),
                updated_at = ?
             WHERE id = ?",
            rusqlite::params![
                input.priority.map(|p| p.to_string()),
                input.expected_date.map(|d| d.to_rfc3339()),
                input.promised_date.map(|d| d.to_rfc3339()),
                input.source_location_id,
                input.notes,
                now.to_rfc3339(),
                id.to_string(),
            ],
        )
        .map_err(map_db_error)?;

        self.get_backorder(id)?.ok_or(CommerceError::NotFound)
    }

    fn list_backorders(&self, filter: BackorderFilter) -> Result<Vec<Backorder>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut sql = String::from(
            "SELECT id, backorder_number, order_id, order_line_id, customer_id, sku,
                    quantity_ordered, quantity_fulfilled, quantity_remaining, status, priority,
                    expected_date, promised_date, source_location_id, notes, created_at, updated_at
             FROM backorders WHERE 1=1",
        );
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(ref order_id) = filter.order_id {
            sql.push_str(" AND order_id = ?");
            params.push(Box::new(order_id.to_string()));
        }
        if let Some(ref customer_id) = filter.customer_id {
            sql.push_str(" AND customer_id = ?");
            params.push(Box::new(customer_id.to_string()));
        }
        if let Some(ref sku) = filter.sku {
            sql.push_str(" AND sku = ?");
            params.push(Box::new(sku.clone()));
        }
        if let Some(ref status) = filter.status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(status.to_string()));
        }
        if let Some(ref priority) = filter.priority {
            sql.push_str(" AND priority = ?");
            params.push(Box::new(priority.to_string()));
        }

        // Order by priority (critical first) then by created_at
        sql.push_str(" ORDER BY CASE priority WHEN 'critical' THEN 1 WHEN 'high' THEN 2 WHEN 'normal' THEN 3 ELSE 4 END, created_at ASC");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {limit}"));
        }

        let param_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt
            .query_map(param_refs.as_slice(), |row| self.row_to_backorder(row))
            .map_err(map_db_error)?;

        let mut backorders = Vec::new();
        for row in rows {
            backorders.push(row.map_err(map_db_error)?);
        }
        Ok(backorders)
    }

    fn cancel_backorder(&self, id: Uuid) -> Result<Backorder> {
        // Status read, allocation release (inventory hand-back) and status
        // flip in one IMMEDIATE transaction. Idempotent for an already
        // cancelled backorder; a fulfilled one cannot be cancelled.
        with_immediate_transaction(&self.pool, |tx| {
            let now = Utc::now();
            let status_str: String = tx
                .query_row("SELECT status FROM backorders WHERE id = ?", [id.to_string()], |row| {
                    row.get(0)
                })
                .optional()?
                .ok_or_else(|| to_sql_err(CommerceError::NotFound))?;
            let status: BackorderStatus = parse_enum_row(&status_str, "backorder", "status")?;
            match status {
                BackorderStatus::Cancelled => return Ok(()),
                BackorderStatus::Fulfilled => {
                    return Err(to_sql_err(CommerceError::ValidationError(
                        "A fulfilled backorder cannot be cancelled".to_string(),
                    )));
                }
                _ => {}
            }

            release_open_allocations_in_tx(tx, &[id])?;
            tx.execute(
                "UPDATE backorders SET status = 'cancelled', updated_at = ? WHERE id = ?",
                [now.to_rfc3339(), id.to_string()],
            )?;
            Ok(())
        })?;

        self.get_backorder(id)?.ok_or(CommerceError::NotFound)
    }

    fn get_backorders_for_order(&self, order_id: Uuid) -> Result<Vec<Backorder>> {
        self.list_backorders(BackorderFilter { order_id: Some(order_id), ..Default::default() })
    }

    fn get_backorders_for_customer(&self, customer_id: Uuid) -> Result<Vec<Backorder>> {
        self.list_backorders(BackorderFilter {
            customer_id: Some(customer_id),
            ..Default::default()
        })
    }

    fn get_backorders_for_sku(&self, sku: &str) -> Result<Vec<Backorder>> {
        self.list_backorders(BackorderFilter { sku: Some(sku.to_string()), ..Default::default() })
    }

    fn fulfill_backorder(&self, input: FulfillBackorder) -> Result<Backorder> {
        if input.quantity <= Decimal::ZERO {
            return Err(CommerceError::ValidationError(
                "Fulfillment quantity must be greater than zero".to_string(),
            ));
        }

        let now = Utc::now();
        let fulfillment_id = Uuid::new_v4();
        let backorder_id_str = input.backorder_id.to_string();

        // The status/quantity read, the guards, the fulfillment INSERT and the
        // backorder UPDATE all run inside ONE `IMMEDIATE` transaction, so
        // concurrent fulfillments serialize (no lost update / over-fulfill) and a
        // failed UPDATE can't leave an orphaned fulfillment row.
        with_immediate_transaction(&self.pool, |tx| {
            let (status_str, remaining_str, fulfilled_str, sku, source_location_id): (
                String,
                String,
                String,
                String,
                Option<i32>,
            ) = tx
                .query_row(
                    "SELECT status, quantity_remaining, quantity_fulfilled, sku, source_location_id
                     FROM backorders WHERE id = ?",
                    [&backorder_id_str],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
                )
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(CommerceError::NotFound))
                    }
                    other => other,
                })?;
            let status: BackorderStatus = parse_enum_row(&status_str, "backorder", "status")?;
            let remaining = parse_decimal_row(&remaining_str, "backorder", "quantity_remaining")?;
            let fulfilled = parse_decimal_row(&fulfilled_str, "backorder", "quantity_fulfilled")?;

            // A cancelled or already-fulfilled backorder cannot be fulfilled.
            if matches!(status, BackorderStatus::Cancelled | BackorderStatus::Fulfilled) {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    CommerceError::ValidationError("Backorder cannot be fulfilled".to_string()),
                )));
            }
            // Cannot fulfill more units than remain.
            if input.quantity > remaining {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    CommerceError::ValidationError(format!(
                        "Cannot fulfill {} - only {} remaining",
                        input.quantity, remaining
                    )),
                )));
            }

            let new_fulfilled = fulfilled + input.quantity;
            let new_remaining = remaining - input.quantity;
            let new_status = if new_remaining <= Decimal::ZERO {
                BackorderStatus::Fulfilled
            } else {
                BackorderStatus::PartiallyFulfilled
            };

            // Consume the stock: allocations first (their reservations leave
            // on-hand and allocated together), then any remainder straight
            // from available stock when fulfilling from inventory.
            Self::consume_stock_for_fulfilment_in_tx(tx, &input, &sku, source_location_id, now)?;

            tx.execute(
                "INSERT INTO backorder_fulfillments (id, backorder_id, quantity, source_type,
                    source_id, notes, fulfilled_at, fulfilled_by)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    fulfillment_id.to_string(),
                    backorder_id_str,
                    input.quantity.to_string(),
                    input.source_type.to_string(),
                    input.source_id.map(|id| id.to_string()),
                    input.notes.as_deref(),
                    now.to_rfc3339(),
                    input.fulfilled_by.as_deref(),
                ],
            )?;

            tx.execute(
                "UPDATE backorders SET quantity_fulfilled = ?, quantity_remaining = ?, status = ?, updated_at = ?
                 WHERE id = ?",
                rusqlite::params![
                    new_fulfilled.to_string(),
                    new_remaining.to_string(),
                    new_status.to_string(),
                    now.to_rfc3339(),
                    backorder_id_str,
                ],
            )?;
            Ok(())
        })?;

        self.get_backorder(input.backorder_id)?.ok_or(CommerceError::NotFound)
    }

    fn get_fulfillment_history(&self, backorder_id: Uuid) -> Result<Vec<BackorderFulfillment>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, backorder_id, quantity, source_type, source_id, notes, fulfilled_at, fulfilled_by
             FROM backorder_fulfillments WHERE backorder_id = ? ORDER BY fulfilled_at"
        ).map_err(map_db_error)?;

        let rows = stmt
            .query_map([backorder_id.to_string()], |row| self.row_to_fulfillment(row))
            .map_err(map_db_error)?;

        let mut fulfillments = Vec::new();
        for row in rows {
            fulfillments.push(row.map_err(map_db_error)?);
        }
        Ok(fulfillments)
    }

    fn allocate_backorder(&self, input: AllocateBackorder) -> Result<BackorderAllocation> {
        if input.quantity <= Decimal::ZERO {
            return Err(CommerceError::ValidationError(
                "Allocation quantity must be greater than zero".to_string(),
            ));
        }
        if input.location_id.is_some_and(|id| id <= 0) {
            return Err(CommerceError::ValidationError("location_id must be positive".into()));
        }

        with_immediate_transaction(&self.pool, |tx| {
            let now = Utc::now();
            let (status_str, remaining_str, sku, source_location_id): (
                String,
                String,
                String,
                Option<i32>,
            ) = tx
                .query_row(
                    "SELECT status, quantity_remaining, sku, source_location_id
                     FROM backorders WHERE id = ?",
                    [input.backorder_id.to_string()],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .optional()?
                .ok_or_else(|| to_sql_err(CommerceError::NotFound))?;
            let status: BackorderStatus = parse_enum_row(&status_str, "backorder", "status")?;
            if matches!(status, BackorderStatus::Cancelled | BackorderStatus::Fulfilled) {
                return Err(to_sql_err(CommerceError::ValidationError(format!(
                    "Backorder is {status} and cannot be allocated"
                ))));
            }
            let remaining = parse_decimal_row(&remaining_str, "backorder", "quantity_remaining")?;
            let already = open_allocated_quantity_in_tx(tx, input.backorder_id)?;
            if input.quantity + already > remaining {
                return Err(to_sql_err(CommerceError::ValidationError(format!(
                    "Cannot allocate {} - only {} of the backorder remains unallocated",
                    input.quantity,
                    remaining - already
                ))));
            }

            let location_id = input.location_id.or(source_location_id).unwrap_or(1);
            allocate_in_tx(
                tx,
                input.backorder_id,
                &sku,
                input.quantity,
                location_id,
                input.lot_id,
                input.expires_at,
                now,
            )
        })
    }

    fn get_allocations(&self, backorder_id: Uuid) -> Result<Vec<BackorderAllocation>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut stmt = conn
            .prepare(&format!(
                "SELECT {ALLOCATION_COLUMNS} FROM backorder_allocations
                 WHERE backorder_id = ? ORDER BY allocated_at, id"
            ))
            .map_err(map_db_error)?;

        let rows = stmt
            .query_map([backorder_id.to_string()], |row| self.row_to_allocation(row))
            .map_err(map_db_error)?;

        let mut allocations = Vec::new();
        for row in rows {
            allocations.push(row.map_err(map_db_error)?);
        }
        Ok(allocations)
    }

    fn release_allocation(&self, allocation_id: Uuid) -> Result<BackorderAllocation> {
        with_immediate_transaction(&self.pool, |tx| {
            let now = Utc::now();
            let allocation = self.get_allocation_in_tx(tx, allocation_id)?;
            if !allocation.status.is_open() {
                // Already released / expired / fulfilled: idempotent.
                return Ok(allocation);
            }
            if let Some(reservation_id) = allocation.reservation_id {
                SqliteInventoryRepository::release_reservation_in_tx(tx, reservation_id)?;
            }
            tx.execute(
                "UPDATE backorder_allocations SET status = 'released' WHERE id = ?",
                [allocation_id.to_string()],
            )?;
            settle_backorder_allocation_status_in_tx(tx, allocation.backorder_id, now)?;
            self.get_allocation_in_tx(tx, allocation_id)
        })
    }

    fn confirm_allocation(&self, allocation_id: Uuid) -> Result<BackorderAllocation> {
        with_immediate_transaction(&self.pool, |tx| {
            let now = Utc::now();
            let allocation = self.get_allocation_in_tx(tx, allocation_id)?;
            match allocation.status {
                AllocationStatus::Confirmed => return Ok(allocation),
                AllocationStatus::Reserved => {}
                other => {
                    return Err(to_sql_err(CommerceError::Conflict(format!(
                        "Backorder allocation {allocation_id} is {other} and cannot be confirmed"
                    ))));
                }
            }
            if let Some(reservation_id) = allocation.reservation_id {
                match SqliteInventoryRepository::confirm_reservation_in_tx_with_now(
                    tx,
                    reservation_id,
                    now,
                )? {
                    ReservationConfirmOutcome::Confirmed => {}
                    ReservationConfirmOutcome::Expired => {
                        tx.execute(
                            "UPDATE backorder_allocations SET status = 'expired' WHERE id = ?",
                            [allocation_id.to_string()],
                        )?;
                        settle_backorder_allocation_status_in_tx(tx, allocation.backorder_id, now)?;
                        return Err(to_sql_err(CommerceError::ReservationExpired(reservation_id)));
                    }
                }
            }
            tx.execute(
                "UPDATE backorder_allocations SET status = 'confirmed' WHERE id = ?",
                [allocation_id.to_string()],
            )?;
            self.get_allocation_in_tx(tx, allocation_id)
        })
    }

    fn expire_allocations(&self) -> Result<u32> {
        with_immediate_transaction(&self.pool, |tx| {
            let now = Utc::now();
            let mut stmt = tx.prepare(
                "SELECT id, backorder_id, reservation_id FROM backorder_allocations
                 WHERE status = 'reserved' AND expires_at IS NOT NULL AND expires_at < ?
                 ORDER BY expires_at, id",
            )?;
            let rows = stmt
                .query_map([now.to_rfc3339()], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;

            let mut count = 0u32;
            for (id, backorder_id, reservation_id) in rows {
                if let Some(reservation_id) = reservation_id {
                    let reservation_id =
                        parse_uuid(&reservation_id, "backorder_allocation", "reservation_id")
                            .map_err(to_sql_err)?;
                    // Idempotent if the inventory sweeper already expired it.
                    SqliteInventoryRepository::release_reservation_in_tx(tx, reservation_id)?;
                }
                tx.execute(
                    "UPDATE backorder_allocations SET status = 'expired' WHERE id = ?",
                    [&id],
                )?;
                let backorder_id =
                    parse_uuid(&backorder_id, "backorder_allocation", "backorder_id")
                        .map_err(to_sql_err)?;
                settle_backorder_allocation_status_in_tx(tx, backorder_id, now)?;
                count += 1;
            }
            Ok(count)
        })
    }

    fn auto_allocate_inventory(&self, sku: &str) -> Result<Vec<BackorderAllocation>> {
        // Oldest/most urgent open backorders of this SKU get stock first,
        // each up to what is still available at its source location. One
        // transaction: the availability read and every reservation commit
        // together, so a concurrent cart cannot slip between them.
        with_immediate_transaction(&self.pool, |tx| {
            let now = Utc::now();
            let mut stmt = tx.prepare(
                "SELECT id, quantity_remaining, source_location_id FROM backorders
                 WHERE sku = ? AND status IN ('pending', 'partially_fulfilled', 'allocated')
                 ORDER BY CASE priority WHEN 'critical' THEN 1 WHEN 'high' THEN 2 WHEN 'normal' THEN 3 ELSE 4 END,
                          created_at ASC, id ASC",
            )?;
            let candidates = stmt
                .query_map([sku], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<i32>>(2)?,
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;

            let mut created = Vec::new();
            for (id, remaining, source_location_id) in candidates {
                let backorder_id = parse_uuid(&id, "backorder", "id").map_err(to_sql_err)?;
                let remaining = parse_decimal_strict(&remaining, "backorder", "quantity_remaining")
                    .map_err(to_sql_err)?;
                let need = remaining - open_allocated_quantity_in_tx(tx, backorder_id)?;
                if need <= Decimal::ZERO {
                    continue;
                }
                let location_id = source_location_id.unwrap_or(1);
                let available: Option<String> = tx
                    .query_row(
                        "SELECT b.quantity_available FROM inventory_balances b
                         JOIN inventory_items i ON i.id = b.item_id
                         WHERE i.sku = ? AND b.location_id = ?",
                        rusqlite::params![sku, location_id],
                        |row| row.get(0),
                    )
                    .optional()?;
                let Some(available) = available else { break };
                let available =
                    parse_decimal_strict(&available, "inventory_balance", "quantity_available")
                        .map_err(to_sql_err)?;
                let take = need.min(available);
                if take <= Decimal::ZERO {
                    continue;
                }
                created.push(allocate_in_tx(
                    tx,
                    backorder_id,
                    sku,
                    take,
                    location_id,
                    None,
                    None,
                    now,
                )?);
            }
            Ok(created)
        })
    }

    fn get_summary(&self) -> Result<BackorderSummary> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let now = Utc::now();

        let (total, pending, allocated, critical): (i32, i32, i32, i32) = conn
            .query_row(
                "SELECT
                    COUNT(*),
                    SUM(CASE WHEN status = 'pending' THEN 1 ELSE 0 END),
                    SUM(CASE WHEN status = 'allocated' THEN 1 ELSE 0 END),
                    SUM(CASE WHEN priority = 'critical' THEN 1 ELSE 0 END)
                 FROM backorders WHERE status NOT IN ('fulfilled', 'cancelled')",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .map_err(map_db_error)?;

        let overdue: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM backorders
                 WHERE status NOT IN ('fulfilled', 'cancelled')
                 AND expected_date IS NOT NULL AND expected_date < ?",
                [now.to_rfc3339()],
                |row| row.get(0),
            )
            .map_err(map_db_error)?;

        let total_quantity = sum_decimal_query(
            &conn,
            "SELECT quantity_remaining FROM backorders WHERE status NOT IN ('fulfilled', 'cancelled')",
            &[],
            "backorders",
            "quantity_remaining",
        )?;

        Ok(BackorderSummary {
            total_backorders: total,
            total_quantity,
            pending_count: pending,
            allocated_count: allocated,
            critical_count: critical,
            overdue_count: overdue,
        })
    }

    fn get_sku_summary(&self, sku: &str) -> Result<Option<SkuBackorderSummary>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let (count, oldest_date_raw, earliest_expected_raw): (i32, Option<String>, Option<String>) =
            conn.query_row(
                "SELECT COUNT(*), MIN(created_at), MIN(expected_date)
                 FROM backorders
                 WHERE sku = ? AND status NOT IN ('fulfilled', 'cancelled')",
                [sku],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(map_db_error)?;

        if count == 0 {
            return Ok(None);
        }

        let sku_param = sku.to_string();
        let sku_params: [&dyn rusqlite::ToSql; 1] = [&sku_param];
        let total_quantity = sum_decimal_query(
            &conn,
            "SELECT quantity_remaining FROM backorders WHERE sku = ? AND status NOT IN ('fulfilled', 'cancelled')",
            &sku_params,
            "backorders",
            "quantity_remaining",
        )?;
        let oldest_date =
            parse_datetime_opt(oldest_date_raw, "sku_backorder_summary", "oldest_date")?;
        let earliest_expected = parse_datetime_opt(
            earliest_expected_raw,
            "sku_backorder_summary",
            "earliest_expected",
        )?;

        Ok(Some(SkuBackorderSummary {
            sku: sku.to_string(),
            total_quantity,
            backorder_count: count,
            oldest_date,
            earliest_expected,
        }))
    }

    fn get_overdue_backorders(&self) -> Result<Vec<Backorder>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let now = Utc::now();

        let mut stmt = conn
            .prepare(
                "SELECT id, backorder_number, order_id, order_line_id, customer_id, sku,
                    quantity_ordered, quantity_fulfilled, quantity_remaining, status, priority,
                    expected_date, promised_date, source_location_id, notes, created_at, updated_at
             FROM backorders
             WHERE status NOT IN ('fulfilled', 'cancelled')
             AND expected_date IS NOT NULL AND expected_date < ?
             ORDER BY expected_date",
            )
            .map_err(map_db_error)?;

        let rows = stmt
            .query_map([now.to_rfc3339()], |row| self.row_to_backorder(row))
            .map_err(map_db_error)?;

        let mut backorders = Vec::new();
        for row in rows {
            backorders.push(row.map_err(map_db_error)?);
        }
        Ok(backorders)
    }

    fn count_pending(&self) -> Result<u64> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM backorders WHERE status = 'pending'", [], |row| {
                row.get(0)
            })
            .map_err(map_db_error)?;
        Ok(count as u64)
    }
}
