//! SQLite inventory repository implementation

use super::{
    build_in_clause, i64_params, map_db_error, params_refs, string_params,
    parse_datetime_opt_row, parse_datetime_row, parse_decimal_opt_row, parse_decimal_row,
    parse_decimal_strict, parse_enum_row, parse_uuid,
};
use chrono::{DateTime, Utc};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rust_decimal::Decimal;
use stateset_core::{
    validate_batch_size, validate_quantity, validate_sku, AdjustInventory, BatchResult, CommerceError,
    CreateInventoryItem, InventoryBalance, InventoryFilter, InventoryItem, InventoryRepository,
    InventoryReservation, InventoryTransaction, LocationStock, ReservationStatus, ReserveInventory,
    Result, StockLevel, TransactionType,
};
use uuid::Uuid;

/// SQLite implementation of InventoryRepository
pub struct SqliteInventoryRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteInventoryRepository {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn expire_reservation_in_tx(
        conn: &rusqlite::Connection,
        reservation_id: Uuid,
        item_id: i64,
        location_id: i32,
        quantity: Decimal,
        now: DateTime<Utc>,
    ) -> Result<()> {
        let current_version: i32 = conn
            .query_row(
                "SELECT version FROM inventory_balances WHERE item_id = ? AND location_id = ?",
                rusqlite::params![item_id, location_id],
                |row| row.get(0),
            )
            .map_err(map_db_error)?;

        let rows_affected = conn
            .execute(
                "UPDATE inventory_balances SET quantity_allocated = quantity_allocated - ?,
                 quantity_available = quantity_available + ?, version = version + 1, updated_at = ?
                 WHERE item_id = ? AND location_id = ? AND version = ?",
                rusqlite::params![
                    quantity.to_string(),
                    quantity.to_string(),
                    now.to_rfc3339(),
                    item_id,
                    location_id,
                    current_version
                ],
            )
            .map_err(map_db_error)?;

        if rows_affected == 0 {
            return Err(CommerceError::VersionConflict {
                entity: "inventory_balance".to_string(),
                id: format!("{}:{}", item_id, location_id),
                expected_version: current_version,
            });
        }

        conn.execute(
            "UPDATE inventory_reservations SET status = 'expired' WHERE id = ?",
            [reservation_id.to_string()],
        )
        .map_err(map_db_error)?;

        Ok(())
    }

    fn expire_reservations_for_item_in_tx(
        conn: &rusqlite::Connection,
        item_id: i64,
        location_id: i32,
        now: DateTime<Utc>,
    ) -> Result<()> {
        let mut stmt = conn
            .prepare(
                "SELECT id, quantity FROM inventory_reservations
                 WHERE item_id = ? AND location_id = ?
                   AND status IN ('pending', 'confirmed', 'allocated')
                   AND expires_at IS NOT NULL AND expires_at < ?",
            )
            .map_err(map_db_error)?;
        let mut rows = stmt
            .query(rusqlite::params![item_id, location_id, now.to_rfc3339()])
            .map_err(map_db_error)?;

        while let Some(row) = rows.next().map_err(map_db_error)? {
            let id_str: String = row.get(0).map_err(map_db_error)?;
            let qty_str: String = row.get(1).map_err(map_db_error)?;
            let reservation_id = parse_uuid(&id_str, "inventory_reservation", "id")?;
            let quantity = parse_decimal_strict(&qty_str, "inventory_reservation", "quantity")?;

            Self::expire_reservation_in_tx(
                conn,
                reservation_id,
                item_id,
                location_id,
                quantity,
                now,
            )?;
        }

        Ok(())
    }
}

impl InventoryRepository for SqliteInventoryRepository {
    fn create_item(&self, input: CreateInventoryItem) -> Result<InventoryItem> {
        // Validate SKU format
        validate_sku(&input.sku)?;

        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let now = Utc::now();
        let sku = input.sku.clone();
        let name = input.name.clone();
        let description = input.description.clone();
        let unit_of_measure = input.unit_of_measure.clone().unwrap_or_else(|| "EA".to_string());

        // Check SKU uniqueness
        let exists: i32 = tx
            .query_row(
                "SELECT COUNT(*) FROM inventory_items WHERE sku = ?",
                [&sku],
                |row| row.get(0),
            )
            .map_err(map_db_error)?;

        if exists > 0 {
            return Err(CommerceError::DuplicateSku(sku));
        }

        tx.execute(
            "INSERT INTO inventory_items (sku, name, description, unit_of_measure, is_active, created_at, updated_at)
             VALUES (?, ?, ?, ?, 1, ?, ?)",
            rusqlite::params![
                &sku,
                &name,
                &description,
                &unit_of_measure,
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;

        let item_id = tx.last_insert_rowid();

        // Create initial balance if quantity provided
        let location_id = input.location_id.unwrap_or(1);
        let initial_qty = input.initial_quantity.unwrap_or_default();

        tx.execute(
            "INSERT INTO inventory_balances (item_id, location_id, quantity_on_hand, quantity_allocated, quantity_available, reorder_point, safety_stock, updated_at)
             VALUES (?, ?, ?, '0', ?, ?, ?, ?)",
            rusqlite::params![
                item_id,
                location_id,
                initial_qty.to_string(),
                initial_qty.to_string(),
                input.reorder_point.map(|d| d.to_string()),
                input.safety_stock.map(|d| d.to_string()),
                now.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;

        // Record initial transaction if quantity > 0
        if initial_qty > Decimal::ZERO {
            tx.execute(
                "INSERT INTO inventory_transactions (item_id, location_id, transaction_type, quantity, reason, created_at)
                 VALUES (?, ?, 'receipt', ?, 'Initial stock', ?)",
                rusqlite::params![item_id, location_id, initial_qty.to_string(), now.to_rfc3339()],
            )
            .map_err(map_db_error)?;
        }

        tx.commit().map_err(map_db_error)?;

        Ok(InventoryItem {
            id: item_id,
            sku,
            name,
            description,
            unit_of_measure,
            is_active: true,
            created_at: now,
            updated_at: now,
        })
    }

    fn get_item(&self, id: i64) -> Result<Option<InventoryItem>> {
        let conn = self.conn()?;
        let result = conn.query_row(
            "SELECT * FROM inventory_items WHERE id = ?",
            [id],
            |row| {
                Ok(InventoryItem {
                    id: row.get("id")?,
                    sku: row.get("sku")?,
                    name: row.get("name")?,
                    description: row.get("description")?,
                    unit_of_measure: row.get("unit_of_measure")?,
                    is_active: row.get::<_, i32>("is_active")? != 0,
                    created_at: parse_datetime_row(&row.get::<_, String>("created_at")?, "inventory_item", "created_at")?,
                    updated_at: parse_datetime_row(&row.get::<_, String>("updated_at")?, "inventory_item", "updated_at")?,
                })
            },
        );

        match result {
            Ok(item) => Ok(Some(item)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn get_item_by_sku(&self, sku: &str) -> Result<Option<InventoryItem>> {
        let conn = self.conn()?;
        let result = conn.query_row(
            "SELECT * FROM inventory_items WHERE sku = ?",
            [sku],
            |row| {
                Ok(InventoryItem {
                    id: row.get("id")?,
                    sku: row.get("sku")?,
                    name: row.get("name")?,
                    description: row.get("description")?,
                    unit_of_measure: row.get("unit_of_measure")?,
                    is_active: row.get::<_, i32>("is_active")? != 0,
                    created_at: parse_datetime_row(&row.get::<_, String>("created_at")?, "inventory_item", "created_at")?,
                    updated_at: parse_datetime_row(&row.get::<_, String>("updated_at")?, "inventory_item", "updated_at")?,
                })
            },
        );

        match result {
            Ok(item) => Ok(Some(item)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn get_stock(&self, sku: &str) -> Result<Option<StockLevel>> {
        let conn = self.conn()?;

        // Get item directly with this connection
        let item_result = conn.query_row(
            "SELECT * FROM inventory_items WHERE sku = ?",
            [sku],
            |row| {
                Ok(InventoryItem {
                    id: row.get("id")?,
                    sku: row.get("sku")?,
                    name: row.get("name")?,
                    description: row.get("description")?,
                    unit_of_measure: row.get("unit_of_measure")?,
                    is_active: row.get::<_, i32>("is_active")? != 0,
                    created_at: parse_datetime_row(&row.get::<_, String>("created_at")?, "inventory_item", "created_at")?,
                    updated_at: parse_datetime_row(&row.get::<_, String>("updated_at")?, "inventory_item", "updated_at")?,
                })
            },
        );

        let item = match item_result {
            Ok(item) => item,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(e) => return Err(map_db_error(e)),
        };

        // Get all balances for item
        let mut stmt = conn
            .prepare(
                "SELECT b.*, l.name as location_name
                 FROM inventory_balances b
                 LEFT JOIN inventory_locations l ON b.location_id = l.id
                 WHERE b.item_id = ?",
            )
            .map_err(map_db_error)?;

        let locations: Vec<LocationStock> = stmt
            .query_map([item.id], |row| {
                Ok(LocationStock {
                    location_id: row.get("location_id")?,
                    location_name: row.get("location_name")?,
                    on_hand: parse_decimal_row(&row.get::<_, String>("quantity_on_hand")?, "inventory_balance", "quantity_on_hand")?,
                    allocated: parse_decimal_row(&row.get::<_, String>("quantity_allocated")?, "inventory_balance", "quantity_allocated")?,
                    available: parse_decimal_row(&row.get::<_, String>("quantity_available")?, "inventory_balance", "quantity_available")?,
                })
            })
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        let total_on_hand: Decimal = locations.iter().map(|l| l.on_hand).sum();
        let total_allocated: Decimal = locations.iter().map(|l| l.allocated).sum();
        let total_available: Decimal = locations.iter().map(|l| l.available).sum();

        Ok(Some(StockLevel {
            sku: item.sku,
            name: item.name,
            total_on_hand,
            total_allocated,
            total_available,
            locations,
        }))
    }

    fn get_balance(&self, item_id: i64, location_id: i32) -> Result<Option<InventoryBalance>> {
        let conn = self.conn()?;
        let result = conn.query_row(
            "SELECT * FROM inventory_balances WHERE item_id = ? AND location_id = ?",
            rusqlite::params![item_id, location_id],
            |row| {
                Ok(InventoryBalance {
                    id: row.get("id")?,
                    item_id: row.get("item_id")?,
                    location_id: row.get("location_id")?,
                    quantity_on_hand: parse_decimal_row(&row.get::<_, String>("quantity_on_hand")?, "inventory_balance", "quantity_on_hand")?,
                    quantity_allocated: parse_decimal_row(&row.get::<_, String>("quantity_allocated")?, "inventory_balance", "quantity_allocated")?,
                    quantity_available: parse_decimal_row(&row.get::<_, String>("quantity_available")?, "inventory_balance", "quantity_available")?,
                    reorder_point: parse_decimal_opt_row(row.get::<_, Option<String>>("reorder_point")?, "inventory_balance", "reorder_point")?,
                    safety_stock: parse_decimal_opt_row(row.get::<_, Option<String>>("safety_stock")?, "inventory_balance", "safety_stock")?,
                    version: row.get("version")?,
                    last_counted_at: parse_datetime_opt_row(row.get::<_, Option<String>>("last_counted_at")?, "inventory_balance", "last_counted_at")?,
                    updated_at: parse_datetime_row(&row.get::<_, String>("updated_at")?, "inventory_balance", "updated_at")?,
                })
            },
        );

        match result {
            Ok(balance) => Ok(Some(balance)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn adjust(&self, input: AdjustInventory) -> Result<InventoryTransaction> {
        // Validate SKU format
        validate_sku(&input.sku)?;

        // Validate that adjustment quantity is not zero
        if input.quantity.is_zero() {
            return Err(CommerceError::ValidationError(
                "Adjustment quantity cannot be zero".into()
            ));
        }

        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let now = Utc::now();

        // Get item directly with this connection
        let item = tx.query_row(
            "SELECT * FROM inventory_items WHERE sku = ?",
            [&input.sku],
            |row| {
                Ok(InventoryItem {
                    id: row.get("id")?,
                    sku: row.get("sku")?,
                    name: row.get("name")?,
                    description: row.get("description")?,
                    unit_of_measure: row.get("unit_of_measure")?,
                    is_active: row.get::<_, i32>("is_active")? != 0,
                    created_at: parse_datetime_row(&row.get::<_, String>("created_at")?, "inventory_item", "created_at")?,
                    updated_at: parse_datetime_row(&row.get::<_, String>("updated_at")?, "inventory_item", "updated_at")?,
                })
            },
        ).map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => CommerceError::InventoryItemNotFound(input.sku.clone()),
            e => map_db_error(e),
        })?;

        let location_id = input.location_id.unwrap_or(1);

        // Get or create balance directly with this connection
        let balance_result = tx.query_row(
            "SELECT * FROM inventory_balances WHERE item_id = ? AND location_id = ?",
            rusqlite::params![item.id, location_id],
            |row| {
                Ok(InventoryBalance {
                    id: row.get("id")?,
                    item_id: row.get("item_id")?,
                    location_id: row.get("location_id")?,
                    quantity_on_hand: parse_decimal_row(&row.get::<_, String>("quantity_on_hand")?, "inventory_balance", "quantity_on_hand")?,
                    quantity_allocated: parse_decimal_row(&row.get::<_, String>("quantity_allocated")?, "inventory_balance", "quantity_allocated")?,
                    quantity_available: parse_decimal_row(&row.get::<_, String>("quantity_available")?, "inventory_balance", "quantity_available")?,
                    reorder_point: parse_decimal_opt_row(row.get::<_, Option<String>>("reorder_point")?, "inventory_balance", "reorder_point")?,
                    safety_stock: parse_decimal_opt_row(row.get::<_, Option<String>>("safety_stock")?, "inventory_balance", "safety_stock")?,
                    version: row.get("version")?,
                    last_counted_at: parse_datetime_opt_row(row.get::<_, Option<String>>("last_counted_at")?, "inventory_balance", "last_counted_at")?,
                    updated_at: parse_datetime_row(&row.get::<_, String>("updated_at")?, "inventory_balance", "updated_at")?,
                })
            },
        );

        let balance = match balance_result {
            Ok(b) => b,
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                tx.execute(
                    "INSERT INTO inventory_balances (item_id, location_id, quantity_on_hand, quantity_allocated, quantity_available, updated_at)
                     VALUES (?, ?, '0', '0', '0', ?)",
                    rusqlite::params![item.id, location_id, now.to_rfc3339()],
                )
                .map_err(map_db_error)?;

                // Query the newly created balance
                tx.query_row(
                    "SELECT * FROM inventory_balances WHERE item_id = ? AND location_id = ?",
                    rusqlite::params![item.id, location_id],
                    |row| {
                        Ok(InventoryBalance {
                            id: row.get("id")?,
                            item_id: row.get("item_id")?,
                            location_id: row.get("location_id")?,
                            quantity_on_hand: parse_decimal_row(&row.get::<_, String>("quantity_on_hand")?, "inventory_balance", "quantity_on_hand")?,
                            quantity_allocated: parse_decimal_row(&row.get::<_, String>("quantity_allocated")?, "inventory_balance", "quantity_allocated")?,
                            quantity_available: parse_decimal_row(&row.get::<_, String>("quantity_available")?, "inventory_balance", "quantity_available")?,
                            reorder_point: parse_decimal_opt_row(row.get::<_, Option<String>>("reorder_point")?, "inventory_balance", "reorder_point")?,
                            safety_stock: parse_decimal_opt_row(row.get::<_, Option<String>>("safety_stock")?, "inventory_balance", "safety_stock")?,
                            version: row.get("version")?,
                            last_counted_at: parse_datetime_opt_row(row.get::<_, Option<String>>("last_counted_at")?, "inventory_balance", "last_counted_at")?,
                            updated_at: parse_datetime_row(&row.get::<_, String>("updated_at")?, "inventory_balance", "updated_at")?,
                        })
                    },
                ).map_err(map_db_error)?
            }
            Err(e) => return Err(map_db_error(e)),
        };

        // Calculate new quantities
        let new_on_hand = balance.quantity_on_hand + input.quantity;
        let new_available = new_on_hand - balance.quantity_allocated;

        if new_on_hand < Decimal::ZERO {
            return Err(CommerceError::InsufficientStock {
                sku: input.sku.clone(),
                requested: input.quantity.abs().to_string(),
                available: balance.quantity_on_hand.to_string(),
            });
        }
        if new_available < Decimal::ZERO {
            return Err(CommerceError::InsufficientStock {
                sku: input.sku.clone(),
                requested: input.quantity.abs().to_string(),
                available: balance.quantity_available.to_string(),
            });
        }

        // Update balance with optimistic locking
        let current_version = balance.version;
        let rows_affected = tx.execute(
            "UPDATE inventory_balances SET quantity_on_hand = ?, quantity_available = ?, version = version + 1, updated_at = ?
             WHERE item_id = ? AND location_id = ? AND version = ?",
            rusqlite::params![
                new_on_hand.to_string(),
                new_available.to_string(),
                now.to_rfc3339(),
                item.id,
                location_id,
                current_version
            ],
        )
        .map_err(map_db_error)?;

        if rows_affected == 0 {
            return Err(CommerceError::VersionConflict {
                entity: "inventory_balance".to_string(),
                id: format!("{}:{}", item.id, location_id),
                expected_version: current_version,
            });
        }

        // Record transaction
        let tx_type = if input.quantity >= Decimal::ZERO { "receipt" } else { "adjustment" };
        tx.execute(
            "INSERT INTO inventory_transactions (item_id, location_id, transaction_type, quantity, reference_type, reference_id, reason, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                item.id,
                location_id,
                tx_type,
                input.quantity.to_string(),
                input.reference_type,
                input.reference_id,
                input.reason,
                now.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;

        let tx_id = tx.last_insert_rowid();
        let transaction = InventoryTransaction {
            id: tx_id,
            item_id: item.id,
            location_id,
            transaction_type: if input.quantity >= Decimal::ZERO {
                TransactionType::Receipt
            } else {
                TransactionType::Adjustment
            },
            quantity: input.quantity,
            reference_type: input.reference_type,
            reference_id: input.reference_id,
            reason: Some(input.reason),
            created_by: None,
            created_at: now,
        };

        tx.commit().map_err(map_db_error)?;

        Ok(transaction)
    }

    fn reserve(&self, input: ReserveInventory) -> Result<InventoryReservation> {
        // Validate quantity is positive to prevent negative reservations
        // that could inflate available stock
        validate_quantity(input.quantity)?;

        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let now = Utc::now();

        // Get item directly with this connection
        let item = tx.query_row(
            "SELECT * FROM inventory_items WHERE sku = ?",
            [&input.sku],
            |row| {
                Ok(InventoryItem {
                    id: row.get("id")?,
                    sku: row.get("sku")?,
                    name: row.get("name")?,
                    description: row.get("description")?,
                    unit_of_measure: row.get("unit_of_measure")?,
                    is_active: row.get::<_, i32>("is_active")? != 0,
                    created_at: parse_datetime_row(&row.get::<_, String>("created_at")?, "inventory_item", "created_at")?,
                    updated_at: parse_datetime_row(&row.get::<_, String>("updated_at")?, "inventory_item", "updated_at")?,
                })
            },
        ).map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => CommerceError::InventoryItemNotFound(input.sku.clone()),
            e => map_db_error(e),
        })?;

        let location_id = input.location_id.unwrap_or(1);

        Self::expire_reservations_for_item_in_tx(&tx, item.id, location_id, now)?;

        // Get balance directly with this connection
        let balance = tx.query_row(
            "SELECT * FROM inventory_balances WHERE item_id = ? AND location_id = ?",
            rusqlite::params![item.id, location_id],
            |row| {
                Ok(InventoryBalance {
                    id: row.get("id")?,
                    item_id: row.get("item_id")?,
                    location_id: row.get("location_id")?,
                    quantity_on_hand: parse_decimal_row(&row.get::<_, String>("quantity_on_hand")?, "inventory_balance", "quantity_on_hand")?,
                    quantity_allocated: parse_decimal_row(&row.get::<_, String>("quantity_allocated")?, "inventory_balance", "quantity_allocated")?,
                    quantity_available: parse_decimal_row(&row.get::<_, String>("quantity_available")?, "inventory_balance", "quantity_available")?,
                    reorder_point: parse_decimal_opt_row(row.get::<_, Option<String>>("reorder_point")?, "inventory_balance", "reorder_point")?,
                    safety_stock: parse_decimal_opt_row(row.get::<_, Option<String>>("safety_stock")?, "inventory_balance", "safety_stock")?,
                    version: row.get("version")?,
                    last_counted_at: parse_datetime_opt_row(row.get::<_, Option<String>>("last_counted_at")?, "inventory_balance", "last_counted_at")?,
                    updated_at: parse_datetime_row(&row.get::<_, String>("updated_at")?, "inventory_balance", "updated_at")?,
                })
            },
        ).map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => CommerceError::InventoryItemNotFound(input.sku.clone()),
            e => map_db_error(e),
        })?;

        // Check availability
        if balance.quantity_available < input.quantity {
            return Err(CommerceError::InsufficientStock {
                sku: input.sku.clone(),
                requested: input.quantity.to_string(),
                available: balance.quantity_available.to_string(),
            });
        }

        let reservation_id = Uuid::new_v4();
        let expires_at = input.expires_in_seconds.map(|secs| {
            now + chrono::Duration::seconds(secs)
        });

        // Create reservation
        tx.execute(
            "INSERT INTO inventory_reservations (id, item_id, location_id, quantity, status, reference_type, reference_id, expires_at, created_at)
             VALUES (?, ?, ?, ?, 'pending', ?, ?, ?, ?)",
            rusqlite::params![
                reservation_id.to_string(),
                item.id,
                location_id,
                input.quantity.to_string(),
                input.reference_type,
                input.reference_id,
                expires_at.map(|t| t.to_rfc3339()),
                now.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;

        // Update balance with optimistic locking
        let new_allocated = balance.quantity_allocated + input.quantity;
        let new_available = balance.quantity_on_hand - new_allocated;
        let current_version = balance.version;

        let rows_affected = tx.execute(
            "UPDATE inventory_balances SET quantity_allocated = ?, quantity_available = ?, version = version + 1, updated_at = ?
             WHERE item_id = ? AND location_id = ? AND version = ?",
            rusqlite::params![
                new_allocated.to_string(),
                new_available.to_string(),
                now.to_rfc3339(),
                item.id,
                location_id,
                current_version
            ],
        )
        .map_err(map_db_error)?;

        if rows_affected == 0 {
            return Err(CommerceError::VersionConflict {
                entity: "inventory_balance".to_string(),
                id: format!("{}:{}", item.id, location_id),
                expected_version: current_version,
            });
        }

        let reservation = InventoryReservation {
            id: reservation_id,
            item_id: item.id,
            location_id,
            quantity: input.quantity,
            status: ReservationStatus::Pending,
            reference_type: input.reference_type,
            reference_id: input.reference_id,
            expires_at,
            created_at: now,
        };

        tx.commit().map_err(map_db_error)?;

        Ok(reservation)
    }

    fn release_reservation(&self, reservation_id: Uuid) -> Result<()> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let now = Utc::now();

        // Get reservation
        let res = tx.query_row(
            "SELECT item_id, location_id, quantity, status, expires_at FROM inventory_reservations WHERE id = ?",
            [reservation_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, i64>("item_id")?,
                    row.get::<_, i32>("location_id")?,
                    parse_decimal_row(&row.get::<_, String>("quantity")?, "inventory_reservation", "quantity")?,
                    row.get::<_, String>("status")?,
                    parse_datetime_opt_row(row.get::<_, Option<String>>("expires_at")?, "inventory_reservation", "expires_at")?,
                ))
            },
        );

        let (item_id, location_id, quantity, status, expires_at) = match res {
            Ok(r) => r,
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                return Err(CommerceError::ReservationNotFound(reservation_id))
            }
            Err(e) => return Err(map_db_error(e)),
        };

        let parsed_status: ReservationStatus = status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid inventory_reservation.status '{}': {}",
                status, e
            ))
        })?;
        if parsed_status == ReservationStatus::Released
            || parsed_status == ReservationStatus::Cancelled
            || parsed_status == ReservationStatus::Expired
        {
            tx.commit().map_err(map_db_error)?;
            return Ok(()); // Already released
        }

        if let Some(expires_at) = expires_at {
            if expires_at < now {
                Self::expire_reservation_in_tx(&tx, reservation_id, item_id, location_id, quantity, now)?;
                tx.commit().map_err(map_db_error)?;
                return Ok(());
            }
        }

        // Update reservation status
        tx.execute(
            "UPDATE inventory_reservations SET status = 'released' WHERE id = ?",
            [reservation_id.to_string()],
        )
        .map_err(map_db_error)?;

        // Get current balance version for optimistic locking
        let current_version: i32 = tx.query_row(
            "SELECT version FROM inventory_balances WHERE item_id = ? AND location_id = ?",
            rusqlite::params![item_id, location_id],
            |row| row.get(0),
        )
        .map_err(map_db_error)?;

        // Update balance with optimistic locking
        let rows_affected = tx.execute(
            "UPDATE inventory_balances SET quantity_allocated = quantity_allocated - ?,
             quantity_available = quantity_available + ?, version = version + 1, updated_at = ?
             WHERE item_id = ? AND location_id = ? AND version = ?",
            rusqlite::params![
                quantity.to_string(),
                quantity.to_string(),
                now.to_rfc3339(),
                item_id,
                location_id,
                current_version
            ],
        )
        .map_err(map_db_error)?;

        if rows_affected == 0 {
            return Err(CommerceError::VersionConflict {
                entity: "inventory_balance".to_string(),
                id: format!("{}:{}", item_id, location_id),
                expected_version: current_version,
            });
        }

        tx.commit().map_err(map_db_error)?;

        Ok(())
    }

    fn confirm_reservation(&self, reservation_id: Uuid) -> Result<()> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let now = Utc::now();

        let res = tx.query_row(
            "SELECT item_id, location_id, quantity, status, expires_at FROM inventory_reservations WHERE id = ?",
            [reservation_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, i64>("item_id")?,
                    row.get::<_, i32>("location_id")?,
                    parse_decimal_row(&row.get::<_, String>("quantity")?, "inventory_reservation", "quantity")?,
                    row.get::<_, String>("status")?,
                    parse_datetime_opt_row(row.get::<_, Option<String>>("expires_at")?, "inventory_reservation", "expires_at")?,
                ))
            },
        );

        let (item_id, location_id, quantity, status, expires_at) = match res {
            Ok(r) => r,
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                return Err(CommerceError::ReservationNotFound(reservation_id))
            }
            Err(e) => return Err(map_db_error(e)),
        };

        let parsed_status: ReservationStatus = status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid inventory_reservation.status '{}': {}",
                status, e
            ))
        })?;

        if parsed_status == ReservationStatus::Released
            || parsed_status == ReservationStatus::Cancelled
        {
            tx.commit().map_err(map_db_error)?;
            return Ok(());
        }
        if parsed_status == ReservationStatus::Expired {
            return Err(CommerceError::ReservationExpired(reservation_id));
        }

        if let Some(expires_at) = expires_at {
            if expires_at < now {
                Self::expire_reservation_in_tx(&tx, reservation_id, item_id, location_id, quantity, now)?;
                tx.commit().map_err(map_db_error)?;
                return Err(CommerceError::ReservationExpired(reservation_id));
            }
        }

        tx.execute(
            "UPDATE inventory_reservations SET status = 'confirmed' WHERE id = ?",
            [reservation_id.to_string()],
        )
        .map_err(map_db_error)?;

        tx.commit().map_err(map_db_error)?;

        Ok(())
    }

    fn list(&self, filter: InventoryFilter) -> Result<Vec<InventoryItem>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM inventory_items WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(sku) = &filter.sku {
            sql.push_str(" AND sku LIKE ?");
            params.push(Box::new(format!("%{}%", sku)));
        }
        if let Some(is_active) = &filter.is_active {
            sql.push_str(" AND is_active = ?");
            params.push(Box::new(*is_active as i32));
        }

        sql.push_str(" ORDER BY sku");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }
        if let Some(offset) = filter.offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;

        let items = stmt
            .query_map(params_refs.as_slice(), |row| {
                Ok(InventoryItem {
                    id: row.get("id")?,
                    sku: row.get("sku")?,
                    name: row.get("name")?,
                    description: row.get("description")?,
                    unit_of_measure: row.get("unit_of_measure")?,
                    is_active: row.get::<_, i32>("is_active")? != 0,
                    created_at: parse_datetime_row(&row.get::<_, String>("created_at")?, "inventory_item", "created_at")?,
                    updated_at: parse_datetime_row(&row.get::<_, String>("updated_at")?, "inventory_item", "updated_at")?,
                })
            })
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        Ok(items)
    }

    fn get_reorder_needed(&self) -> Result<Vec<StockLevel>> {
        let conn = self.conn()?;

        let mut stmt = conn
            .prepare(
                "SELECT DISTINCT i.sku FROM inventory_items i
                 JOIN inventory_balances b ON i.id = b.item_id
                 WHERE b.reorder_point IS NOT NULL
                 AND CAST(b.quantity_available AS REAL) < CAST(b.reorder_point AS REAL)
                 AND i.is_active = 1",
            )
            .map_err(map_db_error)?;

        let skus: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        let mut result = vec![];
        for sku in skus {
            if let Some(stock) = self.get_stock(&sku)? {
                result.push(stock);
            }
        }

        Ok(result)
    }

    fn record_transaction(&self, transaction: InventoryTransaction) -> Result<InventoryTransaction> {
        let conn = self.conn()?;

        conn.execute(
            "INSERT INTO inventory_transactions (item_id, location_id, transaction_type, quantity, reference_type, reference_id, reason, created_by, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                transaction.item_id,
                transaction.location_id,
                transaction.transaction_type.to_string(),
                transaction.quantity.to_string(),
                transaction.reference_type,
                transaction.reference_id,
                transaction.reason,
                transaction.created_by,
                transaction.created_at.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;

        let id = conn.last_insert_rowid();

        Ok(InventoryTransaction {
            id,
            ..transaction
        })
    }

    fn get_transactions(&self, item_id: i64, limit: u32) -> Result<Vec<InventoryTransaction>> {
        let conn = self.conn()?;

        let mut stmt = conn
            .prepare(&format!(
                "SELECT * FROM inventory_transactions WHERE item_id = ? ORDER BY created_at DESC LIMIT {}",
                limit
            ))
            .map_err(map_db_error)?;

        let transactions = stmt
            .query_map([item_id], |row| {
                Ok(InventoryTransaction {
                    id: row.get("id")?,
                    item_id: row.get("item_id")?,
                    location_id: row.get("location_id")?,
                    transaction_type: parse_enum_row(
                        &row.get::<_, String>("transaction_type")?,
                        "inventory_transaction",
                        "transaction_type",
                    )?,
                    quantity: parse_decimal_row(&row.get::<_, String>("quantity")?, "inventory_transaction", "quantity")?,
                    reference_type: row.get("reference_type")?,
                    reference_id: row.get("reference_id")?,
                    reason: row.get("reason")?,
                    created_by: row.get("created_by")?,
                    created_at: parse_datetime_row(&row.get::<_, String>("created_at")?, "inventory_transaction", "created_at")?,
                })
            })
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        Ok(transactions)
    }

    // === Batch Operations ===

    fn create_item_batch(&self, inputs: Vec<CreateInventoryItem>) -> Result<BatchResult<InventoryItem>> {
        validate_batch_size(&inputs)?;
        let mut result = BatchResult::with_capacity(inputs.len());

        for (index, input) in inputs.into_iter().enumerate() {
            match self.create_item(input) {
                Ok(item) => result.record_success(item),
                Err(e) => result.record_failure(index, None, &e),
            }
        }

        Ok(result)
    }

    fn create_item_batch_atomic(&self, inputs: Vec<CreateInventoryItem>) -> Result<Vec<InventoryItem>> {
        validate_batch_size(&inputs)?;
        if inputs.is_empty() {
            return Ok(vec![]);
        }

        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let mut results = Vec::with_capacity(inputs.len());

        for input in inputs {
            let now = Utc::now();
            let sku = input.sku.clone();
            let name = input.name.clone();
            let description = input.description.clone();
            let unit_of_measure = input.unit_of_measure.clone().unwrap_or_else(|| "EA".to_string());

            // Check SKU uniqueness
            let exists: i32 = tx
                .query_row(
                    "SELECT COUNT(*) FROM inventory_items WHERE sku = ?",
                    [&sku],
                    |row| row.get(0),
                )
                .map_err(map_db_error)?;

            if exists > 0 {
                return Err(CommerceError::DuplicateSku(sku));
            }

            tx.execute(
                "INSERT INTO inventory_items (sku, name, description, unit_of_measure, is_active, created_at, updated_at)
                 VALUES (?, ?, ?, ?, 1, ?, ?)",
                rusqlite::params![
                    &sku,
                    &name,
                    &description,
                    &unit_of_measure,
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                ],
            )
            .map_err(map_db_error)?;

            let item_id = tx.last_insert_rowid();

            // Create initial balance if quantity provided
            let location_id = input.location_id.unwrap_or(1);
            let initial_qty = input.initial_quantity.unwrap_or_default();

            tx.execute(
                "INSERT INTO inventory_balances (item_id, location_id, quantity_on_hand, quantity_allocated, quantity_available, reorder_point, safety_stock, updated_at)
                 VALUES (?, ?, ?, '0', ?, ?, ?, ?)",
                rusqlite::params![
                    item_id,
                    location_id,
                    initial_qty.to_string(),
                    initial_qty.to_string(),
                    input.reorder_point.map(|d| d.to_string()),
                    input.safety_stock.map(|d| d.to_string()),
                    now.to_rfc3339(),
                ],
            )
            .map_err(map_db_error)?;

            // Record initial transaction if quantity > 0
            if initial_qty > Decimal::ZERO {
                tx.execute(
                    "INSERT INTO inventory_transactions (item_id, location_id, transaction_type, quantity, reason, created_at)
                     VALUES (?, ?, 'receipt', ?, 'Initial stock', ?)",
                    rusqlite::params![item_id, location_id, initial_qty.to_string(), now.to_rfc3339()],
                )
                .map_err(map_db_error)?;
            }

            results.push(InventoryItem {
                id: item_id,
                sku,
                name,
                description,
                unit_of_measure,
                is_active: true,
                created_at: now,
                updated_at: now,
            });
        }

        tx.commit().map_err(map_db_error)?;
        Ok(results)
    }

    fn adjust_batch(&self, adjustments: Vec<AdjustInventory>) -> Result<BatchResult<InventoryTransaction>> {
        validate_batch_size(&adjustments)?;
        let mut result = BatchResult::with_capacity(adjustments.len());

        for (index, input) in adjustments.into_iter().enumerate() {
            let sku = input.sku.clone();
            match self.adjust(input) {
                Ok(transaction) => result.record_success(transaction),
                Err(e) => result.record_failure(index, Some(sku), &e),
            }
        }

        Ok(result)
    }

    fn adjust_batch_atomic(&self, adjustments: Vec<AdjustInventory>) -> Result<Vec<InventoryTransaction>> {
        validate_batch_size(&adjustments)?;
        if adjustments.is_empty() {
            return Ok(vec![]);
        }

        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let mut results = Vec::with_capacity(adjustments.len());
        let now = Utc::now();

        for input in adjustments {
            // Get item directly with this connection
            let item = tx.query_row(
                "SELECT * FROM inventory_items WHERE sku = ?",
                [&input.sku],
                |row| {
                    Ok(InventoryItem {
                        id: row.get("id")?,
                        sku: row.get("sku")?,
                        name: row.get("name")?,
                        description: row.get("description")?,
                        unit_of_measure: row.get("unit_of_measure")?,
                        is_active: row.get::<_, i32>("is_active")? != 0,
                        created_at: parse_datetime_row(&row.get::<_, String>("created_at")?, "inventory_item", "created_at")?,
                        updated_at: parse_datetime_row(&row.get::<_, String>("updated_at")?, "inventory_item", "updated_at")?,
                    })
                },
            ).map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => CommerceError::InventoryItemNotFound(input.sku.clone()),
                e => map_db_error(e),
            })?;

            let location_id = input.location_id.unwrap_or(1);

            // Get or create balance directly with this connection
            let balance_result = tx.query_row(
                "SELECT * FROM inventory_balances WHERE item_id = ? AND location_id = ?",
                rusqlite::params![item.id, location_id],
                |row| {
                    Ok(InventoryBalance {
                        id: row.get("id")?,
                        item_id: row.get("item_id")?,
                        location_id: row.get("location_id")?,
                        quantity_on_hand: parse_decimal_row(&row.get::<_, String>("quantity_on_hand")?, "inventory_balance", "quantity_on_hand")?,
                        quantity_allocated: parse_decimal_row(&row.get::<_, String>("quantity_allocated")?, "inventory_balance", "quantity_allocated")?,
                        quantity_available: parse_decimal_row(&row.get::<_, String>("quantity_available")?, "inventory_balance", "quantity_available")?,
                        reorder_point: parse_decimal_opt_row(row.get::<_, Option<String>>("reorder_point")?, "inventory_balance", "reorder_point")?,
                        safety_stock: parse_decimal_opt_row(row.get::<_, Option<String>>("safety_stock")?, "inventory_balance", "safety_stock")?,
                        version: row.get("version")?,
                        last_counted_at: parse_datetime_opt_row(row.get::<_, Option<String>>("last_counted_at")?, "inventory_balance", "last_counted_at")?,
                        updated_at: parse_datetime_row(&row.get::<_, String>("updated_at")?, "inventory_balance", "updated_at")?,
                    })
                },
            );

            let balance = match balance_result {
                Ok(b) => b,
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    tx.execute(
                        "INSERT INTO inventory_balances (item_id, location_id, quantity_on_hand, quantity_allocated, quantity_available, updated_at)
                         VALUES (?, ?, '0', '0', '0', ?)",
                        rusqlite::params![item.id, location_id, now.to_rfc3339()],
                    )
                    .map_err(map_db_error)?;

                    // Query the newly created balance
                    tx.query_row(
                        "SELECT * FROM inventory_balances WHERE item_id = ? AND location_id = ?",
                        rusqlite::params![item.id, location_id],
                        |row| {
                            Ok(InventoryBalance {
                                id: row.get("id")?,
                                item_id: row.get("item_id")?,
                                location_id: row.get("location_id")?,
                                quantity_on_hand: parse_decimal_row(&row.get::<_, String>("quantity_on_hand")?, "inventory_balance", "quantity_on_hand")?,
                                quantity_allocated: parse_decimal_row(&row.get::<_, String>("quantity_allocated")?, "inventory_balance", "quantity_allocated")?,
                                quantity_available: parse_decimal_row(&row.get::<_, String>("quantity_available")?, "inventory_balance", "quantity_available")?,
                                reorder_point: parse_decimal_opt_row(row.get::<_, Option<String>>("reorder_point")?, "inventory_balance", "reorder_point")?,
                                safety_stock: parse_decimal_opt_row(row.get::<_, Option<String>>("safety_stock")?, "inventory_balance", "safety_stock")?,
                                version: row.get("version")?,
                                last_counted_at: parse_datetime_opt_row(row.get::<_, Option<String>>("last_counted_at")?, "inventory_balance", "last_counted_at")?,
                                updated_at: parse_datetime_row(&row.get::<_, String>("updated_at")?, "inventory_balance", "updated_at")?,
                            })
                        },
                    ).map_err(map_db_error)?
                }
                Err(e) => return Err(map_db_error(e)),
            };

            // Calculate new quantities
            let new_on_hand = balance.quantity_on_hand + input.quantity;
            let new_available = new_on_hand - balance.quantity_allocated;

            if new_on_hand < Decimal::ZERO {
                return Err(CommerceError::InsufficientStock {
                    sku: input.sku.clone(),
                    requested: input.quantity.abs().to_string(),
                    available: balance.quantity_on_hand.to_string(),
                });
            }
            if new_available < Decimal::ZERO {
                return Err(CommerceError::InsufficientStock {
                    sku: input.sku.clone(),
                    requested: input.quantity.abs().to_string(),
                    available: balance.quantity_available.to_string(),
                });
            }

            // Update balance with optimistic locking
            let current_version = balance.version;
            let rows_affected = tx.execute(
                "UPDATE inventory_balances SET quantity_on_hand = ?, quantity_available = ?, version = version + 1, updated_at = ?
                 WHERE item_id = ? AND location_id = ? AND version = ?",
                rusqlite::params![
                    new_on_hand.to_string(),
                    new_available.to_string(),
                    now.to_rfc3339(),
                    item.id,
                    location_id,
                    current_version
                ],
            )
            .map_err(map_db_error)?;

            if rows_affected == 0 {
                return Err(CommerceError::VersionConflict {
                    entity: "inventory_balance".to_string(),
                    id: format!("{}:{}", item.id, location_id),
                    expected_version: current_version,
                });
            }

            // Record transaction
            let tx_type = if input.quantity >= Decimal::ZERO { "receipt" } else { "adjustment" };
            tx.execute(
                "INSERT INTO inventory_transactions (item_id, location_id, transaction_type, quantity, reference_type, reference_id, reason, created_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    item.id,
                    location_id,
                    tx_type,
                    input.quantity.to_string(),
                    input.reference_type,
                    input.reference_id,
                    input.reason,
                    now.to_rfc3339(),
                ],
            )
            .map_err(map_db_error)?;

            let tx_id = tx.last_insert_rowid();
            results.push(InventoryTransaction {
                id: tx_id,
                item_id: item.id,
                location_id,
                transaction_type: if input.quantity >= Decimal::ZERO {
                    TransactionType::Receipt
                } else {
                    TransactionType::Adjustment
                },
                quantity: input.quantity,
                reference_type: input.reference_type,
                reference_id: input.reference_id,
                reason: Some(input.reason),
                created_by: None,
                created_at: now,
            });
        }

        tx.commit().map_err(map_db_error)?;
        Ok(results)
    }

    fn get_item_batch(&self, ids: Vec<i64>) -> Result<Vec<InventoryItem>> {
        validate_batch_size(&ids)?;
        if ids.is_empty() {
            return Ok(vec![]);
        }

        let conn = self.conn()?;
        let placeholders = build_in_clause(ids.len());
        let sql = format!("SELECT * FROM inventory_items WHERE id IN ({})", placeholders);

        let params = i64_params(&ids);
        let params_refs = params_refs(&params);

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let items = stmt
            .query_map(params_refs.as_slice(), |row| {
                Ok(InventoryItem {
                    id: row.get("id")?,
                    sku: row.get("sku")?,
                    name: row.get("name")?,
                    description: row.get("description")?,
                    unit_of_measure: row.get("unit_of_measure")?,
                    is_active: row.get::<_, i32>("is_active")? != 0,
                    created_at: parse_datetime_row(&row.get::<_, String>("created_at")?, "inventory_item", "created_at")?,
                    updated_at: parse_datetime_row(&row.get::<_, String>("updated_at")?, "inventory_item", "updated_at")?,
                })
            })
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        Ok(items)
    }

    fn get_stock_batch(&self, skus: Vec<String>) -> Result<Vec<StockLevel>> {
        validate_batch_size(&skus)?;
        if skus.is_empty() {
            return Ok(vec![]);
        }

        let conn = self.conn()?;
        let placeholders = build_in_clause(skus.len());
        let sql = format!("SELECT * FROM inventory_items WHERE sku IN ({})", placeholders);

        let params = string_params(&skus);
        let params_refs = params_refs(&params);

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let items: Vec<InventoryItem> = stmt
            .query_map(params_refs.as_slice(), |row| {
                Ok(InventoryItem {
                    id: row.get("id")?,
                    sku: row.get("sku")?,
                    name: row.get("name")?,
                    description: row.get("description")?,
                    unit_of_measure: row.get("unit_of_measure")?,
                    is_active: row.get::<_, i32>("is_active")? != 0,
                    created_at: parse_datetime_row(&row.get::<_, String>("created_at")?, "inventory_item", "created_at")?,
                    updated_at: parse_datetime_row(&row.get::<_, String>("updated_at")?, "inventory_item", "updated_at")?,
                })
            })
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        // Build stock levels for each item
        let mut results = Vec::with_capacity(items.len());
        for item in items {
            let mut balance_stmt = conn
                .prepare(
                    "SELECT b.*, l.name as location_name
                     FROM inventory_balances b
                     LEFT JOIN inventory_locations l ON b.location_id = l.id
                     WHERE b.item_id = ?",
                )
                .map_err(map_db_error)?;

            let locations: Vec<LocationStock> = balance_stmt
                .query_map([item.id], |row| {
                    Ok(LocationStock {
                        location_id: row.get("location_id")?,
                        location_name: row.get("location_name")?,
                        on_hand: parse_decimal_row(&row.get::<_, String>("quantity_on_hand")?, "inventory_balance", "quantity_on_hand")?,
                        allocated: parse_decimal_row(&row.get::<_, String>("quantity_allocated")?, "inventory_balance", "quantity_allocated")?,
                        available: parse_decimal_row(&row.get::<_, String>("quantity_available")?, "inventory_balance", "quantity_available")?,
                    })
                })
                .map_err(map_db_error)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(map_db_error)?;

            let total_on_hand: Decimal = locations.iter().map(|l| l.on_hand).sum();
            let total_allocated: Decimal = locations.iter().map(|l| l.allocated).sum();
            let total_available: Decimal = locations.iter().map(|l| l.available).sum();

            results.push(StockLevel {
                sku: item.sku,
                name: item.name,
                total_on_hand,
                total_allocated,
                total_available,
                locations,
            });
        }

        Ok(results)
    }
}
