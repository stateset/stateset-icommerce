//! PostgreSQL implementation for fulfillment (pick/pack/ship) management

use super::{block_on, map_db_error};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;
use sqlx::{FromRow, Postgres, QueryBuilder};
use stateset_core::{
    AddCarton, AddCartonItem, BatchResult, Carton, CartonItem, CommerceError, CompletePick,
    CompleteShip, CreatePackTask, CreatePickTask, CreateShipTask, CreateWave, FulfillmentId,
    FulfillmentRepository, OrderId, PackStatus, PackTask, PackTaskFilter, PackageType, PickStatus,
    PickTask, PickTaskFilter, Result, ShipStatus, ShipTask, ShipTaskFilter, Wave, WaveFilter,
    WaveStatus, generate_carton_number, generate_wave_number, validate_batch_size,
};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct PgFulfillmentRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct WaveRow {
    id: Uuid,
    wave_number: String,
    warehouse_id: i32,
    status: String,
    order_count: i32,
    pick_count: i32,
    completed_pick_count: i32,
    priority: i32,
    started_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
    notes: Option<String>,
    created_by: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct PickRow {
    id: Uuid,
    wave_id: Option<Uuid>,
    order_id: Uuid,
    order_item_id: Uuid,
    warehouse_id: i32,
    status: String,
    sku: String,
    product_name: Option<String>,
    source_location_id: i32,
    source_location_code: Option<String>,
    quantity_requested: Decimal,
    quantity_picked: Decimal,
    quantity_short: Decimal,
    lot_id: Option<Uuid>,
    serial_number: Option<String>,
    assigned_to: Option<String>,
    priority: i32,
    pick_sequence: i32,
    started_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
    notes: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct PackRow {
    id: Uuid,
    order_id: Uuid,
    shipment_id: Option<Uuid>,
    status: String,
    carton_count: i32,
    total_weight_kg: Option<Decimal>,
    assigned_to: Option<String>,
    packing_station: Option<String>,
    started_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
    notes: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct CartonRow {
    id: Uuid,
    pack_task_id: Uuid,
    carton_number: String,
    package_type: String,
    weight_kg: Option<Decimal>,
    length_cm: Option<Decimal>,
    width_cm: Option<Decimal>,
    height_cm: Option<Decimal>,
    tracking_number: Option<String>,
    label_printed: bool,
    created_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct CartonItemRow {
    id: Uuid,
    carton_id: Uuid,
    sku: String,
    quantity: Decimal,
    lot_id: Option<Uuid>,
    serial_number: Option<String>,
}

#[derive(FromRow)]
struct ShipRow {
    id: Uuid,
    order_id: Uuid,
    shipment_id: Uuid,
    pack_task_id: Uuid,
    status: String,
    carrier: Option<String>,
    service_level: Option<String>,
    tracking_number: Option<String>,
    label_url: Option<String>,
    shipping_cost: Option<Decimal>,
    assigned_to: Option<String>,
    shipped_at: Option<DateTime<Utc>>,
    notes: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl PgFulfillmentRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Finish a status-guarded transition inside its transaction.
    ///
    /// A 0-row UPDATE means the guard rejected the write: the row is either gone
    /// (`NotFound`) or in a status the transition does not allow (`Conflict`,
    /// naming the status). Otherwise the row is read back inside the same
    /// transaction, which is then committed. `table` and `entity` are always
    /// in-crate literals, never caller input.
    async fn finish_transition<T, R>(
        mut tx: sqlx::Transaction<'_, Postgres>,
        changed: u64,
        table: &str,
        entity: &str,
        id: Uuid,
        action: &str,
        map: impl FnOnce(R) -> Result<T>,
    ) -> Result<T>
    where
        R: for<'r> FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
    {
        if changed == 0 {
            let current: Option<String> =
                sqlx::query_scalar(&format!("SELECT status FROM {table} WHERE id = $1"))
                    .bind(id)
                    .fetch_optional(tx.as_mut())
                    .await
                    .map_err(map_db_error)?;
            return Err(current.map_or(CommerceError::NotFound, |status| {
                CommerceError::Conflict(format!(
                    "cannot {action} {entity} {id}: status is {status}"
                ))
            }));
        }

        let row = sqlx::query_as::<_, R>(&format!("SELECT * FROM {table} WHERE id = $1"))
            .bind(id)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(map_db_error)?
            .ok_or(CommerceError::NotFound)?;
        tx.commit().await.map_err(map_db_error)?;
        map(row)
    }

    /// Guard that a pack task is still open for carton changes.
    ///
    /// Cartons may only be added while the pack task is in one of
    /// `Pending`/`ReadyToPack`/`Assigned`/`InProgress`; adding one to a
    /// `Completed` or `Cancelled` pack task inflates `pack_tasks.carton_count`
    /// for a sealed shipment.
    async fn ensure_pack_open(
        conn: &mut sqlx::PgConnection,
        pack_task_id: Uuid,
        action: &str,
    ) -> Result<()> {
        let status: Option<String> =
            sqlx::query_scalar("SELECT status FROM pack_tasks WHERE id = $1 FOR UPDATE")
                .bind(pack_task_id)
                .fetch_optional(conn)
                .await
                .map_err(map_db_error)?;
        match status {
            None => Err(CommerceError::NotFound),
            Some(status) if matches!(status.as_str(), "completed" | "cancelled") => {
                Err(CommerceError::Conflict(format!(
                    "cannot {action} pack task {pack_task_id}: status is {status}"
                )))
            }
            Some(_) => Ok(()),
        }
    }

    fn row_to_wave(row: WaveRow) -> Result<Wave> {
        let status: WaveStatus = row.status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid wave.status '{}': {}", row.status, e))
        })?;

        Ok(Wave {
            id: row.id.into(),
            wave_number: row.wave_number,
            warehouse_id: row.warehouse_id,
            status,
            order_count: row.order_count,
            pick_count: row.pick_count,
            completed_pick_count: row.completed_pick_count,
            priority: row.priority,
            started_at: row.started_at,
            completed_at: row.completed_at,
            notes: row.notes,
            created_by: row.created_by,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    fn row_to_pick(row: PickRow) -> Result<PickTask> {
        let status: PickStatus = row.status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid pick_task.status '{}': {}",
                row.status, e
            ))
        })?;

        Ok(PickTask {
            id: row.id,
            wave_id: row.wave_id.map(Into::into),
            order_id: row.order_id.into(),
            order_item_id: row.order_item_id.into(),
            warehouse_id: row.warehouse_id,
            status,
            sku: row.sku,
            product_name: row.product_name,
            source_location_id: row.source_location_id,
            source_location_code: row.source_location_code,
            quantity_requested: row.quantity_requested,
            quantity_picked: row.quantity_picked,
            quantity_short: row.quantity_short,
            lot_id: row.lot_id,
            serial_number: row.serial_number,
            assigned_to: row.assigned_to,
            priority: row.priority,
            pick_sequence: row.pick_sequence,
            started_at: row.started_at,
            completed_at: row.completed_at,
            notes: row.notes,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    fn row_to_pack(row: PackRow) -> Result<PackTask> {
        let status: PackStatus = row.status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid pack_task.status '{}': {}",
                row.status, e
            ))
        })?;

        Ok(PackTask {
            id: row.id,
            order_id: row.order_id.into(),
            shipment_id: row.shipment_id.map(Into::into),
            status,
            carton_count: row.carton_count,
            total_weight_kg: row.total_weight_kg,
            assigned_to: row.assigned_to,
            packing_station: row.packing_station,
            started_at: row.started_at,
            completed_at: row.completed_at,
            notes: row.notes,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    fn row_to_carton(row: CartonRow) -> Result<Carton> {
        let package_type: PackageType = row.package_type.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid carton.package_type '{}': {}",
                row.package_type, e
            ))
        })?;

        Ok(Carton {
            id: row.id,
            pack_task_id: row.pack_task_id,
            carton_number: row.carton_number,
            package_type,
            weight_kg: row.weight_kg,
            length_cm: row.length_cm,
            width_cm: row.width_cm,
            height_cm: row.height_cm,
            tracking_number: row.tracking_number,
            label_printed: row.label_printed,
            created_at: row.created_at,
        })
    }

    fn row_to_carton_item(row: CartonItemRow) -> CartonItem {
        CartonItem {
            id: row.id,
            carton_id: row.carton_id,
            sku: row.sku,
            quantity: row.quantity,
            lot_id: row.lot_id,
            serial_number: row.serial_number,
        }
    }

    fn row_to_ship(row: ShipRow) -> Result<ShipTask> {
        let status: ShipStatus = row.status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid ship_task.status '{}': {}",
                row.status, e
            ))
        })?;

        Ok(ShipTask {
            id: row.id,
            order_id: row.order_id.into(),
            shipment_id: row.shipment_id.into(),
            pack_task_id: row.pack_task_id,
            status,
            carrier: row.carrier,
            service_level: row.service_level,
            tracking_number: row.tracking_number,
            label_url: row.label_url,
            shipping_cost: row.shipping_cost,
            assigned_to: row.assigned_to,
            shipped_at: row.shipped_at,
            notes: row.notes,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    pub async fn create_wave_async(&self, input: CreateWave) -> Result<Wave> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let now = Utc::now();
        let id = Uuid::new_v4();
        let wave_number = generate_wave_number();
        let order_count = input.order_ids.len() as i32;

        sqlx::query(
            r#"
            INSERT INTO waves (id, wave_number, warehouse_id, status, order_count, priority, notes, created_by, created_at, updated_at)
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$9)
            "#,
        )
        .bind(id)
        .bind(&wave_number)
        .bind(input.warehouse_id)
        .bind(WaveStatus::Draft.to_string())
        .bind(order_count)
        .bind(input.priority.unwrap_or(0))
        .bind(&input.notes)
        .bind(&input.created_by)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        for order_id in &input.order_ids {
            sqlx::query("INSERT INTO wave_orders (wave_id, order_id) VALUES ($1,$2)")
                .bind(id)
                .bind(order_id)
                .execute(tx.as_mut())
                .await
                .map_err(map_db_error)?;
        }

        tx.commit().await.map_err(map_db_error)?;

        self.get_wave_async(id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to create wave".into()))
    }

    pub async fn get_wave_async(&self, id: Uuid) -> Result<Option<Wave>> {
        let row = sqlx::query_as::<_, WaveRow>("SELECT * FROM waves WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        row.map(Self::row_to_wave).transpose()
    }

    pub async fn get_wave_by_number_async(&self, number: &str) -> Result<Option<Wave>> {
        let row = sqlx::query_as::<_, WaveRow>("SELECT * FROM waves WHERE wave_number = $1")
            .bind(number)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        row.map(Self::row_to_wave).transpose()
    }

    pub async fn list_waves_async(&self, filter: WaveFilter) -> Result<Vec<Wave>> {
        let mut builder = QueryBuilder::<Postgres>::new("SELECT * FROM waves WHERE 1=1");

        if let Some(warehouse_id) = filter.warehouse_id {
            builder.push(" AND warehouse_id = ").push_bind(warehouse_id);
        }
        if let Some(status) = filter.status {
            builder.push(" AND status = ").push_bind(status.to_string());
        }
        if let Some(from_date) = filter.from_date {
            builder.push(" AND created_at >= ").push_bind(from_date);
        }
        if let Some(to_date) = filter.to_date {
            builder.push(" AND created_at <= ").push_bind(to_date);
        }

        builder.push(" ORDER BY created_at DESC");

        builder.push(" LIMIT ").push_bind(super::effective_limit(filter.limit));
        if let Some(offset) = filter.offset {
            builder.push(" OFFSET ").push_bind(offset as i64);
        }

        let rows = builder
            .build_query_as::<WaveRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_wave).collect::<Result<Vec<_>>>()
    }

    /// Release a wave for picking.
    ///
    /// Legal only from [`WaveStatus::Draft`]; the guard used to be a silent
    /// `AND status = 'draft'` that reported success (returning the untouched
    /// wave) when it matched nothing.
    pub async fn release_wave_async(&self, id: Uuid) -> Result<Wave> {
        let now = Utc::now();
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let changed = sqlx::query(
            "UPDATE waves SET status = $1, started_at = $2 WHERE id = $3 AND status = 'draft'",
        )
        .bind(WaveStatus::Released.to_string())
        .bind(now)
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .rows_affected();

        Self::finish_transition(tx, changed, "waves", "wave", id, "release", Self::row_to_wave)
            .await
    }

    /// Complete a wave.
    ///
    /// Legal only from [`WaveStatus::Released`] or [`WaveStatus::InProgress`]:
    /// a `Draft` wave was never on the floor, and a `Cancelled` or already
    /// `Completed` wave is terminal. Completing a cancelled wave used to
    /// succeed, resurrecting it with counters that describe nothing.
    pub async fn complete_wave_async(&self, id: Uuid) -> Result<Wave> {
        let now = Utc::now();
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let changed = sqlx::query(
            "UPDATE waves SET status = $1, completed_at = $2
             WHERE id = $3 AND status IN ('released', 'in_progress')",
        )
        .bind(WaveStatus::Completed.to_string())
        .bind(now)
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .rows_affected();

        Self::finish_transition(tx, changed, "waves", "wave", id, "complete", Self::row_to_wave)
            .await
    }

    /// Cancel a wave.
    ///
    /// Legal from [`WaveStatus::Draft`], [`WaveStatus::Released`] or
    /// [`WaveStatus::InProgress`]; a `Completed` wave's picks are already folded
    /// into its counters and a `Cancelled` one is terminal.
    pub async fn cancel_wave_async(&self, id: Uuid) -> Result<Wave> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let changed = sqlx::query(
            "UPDATE waves SET status = $1
             WHERE id = $2 AND status IN ('draft', 'released', 'in_progress')",
        )
        .bind(WaveStatus::Cancelled.to_string())
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .rows_affected();

        Self::finish_transition(tx, changed, "waves", "wave", id, "cancel", Self::row_to_wave).await
    }

    pub async fn get_wave_orders_async(&self, wave_id: Uuid) -> Result<Vec<Uuid>> {
        let rows =
            sqlx::query_as::<_, (Uuid,)>("SELECT order_id FROM wave_orders WHERE wave_id = $1")
                .bind(wave_id)
                .fetch_all(&self.pool)
                .await
                .map_err(map_db_error)?;

        Ok(rows.into_iter().map(|row| row.0).collect())
    }

    /// Count waves matching `filter` (same filters as `list_waves_async`).
    pub async fn count_waves_async(&self, filter: WaveFilter) -> Result<u64> {
        let mut builder = QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM waves WHERE 1=1");

        if let Some(warehouse_id) = filter.warehouse_id {
            builder.push(" AND warehouse_id = ").push_bind(warehouse_id);
        }
        if let Some(status) = filter.status {
            builder.push(" AND status = ").push_bind(status.to_string());
        }
        if let Some(from_date) = filter.from_date {
            builder.push(" AND created_at >= ").push_bind(from_date);
        }
        if let Some(to_date) = filter.to_date {
            builder.push(" AND created_at <= ").push_bind(to_date);
        }

        let row =
            builder.build_query_as::<(i64,)>().fetch_one(&self.pool).await.map_err(map_db_error)?;

        Ok(row.0 as u64)
    }

    /// Insert one pick task on `conn` and read it back.
    ///
    /// Shared by `create_pick_async` and `create_picks_for_order_async` so a
    /// multi-line order's picks are created by the same statement in one
    /// transaction.
    async fn insert_pick_on(
        conn: &mut sqlx::PgConnection,
        input: &CreatePickTask,
    ) -> Result<PickTask> {
        let now = Utc::now();
        let id = Uuid::new_v4();

        sqlx::query(
            r#"
            INSERT INTO pick_tasks (
                id, wave_id, order_id, order_item_id, warehouse_id, status, sku, product_name,
                source_location_id, quantity_requested, lot_id, serial_number, priority, notes, created_at, updated_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$15)
            "#,
        )
        .bind(id)
        .bind(input.wave_id)
        .bind(input.order_id)
        .bind(input.order_item_id)
        .bind(input.warehouse_id)
        .bind(PickStatus::Pending.to_string())
        .bind(&input.sku)
        .bind(&input.product_name)
        .bind(input.source_location_id)
        .bind(input.quantity_requested)
        .bind(input.lot_id)
        .bind(&input.serial_number)
        .bind(input.priority.unwrap_or(0))
        .bind(&input.notes)
        .bind(now)
        .execute(&mut *conn)
        .await
        .map_err(map_db_error)?;

        let row = sqlx::query_as::<_, PickRow>("SELECT * FROM pick_tasks WHERE id = $1")
            .bind(id)
            .fetch_optional(conn)
            .await
            .map_err(map_db_error)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to create pick".into()))?;

        Self::row_to_pick(row)
    }

    pub async fn create_pick_async(&self, input: CreatePickTask) -> Result<PickTask> {
        let mut conn = self.pool.acquire().await.map_err(map_db_error)?;
        Self::insert_pick_on(&mut conn, &input).await
    }

    pub async fn get_pick_async(&self, id: Uuid) -> Result<Option<PickTask>> {
        let row = sqlx::query_as::<_, PickRow>("SELECT * FROM pick_tasks WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        row.map(Self::row_to_pick).transpose()
    }

    pub async fn list_picks_async(&self, filter: PickTaskFilter) -> Result<Vec<PickTask>> {
        let mut builder = QueryBuilder::<Postgres>::new("SELECT * FROM pick_tasks WHERE 1=1");

        if let Some(warehouse_id) = filter.warehouse_id {
            builder.push(" AND warehouse_id = ").push_bind(warehouse_id);
        }
        if let Some(wave_id) = filter.wave_id {
            builder.push(" AND wave_id = ").push_bind(wave_id);
        }
        if let Some(order_id) = filter.order_id {
            builder.push(" AND order_id = ").push_bind(order_id);
        }
        if let Some(status) = filter.status {
            builder.push(" AND status = ").push_bind(status.to_string());
        }
        if let Some(assigned_to) = filter.assigned_to {
            builder.push(" AND assigned_to = ").push_bind(assigned_to);
        }

        builder.push(" ORDER BY created_at DESC");

        builder.push(" LIMIT ").push_bind(super::effective_limit(filter.limit));
        if let Some(offset) = filter.offset {
            builder.push(" OFFSET ").push_bind(offset as i64);
        }

        let rows = builder
            .build_query_as::<PickRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_pick).collect::<Result<Vec<_>>>()
    }

    /// Assign (or re-assign) a pick task to a worker.
    ///
    /// Legal from [`PickStatus::Pending`] or [`PickStatus::Assigned`]. It is
    /// refused once the pick has started or finished: the UPDATE also writes
    /// `status = 'assigned'`, so assigning a started pick would rewind it and
    /// assigning a finished one would resurrect it (and a later completion would
    /// double-count it into the wave).
    pub async fn assign_pick_async(&self, id: Uuid, assigned_to: &str) -> Result<PickTask> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let changed = sqlx::query(
            "UPDATE pick_tasks SET assigned_to = $1, status = $2
             WHERE id = $3 AND status IN ('pending', 'assigned')",
        )
        .bind(assigned_to)
        .bind(PickStatus::Assigned.to_string())
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .rows_affected();

        Self::finish_transition(
            tx,
            changed,
            "pick_tasks",
            "pick task",
            id,
            "assign",
            Self::row_to_pick,
        )
        .await
    }

    /// Start a pick task.
    ///
    /// Legal from [`PickStatus::Pending`] or [`PickStatus::Assigned`]. Starting
    /// an already-started pick would reset `started_at` (destroying the pick's
    /// measured duration); starting a finished or cancelled one is refused.
    pub async fn start_pick_async(&self, id: Uuid) -> Result<PickTask> {
        let now = Utc::now();
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let changed = sqlx::query(
            "UPDATE pick_tasks SET status = $1, started_at = $2
             WHERE id = $3 AND status IN ('pending', 'assigned')",
        )
        .bind(PickStatus::InProgress.to_string())
        .bind(now)
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .rows_affected();

        Self::finish_transition(
            tx,
            changed,
            "pick_tasks",
            "pick task",
            id,
            "start",
            Self::row_to_pick,
        )
        .await
    }

    pub async fn complete_pick_async(&self, input: CompletePick) -> Result<PickTask> {
        let now = Utc::now();
        let short_qty = input.quantity_short.unwrap_or(Decimal::ZERO);
        let status =
            if short_qty > Decimal::ZERO { PickStatus::Short } else { PickStatus::Completed };

        // The status read, guards, pick UPDATE and wave-counter increment all run
        // inside one transaction (with `SELECT ... FOR UPDATE`) so concurrent
        // completions serialize and only a real state transition folds into the
        // wave counter.
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let (status_str, requested): (String, Decimal) = sqlx::query_as(
            "SELECT status, quantity_requested FROM pick_tasks WHERE id = $1 FOR UPDATE",
        )
        .bind(input.pick_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .ok_or(CommerceError::NotFound)?;
        let current_status: PickStatus = status_str.parse().map_err(|_| {
            CommerceError::DatabaseError(format!("Invalid pick status '{status_str}'"))
        })?;

        // A pick that is already finalized (Completed/Short) has already
        // incremented the wave counter; re-completing it must be an idempotent
        // no-op, never a double-count.
        if matches!(current_status, PickStatus::Completed | PickStatus::Short) {
            drop(tx);
            return self.get_pick_async(input.pick_id).await?.ok_or(CommerceError::NotFound);
        }
        // A cancelled pick cannot be completed.
        if current_status == PickStatus::Cancelled {
            return Err(CommerceError::ValidationError(
                "Cannot complete a cancelled pick task".into(),
            ));
        }
        // Over-pick guard: cannot pick more than was requested.
        if input.quantity_picked > requested {
            return Err(CommerceError::ValidationError(format!(
                "Cannot pick {} of pick task {}: only {} were requested",
                input.quantity_picked, input.pick_id, requested
            )));
        }

        sqlx::query(
            r#"
            UPDATE pick_tasks SET
                status = $1,
                quantity_picked = $2,
                quantity_short = $3,
                lot_id = COALESCE($4, lot_id),
                serial_number = COALESCE($5, serial_number),
                completed_at = $6
            WHERE id = $7
            "#,
        )
        .bind(status.to_string())
        .bind(input.quantity_picked)
        .bind(short_qty)
        .bind(input.lot_id)
        .bind(&input.serial_number)
        .bind(now)
        .bind(input.pick_id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let wave_id: Option<Uuid> =
            sqlx::query_scalar("SELECT wave_id FROM pick_tasks WHERE id = $1")
                .bind(input.pick_id)
                .fetch_one(tx.as_mut())
                .await
                .map_err(map_db_error)?;
        if let Some(wave_id) = wave_id {
            sqlx::query(
                "UPDATE waves SET completed_pick_count = completed_pick_count + 1 WHERE id = $1",
            )
            .bind(wave_id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        }

        tx.commit().await.map_err(map_db_error)?;

        self.get_pick_async(input.pick_id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to complete pick".into()))
    }

    /// Report a shortage against a pick task, finalizing it as
    /// [`PickStatus::Short`].
    ///
    /// Legal from `Pending`/`Assigned`/`InProgress` only — the terminal states
    /// (`Completed`/`Short`/`Cancelled`) are refused, so a completed pick can no
    /// longer be silently rewritten as short.
    ///
    /// `Short` is a finalized outcome exactly like `Completed` (both end a pick
    /// for `is_order_ready_to_pack`), so — like `complete_pick` — the wave's
    /// `completed_pick_count` is incremented in the same transaction, and only
    /// when the guarded UPDATE actually transitioned the row.
    pub async fn report_short_async(
        &self,
        id: Uuid,
        short_qty: Decimal,
        reason: &str,
    ) -> Result<PickTask> {
        let now = Utc::now();
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let changed = sqlx::query(
            "UPDATE pick_tasks SET status = $1, quantity_short = $2, notes = $3, completed_at = $4
             WHERE id = $5 AND status IN ('pending', 'assigned', 'in_progress')",
        )
        .bind(PickStatus::Short.to_string())
        .bind(short_qty)
        .bind(reason)
        .bind(now)
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .rows_affected();

        if changed > 0 {
            let wave_id: Option<Uuid> =
                sqlx::query_scalar("SELECT wave_id FROM pick_tasks WHERE id = $1")
                    .bind(id)
                    .fetch_one(tx.as_mut())
                    .await
                    .map_err(map_db_error)?;
            if let Some(wave_id) = wave_id {
                sqlx::query(
                    "UPDATE waves SET completed_pick_count = completed_pick_count + 1 WHERE id = $1",
                )
                .bind(wave_id)
                .execute(tx.as_mut())
                .await
                .map_err(map_db_error)?;
            }
        }

        Self::finish_transition(
            tx,
            changed,
            "pick_tasks",
            "pick task",
            id,
            "report a shortage on",
            Self::row_to_pick,
        )
        .await
    }

    /// Cancel a pick task.
    ///
    /// Legal from `Pending`/`Assigned`/`InProgress` only. Cancelling a finished
    /// pick (`Completed`/`Short`) used to succeed while leaving the wave's
    /// `completed_pick_count` counting a pick that no longer claims to have
    /// happened, so the wave's counters stopped describing reality.
    pub async fn cancel_pick_async(&self, id: Uuid) -> Result<PickTask> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let changed = sqlx::query(
            "UPDATE pick_tasks SET status = $1
             WHERE id = $2 AND status IN ('pending', 'assigned', 'in_progress')",
        )
        .bind(PickStatus::Cancelled.to_string())
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .rows_affected();

        Self::finish_transition(
            tx,
            changed,
            "pick_tasks",
            "pick task",
            id,
            "cancel",
            Self::row_to_pick,
        )
        .await
    }

    pub async fn get_picks_for_order_async(&self, order_id: Uuid) -> Result<Vec<PickTask>> {
        self.list_picks_async(PickTaskFilter {
            order_id: Some(order_id.into()),
            ..Default::default()
        })
        .await
    }

    pub async fn get_picks_for_wave_async(&self, wave_id: Uuid) -> Result<Vec<PickTask>> {
        self.list_picks_async(PickTaskFilter {
            wave_id: Some(wave_id.into()),
            ..Default::default()
        })
        .await
    }

    /// Count pick tasks matching `filter` (same filters as `list_picks_async`).
    pub async fn count_picks_async(&self, filter: PickTaskFilter) -> Result<u64> {
        let mut builder =
            QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM pick_tasks WHERE 1=1");

        if let Some(warehouse_id) = filter.warehouse_id {
            builder.push(" AND warehouse_id = ").push_bind(warehouse_id);
        }
        if let Some(status) = filter.status {
            builder.push(" AND status = ").push_bind(status.to_string());
        }
        if let Some(wave_id) = filter.wave_id {
            builder.push(" AND wave_id = ").push_bind(wave_id);
        }
        if let Some(order_id) = filter.order_id {
            builder.push(" AND order_id = ").push_bind(order_id);
        }
        if let Some(assigned_to) = filter.assigned_to {
            builder.push(" AND assigned_to = ").push_bind(assigned_to);
        }

        let row =
            builder.build_query_as::<(i64,)>().fetch_one(&self.pool).await.map_err(map_db_error)?;

        Ok(row.0 as u64)
    }

    pub async fn create_pack_async(&self, input: CreatePackTask) -> Result<PackTask> {
        let now = Utc::now();
        let id = Uuid::new_v4();

        sqlx::query(
            "INSERT INTO pack_tasks (id, order_id, status, notes, created_at, updated_at) VALUES ($1,$2,$3,$4,$5,$5)",
        )
        .bind(id)
        .bind(input.order_id)
        .bind(PackStatus::Pending.to_string())
        .bind(&input.notes)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_pack_async(id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to create pack".into()))
    }

    pub async fn get_pack_async(&self, id: Uuid) -> Result<Option<PackTask>> {
        let row = sqlx::query_as::<_, PackRow>("SELECT * FROM pack_tasks WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        row.map(Self::row_to_pack).transpose()
    }

    pub async fn list_packs_async(&self, filter: PackTaskFilter) -> Result<Vec<PackTask>> {
        let mut builder = QueryBuilder::<Postgres>::new("SELECT * FROM pack_tasks WHERE 1=1");

        if let Some(order_id) = filter.order_id {
            builder.push(" AND order_id = ").push_bind(order_id);
        }
        if let Some(status) = filter.status {
            builder.push(" AND status = ").push_bind(status.to_string());
        }
        if let Some(assigned_to) = filter.assigned_to {
            builder.push(" AND assigned_to = ").push_bind(assigned_to);
        }

        builder.push(" ORDER BY created_at DESC");

        builder.push(" LIMIT ").push_bind(super::effective_limit(filter.limit));
        if let Some(offset) = filter.offset {
            builder.push(" OFFSET ").push_bind(offset as i64);
        }

        let rows = builder
            .build_query_as::<PackRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_pack).collect::<Result<Vec<_>>>()
    }

    /// Assign (or re-assign) a pack task to a packer.
    ///
    /// Legal from `Pending`/`ReadyToPack`/`Assigned`.
    pub async fn assign_pack_async(&self, id: Uuid, assigned_to: &str) -> Result<PackTask> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let changed = sqlx::query(
            "UPDATE pack_tasks SET assigned_to = $1, status = $2
             WHERE id = $3 AND status IN ('pending', 'ready_to_pack', 'assigned')",
        )
        .bind(assigned_to)
        .bind(PackStatus::Assigned.to_string())
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .rows_affected();

        Self::finish_transition(
            tx,
            changed,
            "pack_tasks",
            "pack task",
            id,
            "assign",
            Self::row_to_pack,
        )
        .await
    }

    /// Start a pack task.
    ///
    /// Legal from `Pending`/`ReadyToPack`/`Assigned`; re-starting an in-progress
    /// pack would reset `started_at`, and a completed or cancelled pack task is
    /// terminal.
    pub async fn start_pack_async(&self, id: Uuid) -> Result<PackTask> {
        let now = Utc::now();
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let changed = sqlx::query(
            "UPDATE pack_tasks SET status = $1, started_at = $2
             WHERE id = $3 AND status IN ('pending', 'ready_to_pack', 'assigned')",
        )
        .bind(PackStatus::InProgress.to_string())
        .bind(now)
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .rows_affected();

        Self::finish_transition(
            tx,
            changed,
            "pack_tasks",
            "pack task",
            id,
            "start",
            Self::row_to_pack,
        )
        .await
    }

    /// Complete a pack task.
    ///
    /// Legal from any open status (`Pending`/`ReadyToPack`/`Assigned`/
    /// `InProgress`) — packing need not be explicitly started — but refused for
    /// the terminal `Completed`/`Cancelled`: re-completing rewrote
    /// `completed_at`, and completing a cancelled pack task resurrected it and
    /// made `is_order_ready_to_ship` true for an order nobody packed.
    pub async fn complete_pack_async(&self, id: Uuid) -> Result<PackTask> {
        let now = Utc::now();
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let changed = sqlx::query(
            "UPDATE pack_tasks SET status = $1, completed_at = $2
             WHERE id = $3 AND status IN ('pending', 'ready_to_pack', 'assigned', 'in_progress')",
        )
        .bind(PackStatus::Completed.to_string())
        .bind(now)
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .rows_affected();

        Self::finish_transition(
            tx,
            changed,
            "pack_tasks",
            "pack task",
            id,
            "complete",
            Self::row_to_pack,
        )
        .await
    }

    /// Add a carton to a pack task.
    ///
    /// The carton INSERT and the `pack_tasks.carton_count` increment are one
    /// transaction: run separately on the pool, a failure between them left a
    /// carton the pack task's count did not know about. The pack task must still
    /// be open — cartons cannot be added to a completed or cancelled one.
    pub async fn add_carton_async(&self, input: AddCarton) -> Result<Carton> {
        let now = Utc::now();
        let id = Uuid::new_v4();
        let carton_number = generate_carton_number();

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        Self::ensure_pack_open(tx.as_mut(), input.pack_task_id, "add a carton to").await?;

        sqlx::query(
            r#"
            INSERT INTO cartons (id, pack_task_id, carton_number, package_type, weight_kg, length_cm, width_cm, height_cm, created_at)
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
            "#,
        )
        .bind(id)
        .bind(input.pack_task_id)
        .bind(&carton_number)
        .bind(input.package_type.to_string())
        .bind(input.weight_kg)
        .bind(input.length_cm)
        .bind(input.width_cm)
        .bind(input.height_cm)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        sqlx::query("UPDATE pack_tasks SET carton_count = carton_count + 1 WHERE id = $1")
            .bind(input.pack_task_id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        let row = sqlx::query_as::<_, CartonRow>("SELECT * FROM cartons WHERE id = $1")
            .bind(id)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(map_db_error)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to create carton".into()))?;

        tx.commit().await.map_err(map_db_error)?;

        Self::row_to_carton(row)
    }

    async fn get_carton_async(&self, id: Uuid) -> Result<Option<Carton>> {
        let row = sqlx::query_as::<_, CartonRow>("SELECT * FROM cartons WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        row.map(Self::row_to_carton).transpose()
    }

    /// Add an item to a carton.
    ///
    /// Refused once the owning pack task is completed or cancelled — the
    /// carton's contents are then a sealed record of what shipped.
    pub async fn add_carton_item_async(&self, input: AddCartonItem) -> Result<CartonItem> {
        let id = Uuid::new_v4();

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let pack_task_id: Uuid =
            sqlx::query_scalar("SELECT pack_task_id FROM cartons WHERE id = $1")
                .bind(input.carton_id)
                .fetch_optional(tx.as_mut())
                .await
                .map_err(map_db_error)?
                .ok_or(CommerceError::NotFound)?;
        Self::ensure_pack_open(tx.as_mut(), pack_task_id, "add carton items to").await?;

        sqlx::query(
            "INSERT INTO carton_items (id, carton_id, sku, quantity, lot_id, serial_number) VALUES ($1,$2,$3,$4,$5,$6)",
        )
        .bind(id)
        .bind(input.carton_id)
        .bind(&input.sku)
        .bind(input.quantity)
        .bind(input.lot_id)
        .bind(&input.serial_number)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let row = sqlx::query_as::<_, CartonItemRow>("SELECT * FROM carton_items WHERE id = $1")
            .bind(id)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(map_db_error)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to create carton item".into()))?;

        tx.commit().await.map_err(map_db_error)?;

        Ok(Self::row_to_carton_item(row))
    }

    pub async fn get_cartons_async(&self, pack_task_id: Uuid) -> Result<Vec<Carton>> {
        let rows = sqlx::query_as::<_, CartonRow>("SELECT * FROM cartons WHERE pack_task_id = $1")
            .bind(pack_task_id)
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_carton).collect::<Result<Vec<_>>>()
    }

    pub async fn get_carton_items_async(&self, carton_id: Uuid) -> Result<Vec<CartonItem>> {
        let rows =
            sqlx::query_as::<_, CartonItemRow>("SELECT * FROM carton_items WHERE carton_id = $1")
                .bind(carton_id)
                .fetch_all(&self.pool)
                .await
                .map_err(map_db_error)?;

        Ok(rows.into_iter().map(Self::row_to_carton_item).collect())
    }

    pub async fn mark_label_printed_async(&self, carton_id: Uuid) -> Result<Carton> {
        let changed = sqlx::query("UPDATE cartons SET label_printed = true WHERE id = $1")
            .bind(carton_id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?
            .rows_affected();
        if changed == 0 {
            // Parity with SQLite: an unknown carton is `NotFound`, not a
            // generic database error.
            return Err(CommerceError::NotFound);
        }

        self.get_carton_async(carton_id).await?.ok_or(CommerceError::NotFound)
    }

    /// Cancel a pack task.
    ///
    /// Legal from any open status; refused for `Completed` (its cartons already
    /// exist and its order counts as ready to ship) and for an already
    /// `Cancelled` task.
    pub async fn cancel_pack_async(&self, id: Uuid) -> Result<PackTask> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let changed = sqlx::query(
            "UPDATE pack_tasks SET status = $1
             WHERE id = $2 AND status IN ('pending', 'ready_to_pack', 'assigned', 'in_progress')",
        )
        .bind(PackStatus::Cancelled.to_string())
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .rows_affected();

        Self::finish_transition(
            tx,
            changed,
            "pack_tasks",
            "pack task",
            id,
            "cancel",
            Self::row_to_pack,
        )
        .await
    }

    pub async fn count_packs_async(&self, filter: PackTaskFilter) -> Result<u64> {
        let mut builder =
            QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM pack_tasks WHERE 1=1");

        if let Some(order_id) = filter.order_id {
            builder.push(" AND order_id = ").push_bind(order_id);
        }
        if let Some(status) = filter.status {
            builder.push(" AND status = ").push_bind(status.to_string());
        }
        if let Some(assigned_to) = filter.assigned_to {
            builder.push(" AND assigned_to = ").push_bind(assigned_to);
        }

        let row =
            builder.build_query_as::<(i64,)>().fetch_one(&self.pool).await.map_err(map_db_error)?;

        Ok(row.0 as u64)
    }

    pub async fn create_ship_async(&self, input: CreateShipTask) -> Result<ShipTask> {
        let now = Utc::now();
        let id = Uuid::new_v4();

        sqlx::query(
            "INSERT INTO ship_tasks (id, order_id, shipment_id, pack_task_id, status, carrier, service_level, notes, created_at, updated_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$9)",
        )
        .bind(id)
        .bind(input.order_id)
        .bind(input.shipment_id)
        .bind(input.pack_task_id)
        .bind(ShipStatus::Pending.to_string())
        .bind(&input.carrier)
        .bind(&input.service_level)
        .bind(&input.notes)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_ship_async(id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to create ship task".into()))
    }

    pub async fn get_ship_async(&self, id: Uuid) -> Result<Option<ShipTask>> {
        let row = sqlx::query_as::<_, ShipRow>("SELECT * FROM ship_tasks WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        row.map(Self::row_to_ship).transpose()
    }

    pub async fn list_ships_async(&self, filter: ShipTaskFilter) -> Result<Vec<ShipTask>> {
        let mut builder = QueryBuilder::<Postgres>::new("SELECT * FROM ship_tasks WHERE 1=1");

        if let Some(order_id) = filter.order_id {
            builder.push(" AND order_id = ").push_bind(order_id);
        }
        if let Some(status) = filter.status {
            builder.push(" AND status = ").push_bind(status.to_string());
        }
        if let Some(carrier) = filter.carrier {
            builder.push(" AND carrier = ").push_bind(carrier);
        }

        builder.push(" ORDER BY created_at DESC");

        builder.push(" LIMIT ").push_bind(super::effective_limit(filter.limit));
        if let Some(offset) = filter.offset {
            builder.push(" OFFSET ").push_bind(offset as i64);
        }

        let rows = builder
            .build_query_as::<ShipRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_ship).collect::<Result<Vec<_>>>()
    }

    /// Assign (or re-assign) a ship task.
    ///
    /// Legal from `Pending`/`ReadyToShip`/`LabelPrinted`; a shipped or cancelled
    /// task is terminal and cannot change hands.
    pub async fn assign_ship_async(&self, id: Uuid, assigned_to: &str) -> Result<ShipTask> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let changed = sqlx::query(
            "UPDATE ship_tasks SET assigned_to = $1
             WHERE id = $2 AND status IN ('pending', 'ready_to_ship', 'label_printed')",
        )
        .bind(assigned_to)
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .rows_affected();

        Self::finish_transition(
            tx,
            changed,
            "ship_tasks",
            "ship task",
            id,
            "assign",
            Self::row_to_ship,
        )
        .await
    }

    /// Record a printed shipping label.
    ///
    /// Legal from `Pending`/`ReadyToShip`/`LabelPrinted` (a re-print carries a
    /// new `label_url`); refused once the package is shipped or the task is
    /// cancelled, where a new label would contradict the carrier handoff.
    pub async fn print_label_async(&self, id: Uuid, label_url: &str) -> Result<ShipTask> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let changed = sqlx::query(
            "UPDATE ship_tasks SET status = $1, label_url = $2
             WHERE id = $3 AND status IN ('pending', 'ready_to_ship', 'label_printed')",
        )
        .bind(ShipStatus::LabelPrinted.to_string())
        .bind(label_url)
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .rows_affected();

        Self::finish_transition(
            tx,
            changed,
            "ship_tasks",
            "ship task",
            id,
            "print a label for",
            Self::row_to_ship,
        )
        .await
    }

    /// Complete a ship task (carrier handoff).
    ///
    /// Legal from `Pending`/`ReadyToShip`/`LabelPrinted`. Re-shipping an
    /// already-shipped task used to overwrite its tracking number, cost and
    /// `shipped_at`, and a cancelled task could be shipped.
    pub async fn complete_ship_async(&self, input: CompleteShip) -> Result<ShipTask> {
        let now = Utc::now();
        let id = input.ship_task_id;
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let changed = sqlx::query(
            "UPDATE ship_tasks SET status = $1, tracking_number = $2, shipping_cost = $3, shipped_at = $4
             WHERE id = $5 AND status IN ('pending', 'ready_to_ship', 'label_printed')",
        )
        .bind(ShipStatus::Shipped.to_string())
        .bind(&input.tracking_number)
        .bind(input.shipping_cost)
        .bind(now)
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .rows_affected();

        Self::finish_transition(
            tx,
            changed,
            "ship_tasks",
            "ship task",
            id,
            "complete",
            Self::row_to_ship,
        )
        .await
    }

    /// Cancel a ship task.
    ///
    /// Legal from `Pending`/`ReadyToShip`/`LabelPrinted`; a package already
    /// tendered to the carrier cannot be un-shipped by a status flip.
    pub async fn cancel_ship_async(&self, id: Uuid) -> Result<ShipTask> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let changed = sqlx::query(
            "UPDATE ship_tasks SET status = $1
             WHERE id = $2 AND status IN ('pending', 'ready_to_ship', 'label_printed')",
        )
        .bind(ShipStatus::Cancelled.to_string())
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .rows_affected();

        Self::finish_transition(
            tx,
            changed,
            "ship_tasks",
            "ship task",
            id,
            "cancel",
            Self::row_to_ship,
        )
        .await
    }

    pub async fn count_ships_async(&self, filter: ShipTaskFilter) -> Result<u64> {
        let mut builder =
            QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM ship_tasks WHERE 1=1");

        if let Some(order_id) = filter.order_id {
            builder.push(" AND order_id = ").push_bind(order_id);
        }
        if let Some(status) = filter.status {
            builder.push(" AND status = ").push_bind(status.to_string());
        }
        if let Some(carrier) = filter.carrier {
            builder.push(" AND carrier = ").push_bind(carrier);
        }

        let row =
            builder.build_query_as::<(i64,)>().fetch_one(&self.pool).await.map_err(map_db_error)?;

        Ok(row.0 as u64)
    }

    pub async fn create_picks_for_order_async(
        &self,
        order_id: Uuid,
        warehouse_id: i32,
    ) -> Result<Vec<PickTask>> {
        let rows = sqlx::query_as::<_, (Uuid, String, Option<String>, i32)>(
            "SELECT id, sku, name, quantity FROM order_items WHERE order_id = $1",
        )
        .bind(order_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        // One transaction for the whole order: a failure part-way through used
        // to leave an order half-picked, with picks for some lines and none for
        // the rest — and `is_order_ready_to_pack` would then report the order
        // ready because the missing lines have no pick task to be incomplete.
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let mut picks = Vec::new();
        for (item_id, sku, name, qty) in rows {
            let location_id = sqlx::query_as::<_, (i32,)>(
                r#"
                SELECT l.id FROM locations l
                JOIN location_inventory li ON l.id = li.location_id
                WHERE l.warehouse_id = $1 AND li.sku = $2 AND l.is_pickable = true
                LIMIT 1
                "#,
            )
            .bind(warehouse_id)
            .bind(&sku)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(map_db_error)?
            .map(|row| row.0)
            .unwrap_or(1);

            let pick = Self::insert_pick_on(
                tx.as_mut(),
                &CreatePickTask {
                    wave_id: None,
                    order_id: order_id.into(),
                    order_item_id: item_id.into(),
                    warehouse_id,
                    sku,
                    product_name: name,
                    source_location_id: location_id,
                    quantity_requested: Decimal::from(qty),
                    lot_id: None,
                    serial_number: None,
                    priority: None,
                    notes: None,
                },
            )
            .await?;

            picks.push(pick);
        }

        tx.commit().await.map_err(map_db_error)?;

        Ok(picks)
    }

    pub async fn is_order_ready_to_pack_async(&self, order_id: Uuid) -> Result<bool> {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM pick_tasks WHERE order_id = $1 AND status NOT IN ('completed', 'short', 'cancelled')",
        )
        .bind(order_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(row.0 == 0)
    }

    pub async fn is_order_ready_to_ship_async(&self, order_id: Uuid) -> Result<bool> {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM pack_tasks WHERE order_id = $1 AND status = 'completed'",
        )
        .bind(order_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(row.0 > 0)
    }

    pub async fn create_waves_batch_async(
        &self,
        inputs: Vec<CreateWave>,
    ) -> Result<BatchResult<Wave>> {
        validate_batch_size(&inputs)?;

        let mut result = BatchResult::new();

        for (index, input) in inputs.into_iter().enumerate() {
            match self.create_wave_async(input).await {
                Ok(wave) => result.record_success(wave),
                Err(e) => result.record_failure(index, None, &e),
            }
        }

        Ok(result)
    }

    pub async fn get_picks_batch_async(&self, ids: Vec<Uuid>) -> Result<Vec<PickTask>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut builder = QueryBuilder::<Postgres>::new("SELECT * FROM pick_tasks WHERE id IN (");
        {
            let mut separated = builder.separated(", ");
            for id in ids {
                separated.push_bind(id);
            }
        }
        builder.push(")");

        let rows = builder
            .build_query_as::<PickRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_pick).collect::<Result<Vec<_>>>()
    }
}

impl FulfillmentRepository for PgFulfillmentRepository {
    fn create_wave(&self, input: CreateWave) -> Result<Wave> {
        block_on(self.create_wave_async(input))
    }

    fn get_wave(&self, id: FulfillmentId) -> Result<Option<Wave>> {
        block_on(self.get_wave_async(id.into_uuid()))
    }

    fn get_wave_by_number(&self, number: &str) -> Result<Option<Wave>> {
        block_on(self.get_wave_by_number_async(number))
    }

    fn list_waves(&self, filter: WaveFilter) -> Result<Vec<Wave>> {
        block_on(self.list_waves_async(filter))
    }

    fn release_wave(&self, id: FulfillmentId) -> Result<Wave> {
        block_on(self.release_wave_async(id.into_uuid()))
    }

    fn complete_wave(&self, id: FulfillmentId) -> Result<Wave> {
        block_on(self.complete_wave_async(id.into_uuid()))
    }

    fn cancel_wave(&self, id: FulfillmentId) -> Result<Wave> {
        block_on(self.cancel_wave_async(id.into_uuid()))
    }

    fn get_wave_orders(&self, wave_id: FulfillmentId) -> Result<Vec<OrderId>> {
        let order_ids = block_on(self.get_wave_orders_async(wave_id.into_uuid()))?;
        Ok(order_ids.into_iter().map(OrderId::from_uuid).collect())
    }

    fn count_waves(&self, filter: WaveFilter) -> Result<u64> {
        block_on(self.count_waves_async(filter))
    }

    fn create_pick(&self, input: CreatePickTask) -> Result<PickTask> {
        block_on(self.create_pick_async(input))
    }

    fn get_pick(&self, id: Uuid) -> Result<Option<PickTask>> {
        block_on(self.get_pick_async(id))
    }

    fn list_picks(&self, filter: PickTaskFilter) -> Result<Vec<PickTask>> {
        block_on(self.list_picks_async(filter))
    }

    fn assign_pick(&self, id: Uuid, assigned_to: &str) -> Result<PickTask> {
        block_on(self.assign_pick_async(id, assigned_to))
    }

    fn start_pick(&self, id: Uuid) -> Result<PickTask> {
        block_on(self.start_pick_async(id))
    }

    fn complete_pick(&self, input: CompletePick) -> Result<PickTask> {
        block_on(self.complete_pick_async(input))
    }

    fn report_short(&self, id: Uuid, short_qty: Decimal, reason: &str) -> Result<PickTask> {
        block_on(self.report_short_async(id, short_qty, reason))
    }

    fn cancel_pick(&self, id: Uuid) -> Result<PickTask> {
        block_on(self.cancel_pick_async(id))
    }

    fn get_picks_for_order(&self, order_id: OrderId) -> Result<Vec<PickTask>> {
        block_on(self.get_picks_for_order_async(order_id.into_uuid()))
    }

    fn get_picks_for_wave(&self, wave_id: FulfillmentId) -> Result<Vec<PickTask>> {
        block_on(self.get_picks_for_wave_async(wave_id.into_uuid()))
    }

    fn count_picks(&self, filter: PickTaskFilter) -> Result<u64> {
        block_on(self.count_picks_async(filter))
    }

    fn create_pack(&self, input: CreatePackTask) -> Result<PackTask> {
        block_on(self.create_pack_async(input))
    }

    fn get_pack(&self, id: Uuid) -> Result<Option<PackTask>> {
        block_on(self.get_pack_async(id))
    }

    fn list_packs(&self, filter: PackTaskFilter) -> Result<Vec<PackTask>> {
        block_on(self.list_packs_async(filter))
    }

    fn assign_pack(&self, id: Uuid, assigned_to: &str) -> Result<PackTask> {
        block_on(self.assign_pack_async(id, assigned_to))
    }

    fn start_pack(&self, id: Uuid) -> Result<PackTask> {
        block_on(self.start_pack_async(id))
    }

    fn complete_pack(&self, id: Uuid) -> Result<PackTask> {
        block_on(self.complete_pack_async(id))
    }

    fn add_carton(&self, input: AddCarton) -> Result<Carton> {
        block_on(self.add_carton_async(input))
    }

    fn add_carton_item(&self, input: AddCartonItem) -> Result<CartonItem> {
        block_on(self.add_carton_item_async(input))
    }

    fn get_cartons(&self, pack_task_id: Uuid) -> Result<Vec<Carton>> {
        block_on(self.get_cartons_async(pack_task_id))
    }

    fn get_carton_items(&self, carton_id: Uuid) -> Result<Vec<CartonItem>> {
        block_on(self.get_carton_items_async(carton_id))
    }

    fn mark_label_printed(&self, carton_id: Uuid) -> Result<Carton> {
        block_on(self.mark_label_printed_async(carton_id))
    }

    fn cancel_pack(&self, id: Uuid) -> Result<PackTask> {
        block_on(self.cancel_pack_async(id))
    }

    fn count_packs(&self, filter: PackTaskFilter) -> Result<u64> {
        block_on(self.count_packs_async(filter))
    }

    fn create_ship(&self, input: CreateShipTask) -> Result<ShipTask> {
        block_on(self.create_ship_async(input))
    }

    fn get_ship(&self, id: Uuid) -> Result<Option<ShipTask>> {
        block_on(self.get_ship_async(id))
    }

    fn list_ships(&self, filter: ShipTaskFilter) -> Result<Vec<ShipTask>> {
        block_on(self.list_ships_async(filter))
    }

    fn assign_ship(&self, id: Uuid, assigned_to: &str) -> Result<ShipTask> {
        block_on(self.assign_ship_async(id, assigned_to))
    }

    fn print_label(&self, id: Uuid, label_url: &str) -> Result<ShipTask> {
        block_on(self.print_label_async(id, label_url))
    }

    fn complete_ship(&self, input: CompleteShip) -> Result<ShipTask> {
        block_on(self.complete_ship_async(input))
    }

    fn cancel_ship(&self, id: Uuid) -> Result<ShipTask> {
        block_on(self.cancel_ship_async(id))
    }

    fn count_ships(&self, filter: ShipTaskFilter) -> Result<u64> {
        block_on(self.count_ships_async(filter))
    }

    fn create_picks_for_order(
        &self,
        order_id: OrderId,
        warehouse_id: i32,
    ) -> Result<Vec<PickTask>> {
        block_on(self.create_picks_for_order_async(order_id.into_uuid(), warehouse_id))
    }

    fn is_order_ready_to_pack(&self, order_id: OrderId) -> Result<bool> {
        block_on(self.is_order_ready_to_pack_async(order_id.into_uuid()))
    }

    fn is_order_ready_to_ship(&self, order_id: OrderId) -> Result<bool> {
        block_on(self.is_order_ready_to_ship_async(order_id.into_uuid()))
    }

    fn create_waves_batch(&self, inputs: Vec<CreateWave>) -> Result<BatchResult<Wave>> {
        block_on(self.create_waves_batch_async(inputs))
    }

    fn get_picks_batch(&self, ids: Vec<Uuid>) -> Result<Vec<PickTask>> {
        block_on(self.get_picks_batch_async(ids))
    }
}
