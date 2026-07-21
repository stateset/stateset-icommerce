//! PostgreSQL implementation for warehouse management

use super::{block_on, map_db_error};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;
use sqlx::{FromRow, Postgres, QueryBuilder};
use stateset_core::{
    AdjustLocationInventory, BatchResult, CommerceError, CreateCycleCount, CreateLocation,
    CreateWarehouse, CreateZone, CycleCount, CycleCountFilter, CycleCountLine, CycleCountStatus,
    Location, LocationFilter, LocationInventory, LocationInventoryFilter, LocationMovement,
    LocationType, MoveInventory, MovementFilter, MovementType, RecordCycleCountLine, Result,
    UpdateLocation, UpdateWarehouse, UpdateZone, Warehouse, WarehouseAddress, WarehouseFilter,
    WarehouseRepository, WarehouseType, Zone, validate_batch_size,
};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct PgWarehouseRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct WarehouseRow {
    id: i32,
    code: String,
    name: String,
    warehouse_type: String,
    address_json: serde_json::Value,
    timezone: Option<String>,
    is_active: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct ZoneRow {
    id: i32,
    warehouse_id: i32,
    code: String,
    name: String,
    description: Option<String>,
    is_active: bool,
    created_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct LocationRow {
    id: i32,
    warehouse_id: i32,
    code: String,
    location_type: String,
    zone: Option<String>,
    aisle: Option<String>,
    rack: Option<String>,
    level: Option<String>,
    bin: Option<String>,
    max_weight_kg: Option<Decimal>,
    max_volume_m3: Option<Decimal>,
    current_weight_kg: Option<Decimal>,
    current_volume_m3: Option<Decimal>,
    is_pickable: bool,
    is_receivable: bool,
    is_active: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct LocationInventoryRow {
    location_id: i32,
    sku: String,
    lot_id: Uuid,
    quantity_on_hand: Decimal,
    quantity_reserved: Decimal,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct CycleCountRow {
    id: Uuid,
    warehouse_id: i32,
    location_id: Option<i32>,
    status: String,
    scheduled_date: Option<DateTime<Utc>>,
    counted_by: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    completed_at: Option<DateTime<Utc>>,
}

#[derive(FromRow)]
struct CycleCountLineRow {
    id: Uuid,
    cycle_count_id: Uuid,
    sku: String,
    lot_id: Uuid,
    expected_quantity: Decimal,
    counted_quantity: Option<Decimal>,
    variance: Option<Decimal>,
}

#[derive(FromRow)]
struct LocationMovementRow {
    id: Uuid,
    movement_type: String,
    from_location_id: Option<i32>,
    to_location_id: Option<i32>,
    sku: String,
    lot_id: Option<Uuid>,
    quantity: Decimal,
    reference_type: Option<String>,
    reference_id: Option<Uuid>,
    reason: Option<String>,
    performed_by: Option<String>,
    created_at: DateTime<Utc>,
}

impl PgWarehouseRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_warehouse(row: WarehouseRow) -> Result<Warehouse> {
        let WarehouseRow {
            id,
            code,
            name,
            warehouse_type,
            address_json,
            timezone,
            is_active,
            created_at,
            updated_at,
        } = row;

        let warehouse_type: WarehouseType = warehouse_type.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid warehouse.warehouse_type '{}': {}",
                warehouse_type, e
            ))
        })?;
        let address: WarehouseAddress = serde_json::from_value(address_json).map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid warehouse.address_json: {}", e))
        })?;

        Ok(Warehouse {
            id,
            code,
            name,
            warehouse_type,
            address,
            timezone,
            is_active,
            created_at,
            updated_at,
        })
    }

    fn row_to_zone(row: ZoneRow) -> Zone {
        Zone {
            id: row.id,
            warehouse_id: row.warehouse_id,
            code: row.code,
            name: row.name,
            description: row.description,
            is_active: row.is_active,
            created_at: row.created_at,
        }
    }

    fn row_to_location(row: LocationRow) -> Result<Location> {
        let LocationRow {
            id,
            warehouse_id,
            code,
            location_type,
            zone,
            aisle,
            rack,
            level,
            bin,
            max_weight_kg,
            max_volume_m3,
            current_weight_kg,
            current_volume_m3,
            is_pickable,
            is_receivable,
            is_active,
            created_at,
            updated_at,
        } = row;

        let location_type: LocationType = location_type.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid location.location_type '{}': {}",
                location_type, e
            ))
        })?;

        Ok(Location {
            id,
            warehouse_id,
            code,
            location_type,
            zone,
            aisle,
            rack,
            level,
            bin,
            max_weight_kg,
            max_volume_m3,
            current_weight_kg,
            current_volume_m3,
            is_pickable,
            is_receivable,
            is_active,
            created_at,
            updated_at,
        })
    }

    fn row_to_location_inventory(row: LocationInventoryRow) -> LocationInventory {
        let lot_id = if row.lot_id == Uuid::nil() { None } else { Some(row.lot_id) };
        let available = row.quantity_on_hand - row.quantity_reserved;
        LocationInventory {
            location_id: row.location_id,
            sku: row.sku,
            lot_id,
            quantity_on_hand: row.quantity_on_hand,
            quantity_reserved: row.quantity_reserved,
            quantity_available: available,
            updated_at: row.updated_at,
        }
    }

    fn row_to_movement(row: LocationMovementRow) -> Result<LocationMovement> {
        let LocationMovementRow {
            id,
            movement_type,
            from_location_id,
            to_location_id,
            sku,
            lot_id,
            quantity,
            reference_type,
            reference_id,
            reason,
            performed_by,
            created_at,
        } = row;

        let movement_type: MovementType = movement_type.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid inventory_movement.movement_type '{}': {}",
                movement_type, e
            ))
        })?;

        Ok(LocationMovement {
            id,
            movement_type,
            from_location_id,
            to_location_id,
            sku,
            lot_id,
            quantity,
            reference_type,
            reference_id,
            reason,
            performed_by,
            created_at,
        })
    }

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

    fn lot_key(lot_id: Option<Uuid>) -> Uuid {
        lot_id.unwrap_or_else(Uuid::nil)
    }

    pub async fn create_warehouse_async(&self, input: CreateWarehouse) -> Result<Warehouse> {
        let now = Utc::now();
        let address_json = serde_json::to_value(&input.address).unwrap_or_default();

        let row: (i32,) = sqlx::query_as(
            r#"
            INSERT INTO warehouses (code, name, warehouse_type, address_json, timezone, is_active, created_at, updated_at)
            VALUES ($1,$2,$3,$4,$5,true,$6,$6)
            RETURNING id
            "#,
        )
        .bind(&input.code)
        .bind(&input.name)
        .bind(input.warehouse_type.to_string())
        .bind(address_json)
        .bind(&input.timezone)
        .bind(now)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(Warehouse {
            id: row.0,
            code: input.code,
            name: input.name,
            warehouse_type: input.warehouse_type,
            address: input.address,
            timezone: input.timezone,
            is_active: true,
            created_at: now,
            updated_at: now,
        })
    }

    pub async fn get_warehouse_async(&self, id: i32) -> Result<Option<Warehouse>> {
        let row = sqlx::query_as::<_, WarehouseRow>("SELECT * FROM warehouses WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        row.map(Self::row_to_warehouse).transpose()
    }

    pub async fn get_warehouse_by_code_async(&self, code: &str) -> Result<Option<Warehouse>> {
        let row = sqlx::query_as::<_, WarehouseRow>("SELECT * FROM warehouses WHERE code = $1")
            .bind(code)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        row.map(Self::row_to_warehouse).transpose()
    }

    pub async fn update_warehouse_async(
        &self,
        id: i32,
        input: UpdateWarehouse,
    ) -> Result<Warehouse> {
        let now = Utc::now();
        let address_json =
            input.address.as_ref().map(|address| serde_json::to_value(address).unwrap_or_default());

        sqlx::query(
            r#"
            UPDATE warehouses SET
                name = COALESCE($1, name),
                warehouse_type = COALESCE($2, warehouse_type),
                address_json = COALESCE($3, address_json),
                timezone = COALESCE($4, timezone),
                is_active = COALESCE($5, is_active),
                updated_at = $6
            WHERE id = $7
            "#,
        )
        .bind(input.name)
        .bind(input.warehouse_type.map(|t| t.to_string()))
        .bind(address_json)
        .bind(input.timezone)
        .bind(input.is_active)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_warehouse_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn list_warehouses_async(&self, filter: WarehouseFilter) -> Result<Vec<Warehouse>> {
        let mut builder = QueryBuilder::<Postgres>::new("SELECT * FROM warehouses WHERE 1=1");

        if let Some(warehouse_type) = filter.warehouse_type {
            builder.push(" AND warehouse_type = ").push_bind(warehouse_type.to_string());
        }
        if let Some(active) = filter.is_active {
            builder.push(" AND is_active = ").push_bind(active);
        }

        builder.push(" ORDER BY name");

        if let Some(limit) = filter.limit {
            builder.push(" LIMIT ").push_bind(limit as i64);
        }
        if let Some(offset) = filter.offset {
            builder.push(" OFFSET ").push_bind(offset as i64);
        }

        let rows = builder
            .build_query_as::<WarehouseRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_warehouse).collect::<Result<Vec<_>>>()
    }

    pub async fn delete_warehouse_async(&self, id: i32) -> Result<()> {
        let count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM locations WHERE warehouse_id = $1")
                .bind(id)
                .fetch_one(&self.pool)
                .await
                .map_err(map_db_error)?;

        if count.0 > 0 {
            return Err(CommerceError::ValidationError(
                "Cannot delete warehouse with existing locations".into(),
            ));
        }

        sqlx::query("DELETE FROM warehouses WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(())
    }

    pub async fn count_warehouses_async(&self, filter: WarehouseFilter) -> Result<u64> {
        let mut builder =
            QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM warehouses WHERE 1=1");

        if let Some(warehouse_type) = filter.warehouse_type {
            builder.push(" AND warehouse_type = ").push_bind(warehouse_type.to_string());
        }
        if let Some(active) = filter.is_active {
            builder.push(" AND is_active = ").push_bind(active);
        }

        let row =
            builder.build_query_as::<(i64,)>().fetch_one(&self.pool).await.map_err(map_db_error)?;

        Ok(row.0 as u64)
    }

    pub async fn create_zone_async(&self, input: CreateZone) -> Result<Zone> {
        let now = Utc::now();
        let row: (i32,) = sqlx::query_as(
            r#"
            INSERT INTO warehouse_zones (warehouse_id, code, name, description, is_active, created_at)
            VALUES ($1,$2,$3,$4,true,$5)
            RETURNING id
            "#,
        )
        .bind(input.warehouse_id)
        .bind(&input.code)
        .bind(&input.name)
        .bind(&input.description)
        .bind(now)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(Zone {
            id: row.0,
            warehouse_id: input.warehouse_id,
            code: input.code,
            name: input.name,
            description: input.description,
            is_active: true,
            created_at: now,
        })
    }

    pub async fn get_zone_async(&self, id: i32) -> Result<Option<Zone>> {
        let row = sqlx::query_as::<_, ZoneRow>("SELECT * FROM warehouse_zones WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(row.map(Self::row_to_zone))
    }

    pub async fn get_zones_async(&self, warehouse_id: i32) -> Result<Vec<Zone>> {
        let rows = sqlx::query_as::<_, ZoneRow>(
            "SELECT * FROM warehouse_zones WHERE warehouse_id = $1 ORDER BY code",
        )
        .bind(warehouse_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(rows.into_iter().map(Self::row_to_zone).collect())
    }

    pub async fn update_zone_async(&self, id: i32, input: UpdateZone) -> Result<Zone> {
        sqlx::query(
            r#"
            UPDATE warehouse_zones SET
                name = COALESCE($1, name),
                description = COALESCE($2, description),
                is_active = COALESCE($3, is_active)
            WHERE id = $4
            "#,
        )
        .bind(input.name)
        .bind(input.description)
        .bind(input.is_active)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_zone_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn delete_zone_async(&self, id: i32) -> Result<()> {
        sqlx::query("DELETE FROM warehouse_zones WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(())
    }

    pub async fn create_location_async(&self, input: CreateLocation) -> Result<Location> {
        let now = Utc::now();
        let code = input.code.clone().unwrap_or_else(|| Self::generate_location_code(&input));
        let is_pickable = input.is_pickable.unwrap_or(true);
        let is_receivable = input.is_receivable.unwrap_or(true);

        let row: (i32,) = sqlx::query_as(
            r#"
            INSERT INTO locations (
                warehouse_id, code, location_type, zone, aisle, rack, level, bin,
                max_weight_kg, max_volume_m3, is_pickable, is_receivable, is_active,
                created_at, updated_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,true,$13,$13)
            RETURNING id
            "#,
        )
        .bind(input.warehouse_id)
        .bind(&code)
        .bind(input.location_type.to_string())
        .bind(&input.zone)
        .bind(&input.aisle)
        .bind(&input.rack)
        .bind(&input.level)
        .bind(&input.bin)
        .bind(input.max_weight_kg)
        .bind(input.max_volume_m3)
        .bind(is_pickable)
        .bind(is_receivable)
        .bind(now)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(Location {
            id: row.0,
            warehouse_id: input.warehouse_id,
            code,
            location_type: input.location_type,
            zone: input.zone,
            aisle: input.aisle,
            rack: input.rack,
            level: input.level,
            bin: input.bin,
            max_weight_kg: input.max_weight_kg,
            max_volume_m3: input.max_volume_m3,
            current_weight_kg: None,
            current_volume_m3: None,
            is_pickable,
            is_receivable,
            is_active: true,
            created_at: now,
            updated_at: now,
        })
    }

    pub async fn get_location_async(&self, id: i32) -> Result<Option<Location>> {
        let row = sqlx::query_as::<_, LocationRow>("SELECT * FROM locations WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        row.map(Self::row_to_location).transpose()
    }

    pub async fn get_location_by_code_async(
        &self,
        warehouse_id: i32,
        code: &str,
    ) -> Result<Option<Location>> {
        let row = sqlx::query_as::<_, LocationRow>(
            "SELECT * FROM locations WHERE warehouse_id = $1 AND code = $2",
        )
        .bind(warehouse_id)
        .bind(code)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_location).transpose()
    }

    pub async fn update_location_async(&self, id: i32, input: UpdateLocation) -> Result<Location> {
        let now = Utc::now();

        sqlx::query(
            r#"
            UPDATE locations SET
                location_type = COALESCE($1, location_type),
                zone = COALESCE($2, zone),
                aisle = COALESCE($3, aisle),
                rack = COALESCE($4, rack),
                level = COALESCE($5, level),
                bin = COALESCE($6, bin),
                max_weight_kg = COALESCE($7, max_weight_kg),
                max_volume_m3 = COALESCE($8, max_volume_m3),
                is_pickable = COALESCE($9, is_pickable),
                is_receivable = COALESCE($10, is_receivable),
                is_active = COALESCE($11, is_active),
                updated_at = $12
            WHERE id = $13
            "#,
        )
        .bind(input.location_type.map(|t| t.to_string()))
        .bind(input.zone)
        .bind(input.aisle)
        .bind(input.rack)
        .bind(input.level)
        .bind(input.bin)
        .bind(input.max_weight_kg)
        .bind(input.max_volume_m3)
        .bind(input.is_pickable)
        .bind(input.is_receivable)
        .bind(input.is_active)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_location_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn list_locations_async(&self, filter: LocationFilter) -> Result<Vec<Location>> {
        let mut builder = QueryBuilder::<Postgres>::new("SELECT * FROM locations WHERE 1=1");

        if let Some(warehouse_id) = filter.warehouse_id {
            builder.push(" AND warehouse_id = ").push_bind(warehouse_id);
        }
        if let Some(location_type) = filter.location_type {
            builder.push(" AND location_type = ").push_bind(location_type.to_string());
        }
        if let Some(zone) = filter.zone {
            builder.push(" AND zone = ").push_bind(zone);
        }
        if let Some(aisle) = filter.aisle {
            builder.push(" AND aisle = ").push_bind(aisle);
        }
        if let Some(pickable) = filter.is_pickable {
            builder.push(" AND is_pickable = ").push_bind(pickable);
        }
        if let Some(receivable) = filter.is_receivable {
            builder.push(" AND is_receivable = ").push_bind(receivable);
        }
        if let Some(active) = filter.is_active {
            builder.push(" AND is_active = ").push_bind(active);
        }

        builder.push(" ORDER BY code");

        if let Some(limit) = filter.limit {
            builder.push(" LIMIT ").push_bind(limit as i64);
        }
        if let Some(offset) = filter.offset {
            builder.push(" OFFSET ").push_bind(offset as i64);
        }

        let rows = builder
            .build_query_as::<LocationRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_location).collect::<Result<Vec<_>>>()
    }

    pub async fn delete_location_async(&self, id: i32) -> Result<()> {
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM location_inventory WHERE location_id = $1 AND quantity_on_hand > 0",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        if count.0 > 0 {
            return Err(CommerceError::ValidationError(
                "Cannot delete location with inventory".into(),
            ));
        }

        sqlx::query("DELETE FROM locations WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(())
    }

    pub async fn count_locations_async(&self, filter: LocationFilter) -> Result<u64> {
        let mut builder = QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM locations WHERE 1=1");

        if let Some(warehouse_id) = filter.warehouse_id {
            builder.push(" AND warehouse_id = ").push_bind(warehouse_id);
        }
        if let Some(location_type) = filter.location_type {
            builder.push(" AND location_type = ").push_bind(location_type.to_string());
        }
        if let Some(zone) = filter.zone {
            builder.push(" AND zone = ").push_bind(zone);
        }
        if let Some(aisle) = filter.aisle {
            builder.push(" AND aisle = ").push_bind(aisle);
        }
        if let Some(pickable) = filter.is_pickable {
            builder.push(" AND is_pickable = ").push_bind(pickable);
        }
        if let Some(receivable) = filter.is_receivable {
            builder.push(" AND is_receivable = ").push_bind(receivable);
        }
        if let Some(active) = filter.is_active {
            builder.push(" AND is_active = ").push_bind(active);
        }

        let row =
            builder.build_query_as::<(i64,)>().fetch_one(&self.pool).await.map_err(map_db_error)?;

        Ok(row.0 as u64)
    }

    pub async fn get_locations_for_warehouse_async(
        &self,
        warehouse_id: i32,
    ) -> Result<Vec<Location>> {
        self.list_locations_async(LocationFilter {
            warehouse_id: Some(warehouse_id),
            ..Default::default()
        })
        .await
    }

    pub async fn get_pickable_locations_async(
        &self,
        warehouse_id: i32,
        sku: &str,
    ) -> Result<Vec<Location>> {
        let rows = sqlx::query_as::<_, LocationRow>(
            r#"
            SELECT l.* FROM locations l
            JOIN location_inventory li ON l.id = li.location_id
            WHERE l.warehouse_id = $1 AND l.is_pickable = true AND l.is_active = true
              AND li.sku = $2 AND li.quantity_on_hand > li.quantity_reserved
            ORDER BY l.code
            "#,
        )
        .bind(warehouse_id)
        .bind(sku)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_location).collect::<Result<Vec<_>>>()
    }

    pub async fn get_receivable_locations_async(&self, warehouse_id: i32) -> Result<Vec<Location>> {
        self.list_locations_async(LocationFilter {
            warehouse_id: Some(warehouse_id),
            is_receivable: Some(true),
            is_active: Some(true),
            ..Default::default()
        })
        .await
    }

    pub async fn get_location_inventory_async(
        &self,
        location_id: i32,
    ) -> Result<Vec<LocationInventory>> {
        let rows = sqlx::query_as::<_, LocationInventoryRow>(
            "SELECT * FROM location_inventory WHERE location_id = $1 AND quantity_on_hand > 0",
        )
        .bind(location_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(rows.into_iter().map(Self::row_to_location_inventory).collect())
    }

    pub async fn get_inventory_for_sku_async(
        &self,
        warehouse_id: i32,
        sku: &str,
    ) -> Result<Vec<LocationInventory>> {
        let rows = sqlx::query_as::<_, LocationInventoryRow>(
            r#"
            SELECT li.* FROM location_inventory li
            JOIN locations l ON li.location_id = l.id
            WHERE l.warehouse_id = $1 AND li.sku = $2 AND li.quantity_on_hand > 0
            ORDER BY l.code
            "#,
        )
        .bind(warehouse_id)
        .bind(sku)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(rows.into_iter().map(Self::row_to_location_inventory).collect())
    }

    pub async fn adjust_inventory_async(
        &self,
        input: AdjustLocationInventory,
    ) -> Result<LocationInventory> {
        let now = Utc::now();
        let lot_key = Self::lot_key(input.lot_id);

        let existing = sqlx::query_as::<_, (Decimal, Decimal)>(
            "SELECT quantity_on_hand, quantity_reserved FROM location_inventory WHERE location_id = $1 AND sku = $2 AND lot_id = $3",
        )
        .bind(input.location_id)
        .bind(&input.sku)
        .bind(lot_key)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        let (new_on_hand, reserved) = if let Some((on_hand, reserved)) = existing {
            let new_qty = on_hand + input.quantity;
            if new_qty < Decimal::ZERO {
                return Err(CommerceError::ValidationError(
                    "Adjustment would result in negative inventory".into(),
                ));
            }

            sqlx::query(
                "UPDATE location_inventory SET quantity_on_hand = $1, updated_at = $2 WHERE location_id = $3 AND sku = $4 AND lot_id = $5",
            )
            .bind(new_qty)
            .bind(now)
            .bind(input.location_id)
            .bind(&input.sku)
            .bind(lot_key)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

            (new_qty, reserved)
        } else {
            if input.quantity < Decimal::ZERO {
                return Err(CommerceError::ValidationError(
                    "Cannot create negative inventory".into(),
                ));
            }

            sqlx::query(
                "INSERT INTO location_inventory (location_id, sku, lot_id, quantity_on_hand, quantity_reserved, updated_at)
                 VALUES ($1,$2,$3,$4,0,$5)",
            )
            .bind(input.location_id)
            .bind(&input.sku)
            .bind(lot_key)
            .bind(input.quantity)
            .bind(now)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

            (input.quantity, Decimal::ZERO)
        };

        let movement_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO inventory_movements (
                id, movement_type, to_location_id, sku, lot_id, quantity,
                reference_type, reference_id, reason, performed_by, created_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
            "#,
        )
        .bind(movement_id)
        .bind(MovementType::Adjustment.to_string())
        .bind(input.location_id)
        .bind(&input.sku)
        .bind(input.lot_id)
        .bind(input.quantity)
        .bind(input.reference_type)
        .bind(input.reference_id)
        .bind(input.reason)
        .bind(input.performed_by)
        .bind(now)
        .execute(&self.pool)
        .await
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

    pub async fn move_inventory_async(&self, input: MoveInventory) -> Result<LocationMovement> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let now = Utc::now();
        let lot_key = Self::lot_key(input.lot_id);

        // Lock the source row (`FOR UPDATE`) before the availability guard so
        // concurrent moves of the same (location, sku, lot) serialize and cannot both
        // pass the guard and over-transfer the source into negative stock. SQLite
        // achieves the same via an IMMEDIATE write transaction.
        let (source_on_hand, source_reserved) = sqlx::query_as::<_, (Decimal, Decimal)>(
            "SELECT quantity_on_hand, quantity_reserved FROM location_inventory WHERE location_id = $1 AND sku = $2 AND lot_id = $3 FOR UPDATE",
        )
        .bind(input.from_location_id)
        .bind(&input.sku)
        .bind(lot_key)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .ok_or(CommerceError::NotFound)?;

        let available = source_on_hand - source_reserved;
        if input.quantity > available {
            return Err(CommerceError::ValidationError(format!(
                "Insufficient available quantity. Requested: {}, Available: {}",
                input.quantity, available
            )));
        }

        sqlx::query(
            "UPDATE location_inventory SET quantity_on_hand = quantity_on_hand - $1, updated_at = $2 WHERE location_id = $3 AND sku = $4 AND lot_id = $5",
        )
        .bind(input.quantity)
        .bind(now)
        .bind(input.from_location_id)
        .bind(&input.sku)
        .bind(lot_key)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        sqlx::query(
            r#"
            INSERT INTO location_inventory (location_id, sku, lot_id, quantity_on_hand, quantity_reserved, updated_at)
            VALUES ($1,$2,$3,$4,0,$5)
            ON CONFLICT (location_id, sku, lot_id) DO UPDATE SET
                quantity_on_hand = location_inventory.quantity_on_hand + EXCLUDED.quantity_on_hand,
                updated_at = EXCLUDED.updated_at
            "#,
        )
        .bind(input.to_location_id)
        .bind(&input.sku)
        .bind(lot_key)
        .bind(input.quantity)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let movement_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO inventory_movements (
                id, movement_type, from_location_id, to_location_id, sku, lot_id,
                quantity, reason, performed_by, created_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
            "#,
        )
        .bind(movement_id)
        .bind(MovementType::Transfer.to_string())
        .bind(input.from_location_id)
        .bind(input.to_location_id)
        .bind(&input.sku)
        .bind(input.lot_id)
        .bind(input.quantity)
        .bind(input.reason.clone())
        .bind(input.performed_by.clone())
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;

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

    pub async fn list_location_inventory_async(
        &self,
        filter: LocationInventoryFilter,
    ) -> Result<Vec<LocationInventory>> {
        let mut builder = QueryBuilder::<Postgres>::new("SELECT li.* FROM location_inventory li");

        if filter.warehouse_id.is_some() {
            builder.push(" JOIN locations l ON li.location_id = l.id");
        }

        builder.push(" WHERE 1=1");

        if let Some(location_id) = filter.location_id {
            builder.push(" AND li.location_id = ").push_bind(location_id);
        }
        if let Some(warehouse_id) = filter.warehouse_id {
            builder.push(" AND l.warehouse_id = ").push_bind(warehouse_id);
        }
        if let Some(sku) = filter.sku {
            builder.push(" AND li.sku = ").push_bind(sku);
        }
        if let Some(lot_id) = filter.lot_id {
            builder.push(" AND li.lot_id = ").push_bind(lot_id);
        }
        if filter.has_quantity == Some(true) {
            builder.push(" AND li.quantity_on_hand > 0");
        }

        if let Some(limit) = filter.limit {
            builder.push(" LIMIT ").push_bind(limit as i64);
        }
        if let Some(offset) = filter.offset {
            builder.push(" OFFSET ").push_bind(offset as i64);
        }

        let rows = builder
            .build_query_as::<LocationInventoryRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(rows.into_iter().map(Self::row_to_location_inventory).collect())
    }

    pub async fn get_movements_async(
        &self,
        filter: MovementFilter,
    ) -> Result<Vec<LocationMovement>> {
        let mut builder = QueryBuilder::<Postgres>::new("SELECT m.* FROM inventory_movements m");

        if filter.warehouse_id.is_some() {
            builder.push(
                " LEFT JOIN locations l_from ON m.from_location_id = l_from.id
                  LEFT JOIN locations l_to ON m.to_location_id = l_to.id",
            );
        }

        builder.push(" WHERE 1=1");

        if let Some(warehouse_id) = filter.warehouse_id {
            builder.push(" AND (l_from.warehouse_id = ").push_bind(warehouse_id);
            builder.push(" OR l_to.warehouse_id = ").push_bind(warehouse_id);
            builder.push(")");
        }
        if let Some(location_id) = filter.location_id {
            builder.push(" AND (m.from_location_id = ").push_bind(location_id);
            builder.push(" OR m.to_location_id = ").push_bind(location_id);
            builder.push(")");
        }
        if let Some(sku) = filter.sku {
            builder.push(" AND m.sku = ").push_bind(sku);
        }
        if let Some(lot_id) = filter.lot_id {
            builder.push(" AND m.lot_id = ").push_bind(lot_id);
        }
        if let Some(movement_type) = filter.movement_type {
            builder.push(" AND m.movement_type = ").push_bind(movement_type.to_string());
        }
        if let Some(from_date) = filter.from_date {
            builder.push(" AND m.created_at >= ").push_bind(from_date);
        }
        if let Some(to_date) = filter.to_date {
            builder.push(" AND m.created_at <= ").push_bind(to_date);
        }

        builder.push(" ORDER BY m.created_at DESC");

        if let Some(limit) = filter.limit {
            builder.push(" LIMIT ").push_bind(limit as i64);
        }
        if let Some(offset) = filter.offset {
            builder.push(" OFFSET ").push_bind(offset as i64);
        }

        let rows = builder
            .build_query_as::<LocationMovementRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_movement).collect::<Result<Vec<_>>>()
    }

    pub async fn count_movements_async(&self, filter: MovementFilter) -> Result<u64> {
        let mut builder =
            QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM inventory_movements m");

        if filter.warehouse_id.is_some() {
            builder.push(
                " LEFT JOIN locations l_from ON m.from_location_id = l_from.id
                  LEFT JOIN locations l_to ON m.to_location_id = l_to.id",
            );
        }

        builder.push(" WHERE 1=1");

        if let Some(warehouse_id) = filter.warehouse_id {
            builder.push(" AND (l_from.warehouse_id = ").push_bind(warehouse_id);
            builder.push(" OR l_to.warehouse_id = ").push_bind(warehouse_id);
            builder.push(")");
        }

        if let Some(movement_type) = filter.movement_type {
            builder.push(" AND m.movement_type = ").push_bind(movement_type.to_string());
        }

        let row =
            builder.build_query_as::<(i64,)>().fetch_one(&self.pool).await.map_err(map_db_error)?;

        Ok(row.0 as u64)
    }

    pub async fn create_locations_batch_async(
        &self,
        inputs: Vec<CreateLocation>,
    ) -> Result<BatchResult<Location>> {
        validate_batch_size(&inputs)?;

        let mut result = BatchResult::new();

        for (index, input) in inputs.into_iter().enumerate() {
            match self.create_location_async(input).await {
                Ok(location) => result.record_success(location),
                Err(e) => result.record_failure(index, None, &e),
            }
        }

        Ok(result)
    }

    pub async fn get_locations_batch_async(&self, ids: Vec<i32>) -> Result<Vec<Location>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut builder = QueryBuilder::<Postgres>::new("SELECT * FROM locations WHERE id IN (");
        {
            let mut separated = builder.separated(", ");
            for id in ids {
                separated.push_bind(id);
            }
        }
        builder.push(")");

        let rows = builder
            .build_query_as::<LocationRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_location).collect::<Result<Vec<_>>>()
    }

    // ========================================================================
    // Cycle counts
    // ========================================================================

    fn row_to_cycle_count(row: CycleCountRow) -> Result<CycleCount> {
        let status: CycleCountStatus = row.status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid cycle_count.status '{}': {}",
                row.status, e
            ))
        })?;
        Ok(CycleCount {
            id: row.id,
            warehouse_id: row.warehouse_id,
            location_id: row.location_id,
            status,
            scheduled_date: row.scheduled_date,
            counted_by: row.counted_by,
            lines: Vec::new(),
            created_at: row.created_at,
            updated_at: row.updated_at,
            completed_at: row.completed_at,
        })
    }

    fn row_to_cycle_count_line(row: CycleCountLineRow) -> CycleCountLine {
        let lot_id = if row.lot_id == Uuid::nil() { None } else { Some(row.lot_id) };
        CycleCountLine {
            id: row.id,
            cycle_count_id: row.cycle_count_id,
            sku: row.sku,
            lot_id,
            expected_quantity: row.expected_quantity,
            counted_quantity: row.counted_quantity,
            variance: row.variance,
        }
    }

    async fn load_cycle_count_lines<'e, E>(executor: E, id: Uuid) -> Result<Vec<CycleCountLine>>
    where
        E: sqlx::Executor<'e, Database = Postgres>,
    {
        let rows = sqlx::query_as::<_, CycleCountLineRow>(
            "SELECT * FROM cycle_count_lines WHERE cycle_count_id = $1 ORDER BY sku, lot_id",
        )
        .bind(id)
        .fetch_all(executor)
        .await
        .map_err(map_db_error)?;
        Ok(rows.into_iter().map(Self::row_to_cycle_count_line).collect())
    }

    pub async fn get_cycle_count_async(&self, id: Uuid) -> Result<Option<CycleCount>> {
        let row = sqlx::query_as::<_, CycleCountRow>("SELECT * FROM cycle_counts WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;
        let Some(row) = row else { return Ok(None) };
        let mut cc = Self::row_to_cycle_count(row)?;
        cc.lines = Self::load_cycle_count_lines(&self.pool, id).await?;
        Ok(Some(cc))
    }

    pub async fn create_cycle_count_async(&self, input: CreateCycleCount) -> Result<CycleCount> {
        input.validate()?;
        let id = Uuid::new_v4();
        let now = Utc::now();
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        sqlx::query(
            "INSERT INTO cycle_counts (id, warehouse_id, location_id, status, scheduled_date, counted_by, created_at, updated_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$7)",
        )
        .bind(id)
        .bind(input.warehouse_id)
        .bind(input.location_id)
        .bind(CycleCountStatus::Draft.to_string())
        .bind(input.scheduled_date)
        .bind(&input.counted_by)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        for line in &input.lines {
            sqlx::query(
                "INSERT INTO cycle_count_lines (id, cycle_count_id, sku, lot_id, expected_quantity)
                 VALUES ($1,$2,$3,$4,$5)",
            )
            .bind(Uuid::new_v4())
            .bind(id)
            .bind(&line.sku)
            .bind(Self::lot_key(line.lot_id))
            .bind(line.expected_quantity)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        }
        tx.commit().await.map_err(map_db_error)?;

        self.get_cycle_count_async(id).await?.ok_or_else(|| {
            CommerceError::DatabaseError("Failed to retrieve created cycle count".into())
        })
    }

    pub async fn list_cycle_counts_async(
        &self,
        filter: CycleCountFilter,
    ) -> Result<Vec<CycleCount>> {
        let mut builder = QueryBuilder::<Postgres>::new("SELECT * FROM cycle_counts WHERE 1=1");
        if let Some(warehouse_id) = filter.warehouse_id {
            builder.push(" AND warehouse_id = ").push_bind(warehouse_id);
        }
        if let Some(location_id) = filter.location_id {
            builder.push(" AND location_id = ").push_bind(location_id);
        }
        if let Some(status) = filter.status {
            builder.push(" AND status = ").push_bind(status.to_string());
        }
        let after_cursor = super::parse_after_cursor(filter.after_cursor.as_ref())?;
        if let Some((cursor_created, cursor_id)) = after_cursor {
            // Keyset cursor: (created_at, id) for stable DESC ordering
            builder
                .push(" AND (created_at < ")
                .push_bind(cursor_created)
                .push(" OR (created_at = ")
                .push_bind(cursor_created)
                .push(" AND id < ")
                .push_bind(cursor_id)
                .push("))");
        }
        builder.push(" ORDER BY created_at DESC, id DESC");
        builder.push(" LIMIT ").push_bind(i64::from(filter.limit.unwrap_or(50)));
        // Offset pagination applies only in non-cursor mode.
        let offset = if after_cursor.is_none() { i64::from(filter.offset.unwrap_or(0)) } else { 0 };
        builder.push(" OFFSET ").push_bind(offset);

        let rows = builder
            .build_query_as::<CycleCountRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        let mut counts =
            rows.into_iter().map(Self::row_to_cycle_count).collect::<Result<Vec<_>>>()?;
        for cc in &mut counts {
            cc.lines = Self::load_cycle_count_lines(&self.pool, cc.id).await?;
        }
        Ok(counts)
    }

    async fn transition_cycle_count_async(
        &self,
        id: Uuid,
        next: CycleCountStatus,
    ) -> Result<CycleCount> {
        let current = self.get_cycle_count_async(id).await?.ok_or(CommerceError::NotFound)?;
        if !current.status.can_transition_to(next) {
            return Err(CommerceError::ValidationError(format!(
                "cannot transition cycle count from {} to {next}",
                current.status
            )));
        }
        sqlx::query("UPDATE cycle_counts SET status = $1, updated_at = $2 WHERE id = $3")
            .bind(next.to_string())
            .bind(Utc::now())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;
        self.get_cycle_count_async(id).await?.ok_or_else(|| {
            CommerceError::DatabaseError("Failed to retrieve updated cycle count".into())
        })
    }

    /// Start a draft cycle count (`draft` → `in_progress`) (async).
    pub async fn start_cycle_count_async(&self, id: Uuid) -> Result<CycleCount> {
        self.transition_cycle_count_async(id, CycleCountStatus::InProgress).await
    }

    /// Cancel a draft or in-progress cycle count (async). No adjustments are applied.
    pub async fn cancel_cycle_count_async(&self, id: Uuid) -> Result<CycleCount> {
        self.transition_cycle_count_async(id, CycleCountStatus::Cancelled).await
    }

    pub async fn record_cycle_counts_async(
        &self,
        id: Uuid,
        counts: Vec<RecordCycleCountLine>,
    ) -> Result<CycleCount> {
        for count in &counts {
            count.validate()?;
        }
        let existing = self.get_cycle_count_async(id).await?.ok_or(CommerceError::NotFound)?;
        if existing.status != CycleCountStatus::InProgress {
            return Err(CommerceError::ValidationError(format!(
                "counts can only be recorded on an in_progress cycle count (status: {})",
                existing.status
            )));
        }

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        for count in &counts {
            let line = existing
                .lines
                .iter()
                .find(|l| l.sku == count.sku && l.lot_id == count.lot_id)
                .ok_or_else(|| {
                    CommerceError::ValidationError(format!(
                        "cycle count has no line for sku {} (lot: {:?})",
                        count.sku, count.lot_id
                    ))
                })?;
            let variance = count.counted_quantity - line.expected_quantity;
            sqlx::query(
                "UPDATE cycle_count_lines SET counted_quantity = $1, variance = $2
                 WHERE cycle_count_id = $3 AND sku = $4 AND lot_id = $5",
            )
            .bind(count.counted_quantity)
            .bind(variance)
            .bind(id)
            .bind(&count.sku)
            .bind(Self::lot_key(count.lot_id))
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        }
        sqlx::query("UPDATE cycle_counts SET updated_at = $1 WHERE id = $2")
            .bind(Utc::now())
            .bind(id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        tx.commit().await.map_err(map_db_error)?;

        self.get_cycle_count_async(id).await?.ok_or_else(|| {
            CommerceError::DatabaseError("Failed to retrieve updated cycle count".into())
        })
    }

    pub async fn complete_cycle_count_async(&self, id: Uuid) -> Result<CycleCount> {
        let existing = self.get_cycle_count_async(id).await?.ok_or(CommerceError::NotFound)?;
        if !existing.status.can_transition_to(CycleCountStatus::Completed) {
            return Err(CommerceError::ValidationError(format!(
                "cannot complete cycle count in status {}",
                existing.status
            )));
        }

        let now = Utc::now();
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        for line in &existing.lines {
            let counted = line.counted_quantity.ok_or_else(|| {
                CommerceError::ValidationError(format!(
                    "cycle count line for sku {} has no recorded count",
                    line.sku
                ))
            })?;
            let variance = counted - line.expected_quantity;
            sqlx::query("UPDATE cycle_count_lines SET variance = $1 WHERE id = $2")
                .bind(variance)
                .bind(line.id)
                .execute(tx.as_mut())
                .await
                .map_err(map_db_error)?;
            if variance == Decimal::ZERO {
                continue;
            }
            let location_id = existing.location_id.ok_or_else(|| {
                CommerceError::ValidationError(
                    "cycle count has no location_id; cannot apply variance adjustments".into(),
                )
            })?;
            let lot_key = Self::lot_key(line.lot_id);

            // Adjust location_inventory (same path as adjust_inventory), with a
            // row lock so concurrent writers serialize.
            let current: Option<Decimal> = sqlx::query_scalar(
                "SELECT quantity_on_hand FROM location_inventory
                 WHERE location_id = $1 AND sku = $2 AND lot_id = $3 FOR UPDATE",
            )
            .bind(location_id)
            .bind(&line.sku)
            .bind(lot_key)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(map_db_error)?;

            let new_qty = current.unwrap_or(Decimal::ZERO) + variance;
            if new_qty < Decimal::ZERO {
                return Err(CommerceError::ValidationError(format!(
                    "cycle count adjustment for sku {} would result in negative inventory",
                    line.sku
                )));
            }
            if current.is_some() {
                sqlx::query(
                    "UPDATE location_inventory SET quantity_on_hand = $1, updated_at = $2
                     WHERE location_id = $3 AND sku = $4 AND lot_id = $5",
                )
                .bind(new_qty)
                .bind(now)
                .bind(location_id)
                .bind(&line.sku)
                .bind(lot_key)
                .execute(tx.as_mut())
                .await
                .map_err(map_db_error)?;
            } else {
                sqlx::query(
                    "INSERT INTO location_inventory (location_id, sku, lot_id, quantity_on_hand, quantity_reserved, updated_at)
                     VALUES ($1,$2,$3,$4,0,$5)",
                )
                .bind(location_id)
                .bind(&line.sku)
                .bind(lot_key)
                .bind(new_qty)
                .bind(now)
                .execute(tx.as_mut())
                .await
                .map_err(map_db_error)?;
            }

            sqlx::query(
                "INSERT INTO inventory_movements (
                    id, movement_type, to_location_id, sku, lot_id, quantity,
                    reference_type, reference_id, reason, performed_by, created_at
                ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)",
            )
            .bind(Uuid::new_v4())
            .bind(MovementType::CycleCount.to_string())
            .bind(location_id)
            .bind(&line.sku)
            .bind(line.lot_id)
            .bind(variance)
            .bind("cycle_count")
            .bind(existing.id)
            .bind("Cycle count variance")
            .bind(&existing.counted_by)
            .bind(now)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        }

        sqlx::query(
            "UPDATE cycle_counts SET status = $1, updated_at = $2, completed_at = $2 WHERE id = $3",
        )
        .bind(CycleCountStatus::Completed.to_string())
        .bind(now)
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        tx.commit().await.map_err(map_db_error)?;

        self.get_cycle_count_async(id).await?.ok_or_else(|| {
            CommerceError::DatabaseError("Failed to retrieve completed cycle count".into())
        })
    }
}

impl WarehouseRepository for PgWarehouseRepository {
    fn create_warehouse(&self, input: CreateWarehouse) -> Result<Warehouse> {
        block_on(self.create_warehouse_async(input))
    }

    fn get_warehouse(&self, id: i32) -> Result<Option<Warehouse>> {
        block_on(self.get_warehouse_async(id))
    }

    fn get_warehouse_by_code(&self, code: &str) -> Result<Option<Warehouse>> {
        block_on(self.get_warehouse_by_code_async(code))
    }

    fn update_warehouse(&self, id: i32, input: UpdateWarehouse) -> Result<Warehouse> {
        block_on(self.update_warehouse_async(id, input))
    }

    fn list_warehouses(&self, filter: WarehouseFilter) -> Result<Vec<Warehouse>> {
        block_on(self.list_warehouses_async(filter))
    }

    fn delete_warehouse(&self, id: i32) -> Result<()> {
        block_on(self.delete_warehouse_async(id))
    }

    fn count_warehouses(&self, filter: WarehouseFilter) -> Result<u64> {
        block_on(self.count_warehouses_async(filter))
    }

    fn create_zone(&self, input: CreateZone) -> Result<Zone> {
        block_on(self.create_zone_async(input))
    }

    fn get_zone(&self, id: i32) -> Result<Option<Zone>> {
        block_on(self.get_zone_async(id))
    }

    fn get_zones(&self, warehouse_id: i32) -> Result<Vec<Zone>> {
        block_on(self.get_zones_async(warehouse_id))
    }

    fn update_zone(&self, id: i32, input: UpdateZone) -> Result<Zone> {
        block_on(self.update_zone_async(id, input))
    }

    fn delete_zone(&self, id: i32) -> Result<()> {
        block_on(self.delete_zone_async(id))
    }

    fn create_location(&self, input: CreateLocation) -> Result<Location> {
        block_on(self.create_location_async(input))
    }

    fn get_location(&self, id: i32) -> Result<Option<Location>> {
        block_on(self.get_location_async(id))
    }

    fn get_location_by_code(&self, warehouse_id: i32, code: &str) -> Result<Option<Location>> {
        block_on(self.get_location_by_code_async(warehouse_id, code))
    }

    fn update_location(&self, id: i32, input: UpdateLocation) -> Result<Location> {
        block_on(self.update_location_async(id, input))
    }

    fn list_locations(&self, filter: LocationFilter) -> Result<Vec<Location>> {
        block_on(self.list_locations_async(filter))
    }

    fn delete_location(&self, id: i32) -> Result<()> {
        block_on(self.delete_location_async(id))
    }

    fn count_locations(&self, filter: LocationFilter) -> Result<u64> {
        block_on(self.count_locations_async(filter))
    }

    fn get_locations_for_warehouse(&self, warehouse_id: i32) -> Result<Vec<Location>> {
        block_on(self.get_locations_for_warehouse_async(warehouse_id))
    }

    fn get_pickable_locations(&self, warehouse_id: i32, sku: &str) -> Result<Vec<Location>> {
        block_on(self.get_pickable_locations_async(warehouse_id, sku))
    }

    fn get_receivable_locations(&self, warehouse_id: i32) -> Result<Vec<Location>> {
        block_on(self.get_receivable_locations_async(warehouse_id))
    }

    fn get_location_inventory(&self, location_id: i32) -> Result<Vec<LocationInventory>> {
        block_on(self.get_location_inventory_async(location_id))
    }

    fn get_inventory_for_sku(
        &self,
        warehouse_id: i32,
        sku: &str,
    ) -> Result<Vec<LocationInventory>> {
        block_on(self.get_inventory_for_sku_async(warehouse_id, sku))
    }

    fn adjust_inventory(&self, input: AdjustLocationInventory) -> Result<LocationInventory> {
        block_on(self.adjust_inventory_async(input))
    }

    fn move_inventory(&self, input: MoveInventory) -> Result<LocationMovement> {
        block_on(self.move_inventory_async(input))
    }

    fn list_location_inventory(
        &self,
        filter: LocationInventoryFilter,
    ) -> Result<Vec<LocationInventory>> {
        block_on(self.list_location_inventory_async(filter))
    }

    fn get_movements(&self, filter: MovementFilter) -> Result<Vec<LocationMovement>> {
        block_on(self.get_movements_async(filter))
    }

    fn count_movements(&self, filter: MovementFilter) -> Result<u64> {
        block_on(self.count_movements_async(filter))
    }

    fn create_locations_batch(&self, inputs: Vec<CreateLocation>) -> Result<BatchResult<Location>> {
        block_on(self.create_locations_batch_async(inputs))
    }

    fn get_locations_batch(&self, ids: Vec<i32>) -> Result<Vec<Location>> {
        block_on(self.get_locations_batch_async(ids))
    }

    fn create_cycle_count(&self, input: CreateCycleCount) -> Result<CycleCount> {
        block_on(self.create_cycle_count_async(input))
    }

    fn get_cycle_count(&self, id: Uuid) -> Result<Option<CycleCount>> {
        block_on(self.get_cycle_count_async(id))
    }

    fn list_cycle_counts(&self, filter: CycleCountFilter) -> Result<Vec<CycleCount>> {
        block_on(self.list_cycle_counts_async(filter))
    }

    fn start_cycle_count(&self, id: Uuid) -> Result<CycleCount> {
        block_on(self.start_cycle_count_async(id))
    }

    fn record_cycle_counts(
        &self,
        id: Uuid,
        counts: Vec<RecordCycleCountLine>,
    ) -> Result<CycleCount> {
        block_on(self.record_cycle_counts_async(id, counts))
    }

    fn complete_cycle_count(&self, id: Uuid) -> Result<CycleCount> {
        block_on(self.complete_cycle_count_async(id))
    }

    fn cancel_cycle_count(&self, id: Uuid) -> Result<CycleCount> {
        block_on(self.cancel_cycle_count_async(id))
    }
}
