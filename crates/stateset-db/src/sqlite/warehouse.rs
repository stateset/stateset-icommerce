//! SQLite implementation for warehouse management
//!
//! Provides full warehouse, zone, location, and inventory movement functionality.

use crate::sqlite::{
    map_db_error, parse_datetime_opt_row, parse_datetime_row, parse_decimal_opt_row,
    parse_decimal_row, parse_decimal_strict, parse_enum_row, parse_json_row, parse_uuid_opt_row,
    parse_uuid_row, with_immediate_transaction,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;
use rust_decimal::Decimal;
use uuid::Uuid;

use stateset_core::{
    AdjustLocationInventory, BatchResult, CommerceError, CreateCycleCount, CreateLocation,
    CreateWarehouse, CreateZone, CycleCount, CycleCountFilter, CycleCountLine, CycleCountStatus,
    Location, LocationFilter, LocationInventory, LocationInventoryFilter, LocationMovement,
    MoveInventory, MovementFilter, MovementType, RecordCycleCountLine, Result, UpdateLocation,
    UpdateWarehouse, UpdateZone, Warehouse, WarehouseFilter, WarehouseRepository, Zone,
};

/// SQLite warehouse repository
#[derive(Debug)]
pub struct SqliteWarehouseRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteWarehouseRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    // Helper to parse warehouse from row
    fn row_to_warehouse(row: &rusqlite::Row<'_>) -> rusqlite::Result<Warehouse> {
        Ok(Warehouse {
            id: row.get("id")?,
            code: row.get("code")?,
            name: row.get("name")?,
            warehouse_type: parse_enum_row(
                &row.get::<_, String>("warehouse_type")?,
                "warehouse",
                "warehouse_type",
            )?,
            address: parse_json_row(
                &row.get::<_, String>("address_json")?,
                "warehouse",
                "address_json",
            )?,
            timezone: row.get("timezone")?,
            is_active: row.get::<_, i32>("is_active")? == 1,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "warehouse",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "warehouse",
                "updated_at",
            )?,
        })
    }

    // Helper to parse location from row
    fn row_to_location(row: &rusqlite::Row<'_>) -> rusqlite::Result<Location> {
        Ok(Location {
            id: row.get("id")?,
            warehouse_id: row.get("warehouse_id")?,
            code: row.get("code")?,
            location_type: parse_enum_row(
                &row.get::<_, String>("location_type")?,
                "location",
                "location_type",
            )?,
            zone: row.get("zone")?,
            aisle: row.get("aisle")?,
            rack: row.get("rack")?,
            level: row.get("level")?,
            bin: row.get("bin")?,
            max_weight_kg: parse_decimal_opt_row(
                row.get::<_, Option<String>>("max_weight_kg")?,
                "location",
                "max_weight_kg",
            )?,
            max_volume_m3: parse_decimal_opt_row(
                row.get::<_, Option<String>>("max_volume_m3")?,
                "location",
                "max_volume_m3",
            )?,
            current_weight_kg: parse_decimal_opt_row(
                row.get::<_, Option<String>>("current_weight_kg")?,
                "location",
                "current_weight_kg",
            )?,
            current_volume_m3: parse_decimal_opt_row(
                row.get::<_, Option<String>>("current_volume_m3")?,
                "location",
                "current_volume_m3",
            )?,
            is_pickable: row.get::<_, i32>("is_pickable")? == 1,
            is_receivable: row.get::<_, i32>("is_receivable")? == 1,
            is_active: row.get::<_, i32>("is_active")? == 1,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "location",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "location",
                "updated_at",
            )?,
        })
    }

    // Helper to parse location inventory from row
    fn row_to_location_inventory(row: &rusqlite::Row<'_>) -> rusqlite::Result<LocationInventory> {
        let on_hand = parse_decimal_row(
            &row.get::<_, String>("quantity_on_hand")?,
            "location_inventory",
            "quantity_on_hand",
        )?;
        let reserved = parse_decimal_row(
            &row.get::<_, String>("quantity_reserved")?,
            "location_inventory",
            "quantity_reserved",
        )?;

        Ok(LocationInventory {
            location_id: row.get("location_id")?,
            sku: row.get("sku")?,
            lot_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("lot_id")?,
                "location_inventory",
                "lot_id",
            )?,
            quantity_on_hand: on_hand,
            quantity_reserved: reserved,
            quantity_available: on_hand - reserved,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "location_inventory",
                "updated_at",
            )?,
        })
    }

    // Helper to parse inventory movement from row
    fn row_to_movement(row: &rusqlite::Row<'_>) -> rusqlite::Result<LocationMovement> {
        Ok(LocationMovement {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "inventory_movement", "id")?,
            movement_type: parse_enum_row(
                &row.get::<_, String>("movement_type")?,
                "inventory_movement",
                "movement_type",
            )?,
            from_location_id: row.get("from_location_id")?,
            to_location_id: row.get("to_location_id")?,
            sku: row.get("sku")?,
            lot_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("lot_id")?,
                "inventory_movement",
                "lot_id",
            )?,
            quantity: parse_decimal_row(
                &row.get::<_, String>("quantity")?,
                "inventory_movement",
                "quantity",
            )?,
            reference_type: row.get("reference_type")?,
            reference_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("reference_id")?,
                "inventory_movement",
                "reference_id",
            )?,
            reason: row.get("reason")?,
            performed_by: row.get("performed_by")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "inventory_movement",
                "created_at",
            )?,
        })
    }

    // Helper to parse zone from row
    fn row_to_zone(row: &rusqlite::Row<'_>) -> rusqlite::Result<Zone> {
        Ok(Zone {
            id: row.get("id")?,
            warehouse_id: row.get("warehouse_id")?,
            code: row.get("code")?,
            name: row.get("name")?,
            description: row.get("description")?,
            is_active: row.get::<_, i32>("is_active")? == 1,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "warehouse_zone",
                "created_at",
            )?,
        })
    }

    // Helper to parse a cycle count header from a row (lines loaded separately)
    fn row_to_cycle_count_header(row: &rusqlite::Row<'_>) -> rusqlite::Result<CycleCount> {
        Ok(CycleCount {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "cycle_count", "id")?,
            warehouse_id: row.get("warehouse_id")?,
            location_id: row.get("location_id")?,
            status: parse_enum_row(&row.get::<_, String>("status")?, "cycle_count", "status")?,
            scheduled_date: parse_datetime_opt_row(
                row.get::<_, Option<String>>("scheduled_date")?,
                "cycle_count",
                "scheduled_date",
            )?,
            counted_by: row.get("counted_by")?,
            lines: Vec::new(),
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "cycle_count",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "cycle_count",
                "updated_at",
            )?,
            completed_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("completed_at")?,
                "cycle_count",
                "completed_at",
            )?,
        })
    }

    // Helper to parse a cycle count line from a row. Lot-less lines store
    // lot_id = '' (NOT NULL), matching the location_inventory convention.
    fn row_to_cycle_count_line(row: &rusqlite::Row<'_>) -> rusqlite::Result<CycleCountLine> {
        let lot_raw: String = row.get("lot_id")?;
        let lot_id = if lot_raw.is_empty() {
            None
        } else {
            Some(parse_uuid_row(&lot_raw, "cycle_count_line", "lot_id")?)
        };
        Ok(CycleCountLine {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "cycle_count_line", "id")?,
            cycle_count_id: parse_uuid_row(
                &row.get::<_, String>("cycle_count_id")?,
                "cycle_count_line",
                "cycle_count_id",
            )?,
            sku: row.get("sku")?,
            lot_id,
            expected_quantity: parse_decimal_row(
                &row.get::<_, String>("expected_quantity")?,
                "cycle_count_line",
                "expected_quantity",
            )?,
            counted_quantity: parse_decimal_opt_row(
                row.get::<_, Option<String>>("counted_quantity")?,
                "cycle_count_line",
                "counted_quantity",
            )?,
            variance: parse_decimal_opt_row(
                row.get::<_, Option<String>>("variance")?,
                "cycle_count_line",
                "variance",
            )?,
        })
    }

    fn load_cycle_count_lines(
        conn: &rusqlite::Connection,
        id: Uuid,
    ) -> Result<Vec<CycleCountLine>> {
        let mut stmt = conn
            .prepare(
                "SELECT * FROM cycle_count_lines WHERE cycle_count_id = ?1 ORDER BY sku, lot_id",
            )
            .map_err(map_db_error)?;
        let mut rows = stmt.query(params![id.to_string()]).map_err(map_db_error)?;
        let mut lines = Vec::new();
        while let Some(row) = rows.next().map_err(map_db_error)? {
            lines.push(Self::row_to_cycle_count_line(row).map_err(map_db_error)?);
        }
        Ok(lines)
    }

    /// Batch-load cycle count lines for many cycle counts in chunked
    /// `IN`-clause queries, grouped by parent cycle count id. Uses the caller's connection.
    fn load_cycle_count_lines_batch(
        conn: &rusqlite::Connection,
        ids: &[Uuid],
    ) -> Result<std::collections::HashMap<Uuid, Vec<CycleCountLine>>> {
        let mut map: std::collections::HashMap<Uuid, Vec<CycleCountLine>> =
            std::collections::HashMap::with_capacity(ids.len());
        let id_strings: Vec<String> = ids.iter().map(Uuid::to_string).collect();
        for chunk in id_strings.chunks(500) {
            let placeholders = crate::sqlite::build_in_clause(chunk.len());
            let sql = format!(
                "SELECT * FROM cycle_count_lines WHERE cycle_count_id IN ({placeholders})
                 ORDER BY sku, lot_id"
            );
            let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
            let param_refs: Vec<&dyn rusqlite::ToSql> =
                chunk.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
            let rows = stmt
                .query_map(param_refs.as_slice(), |row| {
                    let line = Self::row_to_cycle_count_line(row)?;
                    Ok((line.cycle_count_id, line))
                })
                .map_err(map_db_error)?;
            for row in rows {
                let (parent, line) = row.map_err(map_db_error)?;
                map.entry(parent).or_default().push(line);
            }
        }
        Ok(map)
    }

    fn get_cycle_count_on(conn: &rusqlite::Connection, id: Uuid) -> Result<Option<CycleCount>> {
        let mut stmt =
            conn.prepare("SELECT * FROM cycle_counts WHERE id = ?1").map_err(map_db_error)?;
        let mut rows = stmt.query(params![id.to_string()]).map_err(map_db_error)?;
        let Some(row) = rows.next().map_err(map_db_error)? else {
            return Ok(None);
        };
        let mut cc = Self::row_to_cycle_count_header(row).map_err(map_db_error)?;
        drop(rows);
        drop(stmt);
        cc.lines = Self::load_cycle_count_lines(conn, id)?;
        Ok(Some(cc))
    }

    /// Update a cycle count's status after verifying the transition is legal.
    fn transition_cycle_count(
        &self,
        id: Uuid,
        next: CycleCountStatus,
        set_completed_at: bool,
    ) -> Result<CycleCount> {
        let conn = self.conn()?;
        let current = Self::get_cycle_count_on(&conn, id)?.ok_or(CommerceError::NotFound)?;
        if !current.status.can_transition_to(next) {
            return Err(CommerceError::ValidationError(format!(
                "cannot transition cycle count from {} to {next}",
                current.status
            )));
        }
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE cycle_counts SET status = ?1, updated_at = ?2,
                 completed_at = CASE WHEN ?3 THEN ?2 ELSE completed_at END
             WHERE id = ?4",
            params![next.to_string(), now, set_completed_at, id.to_string()],
        )
        .map_err(map_db_error)?;
        Self::get_cycle_count_on(&conn, id)?.ok_or_else(|| {
            CommerceError::DatabaseError("Failed to retrieve updated cycle count".into())
        })
    }

    // Generate location code from parts
    fn generate_location_code(input: &CreateLocation) -> String {
        let parts: Vec<&str> = [
            input.zone.as_deref(),
            input.aisle.as_deref(),
            input.rack.as_deref(),
            input.level.as_deref(),
            input.bin.as_deref(),
        ]
        .iter()
        .filter_map(|p| *p)
        .collect();

        if parts.is_empty() {
            format!("LOC-{}", &Uuid::new_v4().to_string()[..8].to_uppercase())
        } else {
            parts.join("-")
        }
    }
}

impl WarehouseRepository for SqliteWarehouseRepository {
    // ========================================================================
    // Warehouse Operations
    // ========================================================================

    fn create_warehouse(&self, input: CreateWarehouse) -> Result<Warehouse> {
        let conn = self.conn()?;
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let address = input.address.clone();
        let address_json = serde_json::to_string(&address).unwrap_or_else(|_| "{}".to_string());

        conn.execute(
            "INSERT INTO warehouses (code, name, warehouse_type, address_json, timezone, is_active, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?6)",
            params![
                input.code,
                input.name,
                input.warehouse_type.to_string(),
                address_json,
                input.timezone,
                now_str,
            ],
        )
        .map_err(map_db_error)?;

        let id = conn.last_insert_rowid() as i32;

        // Construct warehouse directly to avoid nested conn() call
        Ok(Warehouse {
            id,
            code: input.code,
            name: input.name,
            warehouse_type: input.warehouse_type,
            address,
            timezone: input.timezone,
            is_active: true,
            created_at: now,
            updated_at: now,
        })
    }

    fn get_warehouse(&self, id: i32) -> Result<Option<Warehouse>> {
        let conn = self.conn()?;
        let mut stmt =
            conn.prepare("SELECT * FROM warehouses WHERE id = ?1").map_err(map_db_error)?;

        let mut rows = stmt.query(params![id]).map_err(map_db_error)?;

        if let Some(row) = rows.next().map_err(map_db_error)? {
            Ok(Some(Self::row_to_warehouse(row).map_err(map_db_error)?))
        } else {
            Ok(None)
        }
    }

    fn get_warehouse_by_code(&self, code: &str) -> Result<Option<Warehouse>> {
        let conn = self.conn()?;
        let mut stmt =
            conn.prepare("SELECT * FROM warehouses WHERE code = ?1").map_err(map_db_error)?;

        let mut rows = stmt.query(params![code]).map_err(map_db_error)?;

        if let Some(row) = rows.next().map_err(map_db_error)? {
            Ok(Some(Self::row_to_warehouse(row).map_err(map_db_error)?))
        } else {
            Ok(None)
        }
    }

    fn update_warehouse(&self, id: i32, input: UpdateWarehouse) -> Result<Warehouse> {
        let conn = self.conn()?;

        // Get existing warehouse using the same connection
        let mut stmt =
            conn.prepare("SELECT * FROM warehouses WHERE id = ?1").map_err(map_db_error)?;
        let mut rows = stmt.query(params![id]).map_err(map_db_error)?;
        let existing = if let Some(row) = rows.next().map_err(map_db_error)? {
            Self::row_to_warehouse(row).map_err(map_db_error)?
        } else {
            return Err(CommerceError::NotFound);
        };
        drop(rows);
        drop(stmt);

        let code = existing.code;
        let name = input.name.unwrap_or(existing.name);
        let warehouse_type = input.warehouse_type.unwrap_or(existing.warehouse_type);
        let address = input.address.unwrap_or(existing.address);
        let timezone = input.timezone.or(existing.timezone);
        let is_active = input.is_active.unwrap_or(existing.is_active);
        let address_json = serde_json::to_string(&address).unwrap_or_else(|_| "{}".to_string());
        let now = Utc::now();

        conn.execute(
            "UPDATE warehouses SET name = ?1, warehouse_type = ?2, address_json = ?3, timezone = ?4, is_active = ?5, updated_at = ?6 WHERE id = ?7",
            params![
                name,
                warehouse_type.to_string(),
                address_json,
                timezone,
                i32::from(is_active),
                now.to_rfc3339(),
                id,
            ],
        )
        .map_err(map_db_error)?;

        // Construct result directly to avoid nested conn() call
        Ok(Warehouse {
            id,
            code,
            name,
            warehouse_type,
            address,
            timezone,
            is_active,
            created_at: existing.created_at,
            updated_at: now,
        })
    }

    fn list_warehouses(&self, filter: WarehouseFilter) -> Result<Vec<Warehouse>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM warehouses WHERE 1=1".to_string();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(wh_type) = filter.warehouse_type {
            sql.push_str(" AND warehouse_type = ?");
            params_vec.push(Box::new(wh_type.to_string()));
        }

        if let Some(active) = filter.is_active {
            sql.push_str(" AND is_active = ?");
            params_vec.push(Box::new(i32::from(active)));
        }

        sql.push_str(" ORDER BY name");

        crate::sqlite::append_limit_offset(&mut sql, filter.limit, filter.offset);

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(std::convert::AsRef::as_ref).collect();

        let mut rows = stmt.query(params_refs.as_slice()).map_err(map_db_error)?;

        let mut warehouses = Vec::new();
        while let Some(row) = rows.next().map_err(map_db_error)? {
            warehouses.push(Self::row_to_warehouse(row).map_err(map_db_error)?);
        }

        Ok(warehouses)
    }

    fn delete_warehouse(&self, id: i32) -> Result<()> {
        let conn = self.conn()?;

        // Check if warehouse has locations
        let loc_count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM locations WHERE warehouse_id = ?1",
                params![id],
                |row| row.get(0),
            )
            .map_err(map_db_error)?;

        if loc_count > 0 {
            return Err(CommerceError::ValidationError(
                "Cannot delete warehouse with existing locations".into(),
            ));
        }

        conn.execute("DELETE FROM warehouses WHERE id = ?1", params![id]).map_err(map_db_error)?;

        Ok(())
    }

    fn count_warehouses(&self, filter: WarehouseFilter) -> Result<u64> {
        let conn = self.conn()?;
        let mut sql = "SELECT COUNT(*) FROM warehouses WHERE 1=1".to_string();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(wh_type) = filter.warehouse_type {
            sql.push_str(" AND warehouse_type = ?");
            params_vec.push(Box::new(wh_type.to_string()));
        }

        if let Some(active) = filter.is_active {
            sql.push_str(" AND is_active = ?");
            params_vec.push(Box::new(i32::from(active)));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(std::convert::AsRef::as_ref).collect();

        let count: i64 =
            conn.query_row(&sql, params_refs.as_slice(), |row| row.get(0)).map_err(map_db_error)?;

        Ok(count as u64)
    }

    // ========================================================================
    // Zone Operations
    // ========================================================================

    fn create_zone(&self, input: CreateZone) -> Result<Zone> {
        let now = Utc::now().to_rfc3339();
        let id = {
            let conn = self.conn()?;
            conn.execute(
                "INSERT INTO warehouse_zones (warehouse_id, code, name, description, is_active, created_at)
                 VALUES (?1, ?2, ?3, ?4, 1, ?5)",
                params![
                    input.warehouse_id,
                    input.code,
                    input.name,
                    input.description,
                    now,
                ],
            )
            .map_err(map_db_error)?;

            conn.last_insert_rowid() as i32
        };
        self.get_zone(id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to retrieve created zone".into()))
    }

    fn get_zone(&self, id: i32) -> Result<Option<Zone>> {
        let conn = self.conn()?;
        let mut stmt =
            conn.prepare("SELECT * FROM warehouse_zones WHERE id = ?1").map_err(map_db_error)?;

        let mut rows = stmt.query(params![id]).map_err(map_db_error)?;

        if let Some(row) = rows.next().map_err(map_db_error)? {
            Ok(Some(Self::row_to_zone(row).map_err(map_db_error)?))
        } else {
            Ok(None)
        }
    }

    fn get_zones(&self, warehouse_id: i32) -> Result<Vec<Zone>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT * FROM warehouse_zones WHERE warehouse_id = ?1 ORDER BY code")
            .map_err(map_db_error)?;

        let mut rows = stmt.query(params![warehouse_id]).map_err(map_db_error)?;

        let mut zones = Vec::new();
        while let Some(row) = rows.next().map_err(map_db_error)? {
            zones.push(Self::row_to_zone(row).map_err(map_db_error)?);
        }

        Ok(zones)
    }

    fn update_zone(&self, id: i32, input: UpdateZone) -> Result<Zone> {
        let existing = self.get_zone(id)?.ok_or(CommerceError::NotFound)?;

        let name = input.name.unwrap_or(existing.name);
        let description = input.description.or(existing.description);
        let is_active = input.is_active.unwrap_or(existing.is_active);

        {
            let conn = self.conn()?;
            conn.execute(
                "UPDATE warehouse_zones SET name = ?1, description = ?2, is_active = ?3 WHERE id = ?4",
                params![name, description, i32::from(is_active), id],
            )
            .map_err(map_db_error)?;
        }

        self.get_zone(id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to retrieve updated zone".into()))
    }

    fn delete_zone(&self, id: i32) -> Result<()> {
        let conn = self.conn()?;
        conn.execute("DELETE FROM warehouse_zones WHERE id = ?1", params![id])
            .map_err(map_db_error)?;
        Ok(())
    }

    // ========================================================================
    // Location Operations
    // ========================================================================

    fn create_location(&self, input: CreateLocation) -> Result<Location> {
        let conn = self.conn()?;
        let now = Utc::now().to_rfc3339();
        let code = input.code.clone().unwrap_or_else(|| Self::generate_location_code(&input));

        conn.execute(
            "INSERT INTO locations (warehouse_id, code, location_type, zone, aisle, rack, level, bin,
                max_weight_kg, max_volume_m3, is_pickable, is_receivable, is_active, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 1, ?13, ?13)",
            params![
                input.warehouse_id,
                code,
                input.location_type.to_string(),
                input.zone,
                input.aisle,
                input.rack,
                input.level,
                input.bin,
                input.max_weight_kg.map(|d| d.to_string()),
                input.max_volume_m3.map(|d| d.to_string()),
                i32::from(input.is_pickable.unwrap_or(true)),
                i32::from(input.is_receivable.unwrap_or(true)),
                now,
            ],
        )
        .map_err(map_db_error)?;

        let id = conn.last_insert_rowid() as i32;
        drop(conn);
        self.get_location(id)?.ok_or_else(|| {
            CommerceError::DatabaseError("Failed to retrieve created location".into())
        })
    }

    fn get_location(&self, id: i32) -> Result<Option<Location>> {
        let conn = self.conn()?;
        let mut stmt =
            conn.prepare("SELECT * FROM locations WHERE id = ?1").map_err(map_db_error)?;

        let mut rows = stmt.query(params![id]).map_err(map_db_error)?;

        if let Some(row) = rows.next().map_err(map_db_error)? {
            Ok(Some(Self::row_to_location(row).map_err(map_db_error)?))
        } else {
            Ok(None)
        }
    }

    fn get_location_by_code(&self, warehouse_id: i32, code: &str) -> Result<Option<Location>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT * FROM locations WHERE warehouse_id = ?1 AND code = ?2")
            .map_err(map_db_error)?;

        let mut rows = stmt.query(params![warehouse_id, code]).map_err(map_db_error)?;

        if let Some(row) = rows.next().map_err(map_db_error)? {
            Ok(Some(Self::row_to_location(row).map_err(map_db_error)?))
        } else {
            Ok(None)
        }
    }

    fn update_location(&self, id: i32, input: UpdateLocation) -> Result<Location> {
        let conn = self.conn()?;
        let existing = self.get_location(id)?.ok_or(CommerceError::NotFound)?;

        let location_type = input.location_type.unwrap_or(existing.location_type);
        let zone = input.zone.or(existing.zone);
        let aisle = input.aisle.or(existing.aisle);
        let rack = input.rack.or(existing.rack);
        let level = input.level.or(existing.level);
        let bin = input.bin.or(existing.bin);
        let max_weight = input.max_weight_kg.or(existing.max_weight_kg);
        let max_volume = input.max_volume_m3.or(existing.max_volume_m3);
        let is_pickable = input.is_pickable.unwrap_or(existing.is_pickable);
        let is_receivable = input.is_receivable.unwrap_or(existing.is_receivable);
        let is_active = input.is_active.unwrap_or(existing.is_active);

        conn.execute(
            "UPDATE locations SET location_type = ?1, zone = ?2, aisle = ?3, rack = ?4, level = ?5,
             bin = ?6, max_weight_kg = ?7, max_volume_m3 = ?8, is_pickable = ?9, is_receivable = ?10,
             is_active = ?11 WHERE id = ?12",
            params![
                location_type.to_string(),
                zone,
                aisle,
                rack,
                level,
                bin,
                max_weight.map(|d| d.to_string()),
                max_volume.map(|d| d.to_string()),
                i32::from(is_pickable),
                i32::from(is_receivable),
                i32::from(is_active),
                id,
            ],
        )
        .map_err(map_db_error)?;

        self.get_location(id)?.ok_or_else(|| {
            CommerceError::DatabaseError("Failed to retrieve updated location".into())
        })
    }

    fn list_locations(&self, filter: LocationFilter) -> Result<Vec<Location>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM locations WHERE 1=1".to_string();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(warehouse_id) = filter.warehouse_id {
            sql.push_str(" AND warehouse_id = ?");
            params_vec.push(Box::new(warehouse_id));
        }

        if let Some(loc_type) = filter.location_type {
            sql.push_str(" AND location_type = ?");
            params_vec.push(Box::new(loc_type.to_string()));
        }

        if let Some(zone) = filter.zone {
            sql.push_str(" AND zone = ?");
            params_vec.push(Box::new(zone));
        }

        if let Some(aisle) = filter.aisle {
            sql.push_str(" AND aisle = ?");
            params_vec.push(Box::new(aisle));
        }

        if let Some(pickable) = filter.is_pickable {
            sql.push_str(" AND is_pickable = ?");
            params_vec.push(Box::new(i32::from(pickable)));
        }

        if let Some(receivable) = filter.is_receivable {
            sql.push_str(" AND is_receivable = ?");
            params_vec.push(Box::new(i32::from(receivable)));
        }

        if let Some(active) = filter.is_active {
            sql.push_str(" AND is_active = ?");
            params_vec.push(Box::new(i32::from(active)));
        }

        sql.push_str(" ORDER BY code");

        crate::sqlite::append_limit_offset(&mut sql, filter.limit, filter.offset);

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(std::convert::AsRef::as_ref).collect();

        let mut rows = stmt.query(params_refs.as_slice()).map_err(map_db_error)?;

        let mut locations = Vec::new();
        while let Some(row) = rows.next().map_err(map_db_error)? {
            locations.push(Self::row_to_location(row).map_err(map_db_error)?);
        }

        Ok(locations)
    }

    fn delete_location(&self, id: i32) -> Result<()> {
        let conn = self.conn()?;

        // Check if location has inventory. `quantity_on_hand` is a TEXT
        // decimal, so compare the exact parsed `Decimal`s in Rust instead of a
        // float-coercing CAST(... AS REAL) in SQL.
        let quantities: Vec<String> = {
            let mut stmt = conn
                .prepare("SELECT quantity_on_hand FROM location_inventory WHERE location_id = ?1")
                .map_err(map_db_error)?;
            let rows = stmt.query_map(params![id], |row| row.get(0)).map_err(map_db_error)?;
            rows.collect::<rusqlite::Result<Vec<_>>>().map_err(map_db_error)?
        };
        for qty in &quantities {
            if parse_decimal_strict(qty, "location_inventory", "quantity_on_hand")? > Decimal::ZERO
            {
                return Err(CommerceError::ValidationError(
                    "Cannot delete location with inventory".into(),
                ));
            }
        }

        conn.execute("DELETE FROM locations WHERE id = ?1", params![id]).map_err(map_db_error)?;

        Ok(())
    }

    fn count_locations(&self, filter: LocationFilter) -> Result<u64> {
        let conn = self.conn()?;
        let mut sql = "SELECT COUNT(*) FROM locations WHERE 1=1".to_string();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(warehouse_id) = filter.warehouse_id {
            sql.push_str(" AND warehouse_id = ?");
            params_vec.push(Box::new(warehouse_id));
        }

        if let Some(loc_type) = filter.location_type {
            sql.push_str(" AND location_type = ?");
            params_vec.push(Box::new(loc_type.to_string()));
        }

        // Honor the same predicates as `list_locations`, so `count_locations(f)`
        // always equals `list_locations(f).len()` (and matches Postgres).
        if let Some(zone) = filter.zone {
            sql.push_str(" AND zone = ?");
            params_vec.push(Box::new(zone));
        }

        if let Some(aisle) = filter.aisle {
            sql.push_str(" AND aisle = ?");
            params_vec.push(Box::new(aisle));
        }

        if let Some(pickable) = filter.is_pickable {
            sql.push_str(" AND is_pickable = ?");
            params_vec.push(Box::new(i32::from(pickable)));
        }

        if let Some(receivable) = filter.is_receivable {
            sql.push_str(" AND is_receivable = ?");
            params_vec.push(Box::new(i32::from(receivable)));
        }

        if let Some(active) = filter.is_active {
            sql.push_str(" AND is_active = ?");
            params_vec.push(Box::new(i32::from(active)));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(std::convert::AsRef::as_ref).collect();

        let count: i64 =
            conn.query_row(&sql, params_refs.as_slice(), |row| row.get(0)).map_err(map_db_error)?;

        Ok(count as u64)
    }

    fn get_locations_for_warehouse(&self, warehouse_id: i32) -> Result<Vec<Location>> {
        // Return ALL locations for the warehouse (active and inactive), matching
        // the Postgres backend and the trait contract; callers that want only
        // active/pickable/receivable locations use the dedicated accessors.
        self.list_locations(LocationFilter {
            warehouse_id: Some(warehouse_id),
            ..Default::default()
        })
    }

    fn get_pickable_locations(&self, warehouse_id: i32, sku: &str) -> Result<Vec<Location>> {
        let conn = self.conn()?;
        // `quantity_on_hand` and `quantity_reserved` are TEXT decimals;
        // comparing them in SQL with CAST(... AS REAL) coerces both operands to
        // IEEE-754 floats and can misclassify locations right at the boundary,
        // so the availability check happens on exact `Decimal`s in Rust.
        let mut stmt = conn
            .prepare(
                "SELECT l.*, li.quantity_on_hand AS li_on_hand,
                        li.quantity_reserved AS li_reserved
                 FROM locations l
                 JOIN location_inventory li ON l.id = li.location_id
                 WHERE l.warehouse_id = ?1 AND l.is_pickable = 1 AND l.is_active = 1
                   AND li.sku = ?2
                 ORDER BY l.code",
            )
            .map_err(map_db_error)?;

        let mut rows = stmt.query(params![warehouse_id, sku]).map_err(map_db_error)?;

        let mut locations = Vec::new();
        while let Some(row) = rows.next().map_err(map_db_error)? {
            let on_hand: String = row.get("li_on_hand").map_err(map_db_error)?;
            let reserved: String = row.get("li_reserved").map_err(map_db_error)?;
            let on_hand = parse_decimal_strict(&on_hand, "location_inventory", "quantity_on_hand")?;
            let reserved =
                parse_decimal_strict(&reserved, "location_inventory", "quantity_reserved")?;
            if on_hand > reserved {
                locations.push(Self::row_to_location(row).map_err(map_db_error)?);
            }
        }

        Ok(locations)
    }

    fn get_receivable_locations(&self, warehouse_id: i32) -> Result<Vec<Location>> {
        self.list_locations(LocationFilter {
            warehouse_id: Some(warehouse_id),
            is_receivable: Some(true),
            is_active: Some(true),
            ..Default::default()
        })
    }

    // ========================================================================
    // Location Inventory Operations
    // ========================================================================

    fn get_location_inventory(&self, location_id: i32) -> Result<Vec<LocationInventory>> {
        let conn = self.conn()?;
        // `quantity_on_hand` is a TEXT decimal; the positive-quantity filter
        // happens on the exact parsed `Decimal` rather than a float-coercing
        // CAST(... AS REAL) in SQL.
        let mut stmt = conn
            .prepare("SELECT * FROM location_inventory WHERE location_id = ?1")
            .map_err(map_db_error)?;

        let mut rows = stmt.query(params![location_id]).map_err(map_db_error)?;

        let mut inventory = Vec::new();
        while let Some(row) = rows.next().map_err(map_db_error)? {
            let entry = Self::row_to_location_inventory(row).map_err(map_db_error)?;
            if entry.quantity_on_hand > Decimal::ZERO {
                inventory.push(entry);
            }
        }

        Ok(inventory)
    }

    fn get_inventory_for_sku(
        &self,
        warehouse_id: i32,
        sku: &str,
    ) -> Result<Vec<LocationInventory>> {
        let conn = self.conn()?;
        // `quantity_on_hand` is a TEXT decimal; the positive-quantity filter
        // happens on the exact parsed `Decimal` rather than a float-coercing
        // CAST(... AS REAL) in SQL.
        let mut stmt = conn
            .prepare(
                "SELECT li.* FROM location_inventory li
                 JOIN locations l ON li.location_id = l.id
                 WHERE l.warehouse_id = ?1 AND li.sku = ?2
                 ORDER BY l.code",
            )
            .map_err(map_db_error)?;

        let mut rows = stmt.query(params![warehouse_id, sku]).map_err(map_db_error)?;

        let mut inventory = Vec::new();
        while let Some(row) = rows.next().map_err(map_db_error)? {
            let entry = Self::row_to_location_inventory(row).map_err(map_db_error)?;
            if entry.quantity_on_hand > Decimal::ZERO {
                inventory.push(entry);
            }
        }

        Ok(inventory)
    }

    fn adjust_inventory(&self, input: AdjustLocationInventory) -> Result<LocationInventory> {
        let conn = self.conn()?;
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let lot_id_str = input.lot_id.map(|id| id.to_string());
        let lot_key = lot_id_str.unwrap_or_default();

        // Try to get existing inventory
        let existing: Option<(String, String)> = conn
            .query_row(
                "SELECT quantity_on_hand, quantity_reserved FROM location_inventory
                 WHERE location_id = ?1 AND sku = ?2 AND COALESCE(lot_id, '') = ?3",
                params![input.location_id, input.sku, lot_key],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .ok();

        let (new_on_hand, reserved) = if let Some((oh_str, res_str)) = existing {
            let on_hand = parse_decimal_strict(&oh_str, "location_inventory", "quantity_on_hand")?;
            let reserved =
                parse_decimal_strict(&res_str, "location_inventory", "quantity_reserved")?;
            let new_qty = on_hand + input.quantity;

            if new_qty < Decimal::ZERO {
                return Err(CommerceError::ValidationError(
                    "Adjustment would result in negative inventory".into(),
                ));
            }

            conn.execute(
                "UPDATE location_inventory SET quantity_on_hand = ?1, updated_at = ?2
                 WHERE location_id = ?3 AND sku = ?4 AND COALESCE(lot_id, '') = ?5",
                params![new_qty.to_string(), now_str, input.location_id, input.sku, lot_key],
            )
            .map_err(map_db_error)?;

            (new_qty, reserved)
        } else {
            if input.quantity < Decimal::ZERO {
                return Err(CommerceError::ValidationError(
                    "Cannot create negative inventory".into(),
                ));
            }

            conn.execute(
                "INSERT INTO location_inventory (location_id, sku, lot_id, quantity_on_hand, quantity_reserved, updated_at)
                 VALUES (?1, ?2, ?3, ?4, '0', ?5)",
                params![
                    input.location_id,
                    input.sku,
                    lot_key,
                    input.quantity.to_string(),
                    now_str,
                ],
            )
            .map_err(map_db_error)?;

            (input.quantity, Decimal::ZERO)
        };

        // Record the movement
        let movement_id = Uuid::new_v4();

        conn.execute(
            "INSERT INTO inventory_movements (id, movement_type, to_location_id, sku, lot_id, quantity, reference_type, reference_id, reason, performed_by, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                movement_id.to_string(),
                MovementType::Adjustment.to_string(),
                input.location_id,
                input.sku,
                input.lot_id.map(|id| id.to_string()),
                input.quantity.to_string(),
                input.reference_type,
                input.reference_id.map(|id| id.to_string()),
                input.reason,
                input.performed_by,
                now_str,
            ],
        )
        .map_err(map_db_error)?;

        Ok(LocationInventory {
            location_id: input.location_id,
            sku: input.sku,
            lot_id: input.lot_id,
            quantity_on_hand: new_on_hand,
            quantity_reserved: reserved,
            quantity_available: new_on_hand - reserved,
            updated_at: now,
        })
    }

    fn move_inventory(&self, input: MoveInventory) -> Result<LocationMovement> {
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let lot_id_str = input.lot_id.map(|id| id.to_string());
        let lot_key = lot_id_str.clone().unwrap_or_default();
        let movement_id = Uuid::new_v4();

        // A move is a source-decrement + destination-increment + movement-insert.
        // Run all three atomically inside one IMMEDIATE (retrying) transaction so a
        // failure partway (e.g. a bad destination) can never leave stock destroyed
        // at the source with nothing credited to the destination — the Postgres
        // backend already wraps the move in a transaction. Domain errors are
        // carried out via `ToSqlConversionFailure` so they are not retried.
        let smuggle = |e: CommerceError| rusqlite::Error::ToSqlConversionFailure(Box::new(e));

        with_immediate_transaction(&self.pool, |tx| {
            // Get source inventory
            let (source_on_hand, source_reserved): (String, String) = match tx.query_row(
                "SELECT quantity_on_hand, quantity_reserved FROM location_inventory
                 WHERE location_id = ?1 AND sku = ?2 AND COALESCE(lot_id, '') = ?3",
                params![input.from_location_id, input.sku, lot_key],
                |row| Ok((row.get(0)?, row.get(1)?)),
            ) {
                Ok(v) => v,
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    return Err(smuggle(CommerceError::NotFound));
                }
                Err(e) => return Err(e),
            };

            let on_hand =
                parse_decimal_strict(&source_on_hand, "location_inventory", "quantity_on_hand")
                    .map_err(smuggle)?;
            let reserved =
                parse_decimal_strict(&source_reserved, "location_inventory", "quantity_reserved")
                    .map_err(smuggle)?;
            let available = on_hand - reserved;

            if input.quantity > available {
                return Err(smuggle(CommerceError::ValidationError(format!(
                    "Insufficient available quantity. Requested: {}, Available: {}",
                    input.quantity, available
                ))));
            }

            // Reduce source location
            let new_source_qty = on_hand - input.quantity;
            tx.execute(
                "UPDATE location_inventory SET quantity_on_hand = ?1, updated_at = ?2
                 WHERE location_id = ?3 AND sku = ?4 AND COALESCE(lot_id, '') = ?5",
                params![
                    new_source_qty.to_string(),
                    now_str,
                    input.from_location_id,
                    input.sku,
                    lot_key,
                ],
            )?;

            // Increase destination location
            let dest_exists: bool = tx
                .query_row(
                    "SELECT 1 FROM location_inventory WHERE location_id = ?1 AND sku = ?2 AND COALESCE(lot_id, '') = ?3",
                    params![input.to_location_id, input.sku, lot_key],
                    |_| Ok(true),
                )
                .unwrap_or(false);

            if dest_exists {
                // `quantity_on_hand` is a TEXT column (migration 016), so adding in
                // SQL ('CAST(quantity_on_hand AS REAL) + ?1') would coerce both
                // operands to IEEE-754 floats ('0.1' + '0.2' = 0.30000000000000004).
                // Read the destination's current quantity, add with
                // `rust_decimal::Decimal` in Rust, and write the exact precomputed
                // string back as a bound parameter.
                let dest_on_hand: String = tx.query_row(
                    "SELECT quantity_on_hand FROM location_inventory
                     WHERE location_id = ?1 AND sku = ?2 AND COALESCE(lot_id, '') = ?3",
                    params![input.to_location_id, input.sku, lot_key],
                    |row| row.get(0),
                )?;
                let new_dest_qty =
                    parse_decimal_strict(&dest_on_hand, "location_inventory", "quantity_on_hand")
                        .map_err(smuggle)?
                        + input.quantity;
                tx.execute(
                    "UPDATE location_inventory SET quantity_on_hand = ?1, updated_at = ?2
                     WHERE location_id = ?3 AND sku = ?4 AND COALESCE(lot_id, '') = ?5",
                    params![
                        new_dest_qty.to_string(),
                        now_str,
                        input.to_location_id,
                        input.sku,
                        lot_key,
                    ],
                )?;
            } else {
                tx.execute(
                    "INSERT INTO location_inventory (location_id, sku, lot_id, quantity_on_hand, quantity_reserved, updated_at)
                     VALUES (?1, ?2, ?3, ?4, '0', ?5)",
                    params![
                        input.to_location_id,
                        input.sku,
                        lot_key,
                        input.quantity.to_string(),
                        now_str,
                    ],
                )?;
            }

            // Record the movement
            tx.execute(
                "INSERT INTO inventory_movements (id, movement_type, from_location_id, to_location_id, sku, lot_id, quantity, reason, performed_by, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    movement_id.to_string(),
                    MovementType::Transfer.to_string(),
                    input.from_location_id,
                    input.to_location_id,
                    input.sku,
                    lot_id_str,
                    input.quantity.to_string(),
                    input.reason,
                    input.performed_by,
                    now_str,
                ],
            )?;

            Ok(LocationMovement {
                id: movement_id,
                movement_type: MovementType::Transfer,
                from_location_id: Some(input.from_location_id),
                to_location_id: Some(input.to_location_id),
                sku: input.sku.clone(),
                lot_id: input.lot_id,
                quantity: input.quantity,
                reference_type: None,
                reference_id: None,
                reason: input.reason.clone(),
                performed_by: input.performed_by.clone(),
                created_at: now,
            })
        })
    }

    fn list_location_inventory(
        &self,
        filter: LocationInventoryFilter,
    ) -> Result<Vec<LocationInventory>> {
        let conn = self.conn()?;
        let mut sql = "SELECT li.* FROM location_inventory li".to_string();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if filter.warehouse_id.is_some() {
            sql.push_str(" JOIN locations l ON li.location_id = l.id");
        }

        sql.push_str(" WHERE 1=1");

        if let Some(location_id) = filter.location_id {
            sql.push_str(" AND li.location_id = ?");
            params_vec.push(Box::new(location_id));
        }

        if let Some(warehouse_id) = filter.warehouse_id {
            sql.push_str(" AND l.warehouse_id = ?");
            params_vec.push(Box::new(warehouse_id));
        }

        if let Some(sku) = filter.sku {
            sql.push_str(" AND li.sku = ?");
            params_vec.push(Box::new(sku));
        }

        if let Some(lot_id) = filter.lot_id {
            sql.push_str(" AND li.lot_id = ?");
            params_vec.push(Box::new(lot_id.to_string()));
        }

        // `quantity_on_hand` is a TEXT decimal, so the has_quantity filter is
        // applied below on the exact parsed `Decimal` (a SQL CAST(... AS REAL)
        // comparison coerces to IEEE-754 floats). LIMIT/OFFSET then also move
        // to Rust so pagination is applied after the filter, as before.
        let has_quantity = filter.has_quantity == Some(true);

        if !has_quantity {
            crate::sqlite::append_limit_offset(&mut sql, filter.limit, filter.offset);
        }

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(std::convert::AsRef::as_ref).collect();

        let mut rows = stmt.query(params_refs.as_slice()).map_err(map_db_error)?;

        let mut inventory = Vec::new();
        while let Some(row) = rows.next().map_err(map_db_error)? {
            inventory.push(Self::row_to_location_inventory(row).map_err(map_db_error)?);
        }

        if has_quantity {
            inventory.retain(|entry| entry.quantity_on_hand > Decimal::ZERO);
            if let Some(offset) = filter.offset {
                let offset = (offset as usize).min(inventory.len());
                inventory.drain(..offset);
            }
            if let Some(limit) = filter.limit {
                inventory.truncate(limit as usize);
            }
        }

        Ok(inventory)
    }

    // ========================================================================
    // Movement Operations
    // ========================================================================

    fn get_movements(&self, filter: MovementFilter) -> Result<Vec<LocationMovement>> {
        let conn = self.conn()?;
        let mut sql = "SELECT m.* FROM inventory_movements m".to_string();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if filter.warehouse_id.is_some() {
            sql.push_str(
                " LEFT JOIN locations l_from ON m.from_location_id = l_from.id
                  LEFT JOIN locations l_to ON m.to_location_id = l_to.id",
            );
        }

        sql.push_str(" WHERE 1=1");

        if let Some(warehouse_id) = filter.warehouse_id {
            sql.push_str(" AND (l_from.warehouse_id = ? OR l_to.warehouse_id = ?)");
            params_vec.push(Box::new(warehouse_id));
            params_vec.push(Box::new(warehouse_id));
        }

        if let Some(location_id) = filter.location_id {
            sql.push_str(" AND (m.from_location_id = ? OR m.to_location_id = ?)");
            params_vec.push(Box::new(location_id));
            params_vec.push(Box::new(location_id));
        }

        if let Some(sku) = filter.sku {
            sql.push_str(" AND m.sku = ?");
            params_vec.push(Box::new(sku));
        }

        if let Some(lot_id) = filter.lot_id {
            sql.push_str(" AND m.lot_id = ?");
            params_vec.push(Box::new(lot_id.to_string()));
        }

        if let Some(movement_type) = filter.movement_type {
            sql.push_str(" AND m.movement_type = ?");
            params_vec.push(Box::new(movement_type.to_string()));
        }

        if let Some(from_date) = filter.from_date {
            sql.push_str(" AND m.created_at >= ?");
            params_vec.push(Box::new(from_date.to_rfc3339()));
        }

        if let Some(to_date) = filter.to_date {
            sql.push_str(" AND m.created_at <= ?");
            params_vec.push(Box::new(to_date.to_rfc3339()));
        }

        sql.push_str(" ORDER BY m.created_at DESC");

        crate::sqlite::append_limit_offset(&mut sql, filter.limit, filter.offset);

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(std::convert::AsRef::as_ref).collect();

        let mut rows = stmt.query(params_refs.as_slice()).map_err(map_db_error)?;

        let mut movements = Vec::new();
        while let Some(row) = rows.next().map_err(map_db_error)? {
            movements.push(Self::row_to_movement(row).map_err(map_db_error)?);
        }

        Ok(movements)
    }

    fn count_movements(&self, filter: MovementFilter) -> Result<u64> {
        let conn = self.conn()?;
        let mut sql = "SELECT COUNT(*) FROM inventory_movements m".to_string();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if filter.warehouse_id.is_some() {
            sql.push_str(
                " LEFT JOIN locations l_from ON m.from_location_id = l_from.id
                  LEFT JOIN locations l_to ON m.to_location_id = l_to.id",
            );
        }

        sql.push_str(" WHERE 1=1");

        if let Some(warehouse_id) = filter.warehouse_id {
            sql.push_str(" AND (l_from.warehouse_id = ? OR l_to.warehouse_id = ?)");
            params_vec.push(Box::new(warehouse_id));
            params_vec.push(Box::new(warehouse_id));
        }

        if let Some(movement_type) = filter.movement_type {
            sql.push_str(" AND m.movement_type = ?");
            params_vec.push(Box::new(movement_type.to_string()));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(std::convert::AsRef::as_ref).collect();

        let count: i64 =
            conn.query_row(&sql, params_refs.as_slice(), |row| row.get(0)).map_err(map_db_error)?;

        Ok(count as u64)
    }

    // ========================================================================
    // Batch Operations
    // ========================================================================

    fn create_locations_batch(&self, inputs: Vec<CreateLocation>) -> Result<BatchResult<Location>> {
        let mut result = BatchResult::new();

        for (index, input) in inputs.into_iter().enumerate() {
            match self.create_location(input) {
                Ok(location) => result.record_success(location),
                Err(e) => result.record_failure(index, None, &e),
            }
        }

        Ok(result)
    }

    fn get_locations_batch(&self, ids: Vec<i32>) -> Result<Vec<Location>> {
        let mut locations = Vec::new();
        for id in ids {
            if let Some(location) = self.get_location(id)? {
                locations.push(location);
            }
        }
        Ok(locations)
    }

    // ========================================================================
    // Cycle Count Operations
    // ========================================================================

    fn create_cycle_count(&self, input: CreateCycleCount) -> Result<CycleCount> {
        input.validate()?;
        let conn = self.conn()?;
        let id = Uuid::new_v4();
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO cycle_counts (id, warehouse_id, location_id, status, scheduled_date, counted_by, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
            params![
                id.to_string(),
                input.warehouse_id,
                input.location_id,
                CycleCountStatus::Draft.to_string(),
                input.scheduled_date.map(|d| d.to_rfc3339()),
                input.counted_by,
                now,
            ],
        )
        .map_err(map_db_error)?;

        for line in &input.lines {
            conn.execute(
                "INSERT INTO cycle_count_lines (id, cycle_count_id, sku, lot_id, expected_quantity)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    Uuid::new_v4().to_string(),
                    id.to_string(),
                    line.sku,
                    line.lot_id.map(|l| l.to_string()).unwrap_or_default(),
                    line.expected_quantity.to_string(),
                ],
            )
            .map_err(map_db_error)?;
        }

        Self::get_cycle_count_on(&conn, id)?.ok_or_else(|| {
            CommerceError::DatabaseError("Failed to retrieve created cycle count".into())
        })
    }

    fn get_cycle_count(&self, id: Uuid) -> Result<Option<CycleCount>> {
        let conn = self.conn()?;
        Self::get_cycle_count_on(&conn, id)
    }

    fn list_cycle_counts(&self, filter: CycleCountFilter) -> Result<Vec<CycleCount>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM cycle_counts WHERE 1=1".to_string();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(warehouse_id) = filter.warehouse_id {
            sql.push_str(" AND warehouse_id = ?");
            params_vec.push(Box::new(warehouse_id));
        }
        if let Some(location_id) = filter.location_id {
            sql.push_str(" AND location_id = ?");
            params_vec.push(Box::new(location_id));
        }
        if let Some(status) = filter.status {
            sql.push_str(" AND status = ?");
            params_vec.push(Box::new(status.to_string()));
        }

        // Keyset cursor: (created_at, id) for stable DESC ordering
        if let Some((cursor_created, cursor_id)) = &filter.after_cursor {
            sql.push_str(" AND (created_at < ? OR (created_at = ? AND id < ?))");
            params_vec.push(Box::new(cursor_created.clone()));
            params_vec.push(Box::new(cursor_created.clone()));
            params_vec.push(Box::new(cursor_id.clone()));
        }
        sql.push_str(" ORDER BY created_at DESC, id DESC");
        // Offset pagination applies only in non-cursor mode.
        let offset = if filter.after_cursor.is_none() { filter.offset } else { None };
        crate::sqlite::append_limit_offset(&mut sql, filter.limit, offset);

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(std::convert::AsRef::as_ref).collect();
        let mut rows = stmt.query(params_refs.as_slice()).map_err(map_db_error)?;

        let mut counts = Vec::new();
        while let Some(row) = rows.next().map_err(map_db_error)? {
            counts.push(Self::row_to_cycle_count_header(row).map_err(map_db_error)?);
        }
        drop(rows);
        drop(stmt);

        // Batch-load lines for all listed cycle counts on the same connection.
        let ids: Vec<Uuid> = counts.iter().map(|c| c.id).collect();
        let mut lines_by_id = Self::load_cycle_count_lines_batch(&conn, &ids)?;
        for cc in &mut counts {
            cc.lines = lines_by_id.remove(&cc.id).unwrap_or_default();
        }
        Ok(counts)
    }

    fn start_cycle_count(&self, id: Uuid) -> Result<CycleCount> {
        self.transition_cycle_count(id, CycleCountStatus::InProgress, false)
    }

    fn record_cycle_counts(
        &self,
        id: Uuid,
        counts: Vec<RecordCycleCountLine>,
    ) -> Result<CycleCount> {
        for count in &counts {
            count.validate()?;
        }
        let conn = self.conn()?;
        let existing = Self::get_cycle_count_on(&conn, id)?.ok_or(CommerceError::NotFound)?;
        if existing.status != CycleCountStatus::InProgress {
            return Err(CommerceError::ValidationError(format!(
                "counts can only be recorded on an in_progress cycle count (status: {})",
                existing.status
            )));
        }

        let now = Utc::now().to_rfc3339();
        for count in &counts {
            let lot_key = count.lot_id.map(|l| l.to_string()).unwrap_or_default();
            let line = existing
                .lines
                .iter()
                .find(|l| l.sku == count.sku && l.lot_id == count.lot_id)
                .ok_or_else(|| {
                    CommerceError::ValidationError(format!(
                        "cycle count has no line for sku {} (lot: {lot_key:?})",
                        count.sku
                    ))
                })?;
            let variance = count.counted_quantity - line.expected_quantity;
            conn.execute(
                "UPDATE cycle_count_lines SET counted_quantity = ?1, variance = ?2
                 WHERE cycle_count_id = ?3 AND sku = ?4 AND lot_id = ?5",
                params![
                    count.counted_quantity.to_string(),
                    variance.to_string(),
                    id.to_string(),
                    count.sku,
                    lot_key,
                ],
            )
            .map_err(map_db_error)?;
        }
        conn.execute(
            "UPDATE cycle_counts SET updated_at = ?1 WHERE id = ?2",
            params![now, id.to_string()],
        )
        .map_err(map_db_error)?;

        Self::get_cycle_count_on(&conn, id)?.ok_or_else(|| {
            CommerceError::DatabaseError("Failed to retrieve updated cycle count".into())
        })
    }

    fn complete_cycle_count(&self, id: Uuid) -> Result<CycleCount> {
        let smuggle = |e: CommerceError| rusqlite::Error::ToSqlConversionFailure(Box::new(e));

        // Completion applies every non-zero variance as an inventory adjustment
        // plus a `cycle_count` movement, then flips the status — all atomically.
        with_immediate_transaction(&self.pool, |tx| {
            let existing = Self::get_cycle_count_on(tx, id)
                .map_err(smuggle)?
                .ok_or_else(|| smuggle(CommerceError::NotFound))?;
            if !existing.status.can_transition_to(CycleCountStatus::Completed) {
                return Err(smuggle(CommerceError::ValidationError(format!(
                    "cannot complete cycle count in status {}",
                    existing.status
                ))));
            }

            let now = Utc::now();
            let now_str = now.to_rfc3339();

            for line in &existing.lines {
                let counted = line.counted_quantity.ok_or_else(|| {
                    smuggle(CommerceError::ValidationError(format!(
                        "cycle count line for sku {} has no recorded count",
                        line.sku
                    )))
                })?;
                let variance = counted - line.expected_quantity;
                // Persist the final variance even when zero.
                tx.execute(
                    "UPDATE cycle_count_lines SET variance = ?1 WHERE id = ?2",
                    params![variance.to_string(), line.id.to_string()],
                )?;
                if variance == Decimal::ZERO {
                    continue;
                }
                let location_id = existing.location_id.ok_or_else(|| {
                    smuggle(CommerceError::ValidationError(
                        "cycle count has no location_id; cannot apply variance adjustments".into(),
                    ))
                })?;
                let lot_key = line.lot_id.map(|l| l.to_string()).unwrap_or_default();

                // Adjust location_inventory (same path as adjust_inventory).
                let current: Option<String> = tx
                    .query_row(
                        "SELECT quantity_on_hand FROM location_inventory
                         WHERE location_id = ?1 AND sku = ?2 AND COALESCE(lot_id, '') = ?3",
                        params![location_id, line.sku, lot_key],
                        |row| row.get(0),
                    )
                    .ok();
                if let Some(oh_str) = current {
                    let on_hand =
                        parse_decimal_strict(&oh_str, "location_inventory", "quantity_on_hand")
                            .map_err(smuggle)?;
                    let new_qty = on_hand + variance;
                    if new_qty < Decimal::ZERO {
                        return Err(smuggle(CommerceError::ValidationError(format!(
                            "cycle count adjustment for sku {} would result in negative inventory",
                            line.sku
                        ))));
                    }
                    tx.execute(
                        "UPDATE location_inventory SET quantity_on_hand = ?1, updated_at = ?2
                         WHERE location_id = ?3 AND sku = ?4 AND COALESCE(lot_id, '') = ?5",
                        params![new_qty.to_string(), now_str, location_id, line.sku, lot_key],
                    )?;
                } else {
                    if variance < Decimal::ZERO {
                        return Err(smuggle(CommerceError::ValidationError(format!(
                            "cycle count adjustment for sku {} would result in negative inventory",
                            line.sku
                        ))));
                    }
                    tx.execute(
                        "INSERT INTO location_inventory (location_id, sku, lot_id, quantity_on_hand, quantity_reserved, updated_at)
                         VALUES (?1, ?2, ?3, ?4, '0', ?5)",
                        params![location_id, line.sku, lot_key, variance.to_string(), now_str],
                    )?;
                }

                // Record the movement as a cycle_count adjustment.
                tx.execute(
                    "INSERT INTO inventory_movements (id, movement_type, to_location_id, sku, lot_id, quantity, reference_type, reference_id, reason, performed_by, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                    params![
                        Uuid::new_v4().to_string(),
                        MovementType::CycleCount.to_string(),
                        location_id,
                        line.sku,
                        line.lot_id.map(|l| l.to_string()),
                        variance.to_string(),
                        "cycle_count",
                        existing.id.to_string(),
                        "Cycle count variance",
                        existing.counted_by.as_deref(),
                        now_str,
                    ],
                )?;
            }

            tx.execute(
                "UPDATE cycle_counts SET status = ?1, updated_at = ?2, completed_at = ?2 WHERE id = ?3",
                params![CycleCountStatus::Completed.to_string(), now_str, id.to_string()],
            )?;

            Self::get_cycle_count_on(tx, id).map_err(smuggle)?.ok_or_else(|| {
                smuggle(CommerceError::DatabaseError(
                    "Failed to retrieve completed cycle count".into(),
                ))
            })
        })
    }

    fn cancel_cycle_count(&self, id: Uuid) -> Result<CycleCount> {
        self.transition_cycle_count(id, CycleCountStatus::Cancelled, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SqliteDatabase;
    use rust_decimal_macros::dec;
    use stateset_core::{
        AdjustLocationInventory, CreateLocation, CreateWarehouse, CreateZone, LocationFilter,
        LocationType, MoveInventory, UpdateLocation, UpdateWarehouse, WarehouseAddress,
        WarehouseFilter, WarehouseRepository, WarehouseType,
    };

    fn fresh_repo() -> SqliteWarehouseRepository {
        SqliteDatabase::in_memory().expect("in-memory").warehouse()
    }

    fn addr() -> WarehouseAddress {
        WarehouseAddress {
            street1: "1 Test St".into(),
            street2: None,
            city: "Test City".into(),
            state: "TC".into(),
            postal_code: "00000".into(),
            country: "US".into(),
            phone: None,
        }
    }

    fn make_wh(repo: &SqliteWarehouseRepository, code: &str) -> Warehouse {
        repo.create_warehouse(CreateWarehouse {
            code: code.into(),
            name: format!("WH {code}"),
            warehouse_type: WarehouseType::Distribution,
            address: addr(),
            timezone: Some("America/Los_Angeles".into()),
        })
        .expect("create warehouse")
    }

    fn make_loc(repo: &SqliteWarehouseRepository, wh_id: i32, code: &str) -> Location {
        repo.create_location(CreateLocation {
            warehouse_id: wh_id,
            code: Some(code.into()),
            location_type: LocationType::Bulk,
            zone: Some("A".into()),
            aisle: Some("01".into()),
            rack: None,
            level: None,
            bin: None,
            max_weight_kg: Some(dec!(500)),
            max_volume_m3: Some(dec!(2.5)),
            is_pickable: Some(true),
            is_receivable: Some(true),
        })
        .expect("create location")
    }

    /// Seed a `location_inventory` row directly with an exact TEXT quantity.
    fn seed_inventory(repo: &SqliteWarehouseRepository, location_id: i32, sku: &str, qty: &str) {
        let conn = repo.conn().expect("conn");
        conn.execute(
            "INSERT INTO location_inventory
                (location_id, sku, lot_id, quantity_on_hand, quantity_reserved, updated_at)
             VALUES (?1, ?2, '', ?3, '0', datetime('now'))",
            params![location_id, sku, qty],
        )
        .expect("seed inventory");
    }

    #[test]
    fn cycle_count_flow_applies_variance_adjustment_exactly() {
        use stateset_core::{
            CreateCycleCount, CreateCycleCountLine, CycleCountStatus, MovementFilter, MovementType,
            RecordCycleCountLine,
        };

        let repo = fresh_repo();
        let wh = make_wh(&repo, "WH-CC");
        let loc = make_loc(&repo, wh.id, "L-CC");
        seed_inventory(&repo, loc.id, "CC-SKU", "10");

        let cc = repo
            .create_cycle_count(CreateCycleCount {
                warehouse_id: wh.id,
                location_id: Some(loc.id),
                scheduled_date: None,
                counted_by: Some("counter".into()),
                lines: vec![CreateCycleCountLine {
                    sku: "CC-SKU".into(),
                    lot_id: None,
                    expected_quantity: dec!(10),
                }],
            })
            .expect("create cycle count");
        assert_eq!(cc.status, CycleCountStatus::Draft);
        assert_eq!(cc.lines.len(), 1);

        // Cannot complete from draft.
        assert!(repo.complete_cycle_count(cc.id).is_err());

        let cc = repo.start_cycle_count(cc.id).expect("start");
        assert_eq!(cc.status, CycleCountStatus::InProgress);

        // Record a short count: 7.5 counted vs 10 expected → variance -2.5.
        let cc = repo
            .record_cycle_counts(
                cc.id,
                vec![RecordCycleCountLine {
                    sku: "CC-SKU".into(),
                    lot_id: None,
                    counted_quantity: dec!(7.5),
                }],
            )
            .expect("record counts");
        assert_eq!(cc.lines[0].counted_quantity, Some(dec!(7.5)));
        assert_eq!(cc.lines[0].variance, Some(dec!(-2.5)));

        let cc = repo.complete_cycle_count(cc.id).expect("complete");
        assert_eq!(cc.status, CycleCountStatus::Completed);
        assert!(cc.completed_at.is_some());

        // Adjustment applied exactly: 10 - 2.5 = 7.5 on hand.
        let inv = repo.get_location_inventory(loc.id).expect("inventory");
        let row = inv.iter().find(|i| i.sku == "CC-SKU").expect("row");
        assert_eq!(row.quantity_on_hand, dec!(7.5));

        // A cycle_count movement was recorded referencing the count.
        let movements = repo
            .get_movements(MovementFilter {
                movement_type: Some(MovementType::CycleCount),
                sku: Some("CC-SKU".into()),
                ..Default::default()
            })
            .expect("movements");
        assert_eq!(movements.len(), 1);
        assert_eq!(movements[0].quantity, dec!(-2.5));
        assert_eq!(movements[0].reference_id, Some(cc.id));
        assert_eq!(movements[0].reference_type.as_deref(), Some("cycle_count"));

        // Terminal: cannot cancel or restart a completed count.
        assert!(repo.cancel_cycle_count(cc.id).is_err());
        assert!(repo.start_cycle_count(cc.id).is_err());
    }

    #[test]
    fn cycle_count_cancel_applies_no_adjustments() {
        use stateset_core::{
            CreateCycleCount, CreateCycleCountLine, CycleCountFilter, CycleCountStatus,
            RecordCycleCountLine,
        };

        let repo = fresh_repo();
        let wh = make_wh(&repo, "WH-CCX");
        let loc = make_loc(&repo, wh.id, "L-CCX");
        seed_inventory(&repo, loc.id, "CCX-SKU", "5");

        let cc = repo
            .create_cycle_count(CreateCycleCount {
                warehouse_id: wh.id,
                location_id: Some(loc.id),
                scheduled_date: None,
                counted_by: None,
                lines: vec![CreateCycleCountLine {
                    sku: "CCX-SKU".into(),
                    lot_id: None,
                    expected_quantity: dec!(5),
                }],
            })
            .expect("create");
        let cc = repo.start_cycle_count(cc.id).expect("start");
        repo.record_cycle_counts(
            cc.id,
            vec![RecordCycleCountLine {
                sku: "CCX-SKU".into(),
                lot_id: None,
                counted_quantity: dec!(2),
            }],
        )
        .expect("record");

        let cc = repo.cancel_cycle_count(cc.id).expect("cancel");
        assert_eq!(cc.status, CycleCountStatus::Cancelled);

        // Inventory untouched.
        let inv = repo.get_location_inventory(loc.id).expect("inventory");
        assert_eq!(inv[0].quantity_on_hand, dec!(5));

        // Filter by status finds it.
        let listed = repo
            .list_cycle_counts(CycleCountFilter {
                warehouse_id: Some(wh.id),
                status: Some(CycleCountStatus::Cancelled),
                ..Default::default()
            })
            .expect("list");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, cc.id);
    }

    #[test]
    fn move_inventory_is_atomic_when_destination_write_fails() {
        let repo = fresh_repo();
        let wh = make_wh(&repo, "WH-ATOMIC");
        let src = make_loc(&repo, wh.id, "SRC");
        seed_inventory(&repo, src.id, "SKU-M", "10");

        // Move to a destination location id that does not exist. The source is
        // decremented first, then the destination write fails — the whole move
        // must roll back, leaving the source stock intact (never destroyed).
        let result = repo.move_inventory(MoveInventory {
            from_location_id: src.id,
            to_location_id: 999_999,
            sku: "SKU-M".into(),
            lot_id: None,
            quantity: dec!(3),
            reason: None,
            performed_by: None,
        });
        assert!(result.is_err(), "move to a non-existent destination must fail");

        let src_inv = repo.get_location_inventory(src.id).expect("inventory");
        let row = src_inv.iter().find(|i| i.sku == "SKU-M").expect("source inventory still exists");
        assert_eq!(
            row.quantity_on_hand,
            dec!(10),
            "source must not lose stock when a move fails partway"
        );
    }

    #[test]
    fn pickable_locations_compare_on_hand_and_reserved_exactly() {
        let repo = fresh_repo();
        let wh = make_wh(&repo, "WH-EXACT");
        let loc = make_loc(&repo, wh.id, "L-EXACT");
        // On hand exceeds reserved by 1e-18: both round to the same f64, so a
        // CAST-AS-REAL comparison would wrongly report nothing pickable.
        seed_inventory(&repo, loc.id, "PICK-X", "1.000000000000000001");
        {
            let conn = repo.conn().expect("conn");
            conn.execute(
                "UPDATE location_inventory SET quantity_reserved = '1'
                 WHERE location_id = ?1 AND sku = 'PICK-X'",
                params![loc.id],
            )
            .expect("reserve");
        }

        let pickable = repo.get_pickable_locations(wh.id, "PICK-X").expect("ok");
        assert_eq!(pickable.len(), 1, "1.000000000000000001 > 1 exactly");
        assert_eq!(pickable[0].id, loc.id);

        // Fully reserved stock must not be pickable.
        {
            let conn = repo.conn().expect("conn");
            conn.execute(
                "UPDATE location_inventory SET quantity_reserved = quantity_on_hand
                 WHERE location_id = ?1 AND sku = 'PICK-X'",
                params![loc.id],
            )
            .expect("reserve all");
        }
        let none = repo.get_pickable_locations(wh.id, "PICK-X").expect("ok");
        assert!(none.is_empty(), "fully reserved stock is not pickable");
    }

    #[test]
    fn location_inventory_filters_zero_quantities_exactly() {
        let repo = fresh_repo();
        let wh = make_wh(&repo, "WH-ZQ");
        let loc = make_loc(&repo, wh.id, "L-ZQ");
        seed_inventory(&repo, loc.id, "ZQ-POS", "0.005");
        // A non-canonical zero, to make sure filtering is numeric.
        seed_inventory(&repo, loc.id, "ZQ-ZERO", "0.00");

        let inv = repo.get_location_inventory(loc.id).expect("ok");
        assert_eq!(inv.len(), 1);
        assert_eq!(inv[0].sku, "ZQ-POS");

        let for_sku = repo.get_inventory_for_sku(wh.id, "ZQ-POS").expect("ok");
        assert_eq!(for_sku.len(), 1);

        let listed = repo
            .list_location_inventory(LocationInventoryFilter {
                location_id: Some(loc.id),
                has_quantity: Some(true),
                limit: Some(1),
                ..Default::default()
            })
            .expect("ok");
        assert_eq!(listed.len(), 1, "limit must apply after the quantity filter");
        assert_eq!(listed[0].sku, "ZQ-POS");

        // Location still holds stock, so it cannot be deleted...
        assert!(repo.delete_location(loc.id).is_err());
        // ...but once the last positive quantity is zeroed out it can be.
        {
            let conn = repo.conn().expect("conn");
            conn.execute(
                "UPDATE location_inventory SET quantity_on_hand = '0.000'
                 WHERE location_id = ?1 AND sku = 'ZQ-POS'",
                params![loc.id],
            )
            .expect("zero out");
        }
        repo.delete_location(loc.id).expect("deletable once empty");
    }

    #[test]
    fn create_warehouse_round_trips() {
        let repo = fresh_repo();
        let wh = make_wh(&repo, "WH-A");
        assert_eq!(wh.code, "WH-A");
        assert_eq!(wh.warehouse_type, WarehouseType::Distribution);
        assert!(wh.is_active);

        let by_id = repo.get_warehouse(wh.id).expect("ok").expect("found");
        assert_eq!(by_id.id, wh.id);
        let by_code = repo.get_warehouse_by_code("WH-A").expect("ok").expect("found");
        assert_eq!(by_code.id, wh.id);
        assert!(repo.get_warehouse_by_code("missing").expect("ok").is_none());
    }

    #[test]
    fn update_warehouse_changes_fields() {
        let repo = fresh_repo();
        let wh = make_wh(&repo, "WH-UP");
        let updated = repo
            .update_warehouse(
                wh.id,
                UpdateWarehouse {
                    name: Some("Renamed Warehouse".into()),
                    is_active: Some(false),
                    ..Default::default()
                },
            )
            .expect("update");
        assert_eq!(updated.name, "Renamed Warehouse");
        assert!(!updated.is_active);
    }

    #[test]
    fn list_warehouses_filters_by_active() {
        let repo = fresh_repo();
        let wh_active = make_wh(&repo, "WH-AC");
        let wh_inactive = make_wh(&repo, "WH-IN");
        repo.update_warehouse(
            wh_inactive.id,
            UpdateWarehouse { is_active: Some(false), ..Default::default() },
        )
        .expect("inactive");

        let active = repo
            .list_warehouses(WarehouseFilter { is_active: Some(true), ..Default::default() })
            .expect("active");
        let inactive = repo
            .list_warehouses(WarehouseFilter { is_active: Some(false), ..Default::default() })
            .expect("inactive");
        assert!(active.iter().any(|w| w.id == wh_active.id));
        assert!(!active.iter().any(|w| w.id == wh_inactive.id));
        assert!(inactive.iter().any(|w| w.id == wh_inactive.id));
    }

    #[test]
    fn delete_warehouse_soft_or_hard() {
        let repo = fresh_repo();
        let wh = make_wh(&repo, "WH-DEL");
        repo.delete_warehouse(wh.id).expect("delete");
        // Either gone entirely or marked inactive — both behaviours are acceptable.
        if let Some(found) = repo.get_warehouse(wh.id).expect("ok") {
            assert!(!found.is_active, "deleted warehouse should be inactive");
        }
    }

    #[test]
    fn create_zone_round_trips() {
        let repo = fresh_repo();
        let wh = make_wh(&repo, "WH-Z");
        let zone = repo
            .create_zone(CreateZone {
                warehouse_id: wh.id,
                code: "PICK-A".into(),
                name: "Pick Aisle A".into(),
                description: Some("Fast-moving SKUs".into()),
            })
            .expect("create zone");
        assert_eq!(zone.code, "PICK-A");

        let by_id = repo.get_zone(zone.id).expect("ok").expect("found");
        assert_eq!(by_id.id, zone.id);

        let zones = repo.get_zones(wh.id).expect("zones");
        assert_eq!(zones.len(), 1);
    }

    #[test]
    fn create_location_with_explicit_code_round_trips() {
        let repo = fresh_repo();
        let wh = make_wh(&repo, "WH-L");
        let loc = make_loc(&repo, wh.id, "A-01-01-001");
        assert_eq!(loc.warehouse_id, wh.id);
        assert_eq!(loc.code, "A-01-01-001");
        assert!(loc.is_pickable);

        let by_id = repo.get_location(loc.id).expect("ok").expect("found");
        assert_eq!(by_id.id, loc.id);
        let by_code = repo.get_location_by_code(wh.id, "A-01-01-001").expect("ok").expect("found");
        assert_eq!(by_code.id, loc.id);
        assert!(repo.get_location_by_code(wh.id, "missing").expect("ok").is_none());
    }

    #[test]
    fn update_location_changes_pickable_flag() {
        let repo = fresh_repo();
        let wh = make_wh(&repo, "WH-UPD");
        let loc = make_loc(&repo, wh.id, "B-02");
        let updated = repo
            .update_location(
                loc.id,
                UpdateLocation { is_pickable: Some(false), ..Default::default() },
            )
            .expect("update");
        assert!(!updated.is_pickable);
    }

    #[test]
    fn list_locations_filters_by_warehouse_and_pickable() {
        let repo = fresh_repo();
        let wh = make_wh(&repo, "WH-LL");
        let pickable = make_loc(&repo, wh.id, "P-1");
        let non_pickable = repo
            .create_location(CreateLocation {
                warehouse_id: wh.id,
                code: Some("NP-1".into()),
                location_type: LocationType::Quarantine,
                is_pickable: Some(false),
                is_receivable: Some(false),
                ..Default::default()
            })
            .expect("create np");

        let pickable_filter = repo
            .list_locations(LocationFilter {
                warehouse_id: Some(wh.id),
                is_pickable: Some(true),
                ..Default::default()
            })
            .expect("pickable");
        assert!(pickable_filter.iter().any(|l| l.id == pickable.id));
        assert!(!pickable_filter.iter().any(|l| l.id == non_pickable.id));
    }

    #[test]
    fn count_locations_honors_same_filters_as_list_locations() {
        let repo = fresh_repo();
        let wh = make_wh(&repo, "WH-CNT");
        // Zone A: pickable + receivable.
        repo.create_location(CreateLocation {
            warehouse_id: wh.id,
            code: Some("A-PK".into()),
            location_type: LocationType::Bulk,
            zone: Some("A".into()),
            aisle: Some("01".into()),
            is_pickable: Some(true),
            is_receivable: Some(true),
            ..Default::default()
        })
        .expect("loc a");
        // Zone B: neither pickable nor receivable.
        repo.create_location(CreateLocation {
            warehouse_id: wh.id,
            code: Some("B-NPK".into()),
            location_type: LocationType::Quarantine,
            zone: Some("B".into()),
            aisle: Some("02".into()),
            is_pickable: Some(false),
            is_receivable: Some(false),
            ..Default::default()
        })
        .expect("loc b");

        // Every predicate `list_locations` honors, `count_locations` must honor
        // too — each of these filters must select exactly one location.
        for filter in [
            LocationFilter {
                warehouse_id: Some(wh.id),
                is_pickable: Some(true),
                ..Default::default()
            },
            LocationFilter {
                warehouse_id: Some(wh.id),
                is_receivable: Some(true),
                ..Default::default()
            },
            LocationFilter {
                warehouse_id: Some(wh.id),
                zone: Some("A".into()),
                ..Default::default()
            },
            LocationFilter {
                warehouse_id: Some(wh.id),
                aisle: Some("01".into()),
                ..Default::default()
            },
        ] {
            let listed = repo.list_locations(filter.clone()).expect("list").len() as u64;
            let counted = repo.count_locations(filter).expect("count");
            assert_eq!(counted, listed, "count_locations must match list_locations for {counted}");
            assert_eq!(counted, 1, "exactly one location should match");
        }
    }

    #[test]
    fn get_locations_for_warehouse_returns_all() {
        let repo = fresh_repo();
        let wh = make_wh(&repo, "WH-ALL");
        make_loc(&repo, wh.id, "L1");
        make_loc(&repo, wh.id, "L2");
        let locs = repo.get_locations_for_warehouse(wh.id).expect("ok");
        assert_eq!(locs.len(), 2);
    }

    #[test]
    fn get_locations_for_warehouse_includes_inactive() {
        let repo = fresh_repo();
        let wh = make_wh(&repo, "WH-INACT");
        let active = make_loc(&repo, wh.id, "ACT");
        let inactive = make_loc(&repo, wh.id, "INACT");
        repo.update_location(
            inactive.id,
            UpdateLocation { is_active: Some(false), ..Default::default() },
        )
        .expect("deactivate");

        // `get_locations_for_warehouse` returns ALL locations (matching Postgres
        // and the trait contract) — active AND inactive. Filtered subsets have
        // their own accessors (get_pickable_locations / get_receivable_locations).
        let locs = repo.get_locations_for_warehouse(wh.id).expect("ok");
        let ids: Vec<i32> = locs.iter().map(|l| l.id).collect();
        assert_eq!(locs.len(), 2, "must return active AND inactive locations: {ids:?}");
        assert!(ids.contains(&active.id));
        assert!(ids.contains(&inactive.id));
    }

    #[test]
    fn get_pickable_locations_excludes_non_pickable() {
        let repo = fresh_repo();
        let wh = make_wh(&repo, "WH-PK");
        make_loc(&repo, wh.id, "PK-1");
        repo.create_location(CreateLocation {
            warehouse_id: wh.id,
            code: Some("NPK-1".into()),
            location_type: LocationType::Quarantine,
            is_pickable: Some(false),
            is_receivable: Some(false),
            ..Default::default()
        })
        .expect("npk");

        let pickable = repo.get_pickable_locations(wh.id, "ANY-SKU").expect("ok");
        assert!(pickable.iter().all(|l| l.is_pickable));
    }

    #[test]
    fn get_receivable_locations_only_returns_receivable() {
        let repo = fresh_repo();
        let wh = make_wh(&repo, "WH-RC");
        let recv = make_loc(&repo, wh.id, "RCV-1");
        let _non_recv = repo
            .create_location(CreateLocation {
                warehouse_id: wh.id,
                code: Some("NRCV-1".into()),
                location_type: LocationType::Quarantine,
                is_pickable: Some(false),
                is_receivable: Some(false),
                ..Default::default()
            })
            .expect("nrcv");
        let receivable = repo.get_receivable_locations(wh.id).expect("ok");
        assert!(receivable.iter().any(|l| l.id == recv.id));
        assert!(receivable.iter().all(|l| l.is_receivable));
    }

    #[test]
    fn get_warehouse_unknown_returns_none() {
        let repo = fresh_repo();
        assert!(repo.get_warehouse(99_999).expect("ok").is_none());
    }

    #[test]
    fn get_location_unknown_returns_none() {
        let repo = fresh_repo();
        assert!(repo.get_location(99_999).expect("ok").is_none());
    }

    #[test]
    fn move_into_existing_dest_keeps_quantity_exact() {
        // Regression: quantity_on_hand is a TEXT column and the destination
        // receipt was mutated via 'CAST(quantity_on_hand AS REAL) + ?1', so
        // 0.1 + 0.2 stored as 0.30000000000000004. With Decimal arithmetic the
        // destination must hold exactly 0.3.
        let repo = fresh_repo();
        let wh = make_wh(&repo, "WH-MV");
        let src = make_loc(&repo, wh.id, "SRC-1");
        let dst = make_loc(&repo, wh.id, "DST-1");
        let lot = Uuid::new_v4();

        // Seed the source with enough stock to move twice.
        repo.adjust_inventory(AdjustLocationInventory {
            location_id: src.id,
            sku: "SKU-1".into(),
            lot_id: Some(lot),
            quantity: dec!(10),
            reason: "seed source".into(),
            reference_type: None,
            reference_id: None,
            performed_by: None,
        })
        .expect("seed source");

        // First move creates the destination row (0.1).
        repo.move_inventory(MoveInventory {
            from_location_id: src.id,
            to_location_id: dst.id,
            sku: "SKU-1".into(),
            lot_id: Some(lot),
            quantity: dec!(0.1),
            reason: None,
            performed_by: None,
        })
        .expect("move 1");

        // Second move hits the dest_exists branch: 0.1 + 0.2 must be exactly 0.3.
        repo.move_inventory(MoveInventory {
            from_location_id: src.id,
            to_location_id: dst.id,
            sku: "SKU-1".into(),
            lot_id: Some(lot),
            quantity: dec!(0.2),
            reason: None,
            performed_by: None,
        })
        .expect("move 2");

        let dst_inv = repo.get_location_inventory(dst.id).expect("dest inventory");
        let on_hand = dst_inv
            .iter()
            .find(|i| i.sku == "SKU-1")
            .map(|i| i.quantity_on_hand)
            .expect("dest row present");
        assert_eq!(on_hand, dec!(0.3));
    }

    #[test]
    fn list_cycle_counts_batched_line_loading_preserves_per_count_lines() {
        use stateset_core::{CreateCycleCount, CreateCycleCountLine, CycleCountFilter};

        let repo = fresh_repo();
        let wh = make_wh(&repo, "WH-BATCH");
        let line = |sku: &str| CreateCycleCountLine {
            sku: sku.into(),
            lot_id: None,
            expected_quantity: dec!(1),
        };
        // Distinct line sets per parent so cross-wiring would be caught.
        for lines in [
            vec![line("BAT-A1"), line("BAT-A2")],
            vec![line("BAT-B1")],
            vec![line("BAT-C1"), line("BAT-C2"), line("BAT-C3")],
        ] {
            repo.create_cycle_count(CreateCycleCount {
                warehouse_id: wh.id,
                location_id: None,
                scheduled_date: None,
                counted_by: None,
                lines,
            })
            .expect("create cycle count");
        }

        let listed = repo.list_cycle_counts(CycleCountFilter::default()).expect("list");
        assert_eq!(listed.len(), 3);
        for cc in &listed {
            let direct = repo.get_cycle_count(cc.id).expect("get").expect("present").lines;
            let listed_skus: Vec<_> = cc.lines.iter().map(|l| l.sku.clone()).collect();
            let direct_skus: Vec<_> = direct.iter().map(|l| l.sku.clone()).collect();
            assert_eq!(listed_skus, direct_skus, "lines must match for cycle count {}", cc.id);
            assert!(
                cc.lines.iter().all(|l| l.cycle_count_id == cc.id),
                "lines must belong to their own cycle count"
            );
        }
    }

    #[test]
    fn list_cycle_counts_after_cursor_paginates_without_overlap() {
        use stateset_core::{CreateCycleCount, CreateCycleCountLine, CycleCountFilter};

        let repo = fresh_repo();
        let wh = make_wh(&repo, "WH-CURSOR");
        for i in 0..3 {
            repo.create_cycle_count(CreateCycleCount {
                warehouse_id: wh.id,
                location_id: None,
                scheduled_date: None,
                counted_by: None,
                lines: vec![CreateCycleCountLine {
                    sku: format!("CUR-SKU-{i}"),
                    lot_id: None,
                    expected_quantity: dec!(1),
                }],
            })
            .expect("create cycle count");
        }

        let all = repo.list_cycle_counts(CycleCountFilter::default()).expect("list all");
        assert_eq!(all.len(), 3);

        let first_page = repo
            .list_cycle_counts(CycleCountFilter { limit: Some(2), ..Default::default() })
            .expect("page 1");
        assert_eq!(first_page.len(), 2);
        assert_eq!(first_page[0].id, all[0].id);

        let last = &first_page[1];
        let second_page = repo
            .list_cycle_counts(CycleCountFilter {
                after_cursor: Some((last.created_at.to_rfc3339(), last.id.to_string())),
                ..Default::default()
            })
            .expect("page 2");
        assert_eq!(second_page.len(), 1);
        assert_eq!(second_page[0].id, all[2].id);
    }
}
