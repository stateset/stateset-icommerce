//! SQLite implementation of [`BinRepository`] (warehouse bins + bin levels).
//!
//! Bins are a sub-allocation of warehouse-level stock. Warehouse-level stock
//! for warehouse `N` lives in `inventory_balances` at `location_id = N`
//! (the `inventory_locations` bridge row is created on demand). Every bin
//! adjustment applies the same delta to that balance inside the same
//! IMMEDIATE transaction, so `Σ bin on_hand == warehouse on_hand` holds for
//! every `(warehouse, sku)`; bin-to-bin moves are stock-neutral.

use crate::sqlite::{
    map_db_error, parse_datetime_row, parse_decimal_opt_row, parse_decimal_row,
    parse_decimal_strict, parse_enum_row, parse_uuid_row, with_immediate_transaction,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{OptionalExtension, params};
use rust_decimal::Decimal;
use uuid::Uuid;

use stateset_core::{
    AdjustBinLevel, BinLevel, BinMovement, BinMovementType, BinReconciliation, BinRepository,
    BinType, CommerceError, CreateWarehouseBin, MoveBetweenBins, Result, UpdateWarehouseBin,
    WarehouseBin, WarehouseBinFilter,
};

const BIN_COLUMNS: &str = "id, warehouse_id, code, zone, aisle, rack, shelf, position, bin_type, \
                           is_active, capacity, created_at, updated_at";

/// SQLite warehouse bin repository
#[derive(Debug)]
pub struct SqliteBinRepository {
    pool: Pool<SqliteConnectionManager>,
}

/// Wrap a domain error so it survives `with_immediate_transaction` without
/// being retried (unwrapped again by `map_db_error`).
pub(crate) fn smuggle(e: CommerceError) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(e))
}

pub(crate) fn row_to_bin(row: &rusqlite::Row<'_>) -> rusqlite::Result<WarehouseBin> {
    Ok(WarehouseBin {
        id: row.get("id")?,
        warehouse_id: row.get("warehouse_id")?,
        code: row.get("code")?,
        zone: row.get("zone")?,
        aisle: row.get("aisle")?,
        rack: row.get("rack")?,
        shelf: row.get("shelf")?,
        position: row.get("position")?,
        bin_type: parse_enum_row(&row.get::<_, String>("bin_type")?, "warehouse_bin", "bin_type")?,
        is_active: row.get::<_, i32>("is_active")? == 1,
        capacity: parse_decimal_opt_row(
            row.get::<_, Option<String>>("capacity")?,
            "warehouse_bin",
            "capacity",
        )?,
        created_at: parse_datetime_row(
            &row.get::<_, String>("created_at")?,
            "warehouse_bin",
            "created_at",
        )?,
        updated_at: parse_datetime_row(
            &row.get::<_, String>("updated_at")?,
            "warehouse_bin",
            "updated_at",
        )?,
    })
}

fn row_to_level(row: &rusqlite::Row<'_>) -> rusqlite::Result<BinLevel> {
    let on_hand = parse_decimal_row(
        &row.get::<_, String>("quantity_on_hand")?,
        "inventory_bin_level",
        "quantity_on_hand",
    )?;
    let allocated = parse_decimal_row(
        &row.get::<_, String>("quantity_allocated")?,
        "inventory_bin_level",
        "quantity_allocated",
    )?;
    Ok(BinLevel {
        bin_id: row.get("bin_id")?,
        warehouse_id: row.get("warehouse_id")?,
        sku: row.get("sku")?,
        quantity_on_hand: on_hand,
        quantity_allocated: allocated,
        quantity_available: on_hand - allocated,
        updated_at: parse_datetime_row(
            &row.get::<_, String>("updated_at")?,
            "inventory_bin_level",
            "updated_at",
        )?,
    })
}

/// Load a bin inside a transaction; `NotFound` when absent.
pub(crate) fn load_bin_tx(
    tx: &rusqlite::Transaction<'_>,
    id: i32,
) -> rusqlite::Result<WarehouseBin> {
    tx.query_row(
        &format!("SELECT {BIN_COLUMNS} FROM warehouse_bins WHERE id = ?1"),
        [id],
        row_to_bin,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => smuggle(CommerceError::NotFound),
        other => other,
    })
}

/// Find a bin for a return disposition: the explicit bin (must belong to the
/// warehouse) or the first active bin of the preferred types, in order.
pub(crate) fn find_disposition_bin_tx(
    tx: &rusqlite::Transaction<'_>,
    warehouse_id: i32,
    explicit_bin_id: Option<i32>,
    preferred: &[BinType],
) -> rusqlite::Result<Option<WarehouseBin>> {
    if let Some(bin_id) = explicit_bin_id {
        let bin = load_bin_tx(tx, bin_id)?;
        if bin.warehouse_id != warehouse_id {
            return Err(smuggle(CommerceError::ValidationError(format!(
                "Bin {bin_id} does not belong to warehouse {warehouse_id}"
            ))));
        }
        if !bin.is_active {
            return Err(smuggle(CommerceError::ValidationError(format!(
                "Bin {bin_id} is inactive"
            ))));
        }
        return Ok(Some(bin));
    }
    for bin_type in preferred {
        let found = tx
            .query_row(
                &format!(
                    "SELECT {BIN_COLUMNS} FROM warehouse_bins
                     WHERE warehouse_id = ?1 AND bin_type = ?2 AND is_active = 1
                     ORDER BY id LIMIT 1"
                ),
                params![warehouse_id, bin_type.to_string()],
                row_to_bin,
            )
            .optional()?;
        if found.is_some() {
            return Ok(found);
        }
    }
    Ok(None)
}

/// Apply a signed `(on_hand, allocated)` delta to one bin level, enforcing
/// non-negative on-hand / available and the bin capacity. Returns the new level.
pub(crate) fn apply_bin_delta_tx(
    tx: &rusqlite::Transaction<'_>,
    bin: &WarehouseBin,
    sku: &str,
    delta_on_hand: Decimal,
    delta_allocated: Decimal,
    now: &str,
) -> rusqlite::Result<BinLevel> {
    let current: Option<(String, String)> = tx
        .query_row(
            "SELECT quantity_on_hand, quantity_allocated FROM inventory_bin_levels
             WHERE bin_id = ?1 AND sku = ?2",
            params![bin.id, sku],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let (on_hand, allocated) = match &current {
        Some((oh, al)) => (
            parse_decimal_strict(oh, "inventory_bin_level", "quantity_on_hand").map_err(smuggle)?,
            parse_decimal_strict(al, "inventory_bin_level", "quantity_allocated")
                .map_err(smuggle)?,
        ),
        None => (Decimal::ZERO, Decimal::ZERO),
    };
    let new_on_hand = on_hand + delta_on_hand;
    let new_allocated = allocated + delta_allocated;
    if new_on_hand < Decimal::ZERO || new_allocated < Decimal::ZERO {
        return Err(smuggle(CommerceError::InsufficientStock {
            sku: sku.to_string(),
            requested: delta_on_hand.abs().to_string(),
            available: on_hand.to_string(),
        }));
    }
    if new_on_hand - new_allocated < Decimal::ZERO {
        return Err(smuggle(CommerceError::InsufficientStock {
            sku: sku.to_string(),
            requested: delta_on_hand.abs().to_string(),
            available: (on_hand - allocated).to_string(),
        }));
    }
    if let Some(capacity) = bin.capacity {
        if new_on_hand > capacity {
            return Err(smuggle(CommerceError::ValidationError(format!(
                "Bin {} capacity {} exceeded: {} on hand after adjustment",
                bin.code, capacity, new_on_hand
            ))));
        }
    }
    if current.is_some() {
        tx.execute(
            "UPDATE inventory_bin_levels SET quantity_on_hand = ?1, quantity_allocated = ?2,
             updated_at = ?3 WHERE bin_id = ?4 AND sku = ?5",
            params![new_on_hand.to_string(), new_allocated.to_string(), now, bin.id, sku],
        )?;
    } else {
        tx.execute(
            "INSERT INTO inventory_bin_levels (bin_id, sku, quantity_on_hand, quantity_allocated, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![bin.id, sku, new_on_hand.to_string(), new_allocated.to_string(), now],
        )?;
    }
    Ok(BinLevel {
        bin_id: bin.id,
        warehouse_id: bin.warehouse_id,
        sku: sku.to_string(),
        quantity_on_hand: new_on_hand,
        quantity_allocated: new_allocated,
        quantity_available: new_on_hand - new_allocated,
        updated_at: chrono::DateTime::parse_from_rfc3339(now)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
    })
}

/// Apply a signed `(on_hand, allocated)` delta to the warehouse-level balance
/// (`inventory_balances` at `location_id = warehouse_id`). The SKU must exist
/// as an inventory item; the `inventory_locations` bridge row is created on
/// demand from the warehouse's code/name.
#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_warehouse_delta_tx(
    tx: &rusqlite::Transaction<'_>,
    warehouse_id: i32,
    sku: &str,
    delta_on_hand: Decimal,
    delta_allocated: Decimal,
    reason: &str,
    reference_type: Option<&str>,
    reference_id: Option<&str>,
    now: &str,
) -> rusqlite::Result<()> {
    let item_id: i64 = tx
        .query_row("SELECT id FROM inventory_items WHERE sku = ?1", [sku], |row| row.get(0))
        .optional()?
        .ok_or_else(|| {
            smuggle(CommerceError::ValidationError(format!(
                "Inventory item {sku} not found; create it before stocking bins"
            )))
        })?;

    // Bridge row so the inventory_balances FK is satisfied.
    let wh: Option<(String, String)> = tx
        .query_row("SELECT code, name FROM warehouses WHERE id = ?1", [warehouse_id], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .optional()?;
    let (code, name) =
        wh.unwrap_or_else(|| (format!("WH-{warehouse_id}"), format!("Warehouse {warehouse_id}")));
    tx.execute(
        "INSERT OR IGNORE INTO inventory_locations (id, name, code) VALUES (?1, ?2, ?3)",
        params![warehouse_id, name, code],
    )?;

    let current: Option<(String, String, i32)> = tx
        .query_row(
            "SELECT quantity_on_hand, quantity_allocated, version FROM inventory_balances
             WHERE item_id = ?1 AND location_id = ?2",
            params![item_id, warehouse_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    let (on_hand, allocated) = match &current {
        Some((oh, al, _)) => (
            parse_decimal_strict(oh, "inventory_balance", "quantity_on_hand").map_err(smuggle)?,
            parse_decimal_strict(al, "inventory_balance", "quantity_allocated").map_err(smuggle)?,
        ),
        None => (Decimal::ZERO, Decimal::ZERO),
    };
    let new_on_hand = on_hand + delta_on_hand;
    let new_allocated = allocated + delta_allocated;
    let new_available = new_on_hand - new_allocated;
    if new_on_hand < Decimal::ZERO || new_allocated < Decimal::ZERO || new_available < Decimal::ZERO
    {
        return Err(smuggle(CommerceError::InsufficientStock {
            sku: sku.to_string(),
            requested: delta_on_hand.abs().to_string(),
            available: (on_hand - allocated).to_string(),
        }));
    }
    match current {
        Some((_, _, version)) => {
            tx.execute(
                "UPDATE inventory_balances SET quantity_on_hand = ?1, quantity_allocated = ?2,
                 quantity_available = ?3, version = version + 1, updated_at = ?4
                 WHERE item_id = ?5 AND location_id = ?6 AND version = ?7",
                params![
                    new_on_hand.to_string(),
                    new_allocated.to_string(),
                    new_available.to_string(),
                    now,
                    item_id,
                    warehouse_id,
                    version
                ],
            )?;
        }
        None => {
            tx.execute(
                "INSERT INTO inventory_balances (item_id, location_id, quantity_on_hand,
                 quantity_allocated, quantity_available, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    item_id,
                    warehouse_id,
                    new_on_hand.to_string(),
                    new_allocated.to_string(),
                    new_available.to_string(),
                    now
                ],
            )?;
        }
    }
    if !delta_on_hand.is_zero() {
        tx.execute(
            "INSERT INTO inventory_transactions (item_id, location_id, transaction_type, quantity,
             reference_type, reference_id, reason, created_at)
             VALUES (?1, ?2, 'adjustment', ?3, ?4, ?5, ?6, ?7)",
            params![
                item_id,
                warehouse_id,
                delta_on_hand.to_string(),
                reference_type,
                reference_id,
                reason,
                now
            ],
        )?;
    }
    Ok(())
}

/// Insert a bin movement audit row.
#[allow(clippy::too_many_arguments)]
pub(crate) fn insert_bin_movement_tx(
    tx: &rusqlite::Transaction<'_>,
    movement_type: BinMovementType,
    from_bin_id: Option<i32>,
    to_bin_id: Option<i32>,
    sku: &str,
    quantity: Decimal,
    reason: Option<&str>,
    reference_type: Option<&str>,
    reference_id: Option<&str>,
    performed_by: Option<&str>,
    now: &str,
) -> rusqlite::Result<Uuid> {
    let id = Uuid::new_v4();
    tx.execute(
        "INSERT INTO inventory_bin_movements (id, movement_type, from_bin_id, to_bin_id, sku,
         quantity, reason, reference_type, reference_id, performed_by, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            id.to_string(),
            movement_type.to_string(),
            from_bin_id,
            to_bin_id,
            sku,
            quantity.to_string(),
            reason,
            reference_type,
            reference_id,
            performed_by,
            now
        ],
    )?;
    Ok(id)
}

impl SqliteBinRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn filter_clauses(filter: &WarehouseBinFilter) -> (String, Vec<Box<dyn rusqlite::ToSql>>) {
        let mut sql = String::from(" WHERE 1=1");
        let mut p: Vec<Box<dyn rusqlite::ToSql>> = vec![];
        if let Some(wh) = filter.warehouse_id {
            sql.push_str(" AND warehouse_id = ?");
            p.push(Box::new(wh));
        }
        if let Some(t) = filter.bin_type {
            sql.push_str(" AND bin_type = ?");
            p.push(Box::new(t.to_string()));
        }
        if let Some(z) = &filter.zone {
            sql.push_str(" AND zone = ?");
            p.push(Box::new(z.clone()));
        }
        if let Some(a) = filter.is_active {
            sql.push_str(" AND is_active = ?");
            p.push(Box::new(i32::from(a)));
        }
        (sql, p)
    }
}

impl BinRepository for SqliteBinRepository {
    fn create_bin(&self, input: CreateWarehouseBin) -> Result<WarehouseBin> {
        let code = input.code.trim().to_string();
        if code.is_empty() {
            return Err(CommerceError::ValidationError("Bin code cannot be empty".into()));
        }
        if input.capacity.is_some_and(|c| c <= Decimal::ZERO) {
            return Err(CommerceError::ValidationError("Bin capacity must be positive".into()));
        }
        let conn = self.conn()?;
        let exists: Option<i32> = conn
            .query_row("SELECT id FROM warehouses WHERE id = ?1", [input.warehouse_id], |r| {
                r.get(0)
            })
            .optional()
            .map_err(map_db_error)?;
        if exists.is_none() {
            return Err(CommerceError::ValidationError(format!(
                "Warehouse {} not found",
                input.warehouse_id
            )));
        }
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO warehouse_bins (warehouse_id, code, zone, aisle, rack, shelf, position,
             bin_type, is_active, capacity, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9, ?10, ?10)",
            params![
                input.warehouse_id,
                code,
                input.zone,
                input.aisle,
                input.rack,
                input.shelf,
                input.position,
                input.bin_type.to_string(),
                input.capacity.map(|c| c.to_string()),
                now
            ],
        )
        .map_err(map_db_error)?;
        let id = conn.last_insert_rowid() as i32;
        self.get_bin(id)?.ok_or(CommerceError::NotFound)
    }

    fn get_bin(&self, id: i32) -> Result<Option<WarehouseBin>> {
        let conn = self.conn()?;
        conn.query_row(
            &format!("SELECT {BIN_COLUMNS} FROM warehouse_bins WHERE id = ?1"),
            [id],
            row_to_bin,
        )
        .optional()
        .map_err(map_db_error)
    }

    fn get_bin_by_code(&self, warehouse_id: i32, code: &str) -> Result<Option<WarehouseBin>> {
        let conn = self.conn()?;
        conn.query_row(
            &format!(
                "SELECT {BIN_COLUMNS} FROM warehouse_bins WHERE warehouse_id = ?1 AND code = ?2"
            ),
            params![warehouse_id, code],
            row_to_bin,
        )
        .optional()
        .map_err(map_db_error)
    }

    fn update_bin(&self, id: i32, input: UpdateWarehouseBin) -> Result<WarehouseBin> {
        if let Some(Some(c)) = input.capacity {
            if c <= Decimal::ZERO {
                return Err(CommerceError::ValidationError("Bin capacity must be positive".into()));
            }
        }
        let conn = self.conn()?;
        let mut sets: Vec<String> = vec!["updated_at = ?".into()];
        let mut p: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(Utc::now().to_rfc3339())];
        macro_rules! set_opt {
            ($field:ident) => {
                if let Some(v) = input.$field {
                    sets.push(concat!(stringify!($field), " = ?").into());
                    p.push(Box::new(v));
                }
            };
        }
        set_opt!(zone);
        set_opt!(aisle);
        set_opt!(rack);
        set_opt!(shelf);
        set_opt!(position);
        if let Some(t) = input.bin_type {
            sets.push("bin_type = ?".into());
            p.push(Box::new(t.to_string()));
        }
        if let Some(a) = input.is_active {
            sets.push("is_active = ?".into());
            p.push(Box::new(i32::from(a)));
        }
        if let Some(cap) = input.capacity {
            sets.push("capacity = ?".into());
            p.push(Box::new(cap.map(|c| c.to_string())));
        }
        p.push(Box::new(id));
        let sql = format!("UPDATE warehouse_bins SET {} WHERE id = ?", sets.join(", "));
        let refs: Vec<&dyn rusqlite::ToSql> = p.iter().map(AsRef::as_ref).collect();
        let n = conn.execute(&sql, refs.as_slice()).map_err(map_db_error)?;
        if n == 0 {
            return Err(CommerceError::NotFound);
        }
        self.get_bin(id)?.ok_or(CommerceError::NotFound)
    }

    fn list_bins(&self, filter: WarehouseBinFilter) -> Result<Vec<WarehouseBin>> {
        let conn = self.conn()?;
        let (clauses, mut p) = Self::filter_clauses(&filter);
        let mut sql = format!(
            "SELECT {BIN_COLUMNS} FROM warehouse_bins{clauses} ORDER BY warehouse_id, code"
        );
        if let Some(limit) = filter.limit {
            sql.push_str(" LIMIT ?");
            p.push(Box::new(i64::from(limit.min(1000))));
            if let Some(offset) = filter.offset {
                sql.push_str(" OFFSET ?");
                p.push(Box::new(i64::from(offset)));
            }
        }
        let refs: Vec<&dyn rusqlite::ToSql> = p.iter().map(AsRef::as_ref).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt.query_map(refs.as_slice(), row_to_bin).map_err(map_db_error)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(map_db_error)
    }

    fn count_bins(&self, filter: WarehouseBinFilter) -> Result<u64> {
        let conn = self.conn()?;
        let (clauses, p) = Self::filter_clauses(&filter);
        let refs: Vec<&dyn rusqlite::ToSql> = p.iter().map(AsRef::as_ref).collect();
        let n: i64 = conn
            .query_row(
                &format!("SELECT COUNT(*) FROM warehouse_bins{clauses}"),
                refs.as_slice(),
                |r| r.get(0),
            )
            .map_err(map_db_error)?;
        Ok(n as u64)
    }

    fn delete_bin(&self, id: i32) -> Result<()> {
        with_immediate_transaction(&self.pool, |tx| {
            load_bin_tx(tx, id)?;
            let mut stmt = tx.prepare(
                "SELECT quantity_on_hand, quantity_allocated FROM inventory_bin_levels WHERE bin_id = ?1",
            )?;
            let levels = stmt
                .query_map([id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            for (oh, al) in levels {
                let on_hand = parse_decimal_strict(&oh, "inventory_bin_level", "quantity_on_hand")
                    .map_err(smuggle)?;
                let allocated =
                    parse_decimal_strict(&al, "inventory_bin_level", "quantity_allocated")
                        .map_err(smuggle)?;
                if !on_hand.is_zero() || !allocated.is_zero() {
                    return Err(smuggle(CommerceError::NotPermitted(format!(
                        "Bin {id} still holds stock; move or adjust it out first"
                    ))));
                }
            }
            tx.execute("DELETE FROM inventory_bin_levels WHERE bin_id = ?1", [id])?;
            tx.execute("DELETE FROM warehouse_bins WHERE id = ?1", [id])?;
            Ok(())
        })
    }

    fn get_bin_levels(&self, bin_id: i32) -> Result<Vec<BinLevel>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT l.bin_id, b.warehouse_id, l.sku, l.quantity_on_hand, l.quantity_allocated, l.updated_at
                 FROM inventory_bin_levels l JOIN warehouse_bins b ON b.id = l.bin_id
                 WHERE l.bin_id = ?1 ORDER BY l.sku",
            )
            .map_err(map_db_error)?;
        let rows = stmt.query_map([bin_id], row_to_level).map_err(map_db_error)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(map_db_error)
    }

    fn get_bin_levels_for_sku(&self, warehouse_id: i32, sku: &str) -> Result<Vec<BinLevel>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT l.bin_id, b.warehouse_id, l.sku, l.quantity_on_hand, l.quantity_allocated, l.updated_at
                 FROM inventory_bin_levels l JOIN warehouse_bins b ON b.id = l.bin_id
                 WHERE b.warehouse_id = ?1 AND l.sku = ?2 ORDER BY l.bin_id",
            )
            .map_err(map_db_error)?;
        let rows =
            stmt.query_map(params![warehouse_id, sku], row_to_level).map_err(map_db_error)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(map_db_error)
    }

    fn adjust_bin_level(&self, input: AdjustBinLevel) -> Result<BinLevel> {
        if input.quantity.is_zero() {
            return Err(CommerceError::ValidationError(
                "Adjustment quantity cannot be zero".into(),
            ));
        }
        with_immediate_transaction(&self.pool, |tx| {
            let now = Utc::now().to_rfc3339();
            let bin = load_bin_tx(tx, input.bin_id)?;
            if !bin.is_active {
                return Err(smuggle(CommerceError::ValidationError(format!(
                    "Bin {} is inactive",
                    bin.code
                ))));
            }
            let level =
                apply_bin_delta_tx(tx, &bin, &input.sku, input.quantity, Decimal::ZERO, &now)?;
            apply_warehouse_delta_tx(
                tx,
                bin.warehouse_id,
                &input.sku,
                input.quantity,
                Decimal::ZERO,
                &input.reason,
                input.reference_type.as_deref(),
                input.reference_id.as_deref(),
                &now,
            )?;
            let (from, to) = if input.quantity.is_sign_negative() {
                (Some(bin.id), None)
            } else {
                (None, Some(bin.id))
            };
            insert_bin_movement_tx(
                tx,
                BinMovementType::Adjustment,
                from,
                to,
                &input.sku,
                input.quantity.abs(),
                Some(&input.reason),
                input.reference_type.as_deref(),
                input.reference_id.as_deref(),
                input.performed_by.as_deref(),
                &now,
            )?;
            Ok(level)
        })
    }

    fn move_between_bins(&self, input: MoveBetweenBins) -> Result<BinMovement> {
        if input.quantity <= Decimal::ZERO {
            return Err(CommerceError::ValidationError("Move quantity must be positive".into()));
        }
        if input.from_bin_id == input.to_bin_id {
            return Err(CommerceError::ValidationError(
                "Source and destination bins must differ".into(),
            ));
        }
        with_immediate_transaction(&self.pool, |tx| {
            let now = Utc::now().to_rfc3339();
            let from = load_bin_tx(tx, input.from_bin_id)?;
            let to = load_bin_tx(tx, input.to_bin_id)?;
            if from.warehouse_id != to.warehouse_id {
                return Err(smuggle(CommerceError::ValidationError(
                    "Bins belong to different warehouses; use a transfer order".into(),
                )));
            }
            if !to.is_active {
                return Err(smuggle(CommerceError::ValidationError(format!(
                    "Destination bin {} is inactive",
                    to.code
                ))));
            }
            // Source decrement first: rejects when available < quantity.
            let src_exists: Option<i32> = tx
                .query_row(
                    "SELECT 1 FROM inventory_bin_levels WHERE bin_id = ?1 AND sku = ?2",
                    params![from.id, input.sku],
                    |r| r.get(0),
                )
                .optional()?;
            if src_exists.is_none() {
                return Err(smuggle(CommerceError::InsufficientStock {
                    sku: input.sku.clone(),
                    requested: input.quantity.to_string(),
                    available: "0".into(),
                }));
            }
            apply_bin_delta_tx(tx, &from, &input.sku, -input.quantity, Decimal::ZERO, &now)?;
            apply_bin_delta_tx(tx, &to, &input.sku, input.quantity, Decimal::ZERO, &now)?;
            let id = insert_bin_movement_tx(
                tx,
                BinMovementType::Transfer,
                Some(from.id),
                Some(to.id),
                &input.sku,
                input.quantity,
                input.reason.as_deref(),
                None,
                None,
                input.performed_by.as_deref(),
                &now,
            )?;
            tx.query_row(
                "SELECT id, movement_type, from_bin_id, to_bin_id, sku, quantity, reason,
                 reference_type, reference_id, performed_by, created_at
                 FROM inventory_bin_movements WHERE id = ?1",
                [id.to_string()],
                row_to_movement,
            )
        })
    }

    fn reconcile(&self, warehouse_id: i32, sku: &str) -> Result<BinReconciliation> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT l.quantity_on_hand FROM inventory_bin_levels l
                 JOIN warehouse_bins b ON b.id = l.bin_id
                 WHERE b.warehouse_id = ?1 AND l.sku = ?2",
            )
            .map_err(map_db_error)?;
        let mut bin_on_hand = Decimal::ZERO;
        for raw in stmt
            .query_map(params![warehouse_id, sku], |r| r.get::<_, String>(0))
            .map_err(map_db_error)?
        {
            bin_on_hand += parse_decimal_strict(
                &raw.map_err(map_db_error)?,
                "inventory_bin_level",
                "quantity_on_hand",
            )?;
        }
        let warehouse_raw: Option<String> = conn
            .query_row(
                "SELECT b.quantity_on_hand FROM inventory_balances b
                 JOIN inventory_items i ON i.id = b.item_id
                 WHERE i.sku = ?1 AND b.location_id = ?2",
                params![sku, warehouse_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(map_db_error)?;
        let warehouse_on_hand = match warehouse_raw {
            Some(raw) => parse_decimal_strict(&raw, "inventory_balance", "quantity_on_hand")?,
            None => Decimal::ZERO,
        };
        Ok(BinReconciliation {
            warehouse_id,
            sku: sku.to_string(),
            bin_on_hand,
            warehouse_on_hand,
            variance: warehouse_on_hand - bin_on_hand,
        })
    }
}

fn row_to_movement(row: &rusqlite::Row<'_>) -> rusqlite::Result<BinMovement> {
    Ok(BinMovement {
        id: parse_uuid_row(&row.get::<_, String>("id")?, "inventory_bin_movement", "id")?,
        movement_type: parse_enum_row(
            &row.get::<_, String>("movement_type")?,
            "inventory_bin_movement",
            "movement_type",
        )?,
        from_bin_id: row.get("from_bin_id")?,
        to_bin_id: row.get("to_bin_id")?,
        sku: row.get("sku")?,
        quantity: parse_decimal_row(
            &row.get::<_, String>("quantity")?,
            "inventory_bin_movement",
            "quantity",
        )?,
        reason: row.get("reason")?,
        reference_type: row.get("reference_type")?,
        reference_id: row.get("reference_id")?,
        performed_by: row.get("performed_by")?,
        created_at: parse_datetime_row(
            &row.get::<_, String>("created_at")?,
            "inventory_bin_movement",
            "created_at",
        )?,
    })
}
