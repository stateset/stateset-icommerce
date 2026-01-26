//! SQLite implementation for warehouse management
//!
//! Provides full warehouse, zone, location, and inventory movement functionality.

use crate::sqlite::{
    map_db_error, parse_datetime_row, parse_decimal_opt_row, parse_decimal_row, parse_decimal_strict,
    parse_enum_row, parse_json_row, parse_uuid_opt_row, parse_uuid_row,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rust_decimal::Decimal;
use rusqlite::params;
use uuid::Uuid;

use stateset_core::{
    AdjustLocationInventory, BatchResult, CommerceError, CreateLocation, CreateWarehouse,
    CreateZone, Location, LocationFilter, LocationInventory, LocationInventoryFilter,
    LocationMovement, LocationType, MoveInventory, MovementFilter, MovementType, Result,
    UpdateLocation, UpdateWarehouse, UpdateZone, Warehouse, WarehouseAddress, WarehouseFilter,
    WarehouseRepository, WarehouseType, Zone,
};

/// SQLite warehouse repository
pub struct SqliteWarehouseRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteWarehouseRepository {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    // Helper to parse warehouse from row
    fn row_to_warehouse(row: &rusqlite::Row) -> rusqlite::Result<Warehouse> {
        Ok(Warehouse {
            id: row.get("id")?,
            code: row.get("code")?,
            name: row.get("name")?,
            warehouse_type: parse_enum_row(
                &row.get::<_, String>("warehouse_type")?,
                "warehouse",
                "warehouse_type",
            )?,
            address: parse_json_row(&row.get::<_, String>("address_json")?, "warehouse", "address_json")?,
            timezone: row.get("timezone")?,
            is_active: row.get::<_, i32>("is_active")? == 1,
            created_at: parse_datetime_row(&row.get::<_, String>("created_at")?, "warehouse", "created_at")?,
            updated_at: parse_datetime_row(&row.get::<_, String>("updated_at")?, "warehouse", "updated_at")?,
        })
    }

    // Helper to parse location from row
    fn row_to_location(row: &rusqlite::Row) -> rusqlite::Result<Location> {
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
            created_at: parse_datetime_row(&row.get::<_, String>("created_at")?, "location", "created_at")?,
            updated_at: parse_datetime_row(&row.get::<_, String>("updated_at")?, "location", "updated_at")?,
        })
    }

    // Helper to parse location inventory from row
    fn row_to_location_inventory(row: &rusqlite::Row) -> rusqlite::Result<LocationInventory> {
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
    fn row_to_movement(row: &rusqlite::Row) -> rusqlite::Result<LocationMovement> {
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
    fn row_to_zone(row: &rusqlite::Row) -> rusqlite::Result<Zone> {
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
        let address_json =
            serde_json::to_string(&address).unwrap_or_else(|_| "{}".to_string());

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
        let mut stmt = conn
            .prepare("SELECT * FROM warehouses WHERE id = ?1")
            .map_err(map_db_error)?;

        let mut rows = stmt.query(params![id]).map_err(map_db_error)?;

        if let Some(row) = rows.next().map_err(map_db_error)? {
            Ok(Some(Self::row_to_warehouse(row).map_err(map_db_error)?))
        } else {
            Ok(None)
        }
    }

    fn get_warehouse_by_code(&self, code: &str) -> Result<Option<Warehouse>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT * FROM warehouses WHERE code = ?1")
            .map_err(map_db_error)?;

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
        let mut stmt = conn
            .prepare("SELECT * FROM warehouses WHERE id = ?1")
            .map_err(map_db_error)?;
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
                is_active as i32,
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
            params_vec.push(Box::new(active as i32));
        }

        sql.push_str(" ORDER BY name");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        if let Some(offset) = filter.offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();

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

        conn.execute("DELETE FROM warehouses WHERE id = ?1", params![id])
            .map_err(map_db_error)?;

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
            params_vec.push(Box::new(active as i32));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();

        let count: i64 = conn
            .query_row(&sql, params_refs.as_slice(), |row| row.get(0))
            .map_err(map_db_error)?;

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
        let mut stmt = conn
            .prepare("SELECT * FROM warehouse_zones WHERE id = ?1")
            .map_err(map_db_error)?;

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
        let existing = self.get_zone(id)?
            .ok_or(CommerceError::NotFound)?;

        let name = input.name.unwrap_or(existing.name);
        let description = input.description.or(existing.description);
        let is_active = input.is_active.unwrap_or(existing.is_active);

        {
            let conn = self.conn()?;
            conn.execute(
                "UPDATE warehouse_zones SET name = ?1, description = ?2, is_active = ?3 WHERE id = ?4",
                params![name, description, is_active as i32, id],
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
                input.is_pickable.unwrap_or(true) as i32,
                input.is_receivable.unwrap_or(true) as i32,
                now,
            ],
        )
        .map_err(map_db_error)?;

        let id = conn.last_insert_rowid() as i32;
        drop(conn);
        self.get_location(id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to retrieve created location".into()))
    }

    fn get_location(&self, id: i32) -> Result<Option<Location>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT * FROM locations WHERE id = ?1")
            .map_err(map_db_error)?;

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
        let existing = self.get_location(id)?
            .ok_or(CommerceError::NotFound)?;

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
                is_pickable as i32,
                is_receivable as i32,
                is_active as i32,
                id,
            ],
        )
        .map_err(map_db_error)?;

        self.get_location(id)?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to retrieve updated location".into()))
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
            params_vec.push(Box::new(pickable as i32));
        }

        if let Some(receivable) = filter.is_receivable {
            sql.push_str(" AND is_receivable = ?");
            params_vec.push(Box::new(receivable as i32));
        }

        if let Some(active) = filter.is_active {
            sql.push_str(" AND is_active = ?");
            params_vec.push(Box::new(active as i32));
        }

        sql.push_str(" ORDER BY code");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        if let Some(offset) = filter.offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();

        let mut rows = stmt.query(params_refs.as_slice()).map_err(map_db_error)?;

        let mut locations = Vec::new();
        while let Some(row) = rows.next().map_err(map_db_error)? {
            locations.push(Self::row_to_location(row).map_err(map_db_error)?);
        }

        Ok(locations)
    }

    fn delete_location(&self, id: i32) -> Result<()> {
        let conn = self.conn()?;

        // Check if location has inventory
        let inv_count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM location_inventory WHERE location_id = ?1 AND CAST(quantity_on_hand AS REAL) > 0",
                params![id],
                |row| row.get(0),
            )
            .map_err(map_db_error)?;

        if inv_count > 0 {
            return Err(CommerceError::ValidationError(
                "Cannot delete location with inventory".into(),
            ));
        }

        conn.execute("DELETE FROM locations WHERE id = ?1", params![id])
            .map_err(map_db_error)?;

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

        if let Some(active) = filter.is_active {
            sql.push_str(" AND is_active = ?");
            params_vec.push(Box::new(active as i32));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();

        let count: i64 = conn
            .query_row(&sql, params_refs.as_slice(), |row| row.get(0))
            .map_err(map_db_error)?;

        Ok(count as u64)
    }

    fn get_locations_for_warehouse(&self, warehouse_id: i32) -> Result<Vec<Location>> {
        self.list_locations(LocationFilter {
            warehouse_id: Some(warehouse_id),
            is_active: Some(true),
            ..Default::default()
        })
    }

    fn get_pickable_locations(&self, warehouse_id: i32, sku: &str) -> Result<Vec<Location>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT l.* FROM locations l
                 JOIN location_inventory li ON l.id = li.location_id
                 WHERE l.warehouse_id = ?1 AND l.is_pickable = 1 AND l.is_active = 1
                   AND li.sku = ?2 AND CAST(li.quantity_on_hand AS REAL) > CAST(li.quantity_reserved AS REAL)
                 ORDER BY l.code",
            )
            .map_err(map_db_error)?;

        let mut rows = stmt.query(params![warehouse_id, sku]).map_err(map_db_error)?;

        let mut locations = Vec::new();
        while let Some(row) = rows.next().map_err(map_db_error)? {
            locations.push(Self::row_to_location(row).map_err(map_db_error)?);
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
        let mut stmt = conn
            .prepare(
                "SELECT * FROM location_inventory WHERE location_id = ?1 AND CAST(quantity_on_hand AS REAL) > 0",
            )
            .map_err(map_db_error)?;

        let mut rows = stmt.query(params![location_id]).map_err(map_db_error)?;

        let mut inventory = Vec::new();
        while let Some(row) = rows.next().map_err(map_db_error)? {
            inventory.push(Self::row_to_location_inventory(row).map_err(map_db_error)?);
        }

        Ok(inventory)
    }

    fn get_inventory_for_sku(&self, warehouse_id: i32, sku: &str) -> Result<Vec<LocationInventory>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT li.* FROM location_inventory li
                 JOIN locations l ON li.location_id = l.id
                 WHERE l.warehouse_id = ?1 AND li.sku = ?2 AND CAST(li.quantity_on_hand AS REAL) > 0
                 ORDER BY l.code",
            )
            .map_err(map_db_error)?;

        let mut rows = stmt.query(params![warehouse_id, sku]).map_err(map_db_error)?;

        let mut inventory = Vec::new();
        while let Some(row) = rows.next().map_err(map_db_error)? {
            inventory.push(Self::row_to_location_inventory(row).map_err(map_db_error)?);
        }

        Ok(inventory)
    }

    fn adjust_inventory(&self, input: AdjustLocationInventory) -> Result<LocationInventory> {
        let conn = self.conn()?;
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let lot_id_str = input.lot_id.map(|id| id.to_string());
        let lot_key = lot_id_str.clone().unwrap_or_default();

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
            let on_hand =
                parse_decimal_strict(&oh_str, "location_inventory", "quantity_on_hand")?;
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
                    lot_id_str,
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
        let conn = self.conn()?;
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let lot_id_str = input.lot_id.map(|id| id.to_string());
        let lot_key = lot_id_str.clone().unwrap_or_default();

        // Get source inventory
        let (source_on_hand, source_reserved): (String, String) = conn
            .query_row(
                "SELECT quantity_on_hand, quantity_reserved FROM location_inventory
                 WHERE location_id = ?1 AND sku = ?2 AND COALESCE(lot_id, '') = ?3",
                params![input.from_location_id, input.sku, lot_key],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|_| CommerceError::NotFound)?;

        let on_hand = parse_decimal_strict(&source_on_hand, "location_inventory", "quantity_on_hand")?;
        let reserved = parse_decimal_strict(&source_reserved, "location_inventory", "quantity_reserved")?;
        let available = on_hand - reserved;

        if input.quantity > available {
            return Err(CommerceError::ValidationError(format!(
                "Insufficient available quantity. Requested: {}, Available: {}",
                input.quantity, available
            )));
        }

        // Reduce source location
        let new_source_qty = on_hand - input.quantity;
        conn.execute(
            "UPDATE location_inventory SET quantity_on_hand = ?1, updated_at = ?2
             WHERE location_id = ?3 AND sku = ?4 AND COALESCE(lot_id, '') = ?5",
            params![
                new_source_qty.to_string(),
                now_str,
                input.from_location_id,
                input.sku,
                lot_key,
            ],
        )
        .map_err(map_db_error)?;

        // Increase destination location
        let dest_exists: bool = conn
            .query_row(
                "SELECT 1 FROM location_inventory WHERE location_id = ?1 AND sku = ?2 AND COALESCE(lot_id, '') = ?3",
                params![input.to_location_id, input.sku, lot_key],
                |_| Ok(true),
            )
            .unwrap_or(false);

        if dest_exists {
            conn.execute(
                "UPDATE location_inventory SET quantity_on_hand = CAST(quantity_on_hand AS REAL) + ?1, updated_at = ?2
                 WHERE location_id = ?3 AND sku = ?4 AND COALESCE(lot_id, '') = ?5",
                params![
                    input.quantity.to_string(),
                    now_str,
                    input.to_location_id,
                    input.sku,
                    lot_key,
                ],
            )
            .map_err(map_db_error)?;
        } else {
            conn.execute(
                "INSERT INTO location_inventory (location_id, sku, lot_id, quantity_on_hand, quantity_reserved, updated_at)
                 VALUES (?1, ?2, ?3, ?4, '0', ?5)",
                params![
                    input.to_location_id,
                    input.sku,
                    lot_id_str.clone(),
                    input.quantity.to_string(),
                    now_str,
                ],
            )
            .map_err(map_db_error)?;
        }

        // Record the movement
        let movement_id = Uuid::new_v4();
        conn.execute(
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
        )
        .map_err(map_db_error)?;

        Ok(LocationMovement {
            id: movement_id,
            movement_type: MovementType::Transfer,
            from_location_id: Some(input.from_location_id),
            to_location_id: Some(input.to_location_id),
            sku: input.sku,
            lot_id: input.lot_id,
            quantity: input.quantity,
            reference_type: None,
            reference_id: None,
            reason: input.reason,
            performed_by: input.performed_by,
            created_at: now,
        })
    }

    fn list_location_inventory(&self, filter: LocationInventoryFilter) -> Result<Vec<LocationInventory>> {
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

        if filter.has_quantity == Some(true) {
            sql.push_str(" AND CAST(li.quantity_on_hand AS REAL) > 0");
        }

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        if let Some(offset) = filter.offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();

        let mut rows = stmt.query(params_refs.as_slice()).map_err(map_db_error)?;

        let mut inventory = Vec::new();
        while let Some(row) = rows.next().map_err(map_db_error)? {
            inventory.push(Self::row_to_location_inventory(row).map_err(map_db_error)?);
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

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        if let Some(offset) = filter.offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();

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

        let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();

        let count: i64 = conn
            .query_row(&sql, params_refs.as_slice(), |row| row.get(0))
            .map_err(map_db_error)?;

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
}
