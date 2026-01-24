//! PostgreSQL implementation for fulfillment (pick/pack/ship) management

use super::{block_on, map_db_error};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::{FromRow, Postgres, QueryBuilder};
use sqlx::postgres::PgPool;
use stateset_core::{
    AddCarton, AddCartonItem, BatchResult, Carton, CartonItem, CompletePick, CompleteShip,
    CommerceError, CreatePackTask, CreatePickTask, CreateShipTask, CreateWave,
    FulfillmentRepository, PackStatus, PackTask, PackTaskFilter, PackageType, PickStatus, PickTask,
    PickTaskFilter, Result, ShipStatus, ShipTask, ShipTaskFilter, Wave, WaveFilter, WaveStatus,
    generate_carton_number, generate_wave_number, validate_batch_size,
};
use uuid::Uuid;

#[derive(Clone)]
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
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_wave(row: WaveRow) -> Result<Wave> {
        let status: WaveStatus = row.status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid wave.status '{}': {}", row.status, e))
        })?;

        Ok(Wave {
            id: row.id,
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
            CommerceError::DatabaseError(format!("Invalid pick_task.status '{}': {}", row.status, e))
        })?;

        Ok(PickTask {
            id: row.id,
            wave_id: row.wave_id,
            order_id: row.order_id,
            order_item_id: row.order_item_id,
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
            CommerceError::DatabaseError(format!("Invalid pack_task.status '{}': {}", row.status, e))
        })?;

        Ok(PackTask {
            id: row.id,
            order_id: row.order_id,
            shipment_id: row.shipment_id,
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
            CommerceError::DatabaseError(format!("Invalid ship_task.status '{}': {}", row.status, e))
        })?;

        Ok(ShipTask {
            id: row.id,
            order_id: row.order_id,
            shipment_id: row.shipment_id,
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

        self.get_wave_async(id).await?.ok_or_else(|| {
            CommerceError::DatabaseError("Failed to create wave".into())
        })
    }

    pub async fn get_wave_async(&self, id: Uuid) -> Result<Option<Wave>> {
        let row = sqlx::query_as::<_, WaveRow>("SELECT * FROM waves WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(row.map(Self::row_to_wave).transpose()?)
    }

    pub async fn get_wave_by_number_async(&self, number: &str) -> Result<Option<Wave>> {
        let row = sqlx::query_as::<_, WaveRow>("SELECT * FROM waves WHERE wave_number = $1")
            .bind(number)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(row.map(Self::row_to_wave).transpose()?)
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

        if let Some(limit) = filter.limit {
            builder.push(" LIMIT ").push_bind(limit as i64);
        }
        if let Some(offset) = filter.offset {
            builder.push(" OFFSET ").push_bind(offset as i64);
        }

        let rows = builder
            .build_query_as::<WaveRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(rows
            .into_iter()
            .map(Self::row_to_wave)
            .collect::<Result<Vec<_>>>()?)
    }

    pub async fn release_wave_async(&self, id: Uuid) -> Result<Wave> {
        let now = Utc::now();

        sqlx::query("UPDATE waves SET status = $1, started_at = $2 WHERE id = $3 AND status = 'draft'")
            .bind(WaveStatus::Released.to_string())
            .bind(now)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_wave_async(id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to release wave".into()))
    }

    pub async fn complete_wave_async(&self, id: Uuid) -> Result<Wave> {
        let now = Utc::now();

        sqlx::query("UPDATE waves SET status = $1, completed_at = $2 WHERE id = $3")
            .bind(WaveStatus::Completed.to_string())
            .bind(now)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_wave_async(id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to complete wave".into()))
    }

    pub async fn cancel_wave_async(&self, id: Uuid) -> Result<Wave> {
        sqlx::query("UPDATE waves SET status = $1 WHERE id = $2")
            .bind(WaveStatus::Cancelled.to_string())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_wave_async(id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to cancel wave".into()))
    }

    pub async fn get_wave_orders_async(&self, wave_id: Uuid) -> Result<Vec<Uuid>> {
        let rows = sqlx::query_as::<_, (Uuid,)>("SELECT order_id FROM wave_orders WHERE wave_id = $1")
            .bind(wave_id)
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(rows.into_iter().map(|row| row.0).collect())
    }

    pub async fn count_waves_async(&self, filter: WaveFilter) -> Result<u64> {
        let mut builder = QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM waves WHERE 1=1");

        if let Some(warehouse_id) = filter.warehouse_id {
            builder.push(" AND warehouse_id = ").push_bind(warehouse_id);
        }
        if let Some(status) = filter.status {
            builder.push(" AND status = ").push_bind(status.to_string());
        }

        let row = builder
            .build_query_as::<(i64,)>()
            .fetch_one(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(row.0 as u64)
    }

    pub async fn create_pick_async(&self, input: CreatePickTask) -> Result<PickTask> {
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
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_pick_async(id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to create pick".into()))
    }

    pub async fn get_pick_async(&self, id: Uuid) -> Result<Option<PickTask>> {
        let row = sqlx::query_as::<_, PickRow>("SELECT * FROM pick_tasks WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(row.map(Self::row_to_pick).transpose()?)
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

        if let Some(limit) = filter.limit {
            builder.push(" LIMIT ").push_bind(limit as i64);
        }
        if let Some(offset) = filter.offset {
            builder.push(" OFFSET ").push_bind(offset as i64);
        }

        let rows = builder
            .build_query_as::<PickRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(rows
            .into_iter()
            .map(Self::row_to_pick)
            .collect::<Result<Vec<_>>>()?)
    }

    pub async fn assign_pick_async(&self, id: Uuid, assigned_to: &str) -> Result<PickTask> {
        sqlx::query("UPDATE pick_tasks SET assigned_to = $1, status = $2 WHERE id = $3")
            .bind(assigned_to)
            .bind(PickStatus::Assigned.to_string())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_pick_async(id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to assign pick".into()))
    }

    pub async fn start_pick_async(&self, id: Uuid) -> Result<PickTask> {
        let now = Utc::now();

        sqlx::query("UPDATE pick_tasks SET status = $1, started_at = $2 WHERE id = $3")
            .bind(PickStatus::InProgress.to_string())
            .bind(now)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_pick_async(id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to start pick".into()))
    }

    pub async fn complete_pick_async(&self, input: CompletePick) -> Result<PickTask> {
        let now = Utc::now();
        let short_qty = input.quantity_short.unwrap_or(Decimal::ZERO);
        let status = if short_qty > Decimal::ZERO {
            PickStatus::Short
        } else {
            PickStatus::Completed
        };

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
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        let pick = self.get_pick_async(input.pick_id).await?.ok_or(CommerceError::NotFound)?;
        if let Some(wave_id) = pick.wave_id {
            sqlx::query("UPDATE waves SET completed_pick_count = completed_pick_count + 1 WHERE id = $1")
                .bind(wave_id)
                .execute(&self.pool)
                .await
                .map_err(map_db_error)?;
        }

        self.get_pick_async(input.pick_id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to complete pick".into()))
    }

    pub async fn report_short_async(&self, id: Uuid, short_qty: Decimal, reason: &str) -> Result<PickTask> {
        sqlx::query("UPDATE pick_tasks SET status = $1, quantity_short = $2, notes = $3 WHERE id = $4")
            .bind(PickStatus::Short.to_string())
            .bind(short_qty)
            .bind(reason)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_pick_async(id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to report short".into()))
    }

    pub async fn cancel_pick_async(&self, id: Uuid) -> Result<PickTask> {
        sqlx::query("UPDATE pick_tasks SET status = $1 WHERE id = $2")
            .bind(PickStatus::Cancelled.to_string())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_pick_async(id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to cancel pick".into()))
    }

    pub async fn get_picks_for_order_async(&self, order_id: Uuid) -> Result<Vec<PickTask>> {
        self
            .list_picks_async(PickTaskFilter {
                order_id: Some(order_id),
                ..Default::default()
            })
            .await
    }

    pub async fn get_picks_for_wave_async(&self, wave_id: Uuid) -> Result<Vec<PickTask>> {
        self
            .list_picks_async(PickTaskFilter {
                wave_id: Some(wave_id),
                ..Default::default()
            })
            .await
    }

    pub async fn count_picks_async(&self, filter: PickTaskFilter) -> Result<u64> {
        let mut builder = QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM pick_tasks WHERE 1=1");

        if let Some(status) = filter.status {
            builder.push(" AND status = ").push_bind(status.to_string());
        }
        if let Some(wave_id) = filter.wave_id {
            builder.push(" AND wave_id = ").push_bind(wave_id);
        }
        if let Some(order_id) = filter.order_id {
            builder.push(" AND order_id = ").push_bind(order_id);
        }

        let row = builder
            .build_query_as::<(i64,)>()
            .fetch_one(&self.pool)
            .await
            .map_err(map_db_error)?;

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

        Ok(row.map(Self::row_to_pack).transpose()?)
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

        if let Some(limit) = filter.limit {
            builder.push(" LIMIT ").push_bind(limit as i64);
        }
        if let Some(offset) = filter.offset {
            builder.push(" OFFSET ").push_bind(offset as i64);
        }

        let rows = builder
            .build_query_as::<PackRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(rows
            .into_iter()
            .map(Self::row_to_pack)
            .collect::<Result<Vec<_>>>()?)
    }

    pub async fn assign_pack_async(&self, id: Uuid, assigned_to: &str) -> Result<PackTask> {
        sqlx::query("UPDATE pack_tasks SET assigned_to = $1, status = $2 WHERE id = $3")
            .bind(assigned_to)
            .bind(PackStatus::Assigned.to_string())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_pack_async(id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to assign pack".into()))
    }

    pub async fn start_pack_async(&self, id: Uuid) -> Result<PackTask> {
        let now = Utc::now();

        sqlx::query("UPDATE pack_tasks SET status = $1, started_at = $2 WHERE id = $3")
            .bind(PackStatus::InProgress.to_string())
            .bind(now)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_pack_async(id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to start pack".into()))
    }

    pub async fn complete_pack_async(&self, id: Uuid) -> Result<PackTask> {
        let now = Utc::now();

        sqlx::query("UPDATE pack_tasks SET status = $1, completed_at = $2 WHERE id = $3")
            .bind(PackStatus::Completed.to_string())
            .bind(now)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_pack_async(id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to complete pack".into()))
    }

    pub async fn add_carton_async(&self, input: AddCarton) -> Result<Carton> {
        let now = Utc::now();
        let id = Uuid::new_v4();
        let carton_number = generate_carton_number();

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
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        sqlx::query("UPDATE pack_tasks SET carton_count = carton_count + 1 WHERE id = $1")
            .bind(input.pack_task_id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_carton_async(id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to create carton".into()))
    }

    async fn get_carton_async(&self, id: Uuid) -> Result<Option<Carton>> {
        let row = sqlx::query_as::<_, CartonRow>("SELECT * FROM cartons WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(row.map(Self::row_to_carton).transpose()?)
    }

    pub async fn add_carton_item_async(&self, input: AddCartonItem) -> Result<CartonItem> {
        let id = Uuid::new_v4();

        sqlx::query(
            "INSERT INTO carton_items (id, carton_id, sku, quantity, lot_id, serial_number) VALUES ($1,$2,$3,$4,$5,$6)",
        )
        .bind(id)
        .bind(input.carton_id)
        .bind(&input.sku)
        .bind(input.quantity)
        .bind(input.lot_id)
        .bind(&input.serial_number)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_carton_item_async(id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to create carton item".into()))
    }

    async fn get_carton_item_async(&self, id: Uuid) -> Result<Option<CartonItem>> {
        let row = sqlx::query_as::<_, CartonItemRow>("SELECT * FROM carton_items WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(row.map(Self::row_to_carton_item))
    }

    pub async fn get_cartons_async(&self, pack_task_id: Uuid) -> Result<Vec<Carton>> {
        let rows = sqlx::query_as::<_, CartonRow>("SELECT * FROM cartons WHERE pack_task_id = $1")
            .bind(pack_task_id)
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(rows
            .into_iter()
            .map(Self::row_to_carton)
            .collect::<Result<Vec<_>>>()?)
    }

    pub async fn get_carton_items_async(&self, carton_id: Uuid) -> Result<Vec<CartonItem>> {
        let rows = sqlx::query_as::<_, CartonItemRow>("SELECT * FROM carton_items WHERE carton_id = $1")
            .bind(carton_id)
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(rows.into_iter().map(Self::row_to_carton_item).collect())
    }

    pub async fn mark_label_printed_async(&self, carton_id: Uuid) -> Result<Carton> {
        sqlx::query("UPDATE cartons SET label_printed = true WHERE id = $1")
            .bind(carton_id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_carton_async(carton_id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to mark label printed".into()))
    }

    pub async fn cancel_pack_async(&self, id: Uuid) -> Result<PackTask> {
        sqlx::query("UPDATE pack_tasks SET status = $1 WHERE id = $2")
            .bind(PackStatus::Cancelled.to_string())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_pack_async(id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to cancel pack".into()))
    }

    pub async fn count_packs_async(&self, filter: PackTaskFilter) -> Result<u64> {
        let mut builder = QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM pack_tasks WHERE 1=1");

        if let Some(order_id) = filter.order_id {
            builder.push(" AND order_id = ").push_bind(order_id);
        }
        if let Some(status) = filter.status {
            builder.push(" AND status = ").push_bind(status.to_string());
        }
        if let Some(assigned_to) = filter.assigned_to {
            builder.push(" AND assigned_to = ").push_bind(assigned_to);
        }

        let row = builder
            .build_query_as::<(i64,)>()
            .fetch_one(&self.pool)
            .await
            .map_err(map_db_error)?;

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

        Ok(row.map(Self::row_to_ship).transpose()?)
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

        if let Some(limit) = filter.limit {
            builder.push(" LIMIT ").push_bind(limit as i64);
        }
        if let Some(offset) = filter.offset {
            builder.push(" OFFSET ").push_bind(offset as i64);
        }

        let rows = builder
            .build_query_as::<ShipRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(rows
            .into_iter()
            .map(Self::row_to_ship)
            .collect::<Result<Vec<_>>>()?)
    }

    pub async fn assign_ship_async(&self, id: Uuid, assigned_to: &str) -> Result<ShipTask> {
        sqlx::query("UPDATE ship_tasks SET assigned_to = $1 WHERE id = $2")
            .bind(assigned_to)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_ship_async(id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to assign ship".into()))
    }

    pub async fn print_label_async(&self, id: Uuid, label_url: &str) -> Result<ShipTask> {
        sqlx::query("UPDATE ship_tasks SET status = $1, label_url = $2 WHERE id = $3")
            .bind(ShipStatus::LabelPrinted.to_string())
            .bind(label_url)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_ship_async(id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to update ship".into()))
    }

    pub async fn complete_ship_async(&self, input: CompleteShip) -> Result<ShipTask> {
        let now = Utc::now();

        sqlx::query(
            "UPDATE ship_tasks SET status = $1, tracking_number = $2, shipping_cost = $3, shipped_at = $4 WHERE id = $5",
        )
        .bind(ShipStatus::Shipped.to_string())
        .bind(&input.tracking_number)
        .bind(input.shipping_cost)
        .bind(now)
        .bind(input.ship_task_id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_ship_async(input.ship_task_id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to complete ship".into()))
    }

    pub async fn cancel_ship_async(&self, id: Uuid) -> Result<ShipTask> {
        sqlx::query("UPDATE ship_tasks SET status = $1 WHERE id = $2")
            .bind(ShipStatus::Cancelled.to_string())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_ship_async(id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to cancel ship".into()))
    }

    pub async fn count_ships_async(&self, filter: ShipTaskFilter) -> Result<u64> {
        let mut builder = QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM ship_tasks WHERE 1=1");

        if let Some(order_id) = filter.order_id {
            builder.push(" AND order_id = ").push_bind(order_id);
        }
        if let Some(status) = filter.status {
            builder.push(" AND status = ").push_bind(status.to_string());
        }
        if let Some(carrier) = filter.carrier {
            builder.push(" AND carrier = ").push_bind(carrier);
        }

        let row = builder
            .build_query_as::<(i64,)>()
            .fetch_one(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(row.0 as u64)
    }

    pub async fn create_picks_for_order_async(&self, order_id: Uuid, warehouse_id: i32) -> Result<Vec<PickTask>> {
        let rows = sqlx::query_as::<_, (Uuid, String, Option<String>, i32)>(
            "SELECT id, sku, name, quantity FROM order_items WHERE order_id = $1",
        )
        .bind(order_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

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
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?
            .map(|row| row.0)
            .unwrap_or(1);

            let pick = self
                .create_pick_async(CreatePickTask {
                    wave_id: None,
                    order_id,
                    order_item_id: item_id,
                    warehouse_id,
                    sku,
                    product_name: name,
                    source_location_id: location_id,
                    quantity_requested: Decimal::from(qty),
                    lot_id: None,
                    serial_number: None,
                    priority: None,
                    notes: None,
                })
                .await?;

            picks.push(pick);
        }

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

    pub async fn create_waves_batch_async(&self, inputs: Vec<CreateWave>) -> Result<BatchResult<Wave>> {
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

        Ok(rows
            .into_iter()
            .map(Self::row_to_pick)
            .collect::<Result<Vec<_>>>()?)
    }
}

impl FulfillmentRepository for PgFulfillmentRepository {
    fn create_wave(&self, input: CreateWave) -> Result<Wave> {
        block_on(self.create_wave_async(input))
    }

    fn get_wave(&self, id: Uuid) -> Result<Option<Wave>> {
        block_on(self.get_wave_async(id))
    }

    fn get_wave_by_number(&self, number: &str) -> Result<Option<Wave>> {
        block_on(self.get_wave_by_number_async(number))
    }

    fn list_waves(&self, filter: WaveFilter) -> Result<Vec<Wave>> {
        block_on(self.list_waves_async(filter))
    }

    fn release_wave(&self, id: Uuid) -> Result<Wave> {
        block_on(self.release_wave_async(id))
    }

    fn complete_wave(&self, id: Uuid) -> Result<Wave> {
        block_on(self.complete_wave_async(id))
    }

    fn cancel_wave(&self, id: Uuid) -> Result<Wave> {
        block_on(self.cancel_wave_async(id))
    }

    fn get_wave_orders(&self, wave_id: Uuid) -> Result<Vec<Uuid>> {
        block_on(self.get_wave_orders_async(wave_id))
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

    fn get_picks_for_order(&self, order_id: Uuid) -> Result<Vec<PickTask>> {
        block_on(self.get_picks_for_order_async(order_id))
    }

    fn get_picks_for_wave(&self, wave_id: Uuid) -> Result<Vec<PickTask>> {
        block_on(self.get_picks_for_wave_async(wave_id))
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

    fn create_picks_for_order(&self, order_id: Uuid, warehouse_id: i32) -> Result<Vec<PickTask>> {
        block_on(self.create_picks_for_order_async(order_id, warehouse_id))
    }

    fn is_order_ready_to_pack(&self, order_id: Uuid) -> Result<bool> {
        block_on(self.is_order_ready_to_pack_async(order_id))
    }

    fn is_order_ready_to_ship(&self, order_id: Uuid) -> Result<bool> {
        block_on(self.is_order_ready_to_ship_async(order_id))
    }

    fn create_waves_batch(&self, inputs: Vec<CreateWave>) -> Result<BatchResult<Wave>> {
        block_on(self.create_waves_batch_async(inputs))
    }

    fn get_picks_batch(&self, ids: Vec<Uuid>) -> Result<Vec<PickTask>> {
        block_on(self.get_picks_batch_async(ids))
    }
}
