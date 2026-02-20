//! SQLite implementation for fulfillment (pick/pack/ship) management

use crate::sqlite::{
    map_db_error, parse_datetime_opt_row, parse_datetime_row, parse_decimal_opt_row,
    parse_decimal_row, parse_enum_row, parse_uuid, parse_uuid_opt_row, parse_uuid_row,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;
use rust_decimal::Decimal;
use uuid::Uuid;

use stateset_core::{
    AddCarton, AddCartonItem, BatchResult, Carton, CartonItem, CommerceError, CompletePick,
    CompleteShip, CreatePackTask, CreatePickTask, CreateShipTask, CreateWave, FulfillmentId,
    FulfillmentRepository, OrderId, OrderItemId, PackStatus, PackTask, PackTaskFilter, PickStatus,
    PickTask, PickTaskFilter, Result, ShipStatus, ShipTask, ShipTaskFilter, ShipmentId, Wave,
    WaveFilter, WaveStatus, generate_carton_number, generate_wave_number,
};

/// SQLite fulfillment repository
#[derive(Debug)]
pub struct SqliteFulfillmentRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteFulfillmentRepository {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_wave(row: &rusqlite::Row<'_>) -> rusqlite::Result<Wave> {
        let id_str: String = row.get("id")?;
        let status_str: String = row.get("status")?;
        let started_str: Option<String> = row.get("started_at")?;
        let completed_str: Option<String> = row.get("completed_at")?;

        Ok(Wave {
            id: FulfillmentId::from(parse_uuid_row(&id_str, "wave", "id")?),
            wave_number: row.get("wave_number")?,
            warehouse_id: row.get("warehouse_id")?,
            status: parse_enum_row(&status_str, "wave", "status")?,
            order_count: row.get("order_count")?,
            pick_count: row.get("pick_count")?,
            completed_pick_count: row.get("completed_pick_count")?,
            priority: row.get("priority")?,
            started_at: parse_datetime_opt_row(started_str, "wave", "started_at")?,
            completed_at: parse_datetime_opt_row(completed_str, "wave", "completed_at")?,
            notes: row.get("notes")?,
            created_by: row.get("created_by")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "wave",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "wave",
                "updated_at",
            )?,
        })
    }

    fn row_to_pick(row: &rusqlite::Row<'_>) -> rusqlite::Result<PickTask> {
        let id_str: String = row.get("id")?;
        let wave_id_str: Option<String> = row.get("wave_id")?;
        let order_id_str: String = row.get("order_id")?;
        let order_item_id_str: String = row.get("order_item_id")?;
        let status_str: String = row.get("status")?;
        let qty_req: String = row.get("quantity_requested")?;
        let qty_pick: String = row.get("quantity_picked")?;
        let qty_short: String = row.get("quantity_short")?;
        let lot_id_str: Option<String> = row.get("lot_id")?;
        let started_str: Option<String> = row.get("started_at")?;
        let completed_str: Option<String> = row.get("completed_at")?;

        Ok(PickTask {
            id: parse_uuid_row(&id_str, "pick_task", "id")?,
            wave_id: parse_uuid_opt_row(wave_id_str, "pick_task", "wave_id")?.map(FulfillmentId::from),
            order_id: OrderId::from(parse_uuid_row(&order_id_str, "pick_task", "order_id")?),
            order_item_id: OrderItemId::from(parse_uuid_row(&order_item_id_str, "pick_task", "order_item_id")?),
            warehouse_id: row.get("warehouse_id")?,
            status: parse_enum_row(&status_str, "pick_task", "status")?,
            sku: row.get("sku")?,
            product_name: row.get("product_name")?,
            source_location_id: row.get("source_location_id")?,
            source_location_code: row.get("source_location_code")?,
            quantity_requested: parse_decimal_row(&qty_req, "pick_task", "quantity_requested")?,
            quantity_picked: parse_decimal_row(&qty_pick, "pick_task", "quantity_picked")?,
            quantity_short: parse_decimal_row(&qty_short, "pick_task", "quantity_short")?,
            lot_id: parse_uuid_opt_row(lot_id_str, "pick_task", "lot_id")?,
            serial_number: row.get("serial_number")?,
            assigned_to: row.get("assigned_to")?,
            priority: row.get("priority")?,
            pick_sequence: row.get("pick_sequence")?,
            started_at: parse_datetime_opt_row(started_str, "pick_task", "started_at")?,
            completed_at: parse_datetime_opt_row(completed_str, "pick_task", "completed_at")?,
            notes: row.get("notes")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "pick_task",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "pick_task",
                "updated_at",
            )?,
        })
    }

    fn row_to_pack(row: &rusqlite::Row<'_>) -> rusqlite::Result<PackTask> {
        let id_str: String = row.get("id")?;
        let order_id_str: String = row.get("order_id")?;
        let shipment_id_str: Option<String> = row.get("shipment_id")?;
        let status_str: String = row.get("status")?;
        let weight_str: Option<String> = row.get("total_weight_kg")?;
        let started_str: Option<String> = row.get("started_at")?;
        let completed_str: Option<String> = row.get("completed_at")?;

        Ok(PackTask {
            id: parse_uuid_row(&id_str, "pack_task", "id")?,
            order_id: OrderId::from(parse_uuid_row(&order_id_str, "pack_task", "order_id")?),
            shipment_id: parse_uuid_opt_row(shipment_id_str, "pack_task", "shipment_id")?.map(ShipmentId::from),
            status: parse_enum_row(&status_str, "pack_task", "status")?,
            carton_count: row.get("carton_count")?,
            total_weight_kg: parse_decimal_opt_row(weight_str, "pack_task", "total_weight_kg")?,
            assigned_to: row.get("assigned_to")?,
            packing_station: row.get("packing_station")?,
            started_at: parse_datetime_opt_row(started_str, "pack_task", "started_at")?,
            completed_at: parse_datetime_opt_row(completed_str, "pack_task", "completed_at")?,
            notes: row.get("notes")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "pack_task",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "pack_task",
                "updated_at",
            )?,
        })
    }

    fn row_to_carton(row: &rusqlite::Row<'_>) -> rusqlite::Result<Carton> {
        let id_str: String = row.get("id")?;
        let pack_task_id_str: String = row.get("pack_task_id")?;
        let pkg_type_str: String = row.get("package_type")?;
        let weight_str: Option<String> = row.get("weight_kg")?;
        let length_str: Option<String> = row.get("length_cm")?;
        let width_str: Option<String> = row.get("width_cm")?;
        let height_str: Option<String> = row.get("height_cm")?;

        Ok(Carton {
            id: parse_uuid_row(&id_str, "carton", "id")?,
            pack_task_id: parse_uuid_row(&pack_task_id_str, "carton", "pack_task_id")?,
            carton_number: row.get("carton_number")?,
            package_type: parse_enum_row(&pkg_type_str, "carton", "package_type")?,
            weight_kg: parse_decimal_opt_row(weight_str, "carton", "weight_kg")?,
            length_cm: parse_decimal_opt_row(length_str, "carton", "length_cm")?,
            width_cm: parse_decimal_opt_row(width_str, "carton", "width_cm")?,
            height_cm: parse_decimal_opt_row(height_str, "carton", "height_cm")?,
            tracking_number: row.get("tracking_number")?,
            label_printed: row.get::<_, i32>("label_printed")? != 0,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "carton",
                "created_at",
            )?,
        })
    }

    fn row_to_carton_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<CartonItem> {
        let id_str: String = row.get("id")?;
        let carton_id_str: String = row.get("carton_id")?;
        let qty_str: String = row.get("quantity")?;
        let lot_id_str: Option<String> = row.get("lot_id")?;

        Ok(CartonItem {
            id: parse_uuid_row(&id_str, "carton_item", "id")?,
            carton_id: parse_uuid_row(&carton_id_str, "carton_item", "carton_id")?,
            sku: row.get("sku")?,
            quantity: parse_decimal_row(&qty_str, "carton_item", "quantity")?,
            lot_id: parse_uuid_opt_row(lot_id_str, "carton_item", "lot_id")?,
            serial_number: row.get("serial_number")?,
        })
    }

    fn row_to_ship(row: &rusqlite::Row<'_>) -> rusqlite::Result<ShipTask> {
        let id_str: String = row.get("id")?;
        let order_id_str: String = row.get("order_id")?;
        let shipment_id_str: String = row.get("shipment_id")?;
        let pack_task_id_str: String = row.get("pack_task_id")?;
        let status_str: String = row.get("status")?;
        let cost_str: Option<String> = row.get("shipping_cost")?;
        let shipped_str: Option<String> = row.get("shipped_at")?;

        Ok(ShipTask {
            id: parse_uuid_row(&id_str, "ship_task", "id")?,
            order_id: OrderId::from(parse_uuid_row(&order_id_str, "ship_task", "order_id")?),
            shipment_id: ShipmentId::from(parse_uuid_row(&shipment_id_str, "ship_task", "shipment_id")?),
            pack_task_id: parse_uuid_row(&pack_task_id_str, "ship_task", "pack_task_id")?,
            status: parse_enum_row(&status_str, "ship_task", "status")?,
            carrier: row.get("carrier")?,
            service_level: row.get("service_level")?,
            tracking_number: row.get("tracking_number")?,
            label_url: row.get("label_url")?,
            shipping_cost: parse_decimal_opt_row(cost_str, "ship_task", "shipping_cost")?,
            assigned_to: row.get("assigned_to")?,
            shipped_at: parse_datetime_opt_row(shipped_str, "ship_task", "shipped_at")?,
            notes: row.get("notes")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "ship_task",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "ship_task",
                "updated_at",
            )?,
        })
    }
}

impl FulfillmentRepository for SqliteFulfillmentRepository {
    // ========================================================================
    // Wave Operations
    // ========================================================================

    fn create_wave(&self, input: CreateWave) -> Result<Wave> {
        let conn = self.conn()?;
        let now = Utc::now().to_rfc3339();
        let id = FulfillmentId::new();
        let wave_number = generate_wave_number();
        let order_count = input.order_ids.len() as i32;

        conn.execute(
            "INSERT INTO waves (id, wave_number, warehouse_id, status, order_count, priority, notes, created_by, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
            params![
                id.to_string(),
                wave_number,
                input.warehouse_id,
                WaveStatus::Draft.to_string(),
                order_count,
                input.priority.unwrap_or(0),
                input.notes,
                input.created_by,
                now,
            ],
        ).map_err(map_db_error)?;

        // Add order associations
        for order_id in &input.order_ids {
            conn.execute(
                "INSERT INTO wave_orders (wave_id, order_id) VALUES (?1, ?2)",
                params![id.to_string(), order_id.to_string()],
            )
            .map_err(map_db_error)?;
        }

        drop(conn);
        self.get_wave(id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to create wave".into()))
    }

    fn get_wave(&self, id: FulfillmentId) -> Result<Option<Wave>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare("SELECT * FROM waves WHERE id = ?1").map_err(map_db_error)?;
        let mut rows = stmt.query(params![id.to_string()]).map_err(map_db_error)?;

        if let Some(row) = rows.next().map_err(map_db_error)? {
            Ok(Some(Self::row_to_wave(row).map_err(map_db_error)?))
        } else {
            Ok(None)
        }
    }

    fn get_wave_by_number(&self, number: &str) -> Result<Option<Wave>> {
        let conn = self.conn()?;
        let mut stmt =
            conn.prepare("SELECT * FROM waves WHERE wave_number = ?1").map_err(map_db_error)?;
        let mut rows = stmt.query(params![number]).map_err(map_db_error)?;

        if let Some(row) = rows.next().map_err(map_db_error)? {
            Ok(Some(Self::row_to_wave(row).map_err(map_db_error)?))
        } else {
            Ok(None)
        }
    }

    fn list_waves(&self, filter: WaveFilter) -> Result<Vec<Wave>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM waves WHERE 1=1".to_string();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(warehouse_id) = filter.warehouse_id {
            sql.push_str(" AND warehouse_id = ?");
            params_vec.push(Box::new(warehouse_id));
        }

        if let Some(status) = filter.status {
            sql.push_str(" AND status = ?");
            params_vec.push(Box::new(status.to_string()));
        }

        sql.push_str(" ORDER BY priority DESC, created_at DESC");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();
        let mut rows = stmt.query(params_refs.as_slice()).map_err(map_db_error)?;

        let mut waves = Vec::new();
        while let Some(row) = rows.next().map_err(map_db_error)? {
            waves.push(Self::row_to_wave(row).map_err(map_db_error)?);
        }
        Ok(waves)
    }

    fn release_wave(&self, id: FulfillmentId) -> Result<Wave> {
        let conn = self.conn()?;
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "UPDATE waves SET status = ?1, started_at = ?2 WHERE id = ?3 AND status = 'draft'",
            params![WaveStatus::Released.to_string(), now, id.to_string()],
        )
        .map_err(map_db_error)?;

        drop(conn);
        self.get_wave(id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to release wave".into()))
    }

    fn complete_wave(&self, id: FulfillmentId) -> Result<Wave> {
        let conn = self.conn()?;
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "UPDATE waves SET status = ?1, completed_at = ?2 WHERE id = ?3",
            params![WaveStatus::Completed.to_string(), now, id.to_string()],
        )
        .map_err(map_db_error)?;

        drop(conn);
        self.get_wave(id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to complete wave".into()))
    }

    fn cancel_wave(&self, id: FulfillmentId) -> Result<Wave> {
        let conn = self.conn()?;

        conn.execute(
            "UPDATE waves SET status = ?1 WHERE id = ?2",
            params![WaveStatus::Cancelled.to_string(), id.to_string()],
        )
        .map_err(map_db_error)?;

        drop(conn);
        self.get_wave(id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to cancel wave".into()))
    }

    fn get_wave_orders(&self, wave_id: FulfillmentId) -> Result<Vec<OrderId>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT order_id FROM wave_orders WHERE wave_id = ?1")
            .map_err(map_db_error)?;
        let mut rows = stmt.query(params![wave_id.to_string()]).map_err(map_db_error)?;

        let mut orders = Vec::new();
        while let Some(row) = rows.next().map_err(map_db_error)? {
            let id_str: String = row.get(0).map_err(map_db_error)?;
            let id = OrderId::from(parse_uuid(&id_str, "wave_order", "order_id")?);
            orders.push(id);
        }
        Ok(orders)
    }

    fn count_waves(&self, filter: WaveFilter) -> Result<u64> {
        let conn = self.conn()?;
        let mut sql = "SELECT COUNT(*) FROM waves WHERE 1=1".to_string();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(status) = filter.status {
            sql.push_str(" AND status = ?");
            params_vec.push(Box::new(status.to_string()));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();
        let count: i64 =
            conn.query_row(&sql, params_refs.as_slice(), |row| row.get(0)).map_err(map_db_error)?;
        Ok(count as u64)
    }

    // ========================================================================
    // Pick Operations
    // ========================================================================

    fn create_pick(&self, input: CreatePickTask) -> Result<PickTask> {
        let conn = self.conn()?;
        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4();

        conn.execute(
            "INSERT INTO pick_tasks (id, wave_id, order_id, order_item_id, warehouse_id, status, sku, product_name,
             source_location_id, quantity_requested, lot_id, serial_number, priority, notes, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?15)",
            params![
                id.to_string(),
                input.wave_id.map(|id| id.to_string()),
                input.order_id.to_string(),
                input.order_item_id.to_string(),
                input.warehouse_id,
                PickStatus::Pending.to_string(),
                input.sku,
                input.product_name,
                input.source_location_id,
                input.quantity_requested.to_string(),
                input.lot_id.map(|id| id.to_string()),
                input.serial_number,
                input.priority.unwrap_or(0),
                input.notes,
                now,
            ],
        ).map_err(map_db_error)?;

        drop(conn);
        self.get_pick(id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to create pick".into()))
    }

    fn get_pick(&self, id: Uuid) -> Result<Option<PickTask>> {
        let conn = self.conn()?;
        let mut stmt =
            conn.prepare("SELECT * FROM pick_tasks WHERE id = ?1").map_err(map_db_error)?;
        let mut rows = stmt.query(params![id.to_string()]).map_err(map_db_error)?;

        if let Some(row) = rows.next().map_err(map_db_error)? {
            Ok(Some(Self::row_to_pick(row).map_err(map_db_error)?))
        } else {
            Ok(None)
        }
    }

    fn list_picks(&self, filter: PickTaskFilter) -> Result<Vec<PickTask>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM pick_tasks WHERE 1=1".to_string();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(warehouse_id) = filter.warehouse_id {
            sql.push_str(" AND warehouse_id = ?");
            params_vec.push(Box::new(warehouse_id));
        }

        if let Some(wave_id) = filter.wave_id {
            sql.push_str(" AND wave_id = ?");
            params_vec.push(Box::new(wave_id.to_string()));
        }

        if let Some(order_id) = filter.order_id {
            sql.push_str(" AND order_id = ?");
            params_vec.push(Box::new(order_id.to_string()));
        }

        if let Some(status) = filter.status {
            sql.push_str(" AND status = ?");
            params_vec.push(Box::new(status.to_string()));
        }

        if let Some(assigned_to) = filter.assigned_to {
            sql.push_str(" AND assigned_to = ?");
            params_vec.push(Box::new(assigned_to));
        }

        sql.push_str(" ORDER BY priority DESC, pick_sequence");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();
        let mut rows = stmt.query(params_refs.as_slice()).map_err(map_db_error)?;

        let mut picks = Vec::new();
        while let Some(row) = rows.next().map_err(map_db_error)? {
            picks.push(Self::row_to_pick(row).map_err(map_db_error)?);
        }
        Ok(picks)
    }

    fn assign_pick(&self, id: Uuid, assigned_to: &str) -> Result<PickTask> {
        let conn = self.conn()?;

        conn.execute(
            "UPDATE pick_tasks SET assigned_to = ?1, status = ?2 WHERE id = ?3",
            params![assigned_to, PickStatus::Assigned.to_string(), id.to_string()],
        )
        .map_err(map_db_error)?;

        drop(conn);
        self.get_pick(id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to assign pick".into()))
    }

    fn start_pick(&self, id: Uuid) -> Result<PickTask> {
        let conn = self.conn()?;
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "UPDATE pick_tasks SET status = ?1, started_at = ?2 WHERE id = ?3",
            params![PickStatus::InProgress.to_string(), now, id.to_string()],
        )
        .map_err(map_db_error)?;

        drop(conn);
        self.get_pick(id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to start pick".into()))
    }

    fn complete_pick(&self, input: CompletePick) -> Result<PickTask> {
        let conn = self.conn()?;
        let now = Utc::now().to_rfc3339();
        let short_qty = input.quantity_short.unwrap_or(Decimal::ZERO);
        let status =
            if short_qty > Decimal::ZERO { PickStatus::Short } else { PickStatus::Completed };

        conn.execute(
            "UPDATE pick_tasks SET status = ?1, quantity_picked = ?2, quantity_short = ?3,
             lot_id = COALESCE(?4, lot_id), serial_number = COALESCE(?5, serial_number),
             completed_at = ?6 WHERE id = ?7",
            params![
                status.to_string(),
                input.quantity_picked.to_string(),
                short_qty.to_string(),
                input.lot_id.map(|id| id.to_string()),
                input.serial_number,
                now,
                input.pick_id.to_string(),
            ],
        )
        .map_err(map_db_error)?;

        let pick = {
            let mut stmt =
                conn.prepare("SELECT * FROM pick_tasks WHERE id = ?1").map_err(map_db_error)?;
            let mut rows = stmt.query(params![input.pick_id.to_string()]).map_err(map_db_error)?;

            if let Some(row) = rows.next().map_err(map_db_error)? {
                Self::row_to_pick(row).map_err(map_db_error)?
            } else {
                return Err(CommerceError::NotFound);
            }
        };

        if let Some(wave_id) = pick.wave_id {
            conn.execute(
                "UPDATE waves SET completed_pick_count = completed_pick_count + 1 WHERE id = ?1",
                params![wave_id.to_string()],
            )
            .map_err(map_db_error)?;
        }

        Ok(pick)
    }

    fn report_short(&self, id: Uuid, short_qty: Decimal, reason: &str) -> Result<PickTask> {
        let conn = self.conn()?;

        conn.execute(
            "UPDATE pick_tasks SET status = ?1, quantity_short = ?2, notes = ?3 WHERE id = ?4",
            params![PickStatus::Short.to_string(), short_qty.to_string(), reason, id.to_string()],
        )
        .map_err(map_db_error)?;

        drop(conn);
        self.get_pick(id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to report short".into()))
    }

    fn cancel_pick(&self, id: Uuid) -> Result<PickTask> {
        let conn = self.conn()?;

        conn.execute(
            "UPDATE pick_tasks SET status = ?1 WHERE id = ?2",
            params![PickStatus::Cancelled.to_string(), id.to_string()],
        )
        .map_err(map_db_error)?;

        drop(conn);
        self.get_pick(id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to cancel pick".into()))
    }

    fn get_picks_for_order(&self, order_id: OrderId) -> Result<Vec<PickTask>> {
        self.list_picks(PickTaskFilter { order_id: Some(order_id), ..Default::default() })
    }

    fn get_picks_for_wave(&self, wave_id: FulfillmentId) -> Result<Vec<PickTask>> {
        self.list_picks(PickTaskFilter { wave_id: Some(wave_id), ..Default::default() })
    }

    fn count_picks(&self, filter: PickTaskFilter) -> Result<u64> {
        let conn = self.conn()?;
        let mut sql = "SELECT COUNT(*) FROM pick_tasks WHERE 1=1".to_string();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(status) = filter.status {
            sql.push_str(" AND status = ?");
            params_vec.push(Box::new(status.to_string()));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();
        let count: i64 =
            conn.query_row(&sql, params_refs.as_slice(), |row| row.get(0)).map_err(map_db_error)?;
        Ok(count as u64)
    }

    // ========================================================================
    // Pack Operations
    // ========================================================================

    fn create_pack(&self, input: CreatePackTask) -> Result<PackTask> {
        let conn = self.conn()?;
        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4();

        conn.execute(
            "INSERT INTO pack_tasks (id, order_id, status, notes, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            params![
                id.to_string(),
                input.order_id.to_string(),
                PackStatus::Pending.to_string(),
                input.notes,
                now,
            ],
        )
        .map_err(map_db_error)?;

        drop(conn);
        self.get_pack(id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to create pack".into()))
    }

    fn get_pack(&self, id: Uuid) -> Result<Option<PackTask>> {
        let conn = self.conn()?;
        let mut stmt =
            conn.prepare("SELECT * FROM pack_tasks WHERE id = ?1").map_err(map_db_error)?;
        let mut rows = stmt.query(params![id.to_string()]).map_err(map_db_error)?;

        if let Some(row) = rows.next().map_err(map_db_error)? {
            Ok(Some(Self::row_to_pack(row).map_err(map_db_error)?))
        } else {
            Ok(None)
        }
    }

    fn list_packs(&self, filter: PackTaskFilter) -> Result<Vec<PackTask>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM pack_tasks WHERE 1=1".to_string();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(order_id) = filter.order_id {
            sql.push_str(" AND order_id = ?");
            params_vec.push(Box::new(order_id.to_string()));
        }

        if let Some(status) = filter.status {
            sql.push_str(" AND status = ?");
            params_vec.push(Box::new(status.to_string()));
        }

        sql.push_str(" ORDER BY created_at");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();
        let mut rows = stmt.query(params_refs.as_slice()).map_err(map_db_error)?;

        let mut packs = Vec::new();
        while let Some(row) = rows.next().map_err(map_db_error)? {
            packs.push(Self::row_to_pack(row).map_err(map_db_error)?);
        }
        Ok(packs)
    }

    fn assign_pack(&self, id: Uuid, assigned_to: &str) -> Result<PackTask> {
        let conn = self.conn()?;

        conn.execute(
            "UPDATE pack_tasks SET assigned_to = ?1 WHERE id = ?2",
            params![assigned_to, id.to_string()],
        )
        .map_err(map_db_error)?;

        drop(conn);
        self.get_pack(id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to assign pack".into()))
    }

    fn start_pack(&self, id: Uuid) -> Result<PackTask> {
        let conn = self.conn()?;
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "UPDATE pack_tasks SET status = ?1, started_at = ?2 WHERE id = ?3",
            params![PackStatus::InProgress.to_string(), now, id.to_string()],
        )
        .map_err(map_db_error)?;

        drop(conn);
        self.get_pack(id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to start pack".into()))
    }

    fn complete_pack(&self, id: Uuid) -> Result<PackTask> {
        let conn = self.conn()?;
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "UPDATE pack_tasks SET status = ?1, completed_at = ?2 WHERE id = ?3",
            params![PackStatus::Completed.to_string(), now, id.to_string()],
        )
        .map_err(map_db_error)?;

        drop(conn);
        self.get_pack(id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to complete pack".into()))
    }

    fn add_carton(&self, input: AddCarton) -> Result<Carton> {
        let conn = self.conn()?;
        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4();
        let carton_number = generate_carton_number();

        conn.execute(
            "INSERT INTO cartons (id, pack_task_id, carton_number, package_type, weight_kg, length_cm, width_cm, height_cm, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                id.to_string(),
                input.pack_task_id.to_string(),
                carton_number,
                input.package_type.to_string(),
                input.weight_kg.map(|d| d.to_string()),
                input.length_cm.map(|d| d.to_string()),
                input.width_cm.map(|d| d.to_string()),
                input.height_cm.map(|d| d.to_string()),
                now,
            ],
        ).map_err(map_db_error)?;

        // Update carton count
        conn.execute(
            "UPDATE pack_tasks SET carton_count = carton_count + 1 WHERE id = ?1",
            params![input.pack_task_id.to_string()],
        )
        .map_err(map_db_error)?;

        let mut stmt = conn.prepare("SELECT * FROM cartons WHERE id = ?1").map_err(map_db_error)?;
        let mut rows = stmt.query(params![id.to_string()]).map_err(map_db_error)?;

        if let Some(row) = rows.next().map_err(map_db_error)? {
            Ok(Self::row_to_carton(row).map_err(map_db_error)?)
        } else {
            Err(CommerceError::DatabaseError("Failed to create carton".into()))
        }
    }

    fn add_carton_item(&self, input: AddCartonItem) -> Result<CartonItem> {
        let conn = self.conn()?;
        let id = Uuid::new_v4();

        conn.execute(
            "INSERT INTO carton_items (id, carton_id, sku, quantity, lot_id, serial_number)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                id.to_string(),
                input.carton_id.to_string(),
                input.sku,
                input.quantity.to_string(),
                input.lot_id.map(|id| id.to_string()),
                input.serial_number,
            ],
        )
        .map_err(map_db_error)?;

        let mut stmt =
            conn.prepare("SELECT * FROM carton_items WHERE id = ?1").map_err(map_db_error)?;
        let mut rows = stmt.query(params![id.to_string()]).map_err(map_db_error)?;

        if let Some(row) = rows.next().map_err(map_db_error)? {
            Ok(Self::row_to_carton_item(row).map_err(map_db_error)?)
        } else {
            Err(CommerceError::DatabaseError("Failed to create carton item".into()))
        }
    }

    fn get_cartons(&self, pack_task_id: Uuid) -> Result<Vec<Carton>> {
        let conn = self.conn()?;
        let mut stmt =
            conn.prepare("SELECT * FROM cartons WHERE pack_task_id = ?1").map_err(map_db_error)?;
        let mut rows = stmt.query(params![pack_task_id.to_string()]).map_err(map_db_error)?;

        let mut cartons = Vec::new();
        while let Some(row) = rows.next().map_err(map_db_error)? {
            cartons.push(Self::row_to_carton(row).map_err(map_db_error)?);
        }
        Ok(cartons)
    }

    fn get_carton_items(&self, carton_id: Uuid) -> Result<Vec<CartonItem>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT * FROM carton_items WHERE carton_id = ?1")
            .map_err(map_db_error)?;
        let mut rows = stmt.query(params![carton_id.to_string()]).map_err(map_db_error)?;

        let mut items = Vec::new();
        while let Some(row) = rows.next().map_err(map_db_error)? {
            items.push(Self::row_to_carton_item(row).map_err(map_db_error)?);
        }
        Ok(items)
    }

    fn mark_label_printed(&self, carton_id: Uuid) -> Result<Carton> {
        let conn = self.conn()?;

        conn.execute(
            "UPDATE cartons SET label_printed = 1 WHERE id = ?1",
            params![carton_id.to_string()],
        )
        .map_err(map_db_error)?;

        let mut stmt = conn.prepare("SELECT * FROM cartons WHERE id = ?1").map_err(map_db_error)?;
        let mut rows = stmt.query(params![carton_id.to_string()]).map_err(map_db_error)?;

        if let Some(row) = rows.next().map_err(map_db_error)? {
            Ok(Self::row_to_carton(row).map_err(map_db_error)?)
        } else {
            Err(CommerceError::NotFound)
        }
    }

    fn cancel_pack(&self, id: Uuid) -> Result<PackTask> {
        let conn = self.conn()?;

        conn.execute(
            "UPDATE pack_tasks SET status = ?1 WHERE id = ?2",
            params![PackStatus::Cancelled.to_string(), id.to_string()],
        )
        .map_err(map_db_error)?;

        drop(conn);
        self.get_pack(id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to cancel pack".into()))
    }

    fn count_packs(&self, filter: PackTaskFilter) -> Result<u64> {
        let conn = self.conn()?;
        let mut sql = "SELECT COUNT(*) FROM pack_tasks WHERE 1=1".to_string();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(status) = filter.status {
            sql.push_str(" AND status = ?");
            params_vec.push(Box::new(status.to_string()));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();
        let count: i64 =
            conn.query_row(&sql, params_refs.as_slice(), |row| row.get(0)).map_err(map_db_error)?;
        Ok(count as u64)
    }

    // ========================================================================
    // Ship Operations
    // ========================================================================

    fn create_ship(&self, input: CreateShipTask) -> Result<ShipTask> {
        let conn = self.conn()?;
        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4();

        conn.execute(
            "INSERT INTO ship_tasks (id, order_id, shipment_id, pack_task_id, status, carrier, service_level, notes, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
            params![
                id.to_string(),
                input.order_id.to_string(),
                input.shipment_id.to_string(),
                input.pack_task_id.to_string(),
                ShipStatus::Pending.to_string(),
                input.carrier,
                input.service_level,
                input.notes,
                now,
            ],
        ).map_err(map_db_error)?;

        drop(conn);
        self.get_ship(id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to create ship task".into()))
    }

    fn get_ship(&self, id: Uuid) -> Result<Option<ShipTask>> {
        let conn = self.conn()?;
        let mut stmt =
            conn.prepare("SELECT * FROM ship_tasks WHERE id = ?1").map_err(map_db_error)?;
        let mut rows = stmt.query(params![id.to_string()]).map_err(map_db_error)?;

        if let Some(row) = rows.next().map_err(map_db_error)? {
            Ok(Some(Self::row_to_ship(row).map_err(map_db_error)?))
        } else {
            Ok(None)
        }
    }

    fn list_ships(&self, filter: ShipTaskFilter) -> Result<Vec<ShipTask>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM ship_tasks WHERE 1=1".to_string();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(order_id) = filter.order_id {
            sql.push_str(" AND order_id = ?");
            params_vec.push(Box::new(order_id.to_string()));
        }

        if let Some(status) = filter.status {
            sql.push_str(" AND status = ?");
            params_vec.push(Box::new(status.to_string()));
        }

        sql.push_str(" ORDER BY created_at");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();
        let mut rows = stmt.query(params_refs.as_slice()).map_err(map_db_error)?;

        let mut ships = Vec::new();
        while let Some(row) = rows.next().map_err(map_db_error)? {
            ships.push(Self::row_to_ship(row).map_err(map_db_error)?);
        }
        Ok(ships)
    }

    fn assign_ship(&self, id: Uuid, assigned_to: &str) -> Result<ShipTask> {
        let conn = self.conn()?;

        conn.execute(
            "UPDATE ship_tasks SET assigned_to = ?1 WHERE id = ?2",
            params![assigned_to, id.to_string()],
        )
        .map_err(map_db_error)?;

        drop(conn);
        self.get_ship(id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to assign ship".into()))
    }

    fn print_label(&self, id: Uuid, label_url: &str) -> Result<ShipTask> {
        let conn = self.conn()?;

        conn.execute(
            "UPDATE ship_tasks SET status = ?1, label_url = ?2 WHERE id = ?3",
            params![ShipStatus::LabelPrinted.to_string(), label_url, id.to_string()],
        )
        .map_err(map_db_error)?;

        drop(conn);
        self.get_ship(id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to update ship".into()))
    }

    fn complete_ship(&self, input: CompleteShip) -> Result<ShipTask> {
        let conn = self.conn()?;
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "UPDATE ship_tasks SET status = ?1, tracking_number = ?2, shipping_cost = ?3, shipped_at = ?4 WHERE id = ?5",
            params![
                ShipStatus::Shipped.to_string(),
                input.tracking_number,
                input.shipping_cost.map(|d| d.to_string()),
                now,
                input.ship_task_id.to_string(),
            ],
        ).map_err(map_db_error)?;

        drop(conn);
        self.get_ship(input.ship_task_id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to complete ship".into()))
    }

    fn cancel_ship(&self, id: Uuid) -> Result<ShipTask> {
        let conn = self.conn()?;

        conn.execute(
            "UPDATE ship_tasks SET status = ?1 WHERE id = ?2",
            params![ShipStatus::Cancelled.to_string(), id.to_string()],
        )
        .map_err(map_db_error)?;

        drop(conn);
        self.get_ship(id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to cancel ship".into()))
    }

    fn count_ships(&self, filter: ShipTaskFilter) -> Result<u64> {
        let conn = self.conn()?;
        let mut sql = "SELECT COUNT(*) FROM ship_tasks WHERE 1=1".to_string();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(status) = filter.status {
            sql.push_str(" AND status = ?");
            params_vec.push(Box::new(status.to_string()));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();
        let count: i64 =
            conn.query_row(&sql, params_refs.as_slice(), |row| row.get(0)).map_err(map_db_error)?;
        Ok(count as u64)
    }

    // ========================================================================
    // Workflow Helpers
    // ========================================================================

    fn create_picks_for_order(&self, order_id: OrderId, warehouse_id: i32) -> Result<Vec<PickTask>> {
        let conn = self.conn()?;

        let mut inputs = Vec::new();
        {
            // Get order items
            let mut stmt = conn
                .prepare("SELECT id, sku, name, quantity FROM order_items WHERE order_id = ?1")
                .map_err(map_db_error)?;

            let mut rows = stmt.query(params![order_id.to_string()]).map_err(map_db_error)?;

            while let Some(row) = rows.next().map_err(map_db_error)? {
                let item_id_str: String = row.get(0).map_err(map_db_error)?;
                let sku: String = row.get(1).map_err(map_db_error)?;
                let name: Option<String> = row.get(2).map_err(map_db_error)?;
                let qty: i32 = row.get(3).map_err(map_db_error)?;

                // Find a location with inventory
                let location_id: i32 = conn
                    .query_row(
                        "SELECT l.id FROM locations l
                     JOIN location_inventory li ON l.id = li.location_id
                     WHERE l.warehouse_id = ?1 AND li.sku = ?2 AND l.is_pickable = 1
                     LIMIT 1",
                        params![warehouse_id, sku],
                        |row| row.get(0),
                    )
                    .unwrap_or(1); // Default to location 1 if not found

                inputs.push(CreatePickTask {
                    wave_id: None,
                    order_id,
                    order_item_id: OrderItemId::from(parse_uuid(&item_id_str, "order_item", "id")?),
                    warehouse_id,
                    sku,
                    product_name: name,
                    source_location_id: location_id,
                    quantity_requested: Decimal::from(qty),
                    lot_id: None,
                    serial_number: None,
                    priority: None,
                    notes: None,
                });
            }
        }

        drop(conn);

        let mut picks = Vec::with_capacity(inputs.len());
        for input in inputs {
            picks.push(self.create_pick(input)?);
        }

        Ok(picks)
    }

    fn is_order_ready_to_pack(&self, order_id: OrderId) -> Result<bool> {
        let conn = self.conn()?;

        let incomplete: i64 = conn.query_row(
            "SELECT COUNT(*) FROM pick_tasks WHERE order_id = ?1 AND status NOT IN ('completed', 'short', 'cancelled')",
            params![order_id.to_string()],
            |row| row.get(0),
        ).map_err(map_db_error)?;

        Ok(incomplete == 0)
    }

    fn is_order_ready_to_ship(&self, order_id: OrderId) -> Result<bool> {
        let conn = self.conn()?;

        let completed: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pack_tasks WHERE order_id = ?1 AND status = 'completed'",
                params![order_id.to_string()],
                |row| row.get(0),
            )
            .map_err(map_db_error)?;

        Ok(completed > 0)
    }

    // ========================================================================
    // Batch Operations
    // ========================================================================

    fn create_waves_batch(&self, inputs: Vec<CreateWave>) -> Result<BatchResult<Wave>> {
        let mut result = BatchResult::new();

        for (index, input) in inputs.into_iter().enumerate() {
            match self.create_wave(input) {
                Ok(wave) => result.record_success(wave),
                Err(e) => result.record_failure(index, None, &e),
            }
        }

        Ok(result)
    }

    fn get_picks_batch(&self, ids: Vec<Uuid>) -> Result<Vec<PickTask>> {
        let mut picks = Vec::new();
        for id in ids {
            if let Some(pick) = self.get_pick(id)? {
                picks.push(pick);
            }
        }
        Ok(picks)
    }
}
