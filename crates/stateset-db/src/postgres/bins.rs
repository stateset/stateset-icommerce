//! PostgreSQL implementation of [`BinRepository`] (warehouse bins + bin levels).
//!
//! Mirrors the SQLite store: warehouse-level stock for warehouse `N` lives in
//! `inventory_balances` at `location_id = N`; bin adjustments apply the same
//! delta there in the same transaction; moves are stock-neutral. Rows are
//! locked with `FOR UPDATE` so concurrent moves cannot over-transfer.

use super::{block_on, map_db_error};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;
use sqlx::{FromRow, PgConnection, Postgres, QueryBuilder};
use stateset_core::{
    AdjustBinLevel, BinLevel, BinMovement, BinMovementType, BinReconciliation, BinRepository,
    BinType, CommerceError, CreateWarehouseBin, MoveBetweenBins, Result, UpdateWarehouseBin,
    WarehouseBin, WarehouseBinFilter,
};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct PgBinRepository {
    pool: PgPool,
}

#[derive(FromRow)]
pub(crate) struct BinRow {
    id: i32,
    warehouse_id: i32,
    code: String,
    zone: Option<String>,
    aisle: Option<String>,
    rack: Option<String>,
    shelf: Option<String>,
    position: Option<String>,
    bin_type: String,
    is_active: bool,
    capacity: Option<Decimal>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl TryFrom<BinRow> for WarehouseBin {
    type Error = CommerceError;

    fn try_from(r: BinRow) -> Result<Self> {
        let bin_type: BinType = r.bin_type.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid warehouse_bin.bin_type '{}': {e}",
                r.bin_type
            ))
        })?;
        Ok(Self {
            id: r.id,
            warehouse_id: r.warehouse_id,
            code: r.code,
            zone: r.zone,
            aisle: r.aisle,
            rack: r.rack,
            shelf: r.shelf,
            position: r.position,
            bin_type,
            is_active: r.is_active,
            capacity: r.capacity,
            created_at: r.created_at,
            updated_at: r.updated_at,
        })
    }
}

#[derive(FromRow)]
struct LevelRow {
    bin_id: i32,
    warehouse_id: i32,
    sku: String,
    quantity_on_hand: Decimal,
    quantity_allocated: Decimal,
    updated_at: DateTime<Utc>,
}

impl From<LevelRow> for BinLevel {
    fn from(r: LevelRow) -> Self {
        Self {
            bin_id: r.bin_id,
            warehouse_id: r.warehouse_id,
            sku: r.sku,
            quantity_on_hand: r.quantity_on_hand,
            quantity_allocated: r.quantity_allocated,
            quantity_available: r.quantity_on_hand - r.quantity_allocated,
            updated_at: r.updated_at,
        }
    }
}

#[derive(FromRow)]
struct MovementRow {
    id: Uuid,
    movement_type: String,
    from_bin_id: Option<i32>,
    to_bin_id: Option<i32>,
    sku: String,
    quantity: Decimal,
    reason: Option<String>,
    reference_type: Option<String>,
    reference_id: Option<String>,
    performed_by: Option<String>,
    created_at: DateTime<Utc>,
}

impl TryFrom<MovementRow> for BinMovement {
    type Error = CommerceError;

    fn try_from(r: MovementRow) -> Result<Self> {
        let movement_type: BinMovementType = r.movement_type.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid inventory_bin_movement.movement_type '{}': {e}",
                r.movement_type
            ))
        })?;
        Ok(Self {
            id: r.id,
            movement_type,
            from_bin_id: r.from_bin_id,
            to_bin_id: r.to_bin_id,
            sku: r.sku,
            quantity: r.quantity,
            reason: r.reason,
            reference_type: r.reference_type,
            reference_id: r.reference_id,
            performed_by: r.performed_by,
            created_at: r.created_at,
        })
    }
}

const LEVEL_SELECT: &str = "SELECT l.bin_id, b.warehouse_id, l.sku, l.quantity_on_hand, \
                            l.quantity_allocated, l.updated_at \
                            FROM inventory_bin_levels l JOIN warehouse_bins b ON b.id = l.bin_id";

/// Load and lock a bin row; `NotFound` when absent.
pub(crate) async fn load_bin_pg(conn: &mut PgConnection, id: i32) -> Result<WarehouseBin> {
    sqlx::query_as::<_, BinRow>("SELECT * FROM warehouse_bins WHERE id = $1 FOR UPDATE")
        .bind(id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(map_db_error)?
        .ok_or(CommerceError::NotFound)?
        .try_into()
}

/// Find a bin for a return disposition (explicit bin, else first active bin
/// of the preferred types, in order).
pub(crate) async fn find_disposition_bin_pg(
    conn: &mut PgConnection,
    warehouse_id: i32,
    explicit_bin_id: Option<i32>,
    preferred: &[BinType],
) -> Result<Option<WarehouseBin>> {
    if let Some(bin_id) = explicit_bin_id {
        let bin = load_bin_pg(conn, bin_id).await?;
        if bin.warehouse_id != warehouse_id {
            return Err(CommerceError::ValidationError(format!(
                "Bin {bin_id} does not belong to warehouse {warehouse_id}"
            )));
        }
        if !bin.is_active {
            return Err(CommerceError::ValidationError(format!("Bin {bin_id} is inactive")));
        }
        return Ok(Some(bin));
    }
    for bin_type in preferred {
        let row = sqlx::query_as::<_, BinRow>(
            "SELECT * FROM warehouse_bins WHERE warehouse_id = $1 AND bin_type = $2 AND is_active
             ORDER BY id LIMIT 1 FOR UPDATE",
        )
        .bind(warehouse_id)
        .bind(bin_type.to_string())
        .fetch_optional(&mut *conn)
        .await
        .map_err(map_db_error)?;
        if let Some(row) = row {
            return Ok(Some(row.try_into()?));
        }
    }
    Ok(None)
}

/// Apply a signed `(on_hand, allocated)` delta to one bin level.
pub(crate) async fn apply_bin_delta_pg(
    conn: &mut PgConnection,
    bin: &WarehouseBin,
    sku: &str,
    delta_on_hand: Decimal,
    delta_allocated: Decimal,
    now: DateTime<Utc>,
) -> Result<BinLevel> {
    let current = sqlx::query_as::<_, (Decimal, Decimal)>(
        "SELECT quantity_on_hand, quantity_allocated FROM inventory_bin_levels
         WHERE bin_id = $1 AND sku = $2 FOR UPDATE",
    )
    .bind(bin.id)
    .bind(sku)
    .fetch_optional(&mut *conn)
    .await
    .map_err(map_db_error)?;
    let (on_hand, allocated) = current.unwrap_or((Decimal::ZERO, Decimal::ZERO));
    let new_on_hand = on_hand + delta_on_hand;
    let new_allocated = allocated + delta_allocated;
    if new_on_hand < Decimal::ZERO
        || new_allocated < Decimal::ZERO
        || new_on_hand - new_allocated < Decimal::ZERO
    {
        return Err(CommerceError::InsufficientStock {
            sku: sku.to_string(),
            requested: delta_on_hand.abs().to_string(),
            available: (on_hand - allocated).to_string(),
        });
    }
    if let Some(capacity) = bin.capacity {
        if new_on_hand > capacity {
            return Err(CommerceError::ValidationError(format!(
                "Bin {} capacity {} exceeded: {} on hand after adjustment",
                bin.code, capacity, new_on_hand
            )));
        }
    }
    sqlx::query(
        "INSERT INTO inventory_bin_levels (bin_id, sku, quantity_on_hand, quantity_allocated, updated_at)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (bin_id, sku) DO UPDATE SET quantity_on_hand = EXCLUDED.quantity_on_hand,
             quantity_allocated = EXCLUDED.quantity_allocated, updated_at = EXCLUDED.updated_at",
    )
    .bind(bin.id)
    .bind(sku)
    .bind(new_on_hand)
    .bind(new_allocated)
    .bind(now)
    .execute(&mut *conn)
    .await
    .map_err(map_db_error)?;
    Ok(BinLevel {
        bin_id: bin.id,
        warehouse_id: bin.warehouse_id,
        sku: sku.to_string(),
        quantity_on_hand: new_on_hand,
        quantity_allocated: new_allocated,
        quantity_available: new_on_hand - new_allocated,
        updated_at: now,
    })
}

/// Apply a signed `(on_hand, allocated)` delta to the warehouse-level balance
/// (`inventory_balances` at `location_id = warehouse_id`).
#[allow(clippy::too_many_arguments)]
pub(crate) async fn apply_warehouse_delta_pg(
    conn: &mut PgConnection,
    warehouse_id: i32,
    sku: &str,
    delta_on_hand: Decimal,
    delta_allocated: Decimal,
    reason: &str,
    reference_type: Option<&str>,
    reference_id: Option<&str>,
    now: DateTime<Utc>,
) -> Result<()> {
    let item_id: i64 = sqlx::query_scalar("SELECT id FROM inventory_items WHERE sku = $1")
        .bind(sku)
        .fetch_optional(&mut *conn)
        .await
        .map_err(map_db_error)?
        .ok_or_else(|| {
            CommerceError::ValidationError(format!(
                "Inventory item {sku} not found; create it before stocking bins"
            ))
        })?;
    let wh: Option<(String, String)> =
        sqlx::query_as("SELECT code, name FROM warehouses WHERE id = $1")
            .bind(warehouse_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(map_db_error)?;
    let (code, name) =
        wh.unwrap_or_else(|| (format!("WH-{warehouse_id}"), format!("Warehouse {warehouse_id}")));
    sqlx::query(
        "INSERT INTO inventory_locations (id, name, code) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
    )
    .bind(warehouse_id)
    .bind(name)
    .bind(code)
    .execute(&mut *conn)
    .await
    .map_err(map_db_error)?;

    let current = sqlx::query_as::<_, (Decimal, Decimal)>(
        "SELECT quantity_on_hand, quantity_allocated FROM inventory_balances
         WHERE item_id = $1 AND location_id = $2 FOR UPDATE",
    )
    .bind(item_id)
    .bind(warehouse_id)
    .fetch_optional(&mut *conn)
    .await
    .map_err(map_db_error)?;
    let (on_hand, allocated) = current.unwrap_or((Decimal::ZERO, Decimal::ZERO));
    let new_on_hand = on_hand + delta_on_hand;
    let new_allocated = allocated + delta_allocated;
    let new_available = new_on_hand - new_allocated;
    if new_on_hand < Decimal::ZERO || new_allocated < Decimal::ZERO || new_available < Decimal::ZERO
    {
        return Err(CommerceError::InsufficientStock {
            sku: sku.to_string(),
            requested: delta_on_hand.abs().to_string(),
            available: (on_hand - allocated).to_string(),
        });
    }
    sqlx::query(
        "INSERT INTO inventory_balances (item_id, location_id, quantity_on_hand, quantity_allocated,
         quantity_available, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (item_id, location_id) DO UPDATE SET
             quantity_on_hand = EXCLUDED.quantity_on_hand,
             quantity_allocated = EXCLUDED.quantity_allocated,
             quantity_available = EXCLUDED.quantity_available,
             version = inventory_balances.version + 1,
             updated_at = EXCLUDED.updated_at",
    )
    .bind(item_id)
    .bind(warehouse_id)
    .bind(new_on_hand)
    .bind(new_allocated)
    .bind(new_available)
    .bind(now)
    .execute(&mut *conn)
    .await
    .map_err(map_db_error)?;
    if !delta_on_hand.is_zero() {
        sqlx::query(
            "INSERT INTO inventory_transactions (item_id, location_id, transaction_type, quantity,
             reference_type, reference_id, reason, created_at)
             VALUES ($1, $2, 'adjustment', $3, $4, $5, $6, $7)",
        )
        .bind(item_id)
        .bind(warehouse_id)
        .bind(delta_on_hand)
        .bind(reference_type)
        .bind(reference_id)
        .bind(reason)
        .bind(now)
        .execute(&mut *conn)
        .await
        .map_err(map_db_error)?;
    }
    Ok(())
}

/// Insert a bin movement audit row.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn insert_bin_movement_pg(
    conn: &mut PgConnection,
    movement_type: BinMovementType,
    from_bin_id: Option<i32>,
    to_bin_id: Option<i32>,
    sku: &str,
    quantity: Decimal,
    reason: Option<&str>,
    reference_type: Option<&str>,
    reference_id: Option<&str>,
    performed_by: Option<&str>,
    now: DateTime<Utc>,
) -> Result<BinMovement> {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO inventory_bin_movements (id, movement_type, from_bin_id, to_bin_id, sku,
         quantity, reason, reference_type, reference_id, performed_by, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
    )
    .bind(id)
    .bind(movement_type.to_string())
    .bind(from_bin_id)
    .bind(to_bin_id)
    .bind(sku)
    .bind(quantity)
    .bind(reason)
    .bind(reference_type)
    .bind(reference_id)
    .bind(performed_by)
    .bind(now)
    .execute(&mut *conn)
    .await
    .map_err(map_db_error)?;
    Ok(BinMovement {
        id,
        movement_type,
        from_bin_id,
        to_bin_id,
        sku: sku.to_string(),
        quantity,
        reason: reason.map(str::to_string),
        reference_type: reference_type.map(str::to_string),
        reference_id: reference_id.map(str::to_string),
        performed_by: performed_by.map(str::to_string),
        created_at: now,
    })
}

impl PgBinRepository {
    #[must_use]
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn push_filters(builder: &mut QueryBuilder<'_, Postgres>, filter: &WarehouseBinFilter) {
        builder.push(" WHERE 1=1");
        if let Some(wh) = filter.warehouse_id {
            builder.push(" AND warehouse_id = ").push_bind(wh);
        }
        if let Some(t) = filter.bin_type {
            builder.push(" AND bin_type = ").push_bind(t.to_string());
        }
        if let Some(z) = &filter.zone {
            builder.push(" AND zone = ").push_bind(z.clone());
        }
        if let Some(a) = filter.is_active {
            builder.push(" AND is_active = ").push_bind(a);
        }
    }

    pub async fn create_bin_async(&self, input: CreateWarehouseBin) -> Result<WarehouseBin> {
        let code = input.code.trim().to_string();
        if code.is_empty() {
            return Err(CommerceError::ValidationError("Bin code cannot be empty".into()));
        }
        if input.capacity.is_some_and(|c| c <= Decimal::ZERO) {
            return Err(CommerceError::ValidationError("Bin capacity must be positive".into()));
        }
        let exists: Option<i32> = sqlx::query_scalar("SELECT id FROM warehouses WHERE id = $1")
            .bind(input.warehouse_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;
        if exists.is_none() {
            return Err(CommerceError::ValidationError(format!(
                "Warehouse {} not found",
                input.warehouse_id
            )));
        }
        let now = Utc::now();
        sqlx::query_as::<_, BinRow>(
            "INSERT INTO warehouse_bins (warehouse_id, code, zone, aisle, rack, shelf, position,
             bin_type, is_active, capacity, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, TRUE, $9, $10, $10) RETURNING *",
        )
        .bind(input.warehouse_id)
        .bind(code)
        .bind(input.zone)
        .bind(input.aisle)
        .bind(input.rack)
        .bind(input.shelf)
        .bind(input.position)
        .bind(input.bin_type.to_string())
        .bind(input.capacity)
        .bind(now)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?
        .try_into()
    }

    pub async fn get_bin_async(&self, id: i32) -> Result<Option<WarehouseBin>> {
        sqlx::query_as::<_, BinRow>("SELECT * FROM warehouse_bins WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?
            .map(TryInto::try_into)
            .transpose()
    }

    pub async fn get_bin_by_code_async(
        &self,
        warehouse_id: i32,
        code: &str,
    ) -> Result<Option<WarehouseBin>> {
        sqlx::query_as::<_, BinRow>(
            "SELECT * FROM warehouse_bins WHERE warehouse_id = $1 AND code = $2",
        )
        .bind(warehouse_id)
        .bind(code)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?
        .map(TryInto::try_into)
        .transpose()
    }

    pub async fn update_bin_async(
        &self,
        id: i32,
        input: UpdateWarehouseBin,
    ) -> Result<WarehouseBin> {
        if let Some(Some(c)) = input.capacity {
            if c <= Decimal::ZERO {
                return Err(CommerceError::ValidationError("Bin capacity must be positive".into()));
            }
        }
        let mut builder = QueryBuilder::<Postgres>::new("UPDATE warehouse_bins SET updated_at = ");
        builder.push_bind(Utc::now());
        if let Some(v) = input.zone {
            builder.push(", zone = ").push_bind(v);
        }
        if let Some(v) = input.aisle {
            builder.push(", aisle = ").push_bind(v);
        }
        if let Some(v) = input.rack {
            builder.push(", rack = ").push_bind(v);
        }
        if let Some(v) = input.shelf {
            builder.push(", shelf = ").push_bind(v);
        }
        if let Some(v) = input.position {
            builder.push(", position = ").push_bind(v);
        }
        if let Some(t) = input.bin_type {
            builder.push(", bin_type = ").push_bind(t.to_string());
        }
        if let Some(a) = input.is_active {
            builder.push(", is_active = ").push_bind(a);
        }
        if let Some(cap) = input.capacity {
            builder.push(", capacity = ").push_bind(cap);
        }
        builder.push(" WHERE id = ").push_bind(id).push(" RETURNING *");
        builder
            .build_query_as::<BinRow>()
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?
            .ok_or(CommerceError::NotFound)?
            .try_into()
    }

    pub async fn list_bins_async(&self, filter: WarehouseBinFilter) -> Result<Vec<WarehouseBin>> {
        let mut builder = QueryBuilder::<Postgres>::new("SELECT * FROM warehouse_bins");
        Self::push_filters(&mut builder, &filter);
        builder.push(" ORDER BY warehouse_id, code");
        if let Some(limit) = filter.limit {
            builder.push(" LIMIT ").push_bind(i64::from(limit.min(1000)));
            if let Some(offset) = filter.offset {
                builder.push(" OFFSET ").push_bind(i64::from(offset));
            }
        }
        builder
            .build_query_as::<BinRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?
            .into_iter()
            .map(TryInto::try_into)
            .collect()
    }

    pub async fn count_bins_async(&self, filter: WarehouseBinFilter) -> Result<u64> {
        let mut builder = QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM warehouse_bins");
        Self::push_filters(&mut builder, &filter);
        let n: i64 =
            builder.build_query_scalar().fetch_one(&self.pool).await.map_err(map_db_error)?;
        Ok(n as u64)
    }

    pub async fn delete_bin_async(&self, id: i32) -> Result<()> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        load_bin_pg(tx.as_mut(), id).await?;
        let holding: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM inventory_bin_levels
             WHERE bin_id = $1 AND (quantity_on_hand <> 0 OR quantity_allocated <> 0)",
        )
        .bind(id)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        if holding > 0 {
            return Err(CommerceError::NotPermitted(format!(
                "Bin {id} still holds stock; move or adjust it out first"
            )));
        }
        sqlx::query("DELETE FROM inventory_bin_levels WHERE bin_id = $1")
            .bind(id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        sqlx::query("DELETE FROM warehouse_bins WHERE id = $1")
            .bind(id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        tx.commit().await.map_err(map_db_error)
    }

    pub async fn get_bin_levels_async(&self, bin_id: i32) -> Result<Vec<BinLevel>> {
        let rows = sqlx::query_as::<_, LevelRow>(&format!(
            "{LEVEL_SELECT} WHERE l.bin_id = $1 ORDER BY l.sku"
        ))
        .bind(bin_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn get_bin_levels_for_sku_async(
        &self,
        warehouse_id: i32,
        sku: &str,
    ) -> Result<Vec<BinLevel>> {
        let rows = sqlx::query_as::<_, LevelRow>(&format!(
            "{LEVEL_SELECT} WHERE b.warehouse_id = $1 AND l.sku = $2 ORDER BY l.bin_id"
        ))
        .bind(warehouse_id)
        .bind(sku)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn adjust_bin_level_async(&self, input: AdjustBinLevel) -> Result<BinLevel> {
        if input.quantity.is_zero() {
            return Err(CommerceError::ValidationError(
                "Adjustment quantity cannot be zero".into(),
            ));
        }
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let now = Utc::now();
        let bin = load_bin_pg(tx.as_mut(), input.bin_id).await?;
        if !bin.is_active {
            return Err(CommerceError::ValidationError(format!("Bin {} is inactive", bin.code)));
        }
        let level =
            apply_bin_delta_pg(tx.as_mut(), &bin, &input.sku, input.quantity, Decimal::ZERO, now)
                .await?;
        apply_warehouse_delta_pg(
            tx.as_mut(),
            bin.warehouse_id,
            &input.sku,
            input.quantity,
            Decimal::ZERO,
            &input.reason,
            input.reference_type.as_deref(),
            input.reference_id.as_deref(),
            now,
        )
        .await?;
        let (from, to) = if input.quantity.is_sign_negative() {
            (Some(bin.id), None)
        } else {
            (None, Some(bin.id))
        };
        insert_bin_movement_pg(
            tx.as_mut(),
            BinMovementType::Adjustment,
            from,
            to,
            &input.sku,
            input.quantity.abs(),
            Some(&input.reason),
            input.reference_type.as_deref(),
            input.reference_id.as_deref(),
            input.performed_by.as_deref(),
            now,
        )
        .await?;
        tx.commit().await.map_err(map_db_error)?;
        Ok(level)
    }

    pub async fn move_between_bins_async(&self, input: MoveBetweenBins) -> Result<BinMovement> {
        if input.quantity <= Decimal::ZERO {
            return Err(CommerceError::ValidationError("Move quantity must be positive".into()));
        }
        if input.from_bin_id == input.to_bin_id {
            return Err(CommerceError::ValidationError(
                "Source and destination bins must differ".into(),
            ));
        }
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let now = Utc::now();
        // Lock in a stable order to avoid deadlocks between opposite moves.
        let (first, second) = if input.from_bin_id < input.to_bin_id {
            (input.from_bin_id, input.to_bin_id)
        } else {
            (input.to_bin_id, input.from_bin_id)
        };
        let a = load_bin_pg(tx.as_mut(), first).await?;
        let b = load_bin_pg(tx.as_mut(), second).await?;
        let (from, to) = if a.id == input.from_bin_id { (a, b) } else { (b, a) };
        if from.warehouse_id != to.warehouse_id {
            return Err(CommerceError::ValidationError(
                "Bins belong to different warehouses; use a transfer order".into(),
            ));
        }
        if !to.is_active {
            return Err(CommerceError::ValidationError(format!(
                "Destination bin {} is inactive",
                to.code
            )));
        }
        apply_bin_delta_pg(tx.as_mut(), &from, &input.sku, -input.quantity, Decimal::ZERO, now)
            .await?;
        apply_bin_delta_pg(tx.as_mut(), &to, &input.sku, input.quantity, Decimal::ZERO, now)
            .await?;
        let movement = insert_bin_movement_pg(
            tx.as_mut(),
            BinMovementType::Transfer,
            Some(from.id),
            Some(to.id),
            &input.sku,
            input.quantity,
            input.reason.as_deref(),
            None,
            None,
            input.performed_by.as_deref(),
            now,
        )
        .await?;
        tx.commit().await.map_err(map_db_error)?;
        Ok(movement)
    }

    pub async fn reconcile_async(&self, warehouse_id: i32, sku: &str) -> Result<BinReconciliation> {
        let bin_on_hand: Option<Decimal> = sqlx::query_scalar(
            "SELECT SUM(l.quantity_on_hand) FROM inventory_bin_levels l
             JOIN warehouse_bins b ON b.id = l.bin_id WHERE b.warehouse_id = $1 AND l.sku = $2",
        )
        .bind(warehouse_id)
        .bind(sku)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;
        let warehouse_on_hand: Option<Decimal> = sqlx::query_scalar(
            "SELECT b.quantity_on_hand FROM inventory_balances b
             JOIN inventory_items i ON i.id = b.item_id WHERE i.sku = $1 AND b.location_id = $2",
        )
        .bind(sku)
        .bind(warehouse_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;
        let bin_on_hand = bin_on_hand.unwrap_or(Decimal::ZERO);
        let warehouse_on_hand = warehouse_on_hand.unwrap_or(Decimal::ZERO);
        Ok(BinReconciliation {
            warehouse_id,
            sku: sku.to_string(),
            bin_on_hand,
            warehouse_on_hand,
            variance: warehouse_on_hand - bin_on_hand,
        })
    }

    /// Movements touching a bin (most recent first).
    pub async fn get_bin_movements_async(
        &self,
        bin_id: i32,
        limit: u32,
    ) -> Result<Vec<BinMovement>> {
        let rows = sqlx::query_as::<_, MovementRow>(
            "SELECT * FROM inventory_bin_movements WHERE from_bin_id = $1 OR to_bin_id = $1
             ORDER BY created_at DESC LIMIT $2",
        )
        .bind(bin_id)
        .bind(i64::from(limit.clamp(1, 1000)))
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;
        rows.into_iter().map(TryInto::try_into).collect()
    }
}

impl BinRepository for PgBinRepository {
    fn create_bin(&self, input: CreateWarehouseBin) -> Result<WarehouseBin> {
        block_on(self.create_bin_async(input))
    }
    fn get_bin(&self, id: i32) -> Result<Option<WarehouseBin>> {
        block_on(self.get_bin_async(id))
    }
    fn get_bin_by_code(&self, warehouse_id: i32, code: &str) -> Result<Option<WarehouseBin>> {
        block_on(self.get_bin_by_code_async(warehouse_id, code))
    }
    fn update_bin(&self, id: i32, input: UpdateWarehouseBin) -> Result<WarehouseBin> {
        block_on(self.update_bin_async(id, input))
    }
    fn list_bins(&self, filter: WarehouseBinFilter) -> Result<Vec<WarehouseBin>> {
        block_on(self.list_bins_async(filter))
    }
    fn count_bins(&self, filter: WarehouseBinFilter) -> Result<u64> {
        block_on(self.count_bins_async(filter))
    }
    fn delete_bin(&self, id: i32) -> Result<()> {
        block_on(self.delete_bin_async(id))
    }
    fn get_bin_levels(&self, bin_id: i32) -> Result<Vec<BinLevel>> {
        block_on(self.get_bin_levels_async(bin_id))
    }
    fn get_bin_levels_for_sku(&self, warehouse_id: i32, sku: &str) -> Result<Vec<BinLevel>> {
        block_on(self.get_bin_levels_for_sku_async(warehouse_id, sku))
    }
    fn adjust_bin_level(&self, input: AdjustBinLevel) -> Result<BinLevel> {
        block_on(self.adjust_bin_level_async(input))
    }
    fn move_between_bins(&self, input: MoveBetweenBins) -> Result<BinMovement> {
        block_on(self.move_between_bins_async(input))
    }
    fn reconcile(&self, warehouse_id: i32, sku: &str) -> Result<BinReconciliation> {
        block_on(self.reconcile_async(warehouse_id, sku))
    }
}
