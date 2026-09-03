//! SQLite implementation for fulfillment (pick/pack/ship) management

use crate::sqlite::{
    map_db_error, parse_datetime_opt_row, parse_datetime_row, parse_decimal_opt_row,
    parse_decimal_row, parse_enum_row, parse_uuid, parse_uuid_opt_row, parse_uuid_row,
    with_immediate_transaction,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{OptionalExtension, params};
use rust_decimal::Decimal;
use uuid::Uuid;

use stateset_core::{
    AddCarton, AddCartonItem, BatchResult, Carton, CartonItem, CommerceError, CompletePick,
    CompleteShip, CreatePackTask, CreatePickTask, CreateShipTask, CreateWave, FulfillmentId,
    FulfillmentRepository, MovementType, OrderId, OrderItemId, PackStatus, PackTask,
    PackTaskFilter, PickStatus, PickTask, PickTaskFilter, Result, ShipStatus, ShipTask,
    ShipTaskFilter, ShipmentId, Wave, WaveFilter, WaveStatus, generate_carton_number,
    generate_wave_number,
};

/// Take the picked units off the shelf.
///
/// The source bin loses them (`location_inventory`) and the warehouse balance
/// marks them allocated: off the shelf, committed to the order, still in the
/// building. Runs on the caller's transaction. A pick that moved nothing
/// (`quantity_picked` zero, or a pure shortage) has no stock effect.
fn apply_pick_stock_effect_tx(
    tx: &rusqlite::Transaction<'_>,
    pick: &PickTask,
    now: &str,
) -> rusqlite::Result<()> {
    if pick.quantity_picked <= Decimal::ZERO {
        return Ok(());
    }
    super::warehouse::ensure_inventory_item_tx(tx, &pick.sku)?;
    super::warehouse::apply_location_delta_tx(
        tx,
        pick.source_location_id,
        &pick.sku,
        pick.lot_id,
        -pick.quantity_picked,
        now,
    )?;
    super::bins::apply_warehouse_delta_tx(
        tx,
        pick.warehouse_id,
        &pick.sku,
        Decimal::ZERO,
        pick.quantity_picked,
        "pick completed",
        Some("pick_task"),
        Some(&pick.id.to_string()),
        now,
    )?;
    super::warehouse::insert_wms_movement_tx(
        tx,
        MovementType::Pick,
        Some(pick.source_location_id),
        None,
        &pick.sku,
        pick.lot_id,
        pick.quantity_picked,
        "pick_task",
        &pick.id.to_string(),
        pick.assigned_to.as_deref(),
        now,
    )?;
    Ok(())
}

/// Ship the units the pack task's cartons hold: warehouse `on_hand` and
/// `allocated` both fall, releasing exactly what the picks allocated.
///
/// The warehouse is taken from the order's pick tasks (a ship task carries no
/// warehouse of its own); with no picks it falls back to the default warehouse.
fn apply_ship_stock_effect_tx(
    tx: &rusqlite::Transaction<'_>,
    ship: &ShipTask,
    shipped_by: Option<&str>,
    now: &str,
) -> rusqlite::Result<()> {
    let shipped: Vec<(String, String, Option<String>)> = {
        let mut stmt = tx.prepare(
            "SELECT ci.sku, ci.quantity, ci.lot_id FROM carton_items ci
             JOIN cartons c ON c.id = ci.carton_id
             WHERE c.pack_task_id = ?1",
        )?;
        let rows = stmt.query_map(params![ship.pack_task_id.to_string()], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    if shipped.is_empty() {
        return Ok(());
    }
    let warehouse_id: i32 = tx
        .query_row(
            "SELECT warehouse_id FROM pick_tasks WHERE order_id = ?1 LIMIT 1",
            params![ship.order_id.to_string()],
            |row| row.get(0),
        )
        .optional()?
        .unwrap_or(1);
    let ship_id = ship.id.to_string();
    for (sku, quantity, lot_id) in shipped {
        let quantity = parse_decimal_row(&quantity, "carton_item", "quantity")?;
        if quantity <= Decimal::ZERO {
            continue;
        }
        let lot_id = parse_uuid_opt_row(lot_id, "carton_item", "lot_id")?;
        super::bins::apply_warehouse_delta_tx(
            tx,
            warehouse_id,
            &sku,
            -quantity,
            -quantity,
            "ship task completed",
            Some("ship_task"),
            Some(&ship_id),
            now,
        )?;
        super::warehouse::insert_wms_movement_tx(
            tx,
            MovementType::Shipment,
            None,
            None,
            &sku,
            lot_id,
            quantity,
            "ship_task",
            &ship_id,
            shipped_by,
            now,
        )?;
    }
    Ok(())
}

/// SQLite fulfillment repository
#[derive(Debug)]
pub struct SqliteFulfillmentRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteFulfillmentRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
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
            wave_id: parse_uuid_opt_row(wave_id_str, "pick_task", "wave_id")?
                .map(FulfillmentId::from),
            order_id: OrderId::from(parse_uuid_row(&order_id_str, "pick_task", "order_id")?),
            order_item_id: OrderItemId::from(parse_uuid_row(
                &order_item_id_str,
                "pick_task",
                "order_item_id",
            )?),
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

    /// Smuggle a domain error through the `rusqlite` closure boundary so
    /// [`map_db_error`] unwraps it again (and `with_retry` never mistakes it for
    /// a lock error and retries it).
    fn smuggle(e: CommerceError) -> rusqlite::Error {
        rusqlite::Error::ToSqlConversionFailure(Box::new(e))
    }

    /// Error for a status-guarded UPDATE that matched no row: either the row is
    /// gone (`NotFound`) or its current status forbids the transition
    /// (`Conflict`, naming the status that blocked it).
    ///
    /// `table` and `entity` are always in-crate string literals, never caller
    /// input, so interpolating `table` into the SQL is safe.
    fn transition_conflict(
        tx: &rusqlite::Transaction<'_>,
        table: &str,
        entity: &str,
        id: &str,
        action: &str,
    ) -> rusqlite::Error {
        let current: Option<String> = tx
            .query_row(&format!("SELECT status FROM {table} WHERE id = ?1"), params![id], |row| {
                row.get(0)
            })
            .ok();
        current.map_or_else(
            || Self::smuggle(CommerceError::NotFound),
            |status| {
                Self::smuggle(CommerceError::Conflict(format!(
                    "cannot {action} {entity} {id}: status is {status}"
                )))
            },
        )
    }

    /// Read a single pick task by id from within a transaction (returns
    /// `NotFound` smuggled through `rusqlite::Error` if it does not exist).
    fn read_pick_by_id_tx(
        tx: &rusqlite::Transaction<'_>,
        id_str: &str,
    ) -> std::result::Result<PickTask, rusqlite::Error> {
        let mut stmt = tx.prepare("SELECT * FROM pick_tasks WHERE id = ?1")?;
        let mut rows = stmt.query(params![id_str])?;
        match rows.next()? {
            Some(row) => Self::row_to_pick(row),
            None => Err(Self::smuggle(CommerceError::NotFound)),
        }
    }

    /// Read a single wave by id from within a transaction.
    fn read_wave_by_id_tx(
        tx: &rusqlite::Transaction<'_>,
        id_str: &str,
    ) -> std::result::Result<Wave, rusqlite::Error> {
        let mut stmt = tx.prepare("SELECT * FROM waves WHERE id = ?1")?;
        let mut rows = stmt.query(params![id_str])?;
        match rows.next()? {
            Some(row) => Self::row_to_wave(row),
            None => Err(Self::smuggle(CommerceError::NotFound)),
        }
    }

    /// Read a single pack task by id from within a transaction.
    fn read_pack_by_id_tx(
        tx: &rusqlite::Transaction<'_>,
        id_str: &str,
    ) -> std::result::Result<PackTask, rusqlite::Error> {
        let mut stmt = tx.prepare("SELECT * FROM pack_tasks WHERE id = ?1")?;
        let mut rows = stmt.query(params![id_str])?;
        match rows.next()? {
            Some(row) => Self::row_to_pack(row),
            None => Err(Self::smuggle(CommerceError::NotFound)),
        }
    }

    /// Read a single ship task by id from within a transaction.
    fn read_ship_by_id_tx(
        tx: &rusqlite::Transaction<'_>,
        id_str: &str,
    ) -> std::result::Result<ShipTask, rusqlite::Error> {
        let mut stmt = tx.prepare("SELECT * FROM ship_tasks WHERE id = ?1")?;
        let mut rows = stmt.query(params![id_str])?;
        match rows.next()? {
            Some(row) => Self::row_to_ship(row),
            None => Err(Self::smuggle(CommerceError::NotFound)),
        }
    }

    /// Read a single carton by id from within a transaction.
    fn read_carton_by_id_tx(
        tx: &rusqlite::Transaction<'_>,
        id_str: &str,
    ) -> std::result::Result<Carton, rusqlite::Error> {
        let mut stmt = tx.prepare("SELECT * FROM cartons WHERE id = ?1")?;
        let mut rows = stmt.query(params![id_str])?;
        match rows.next()? {
            Some(row) => Self::row_to_carton(row),
            None => Err(Self::smuggle(CommerceError::NotFound)),
        }
    }

    /// Insert one pick task inside a transaction and read it back.
    ///
    /// Shared by `create_pick` and `create_picks_for_order` so a multi-line
    /// order's picks are created by the same statement in one transaction.
    fn insert_pick_tx(
        tx: &rusqlite::Transaction<'_>,
        input: &CreatePickTask,
    ) -> std::result::Result<PickTask, rusqlite::Error> {
        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4();
        let id_str = id.to_string();

        // Count the pick on its wave. A wave that is already completed or
        // cancelled cannot take new picks: they would never be reflected in
        // its counters and `complete_wave` could no longer reconcile them.
        if let Some(wave_id) = input.wave_id {
            let wave_id_str = wave_id.to_string();
            let changed = tx.execute(
                "UPDATE waves SET pick_count = pick_count + 1
                 WHERE id = ?1 AND status IN ('draft', 'released', 'in_progress')",
                params![wave_id_str],
            )?;
            if changed == 0 {
                return Err(Self::transition_conflict(
                    tx,
                    "waves",
                    "wave",
                    &wave_id_str,
                    "add a pick to",
                ));
            }
        }

        tx.execute(
            "INSERT INTO pick_tasks (id, wave_id, order_id, order_item_id, warehouse_id, status, sku, product_name,
             source_location_id, quantity_requested, lot_id, serial_number, priority, notes, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?15)",
            params![
                id_str,
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
        )?;

        Self::read_pick_by_id_tx(tx, &id_str)
    }

    /// Guard that a pack task is still open for carton changes.
    ///
    /// Cartons may only be added while the pack task is in one of
    /// `Pending`/`ReadyToPack`/`Assigned`/`InProgress`; adding one to a
    /// `Completed` or `Cancelled` pack task inflates `pack_tasks.carton_count`
    /// for a sealed shipment.
    fn ensure_pack_open_tx(
        tx: &rusqlite::Transaction<'_>,
        pack_task_id: &str,
        action: &str,
    ) -> std::result::Result<(), rusqlite::Error> {
        let status: Option<String> = tx
            .query_row(
                "SELECT status FROM pack_tasks WHERE id = ?1",
                params![pack_task_id],
                |row| row.get(0),
            )
            .optional()?;
        match status {
            None => Err(Self::smuggle(CommerceError::NotFound)),
            Some(status) if matches!(status.as_str(), "completed" | "cancelled") => {
                Err(Self::smuggle(CommerceError::Conflict(format!(
                    "cannot {action} pack task {pack_task_id}: status is {status}"
                ))))
            }
            Some(_) => Ok(()),
        }
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
            shipment_id: parse_uuid_opt_row(shipment_id_str, "pack_task", "shipment_id")?
                .map(ShipmentId::from),
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
            shipment_id: ShipmentId::from(parse_uuid_row(
                &shipment_id_str,
                "ship_task",
                "shipment_id",
            )?),
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
        let now = Utc::now().to_rfc3339();
        let id = FulfillmentId::new();
        let id_str = id.to_string();
        let wave_number = generate_wave_number();
        let order_count = input.order_ids.len() as i32;

        // The wave header and its `wave_orders` rows are one document: writing
        // them on a bare connection let a mid-loop failure leave a wave whose
        // `order_count` does not match the orders actually attached to it.
        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "INSERT INTO waves (id, wave_number, warehouse_id, status, order_count, priority, notes, created_by, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
                params![
                    id_str,
                    wave_number,
                    input.warehouse_id,
                    WaveStatus::Draft.to_string(),
                    order_count,
                    input.priority.unwrap_or(0),
                    input.notes,
                    input.created_by,
                    now,
                ],
            )?;

            // Add order associations
            for order_id in &input.order_ids {
                tx.execute(
                    "INSERT INTO wave_orders (wave_id, order_id) VALUES (?1, ?2)",
                    params![id_str, order_id.to_string()],
                )?;
            }

            Self::read_wave_by_id_tx(tx, &id_str)
        })
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

        if let Some(from_date) = filter.from_date {
            sql.push_str(" AND created_at >= ?");
            params_vec.push(Box::new(from_date.to_rfc3339()));
        }

        if let Some(to_date) = filter.to_date {
            sql.push_str(" AND created_at <= ?");
            params_vec.push(Box::new(to_date.to_rfc3339()));
        }

        sql.push_str(" ORDER BY priority DESC, created_at DESC");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {limit}"));
        }

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(std::convert::AsRef::as_ref).collect();
        let mut rows = stmt.query(params_refs.as_slice()).map_err(map_db_error)?;

        let mut waves = Vec::new();
        while let Some(row) = rows.next().map_err(map_db_error)? {
            waves.push(Self::row_to_wave(row).map_err(map_db_error)?);
        }
        Ok(waves)
    }

    /// Release a wave for picking.
    ///
    /// Legal only from [`WaveStatus::Draft`]; the guard used to be a silent
    /// `AND status = 'draft'` that reported success (returning the untouched
    /// wave) when it matched nothing.
    fn release_wave(&self, id: FulfillmentId) -> Result<Wave> {
        let now = Utc::now().to_rfc3339();
        let id_str = id.to_string();

        with_immediate_transaction(&self.pool, |tx| {
            let changed = tx.execute(
                "UPDATE waves SET status = ?1, started_at = ?2
                 WHERE id = ?3 AND status = 'draft'",
                params![WaveStatus::Released.to_string(), now, id_str],
            )?;
            if changed == 0 {
                return Err(Self::transition_conflict(tx, "waves", "wave", &id_str, "release"));
            }
            Self::read_wave_by_id_tx(tx, &id_str)
        })
    }

    /// Complete a wave.
    ///
    /// Legal only from [`WaveStatus::Released`] or [`WaveStatus::InProgress`]:
    /// a `Draft` wave was never on the floor, and a `Cancelled` or already
    /// `Completed` wave is terminal. Completing a cancelled wave used to
    /// succeed, resurrecting it with counters that describe nothing.
    ///
    /// A wave also cannot complete while any of its picks is still open
    /// (`pending`/`assigned`/`in_progress`): the predicate is computed from the
    /// `pick_tasks` table rather than the `pick_count` counter so waves created
    /// before the counter was maintained are judged correctly.
    fn complete_wave(&self, id: FulfillmentId) -> Result<Wave> {
        let now = Utc::now().to_rfc3339();
        let id_str = id.to_string();

        with_immediate_transaction(&self.pool, |tx| {
            let changed = tx.execute(
                "UPDATE waves SET status = ?1, completed_at = ?2
                 WHERE id = ?3 AND status IN ('released', 'in_progress')
                   AND NOT EXISTS (
                       SELECT 1 FROM pick_tasks
                       WHERE wave_id = waves.id
                         AND status IN ('pending', 'assigned', 'in_progress'))",
                params![WaveStatus::Completed.to_string(), now, id_str],
            )?;
            if changed == 0 {
                let open: i64 = tx.query_row(
                    "SELECT COUNT(*) FROM pick_tasks WHERE wave_id = ?1
                     AND status IN ('pending', 'assigned', 'in_progress')",
                    params![id_str],
                    |row| row.get(0),
                )?;
                let status: Option<String> = tx
                    .query_row("SELECT status FROM waves WHERE id = ?1", params![id_str], |row| {
                        row.get(0)
                    })
                    .ok();
                if open > 0 && matches!(status.as_deref(), Some("released" | "in_progress")) {
                    return Err(Self::smuggle(CommerceError::ValidationError(format!(
                        "cannot complete wave {id_str}: {open} pick task(s) still open"
                    ))));
                }
                return Err(Self::transition_conflict(tx, "waves", "wave", &id_str, "complete"));
            }
            Self::read_wave_by_id_tx(tx, &id_str)
        })
    }

    /// Cancel a wave.
    ///
    /// Legal from [`WaveStatus::Draft`], [`WaveStatus::Released`] or
    /// [`WaveStatus::InProgress`]; a `Completed` wave's picks are already
    /// folded into its counters and a `Cancelled` one is terminal.
    fn cancel_wave(&self, id: FulfillmentId) -> Result<Wave> {
        let id_str = id.to_string();

        with_immediate_transaction(&self.pool, |tx| {
            let changed = tx.execute(
                "UPDATE waves SET status = ?1
                 WHERE id = ?2 AND status IN ('draft', 'released', 'in_progress')",
                params![WaveStatus::Cancelled.to_string(), id_str],
            )?;
            if changed == 0 {
                return Err(Self::transition_conflict(tx, "waves", "wave", &id_str, "cancel"));
            }
            Self::read_wave_by_id_tx(tx, &id_str)
        })
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

    /// Count waves matching `filter`.
    ///
    /// Applies exactly the filters `list_waves` applies (and that the Postgres
    /// backend counts on): a count that ignored `warehouse_id` reported another
    /// warehouse's waves as the page total.
    fn count_waves(&self, filter: WaveFilter) -> Result<u64> {
        let conn = self.conn()?;
        let mut sql = "SELECT COUNT(*) FROM waves WHERE 1=1".to_string();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(warehouse_id) = filter.warehouse_id {
            sql.push_str(" AND warehouse_id = ?");
            params_vec.push(Box::new(warehouse_id));
        }

        if let Some(status) = filter.status {
            sql.push_str(" AND status = ?");
            params_vec.push(Box::new(status.to_string()));
        }

        if let Some(from_date) = filter.from_date {
            sql.push_str(" AND created_at >= ?");
            params_vec.push(Box::new(from_date.to_rfc3339()));
        }

        if let Some(to_date) = filter.to_date {
            sql.push_str(" AND created_at <= ?");
            params_vec.push(Box::new(to_date.to_rfc3339()));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(std::convert::AsRef::as_ref).collect();
        let count: i64 =
            conn.query_row(&sql, params_refs.as_slice(), |row| row.get(0)).map_err(map_db_error)?;
        Ok(count as u64)
    }

    // ========================================================================
    // Pick Operations
    // ========================================================================

    fn create_pick(&self, input: CreatePickTask) -> Result<PickTask> {
        with_immediate_transaction(&self.pool, |tx| Self::insert_pick_tx(tx, &input))
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
            sql.push_str(&format!(" LIMIT {limit}"));
        }

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(std::convert::AsRef::as_ref).collect();
        let mut rows = stmt.query(params_refs.as_slice()).map_err(map_db_error)?;

        let mut picks = Vec::new();
        while let Some(row) = rows.next().map_err(map_db_error)? {
            picks.push(Self::row_to_pick(row).map_err(map_db_error)?);
        }
        Ok(picks)
    }

    /// Assign (or re-assign) a pick task to a worker.
    ///
    /// Legal from [`PickStatus::Pending`] or [`PickStatus::Assigned`]. It is
    /// refused once the pick has started or finished: the UPDATE also writes
    /// `status = 'assigned'`, so assigning a started pick would rewind it and
    /// assigning a finished one would resurrect it (and a later completion
    /// would double-count it into the wave).
    fn assign_pick(&self, id: Uuid, assigned_to: &str) -> Result<PickTask> {
        let id_str = id.to_string();

        with_immediate_transaction(&self.pool, |tx| {
            let changed = tx.execute(
                "UPDATE pick_tasks SET assigned_to = ?1, status = ?2
                 WHERE id = ?3 AND status IN ('pending', 'assigned')",
                params![assigned_to, PickStatus::Assigned.to_string(), id_str],
            )?;
            if changed == 0 {
                return Err(Self::transition_conflict(
                    tx,
                    "pick_tasks",
                    "pick task",
                    &id_str,
                    "assign",
                ));
            }
            Self::read_pick_by_id_tx(tx, &id_str)
        })
    }

    /// Start a pick task.
    ///
    /// Legal from [`PickStatus::Pending`] or [`PickStatus::Assigned`]. Starting
    /// an already-started pick would reset `started_at` (destroying the pick's
    /// measured duration); starting a finished or cancelled one is refused.
    fn start_pick(&self, id: Uuid) -> Result<PickTask> {
        let now = Utc::now().to_rfc3339();
        let id_str = id.to_string();

        with_immediate_transaction(&self.pool, |tx| {
            let changed = tx.execute(
                "UPDATE pick_tasks SET status = ?1, started_at = ?2
                 WHERE id = ?3 AND status IN ('pending', 'assigned')",
                params![PickStatus::InProgress.to_string(), now, id_str],
            )?;
            if changed == 0 {
                return Err(Self::transition_conflict(
                    tx,
                    "pick_tasks",
                    "pick task",
                    &id_str,
                    "start",
                ));
            }
            Self::read_pick_by_id_tx(tx, &id_str)
        })
    }

    /// Complete a pick task and take the picked units off the shelf.
    ///
    /// **Stock effect**: `quantity_picked` leaves the source location's on-hand
    /// and becomes `allocated` at warehouse level — the units are on the cart,
    /// committed to the order, no longer sellable but still in the building.
    /// The matching ship task releases exactly that allocation when the package
    /// leaves, so the pick/ship pair is self-balancing. The movement runs on the
    /// same transaction as the status write, and the "already finalized" guard
    /// above returns early, so completing twice never moves stock twice.
    fn complete_pick(&self, input: CompletePick) -> Result<PickTask> {
        let now = Utc::now().to_rfc3339();
        let short_qty = input.quantity_short.unwrap_or(Decimal::ZERO);
        let status =
            if short_qty > Decimal::ZERO { PickStatus::Short } else { PickStatus::Completed };
        let pick_id_str = input.pick_id.to_string();

        // The status read, guards, pick UPDATE and wave-counter increment all run
        // inside ONE `IMMEDIATE` transaction so concurrent completions serialize
        // and only a real state transition folds into the wave counter.
        with_immediate_transaction(&self.pool, |tx| {
            let (status_str, requested_str): (String, String) = tx
                .query_row(
                    "SELECT status, quantity_requested FROM pick_tasks WHERE id = ?1",
                    params![pick_id_str],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(CommerceError::NotFound))
                    }
                    other => other,
                })?;
            let current_status: PickStatus = parse_enum_row(&status_str, "pick_task", "status")?;
            let requested = parse_decimal_row(&requested_str, "pick_task", "quantity_requested")?;

            // A pick that is already finalized (Completed/Short) has already
            // incremented the wave's completed_pick_count; re-completing it must
            // be an idempotent no-op, never a double-count.
            if matches!(current_status, PickStatus::Completed | PickStatus::Short) {
                return Self::read_pick_by_id_tx(tx, &pick_id_str);
            }
            // A cancelled pick cannot be completed.
            if current_status == PickStatus::Cancelled {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    CommerceError::ValidationError("Cannot complete a cancelled pick task".into()),
                )));
            }
            // Over-pick guard: cannot pick more than was requested.
            if input.quantity_picked > requested {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    CommerceError::ValidationError(format!(
                        "Cannot pick {} of pick task {}: only {} were requested",
                        input.quantity_picked, input.pick_id, requested
                    )),
                )));
            }

            tx.execute(
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
                    pick_id_str,
                ],
            )?;

            let pick = Self::read_pick_by_id_tx(tx, &pick_id_str)?;
            if let Some(wave_id) = pick.wave_id {
                tx.execute(
                    "UPDATE waves SET completed_pick_count = completed_pick_count + 1 WHERE id = ?1",
                    params![wave_id.to_string()],
                )?;
            }
            apply_pick_stock_effect_tx(tx, &pick, &now)?;
            Ok(pick)
        })
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
    fn report_short(&self, id: Uuid, short_qty: Decimal, reason: &str) -> Result<PickTask> {
        let id_str = id.to_string();
        let now = Utc::now().to_rfc3339();

        with_immediate_transaction(&self.pool, |tx| {
            let changed = tx.execute(
                "UPDATE pick_tasks SET status = ?1, quantity_short = ?2, notes = ?3,
                 completed_at = ?4
                 WHERE id = ?5 AND status IN ('pending', 'assigned', 'in_progress')",
                params![PickStatus::Short.to_string(), short_qty.to_string(), reason, now, id_str,],
            )?;
            if changed == 0 {
                return Err(Self::transition_conflict(
                    tx,
                    "pick_tasks",
                    "pick task",
                    &id_str,
                    "report a shortage on",
                ));
            }

            let pick = Self::read_pick_by_id_tx(tx, &id_str)?;
            if let Some(wave_id) = pick.wave_id {
                tx.execute(
                    "UPDATE waves SET completed_pick_count = completed_pick_count + 1 WHERE id = ?1",
                    params![wave_id.to_string()],
                )?;
            }
            Ok(pick)
        })
    }

    /// Cancel a pick task.
    ///
    /// Legal from `Pending`/`Assigned`/`InProgress` only. Cancelling a finished
    /// pick (`Completed`/`Short`) used to succeed while leaving the wave's
    /// `completed_pick_count` counting a pick that no longer claims to have
    /// happened, so the wave's counters stopped describing reality.
    fn cancel_pick(&self, id: Uuid) -> Result<PickTask> {
        let id_str = id.to_string();

        with_immediate_transaction(&self.pool, |tx| {
            let changed = tx.execute(
                "UPDATE pick_tasks SET status = ?1
                 WHERE id = ?2 AND status IN ('pending', 'assigned', 'in_progress')",
                params![PickStatus::Cancelled.to_string(), id_str],
            )?;
            if changed == 0 {
                return Err(Self::transition_conflict(
                    tx,
                    "pick_tasks",
                    "pick task",
                    &id_str,
                    "cancel",
                ));
            }
            let pick = Self::read_pick_by_id_tx(tx, &id_str)?;
            // A cancelled pick no longer counts toward the wave's workload.
            if let Some(wave_id) = pick.wave_id {
                tx.execute(
                    "UPDATE waves SET pick_count = MAX(pick_count - 1, 0) WHERE id = ?1",
                    params![wave_id.to_string()],
                )?;
            }
            Ok(pick)
        })
    }

    fn get_picks_for_order(&self, order_id: OrderId) -> Result<Vec<PickTask>> {
        self.list_picks(PickTaskFilter { order_id: Some(order_id), ..Default::default() })
    }

    fn get_picks_for_wave(&self, wave_id: FulfillmentId) -> Result<Vec<PickTask>> {
        self.list_picks(PickTaskFilter { wave_id: Some(wave_id), ..Default::default() })
    }

    /// Count pick tasks matching `filter` (same filters as `list_picks`).
    fn count_picks(&self, filter: PickTaskFilter) -> Result<u64> {
        let conn = self.conn()?;
        let mut sql = "SELECT COUNT(*) FROM pick_tasks WHERE 1=1".to_string();
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

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(std::convert::AsRef::as_ref).collect();
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

        if let Some(assigned_to) = filter.assigned_to {
            sql.push_str(" AND assigned_to = ?");
            params_vec.push(Box::new(assigned_to));
        }

        sql.push_str(" ORDER BY created_at");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {limit}"));
        }

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(std::convert::AsRef::as_ref).collect();
        let mut rows = stmt.query(params_refs.as_slice()).map_err(map_db_error)?;

        let mut packs = Vec::new();
        while let Some(row) = rows.next().map_err(map_db_error)? {
            packs.push(Self::row_to_pack(row).map_err(map_db_error)?);
        }
        Ok(packs)
    }

    /// Assign (or re-assign) a pack task to a packer.
    ///
    /// Legal from `Pending`/`ReadyToPack`/`Assigned`. Also writes
    /// `status = 'assigned'`, which the Postgres backend already did — SQLite
    /// silently left the status untouched, so the two backends disagreed about
    /// what an assigned pack task looks like.
    fn assign_pack(&self, id: Uuid, assigned_to: &str) -> Result<PackTask> {
        let id_str = id.to_string();

        with_immediate_transaction(&self.pool, |tx| {
            let changed = tx.execute(
                "UPDATE pack_tasks SET assigned_to = ?1, status = ?2
                 WHERE id = ?3 AND status IN ('pending', 'ready_to_pack', 'assigned')",
                params![assigned_to, PackStatus::Assigned.to_string(), id_str],
            )?;
            if changed == 0 {
                return Err(Self::transition_conflict(
                    tx,
                    "pack_tasks",
                    "pack task",
                    &id_str,
                    "assign",
                ));
            }
            Self::read_pack_by_id_tx(tx, &id_str)
        })
    }

    /// Start a pack task.
    ///
    /// Legal from `Pending`/`ReadyToPack`/`Assigned`; re-starting an in-progress
    /// pack would reset `started_at`, and a completed or cancelled pack task is
    /// terminal.
    fn start_pack(&self, id: Uuid) -> Result<PackTask> {
        let now = Utc::now().to_rfc3339();
        let id_str = id.to_string();

        with_immediate_transaction(&self.pool, |tx| {
            let changed = tx.execute(
                "UPDATE pack_tasks SET status = ?1, started_at = ?2
                 WHERE id = ?3 AND status IN ('pending', 'ready_to_pack', 'assigned')",
                params![PackStatus::InProgress.to_string(), now, id_str],
            )?;
            if changed == 0 {
                return Err(Self::transition_conflict(
                    tx,
                    "pack_tasks",
                    "pack task",
                    &id_str,
                    "start",
                ));
            }
            Self::read_pack_by_id_tx(tx, &id_str)
        })
    }

    /// Complete a pack task.
    ///
    /// Legal from any open status (`Pending`/`ReadyToPack`/`Assigned`/
    /// `InProgress`) — packing need not be explicitly started — but refused for
    /// the terminal `Completed`/`Cancelled`: re-completing rewrote
    /// `completed_at`, and completing a cancelled pack task resurrected it and
    /// made `is_order_ready_to_ship` true for an order nobody packed.
    fn complete_pack(&self, id: Uuid) -> Result<PackTask> {
        let now = Utc::now().to_rfc3339();
        let id_str = id.to_string();

        with_immediate_transaction(&self.pool, |tx| {
            let changed = tx.execute(
                "UPDATE pack_tasks SET status = ?1, completed_at = ?2
                 WHERE id = ?3 AND status IN ('pending', 'ready_to_pack', 'assigned', 'in_progress')",
                params![PackStatus::Completed.to_string(), now, id_str],
            )?;
            if changed == 0 {
                return Err(Self::transition_conflict(
                    tx,
                    "pack_tasks",
                    "pack task",
                    &id_str,
                    "complete",
                ));
            }
            Self::read_pack_by_id_tx(tx, &id_str)
        })
    }

    /// Add a carton to a pack task.
    ///
    /// The carton INSERT and the `pack_tasks.carton_count` increment are one
    /// transaction: on a bare connection a failure between them left a carton
    /// the pack task's count did not know about. The pack task must still be
    /// open — cartons cannot be added to a completed or cancelled one.
    fn add_carton(&self, input: AddCarton) -> Result<Carton> {
        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4();
        let id_str = id.to_string();
        let pack_task_id = input.pack_task_id.to_string();
        let carton_number = generate_carton_number();

        with_immediate_transaction(&self.pool, |tx| {
            Self::ensure_pack_open_tx(tx, &pack_task_id, "add a carton to")?;

            tx.execute(
                "INSERT INTO cartons (id, pack_task_id, carton_number, package_type, weight_kg, length_cm, width_cm, height_cm, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    id_str,
                    pack_task_id,
                    carton_number,
                    input.package_type.to_string(),
                    input.weight_kg.map(|d| d.to_string()),
                    input.length_cm.map(|d| d.to_string()),
                    input.width_cm.map(|d| d.to_string()),
                    input.height_cm.map(|d| d.to_string()),
                    now,
                ],
            )?;

            // Update carton count
            tx.execute(
                "UPDATE pack_tasks SET carton_count = carton_count + 1 WHERE id = ?1",
                params![pack_task_id],
            )?;

            Self::read_carton_by_id_tx(tx, &id_str)
        })
    }

    /// Add an item to a carton.
    ///
    /// Refused once the owning pack task is completed or cancelled — the
    /// carton's contents are then a sealed record of what shipped.
    fn add_carton_item(&self, input: AddCartonItem) -> Result<CartonItem> {
        let id = Uuid::new_v4();
        let id_str = id.to_string();
        let carton_id = input.carton_id.to_string();

        with_immediate_transaction(&self.pool, |tx| {
            let pack_task_id: String = tx
                .query_row(
                    "SELECT pack_task_id FROM cartons WHERE id = ?1",
                    params![carton_id],
                    |row| row.get(0),
                )
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => Self::smuggle(CommerceError::NotFound),
                    other => other,
                })?;
            Self::ensure_pack_open_tx(tx, &pack_task_id, "add carton items to")?;

            tx.execute(
                "INSERT INTO carton_items (id, carton_id, sku, quantity, lot_id, serial_number)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    id_str,
                    carton_id,
                    input.sku,
                    input.quantity.to_string(),
                    input.lot_id.map(|id| id.to_string()),
                    input.serial_number,
                ],
            )?;

            let mut stmt = tx.prepare("SELECT * FROM carton_items WHERE id = ?1")?;
            let mut rows = stmt.query(params![id_str])?;
            match rows.next()? {
                Some(row) => Self::row_to_carton_item(row),
                None => Err(Self::smuggle(CommerceError::DatabaseError(
                    "Failed to create carton item".into(),
                ))),
            }
        })
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
        let id_str = carton_id.to_string();

        with_immediate_transaction(&self.pool, |tx| {
            let changed =
                tx.execute("UPDATE cartons SET label_printed = 1 WHERE id = ?1", params![id_str])?;
            if changed == 0 {
                return Err(Self::smuggle(CommerceError::NotFound));
            }
            Self::read_carton_by_id_tx(tx, &id_str)
        })
    }

    /// Cancel a pack task.
    ///
    /// Legal from any open status; refused for `Completed` (its cartons already
    /// exist and its order counts as ready to ship) and for an already
    /// `Cancelled` task.
    fn cancel_pack(&self, id: Uuid) -> Result<PackTask> {
        let id_str = id.to_string();

        with_immediate_transaction(&self.pool, |tx| {
            let changed = tx.execute(
                "UPDATE pack_tasks SET status = ?1
                 WHERE id = ?2 AND status IN ('pending', 'ready_to_pack', 'assigned', 'in_progress')",
                params![PackStatus::Cancelled.to_string(), id_str],
            )?;
            if changed == 0 {
                return Err(Self::transition_conflict(
                    tx,
                    "pack_tasks",
                    "pack task",
                    &id_str,
                    "cancel",
                ));
            }
            Self::read_pack_by_id_tx(tx, &id_str)
        })
    }

    /// Count pack tasks matching `filter` (same filters as `list_packs`).
    fn count_packs(&self, filter: PackTaskFilter) -> Result<u64> {
        let conn = self.conn()?;
        let mut sql = "SELECT COUNT(*) FROM pack_tasks WHERE 1=1".to_string();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

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

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(std::convert::AsRef::as_ref).collect();
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

        if let Some(carrier) = filter.carrier {
            sql.push_str(" AND carrier = ?");
            params_vec.push(Box::new(carrier));
        }

        sql.push_str(" ORDER BY created_at");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {limit}"));
        }

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(std::convert::AsRef::as_ref).collect();
        let mut rows = stmt.query(params_refs.as_slice()).map_err(map_db_error)?;

        let mut ships = Vec::new();
        while let Some(row) = rows.next().map_err(map_db_error)? {
            ships.push(Self::row_to_ship(row).map_err(map_db_error)?);
        }
        Ok(ships)
    }

    /// Assign (or re-assign) a ship task.
    ///
    /// Legal from `Pending`/`ReadyToShip`/`LabelPrinted`; a shipped or
    /// cancelled task is terminal and cannot change hands.
    fn assign_ship(&self, id: Uuid, assigned_to: &str) -> Result<ShipTask> {
        let id_str = id.to_string();

        with_immediate_transaction(&self.pool, |tx| {
            let changed = tx.execute(
                "UPDATE ship_tasks SET assigned_to = ?1
                 WHERE id = ?2 AND status IN ('pending', 'ready_to_ship', 'label_printed')",
                params![assigned_to, id_str],
            )?;
            if changed == 0 {
                return Err(Self::transition_conflict(
                    tx,
                    "ship_tasks",
                    "ship task",
                    &id_str,
                    "assign",
                ));
            }
            Self::read_ship_by_id_tx(tx, &id_str)
        })
    }

    /// Record a printed shipping label.
    ///
    /// Legal from `Pending`/`ReadyToShip`/`LabelPrinted` (a re-print carries a
    /// new `label_url`); refused once the package is shipped or the task is
    /// cancelled, where a new label would contradict the carrier handoff.
    fn print_label(&self, id: Uuid, label_url: &str) -> Result<ShipTask> {
        let id_str = id.to_string();

        with_immediate_transaction(&self.pool, |tx| {
            let changed = tx.execute(
                "UPDATE ship_tasks SET status = ?1, label_url = ?2
                 WHERE id = ?3 AND status IN ('pending', 'ready_to_ship', 'label_printed')",
                params![ShipStatus::LabelPrinted.to_string(), label_url, id_str],
            )?;
            if changed == 0 {
                return Err(Self::transition_conflict(
                    tx,
                    "ship_tasks",
                    "ship task",
                    &id_str,
                    "print a label for",
                ));
            }
            Self::read_ship_by_id_tx(tx, &id_str)
        })
    }

    /// Complete a ship task (carrier handoff).
    ///
    /// Legal from `Pending`/`ReadyToShip`/`LabelPrinted`. Re-shipping an
    /// already-shipped task used to overwrite its tracking number, cost and
    /// `shipped_at`, and a cancelled task could be shipped.
    /// Complete a ship task: the package is tendered to the carrier.
    ///
    /// **Stock effect**: the units in the pack task's cartons leave the
    /// warehouse balance — `on_hand` and `allocated` both fall by the shipped
    /// quantity, releasing exactly the allocation the picks created. A pack task
    /// with no carton items has nothing to consume and moves no stock (the
    /// cartons *are* the record of what went in the box). The movements run on
    /// the same transaction as the status write, and the guarded UPDATE matches
    /// zero rows on a second attempt, so a re-ship never double-decrements.
    fn complete_ship(&self, input: CompleteShip) -> Result<ShipTask> {
        let now = Utc::now().to_rfc3339();
        let id_str = input.ship_task_id.to_string();

        with_immediate_transaction(&self.pool, |tx| {
            let changed = tx.execute(
                "UPDATE ship_tasks SET status = ?1, tracking_number = ?2, shipping_cost = ?3, shipped_at = ?4
                 WHERE id = ?5 AND status IN ('pending', 'ready_to_ship', 'label_printed')",
                params![
                    ShipStatus::Shipped.to_string(),
                    input.tracking_number,
                    input.shipping_cost.map(|d| d.to_string()),
                    now,
                    id_str,
                ],
            )?;
            if changed == 0 {
                return Err(Self::transition_conflict(
                    tx,
                    "ship_tasks",
                    "ship task",
                    &id_str,
                    "complete",
                ));
            }
            let ship = Self::read_ship_by_id_tx(tx, &id_str)?;
            apply_ship_stock_effect_tx(tx, &ship, input.shipped_by.as_deref(), &now)?;
            Ok(ship)
        })
    }

    /// Cancel a ship task.
    ///
    /// Legal from `Pending`/`ReadyToShip`/`LabelPrinted`; a package already
    /// tendered to the carrier cannot be un-shipped by a status flip.
    fn cancel_ship(&self, id: Uuid) -> Result<ShipTask> {
        let id_str = id.to_string();

        with_immediate_transaction(&self.pool, |tx| {
            let changed = tx.execute(
                "UPDATE ship_tasks SET status = ?1
                 WHERE id = ?2 AND status IN ('pending', 'ready_to_ship', 'label_printed')",
                params![ShipStatus::Cancelled.to_string(), id_str],
            )?;
            if changed == 0 {
                return Err(Self::transition_conflict(
                    tx,
                    "ship_tasks",
                    "ship task",
                    &id_str,
                    "cancel",
                ));
            }
            Self::read_ship_by_id_tx(tx, &id_str)
        })
    }

    /// Count ship tasks matching `filter` (same filters as `list_ships`).
    fn count_ships(&self, filter: ShipTaskFilter) -> Result<u64> {
        let conn = self.conn()?;
        let mut sql = "SELECT COUNT(*) FROM ship_tasks WHERE 1=1".to_string();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(order_id) = filter.order_id {
            sql.push_str(" AND order_id = ?");
            params_vec.push(Box::new(order_id.to_string()));
        }

        if let Some(status) = filter.status {
            sql.push_str(" AND status = ?");
            params_vec.push(Box::new(status.to_string()));
        }

        if let Some(carrier) = filter.carrier {
            sql.push_str(" AND carrier = ?");
            params_vec.push(Box::new(carrier));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(std::convert::AsRef::as_ref).collect();
        let count: i64 =
            conn.query_row(&sql, params_refs.as_slice(), |row| row.get(0)).map_err(map_db_error)?;
        Ok(count as u64)
    }

    // ========================================================================
    // Workflow Helpers
    // ========================================================================

    fn create_picks_for_order(
        &self,
        order_id: OrderId,
        warehouse_id: i32,
    ) -> Result<Vec<PickTask>> {
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

        // One transaction for the whole order: a failure part-way through used
        // to leave an order half-picked, with picks for some lines and none for
        // the rest — and `is_order_ready_to_pack` would then report the order
        // ready because the missing lines have no pick task to be incomplete.
        with_immediate_transaction(&self.pool, |tx| {
            let mut picks = Vec::with_capacity(inputs.len());
            for input in &inputs {
                picks.push(Self::insert_pick_tx(tx, input)?);
            }
            Ok(picks)
        })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SqliteDatabase;
    use rust_decimal_macros::dec;
    use stateset_core::{
        CompletePick, CreatePackTask, CreatePickTask, CreateWave, FulfillmentRepository, OrderId,
        OrderItemId, PickTaskFilter, WarehouseRepository, WaveFilter, WaveStatus,
    };

    /// Build an in-memory DB and bootstrap a warehouse + location.
    /// Returns (fulfillment repo, warehouse id, location id).
    /// Pick tasks FK both warehouses(id) and locations(id) per migration 018.
    fn fresh_setup() -> (SqliteFulfillmentRepository, i32, i32) {
        let db = SqliteDatabase::in_memory().expect("in-memory");
        let wh = db
            .warehouse()
            .create_warehouse(stateset_core::CreateWarehouse {
                code: "WH-FULFIL".into(),
                name: "Fulfillment Test Warehouse".into(),
                warehouse_type: stateset_core::WarehouseType::Distribution,
                address: stateset_core::WarehouseAddress {
                    street1: "1 Test St".into(),
                    street2: None,
                    city: "Test City".into(),
                    state: "TC".into(),
                    postal_code: "00000".into(),
                    country: "US".into(),
                    phone: None,
                },
                timezone: None,
            })
            .expect("create warehouse");
        let loc = db
            .warehouse()
            .create_location(stateset_core::CreateLocation {
                warehouse_id: wh.id,
                code: Some("PICK-1".into()),
                location_type: stateset_core::LocationType::Pick,
                is_pickable: Some(true),
                is_receivable: Some(true),
                ..Default::default()
            })
            .expect("create location");
        (db.fulfillment(), wh.id, loc.id)
    }

    fn fresh_repo() -> SqliteFulfillmentRepository {
        fresh_setup().0
    }

    /// Put `qty` units of `sku` on the shelf at `location_id`, at both ledger
    /// levels (bin and warehouse balance).
    ///
    /// Completing a pick now takes the units out of the bin and allocates them
    /// at warehouse level, so a pick test has to stock both first — picking
    /// stock the warehouse does not hold is exactly what the ledger refuses.
    fn seed_location_stock(
        repo: &SqliteFulfillmentRepository,
        warehouse_id: i32,
        location_id: i32,
        sku: &str,
        qty: &str,
    ) {
        let conn = repo.pool.get().expect("conn");
        conn.execute(
            "INSERT INTO location_inventory
             (location_id, sku, lot_id, quantity_on_hand, quantity_reserved, updated_at)
             VALUES (?1, ?2, '', ?3, '0', datetime('now'))
             ON CONFLICT(location_id, sku, lot_id)
             DO UPDATE SET quantity_on_hand = excluded.quantity_on_hand",
            params![location_id, sku, qty],
        )
        .expect("seed location stock");
        conn.execute(
            "INSERT OR IGNORE INTO inventory_items (sku, name) VALUES (?1, ?1)",
            params![sku],
        )
        .expect("seed item");
        conn.execute(
            "INSERT OR IGNORE INTO inventory_locations (id, name, code)
             SELECT id, name, code FROM warehouses WHERE id = ?1",
            params![warehouse_id],
        )
        .expect("seed inventory location");
        conn.execute(
            "INSERT INTO inventory_balances
             (item_id, location_id, quantity_on_hand, quantity_allocated, quantity_available,
              updated_at)
             SELECT id, ?2, ?3, '0', ?3, datetime('now') FROM inventory_items WHERE sku = ?1
             ON CONFLICT(item_id, location_id) DO UPDATE SET
                 quantity_on_hand = excluded.quantity_on_hand,
                 quantity_available = excluded.quantity_available",
            params![sku, warehouse_id, qty],
        )
        .expect("seed warehouse balance");
    }

    fn make_wave(
        repo: &SqliteFulfillmentRepository,
        warehouse_id: i32,
        orders: Vec<OrderId>,
    ) -> Wave {
        repo.create_wave(CreateWave {
            warehouse_id,
            order_ids: orders,
            priority: Some(5),
            notes: Some("test wave".into()),
            created_by: Some("alice".into()),
        })
        .expect("create wave")
    }

    fn make_pick(
        repo: &SqliteFulfillmentRepository,
        warehouse_id: i32,
        location_id: i32,
        wave: Option<FulfillmentId>,
        order: OrderId,
        sku: &str,
    ) -> PickTask {
        repo.create_pick(CreatePickTask {
            wave_id: wave,
            order_id: order,
            order_item_id: OrderItemId::new(),
            warehouse_id,
            sku: sku.into(),
            product_name: Some(format!("Product {sku}")),
            source_location_id: location_id,
            quantity_requested: dec!(5),
            lot_id: None,
            serial_number: None,
            priority: Some(1),
            notes: None,
        })
        .expect("create pick")
    }

    #[test]
    fn create_wave_starts_in_draft_with_orders() {
        let (repo, wh_id, _) = fresh_setup();
        let order_a = OrderId::new();
        let order_b = OrderId::new();
        let wave = make_wave(&repo, wh_id, vec![order_a, order_b]);
        assert_eq!(wave.warehouse_id, wh_id);
        assert_eq!(wave.status, WaveStatus::Draft);
        assert!(!wave.wave_number.is_empty());

        let orders = repo.get_wave_orders(wave.id).expect("ok");
        assert_eq!(orders.len(), 2);
        assert!(orders.contains(&order_a) && orders.contains(&order_b));
    }

    #[test]
    fn get_wave_and_get_wave_by_number_round_trip() {
        let (repo, wh_id, _) = fresh_setup();
        let wave = make_wave(&repo, wh_id, vec![OrderId::new()]);
        let by_id = repo.get_wave(wave.id).expect("ok").expect("found");
        assert_eq!(by_id.id, wave.id);
        let by_num = repo.get_wave_by_number(&wave.wave_number).expect("ok").expect("found");
        assert_eq!(by_num.id, wave.id);
        assert!(repo.get_wave_by_number("missing").expect("ok").is_none());
    }

    #[test]
    fn complete_wave_transitions_status() {
        // NOTE: this test used to complete straight from `Draft`. Completing a
        // wave that was never released is no longer a legal transition (only
        // Released/InProgress -> Completed), so the wave is released first;
        // `complete_wave_rejects_draft_wave` pins the new rule.
        let (repo, wh_id, _) = fresh_setup();
        let wave = make_wave(&repo, wh_id, vec![OrderId::new()]);
        repo.release_wave(wave.id).expect("release");
        let done = repo.complete_wave(wave.id).expect("complete");
        assert_eq!(done.status, WaveStatus::Completed);
    }

    #[test]
    fn complete_wave_rejects_draft_wave() {
        let (repo, wh_id, _) = fresh_setup();
        let wave = make_wave(&repo, wh_id, vec![OrderId::new()]);
        let err = repo.complete_wave(wave.id).expect_err("a draft wave was never on the floor");
        assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");
        let after = repo.get_wave(wave.id).expect("get").expect("exists");
        assert_eq!(after.status, WaveStatus::Draft, "status must be unchanged");
    }

    /// The status guards are expressed as SQL string literals; if an enum's
    /// `Display` ever drifts from those literals the guards would silently stop
    /// matching (allowing everything, or nothing). Pin the mapping.
    #[test]
    fn status_sql_literals_match_enum_display() {
        assert_eq!(WaveStatus::Draft.to_string(), "draft");
        assert_eq!(WaveStatus::Released.to_string(), "released");
        assert_eq!(WaveStatus::InProgress.to_string(), "in_progress");
        assert_eq!(WaveStatus::Completed.to_string(), "completed");
        assert_eq!(WaveStatus::Cancelled.to_string(), "cancelled");

        assert_eq!(PickStatus::Pending.to_string(), "pending");
        assert_eq!(PickStatus::Assigned.to_string(), "assigned");
        assert_eq!(PickStatus::InProgress.to_string(), "in_progress");
        assert_eq!(PickStatus::Completed.to_string(), "completed");
        assert_eq!(PickStatus::Short.to_string(), "short");
        assert_eq!(PickStatus::Cancelled.to_string(), "cancelled");

        assert_eq!(PackStatus::Pending.to_string(), "pending");
        assert_eq!(PackStatus::ReadyToPack.to_string(), "ready_to_pack");
        assert_eq!(PackStatus::Assigned.to_string(), "assigned");
        assert_eq!(PackStatus::InProgress.to_string(), "in_progress");
        assert_eq!(PackStatus::Completed.to_string(), "completed");
        assert_eq!(PackStatus::Cancelled.to_string(), "cancelled");

        assert_eq!(ShipStatus::Pending.to_string(), "pending");
        assert_eq!(ShipStatus::ReadyToShip.to_string(), "ready_to_ship");
        assert_eq!(ShipStatus::LabelPrinted.to_string(), "label_printed");
        assert_eq!(ShipStatus::Shipped.to_string(), "shipped");
        assert_eq!(ShipStatus::Cancelled.to_string(), "cancelled");
    }

    #[test]
    fn cancel_wave_transitions_status() {
        let (repo, wh_id, _) = fresh_setup();
        let wave = make_wave(&repo, wh_id, vec![OrderId::new()]);
        let cancelled = repo.cancel_wave(wave.id).expect("cancel");
        assert_eq!(cancelled.status, WaveStatus::Cancelled);
    }

    #[test]
    fn list_waves_filters_by_warehouse() {
        let (repo, wh_id, _) = fresh_setup();
        make_wave(&repo, wh_id, vec![OrderId::new()]);
        make_wave(&repo, wh_id, vec![OrderId::new()]);
        let waves = repo
            .list_waves(WaveFilter { warehouse_id: Some(wh_id), ..Default::default() })
            .expect("list");
        assert!(waves.len() >= 2);
        assert!(waves.iter().all(|w| w.warehouse_id == wh_id));
    }

    #[test]
    fn create_pick_round_trips_and_lists_for_order() {
        let (repo, wh_id, loc_id) = fresh_setup();
        let order = OrderId::new();
        let pick = make_pick(&repo, wh_id, loc_id, None, order, "SKU-1");
        assert_eq!(pick.order_id, order);
        assert_eq!(pick.quantity_requested, dec!(5));

        let by_id = repo.get_pick(pick.id).expect("ok").expect("found");
        assert_eq!(by_id.id, pick.id);

        let for_order = repo.get_picks_for_order(order).expect("ok");
        assert_eq!(for_order.len(), 1);
    }

    #[test]
    fn start_and_complete_pick_transitions() {
        let (repo, wh_id, loc_id) = fresh_setup();
        let order = OrderId::new();
        seed_location_stock(&repo, wh_id, loc_id, "SKU-START", "5");
        let pick = make_pick(&repo, wh_id, loc_id, None, order, "SKU-START");
        let started = repo.start_pick(pick.id).expect("start");
        assert_ne!(started.status, pick.status, "status should change after start");

        let completed = repo
            .complete_pick(CompletePick {
                pick_id: pick.id,
                quantity_picked: dec!(5),
                quantity_short: None,
                short_reason: None,
                lot_id: None,
                serial_number: None,
                completed_by: Some("alice".into()),
            })
            .expect("complete");
        assert_eq!(completed.id, pick.id);
        assert_eq!(completed.quantity_picked, dec!(5));
    }

    #[test]
    fn cancel_pick_changes_status() {
        let (repo, wh_id, loc_id) = fresh_setup();
        let pick = make_pick(&repo, wh_id, loc_id, None, OrderId::new(), "SKU-CN");
        let cancelled = repo.cancel_pick(pick.id).expect("cancel");
        assert_ne!(cancelled.status, pick.status);
    }

    #[test]
    fn list_picks_filters_by_order() {
        let (repo, wh_id, loc_id) = fresh_setup();
        let order_a = OrderId::new();
        let order_b = OrderId::new();
        make_pick(&repo, wh_id, loc_id, None, order_a, "A1");
        make_pick(&repo, wh_id, loc_id, None, order_a, "A2");
        make_pick(&repo, wh_id, loc_id, None, order_b, "B1");

        let picks_a = repo
            .list_picks(PickTaskFilter { order_id: Some(order_a), ..Default::default() })
            .expect("a");
        assert_eq!(picks_a.len(), 2);
    }

    #[test]
    fn get_picks_for_wave_returns_picks() {
        let (repo, wh_id, loc_id) = fresh_setup();
        let wave = make_wave(&repo, wh_id, vec![OrderId::new()]);
        let order = OrderId::new();
        make_pick(&repo, wh_id, loc_id, Some(wave.id), order, "WV-1");
        make_pick(&repo, wh_id, loc_id, Some(wave.id), order, "WV-2");
        let picks = repo.get_picks_for_wave(wave.id).expect("ok");
        assert_eq!(picks.len(), 2);
    }

    #[test]
    fn create_pack_task_round_trips() {
        let repo = fresh_repo();
        let order = OrderId::new();
        let pack = repo
            .create_pack(CreatePackTask { order_id: order, notes: Some("ship by tomorrow".into()) })
            .expect("create pack");
        assert_eq!(pack.order_id, order);
        let by_id = repo.get_pack(pack.id).expect("ok").expect("found");
        assert_eq!(by_id.id, pack.id);
    }

    #[test]
    fn get_unknown_wave_returns_none() {
        let repo = fresh_repo();
        assert!(repo.get_wave(stateset_core::FulfillmentId::new()).expect("ok").is_none());
    }

    #[test]
    fn get_unknown_pick_returns_none() {
        let repo = fresh_repo();
        assert!(repo.get_pick(Uuid::new_v4()).expect("ok").is_none());
    }

    // ----- W2: waves.pick_count is maintained and gates completion -----

    #[test]
    fn pick_count_tracks_inserted_and_cancelled_picks() {
        let (repo, wh_id, loc_id) = fresh_setup();
        let order = OrderId::new();
        let wave = make_wave(&repo, wh_id, vec![order]);
        assert_eq!(wave.pick_count, 0);

        let a = make_pick(&repo, wh_id, loc_id, Some(wave.id), order, "SKU-A");
        let _b = make_pick(&repo, wh_id, loc_id, Some(wave.id), order, "SKU-B");
        let w = repo.get_wave(wave.id).expect("get").expect("exists");
        assert_eq!(w.pick_count, 2);

        repo.cancel_pick(a.id).expect("cancel");
        let w = repo.get_wave(wave.id).expect("get").expect("exists");
        assert_eq!(w.pick_count, 1);

        // Picks without a wave never touch a counter.
        make_pick(&repo, wh_id, loc_id, None, order, "SKU-C");
        let w = repo.get_wave(wave.id).expect("get").expect("exists");
        assert_eq!(w.pick_count, 1);
    }

    #[test]
    fn complete_wave_refuses_while_picks_are_open() {
        let (repo, wh_id, loc_id) = fresh_setup();
        let order = OrderId::new();
        let wave = make_wave(&repo, wh_id, vec![order]);
        seed_location_stock(&repo, wh_id, loc_id, "SKU-A", "5");
        let p1 = make_pick(&repo, wh_id, loc_id, Some(wave.id), order, "SKU-A");
        let p2 = make_pick(&repo, wh_id, loc_id, Some(wave.id), order, "SKU-B");
        repo.release_wave(wave.id).expect("release");

        let err = repo.complete_wave(wave.id).expect_err("0 of 2 picks done");
        assert!(
            matches!(err, CommerceError::ValidationError(ref m) if m.contains("2 pick task(s) still open")),
            "got {err:?}"
        );
        let w = repo.get_wave(wave.id).expect("get").expect("exists");
        assert_eq!(w.status, WaveStatus::Released, "status must be unchanged");

        repo.complete_pick(CompletePick {
            pick_id: p1.id,
            quantity_picked: dec!(5),
            quantity_short: None,
            short_reason: None,
            lot_id: None,
            serial_number: None,
            completed_by: None,
        })
        .expect("complete p1");
        assert!(repo.complete_wave(wave.id).is_err(), "1 of 2 picks done");

        // A cancelled pick is no longer open, so the wave can complete.
        repo.cancel_pick(p2.id).expect("cancel p2");
        let done = repo.complete_wave(wave.id).expect("all picks finalized");
        assert_eq!(done.status, WaveStatus::Completed);
        assert_eq!(done.pick_count, 1);
        assert_eq!(done.completed_pick_count, 1);
    }

    #[test]
    fn complete_wave_treats_short_picks_as_finalized() {
        let (repo, wh_id, loc_id) = fresh_setup();
        let order = OrderId::new();
        let wave = make_wave(&repo, wh_id, vec![order]);
        let p = make_pick(&repo, wh_id, loc_id, Some(wave.id), order, "SKU-A");
        repo.release_wave(wave.id).expect("release");
        repo.report_short(p.id, dec!(5), "out of stock").expect("short");
        repo.complete_wave(wave.id).expect("short pick is finalized");
    }

    #[test]
    fn create_pick_rejects_completed_or_cancelled_wave() {
        let (repo, wh_id, loc_id) = fresh_setup();
        let order = OrderId::new();
        let wave = make_wave(&repo, wh_id, vec![order]);
        repo.release_wave(wave.id).expect("release");
        repo.complete_wave(wave.id).expect("complete empty wave");

        let err = repo
            .create_pick(CreatePickTask {
                wave_id: Some(wave.id),
                order_id: order,
                order_item_id: OrderItemId::new(),
                warehouse_id: wh_id,
                sku: "SKU-LATE".into(),
                product_name: None,
                source_location_id: loc_id,
                quantity_requested: dec!(1),
                lot_id: None,
                serial_number: None,
                priority: None,
                notes: None,
            })
            .expect_err("wave is completed");
        assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");
        assert!(repo.get_picks_for_wave(wave.id).expect("picks").is_empty(), "insert rolled back");

        let err = repo
            .create_pick(CreatePickTask {
                wave_id: Some(FulfillmentId::new()),
                order_id: order,
                order_item_id: OrderItemId::new(),
                warehouse_id: wh_id,
                sku: "SKU-NOWAVE".into(),
                product_name: None,
                source_location_id: loc_id,
                quantity_requested: dec!(1),
                lot_id: None,
                serial_number: None,
                priority: None,
                notes: None,
            })
            .expect_err("wave does not exist");
        assert!(matches!(err, CommerceError::NotFound), "got {err:?}");
    }
}
