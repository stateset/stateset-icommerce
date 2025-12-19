//! PostgreSQL inventory repository implementation

use super::map_db_error;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;
use sqlx::FromRow;
use stateset_core::{
    AdjustInventory, CommerceError, CreateInventoryItem, InventoryBalance, InventoryFilter,
    InventoryItem, InventoryRepository, InventoryReservation, InventoryTransaction, LocationStock,
    ReservationStatus, ReserveInventory, Result, StockLevel, TransactionType,
};
use uuid::Uuid;

/// PostgreSQL implementation of InventoryRepository
#[derive(Clone)]
pub struct PgInventoryRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct InventoryItemRow {
    id: i64,
    sku: String,
    name: String,
    description: Option<String>,
    unit_of_measure: String,
    is_active: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct InventoryBalanceRow {
    id: i64,
    item_id: i64,
    location_id: i32,
    quantity_on_hand: Decimal,
    quantity_allocated: Decimal,
    quantity_available: Decimal,
    reorder_point: Option<Decimal>,
    safety_stock: Option<Decimal>,
    version: i32,
    last_counted_at: Option<DateTime<Utc>>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct ReservationRow {
    id: Uuid,
    item_id: i64,
    location_id: i32,
    quantity: Decimal,
    status: String,
    reference_type: String,
    reference_id: String,
    expires_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct TransactionRow {
    id: i64,
    item_id: i64,
    location_id: i32,
    transaction_type: String,
    quantity: Decimal,
    reference_type: Option<String>,
    reference_id: Option<String>,
    reason: Option<String>,
    created_by: Option<String>,
    created_at: DateTime<Utc>,
}

impl PgInventoryRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_item(row: InventoryItemRow) -> InventoryItem {
        InventoryItem {
            id: row.id,
            sku: row.sku,
            name: row.name,
            description: row.description,
            unit_of_measure: row.unit_of_measure,
            is_active: row.is_active,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }

    fn row_to_balance(row: InventoryBalanceRow) -> InventoryBalance {
        InventoryBalance {
            id: row.id,
            item_id: row.item_id,
            location_id: row.location_id,
            quantity_on_hand: row.quantity_on_hand,
            quantity_allocated: row.quantity_allocated,
            quantity_available: row.quantity_available,
            reorder_point: row.reorder_point,
            safety_stock: row.safety_stock,
            version: row.version,
            last_counted_at: row.last_counted_at,
            updated_at: row.updated_at,
        }
    }

    fn row_to_reservation(row: ReservationRow) -> InventoryReservation {
        InventoryReservation {
            id: row.id,
            item_id: row.item_id,
            location_id: row.location_id,
            quantity: row.quantity,
            status: parse_reservation_status(&row.status),
            reference_type: row.reference_type,
            reference_id: row.reference_id,
            expires_at: row.expires_at,
            created_at: row.created_at,
        }
    }

    fn row_to_transaction(row: TransactionRow) -> InventoryTransaction {
        InventoryTransaction {
            id: row.id,
            item_id: row.item_id,
            location_id: row.location_id,
            transaction_type: parse_transaction_type(&row.transaction_type),
            quantity: row.quantity,
            reference_type: row.reference_type,
            reference_id: row.reference_id,
            reason: row.reason,
            created_by: row.created_by,
            created_at: row.created_at,
        }
    }

    /// Create an inventory item (async)
    pub async fn create_item_async(&self, input: CreateInventoryItem) -> Result<InventoryItem> {
        let now = Utc::now();

        let row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO inventory_items (sku, name, description, unit_of_measure, is_active, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id
            "#,
        )
        .bind(&input.sku)
        .bind(&input.name)
        .bind(&input.description)
        .bind(input.unit_of_measure.as_deref().unwrap_or("EA"))
        .bind(true)
        .bind(now)
        .bind(now)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        let id = row.0;
        let location_id = input.location_id.unwrap_or(1);

        // Create initial balance
        let initial_qty = input.initial_quantity.unwrap_or(Decimal::ZERO);
        sqlx::query(
            r#"
            INSERT INTO inventory_balances (item_id, location_id, quantity_on_hand, quantity_allocated,
                                            quantity_available, reorder_point, safety_stock, version, updated_at)
            VALUES ($1, $2, $3, 0, $4, $5, $6, 1, $7)
            "#,
        )
        .bind(id)
        .bind(location_id)
        .bind(initial_qty)
        .bind(initial_qty)
        .bind(input.reorder_point)
        .bind(input.safety_stock)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(InventoryItem {
            id,
            sku: input.sku,
            name: input.name,
            description: input.description,
            unit_of_measure: input.unit_of_measure.unwrap_or_else(|| "EA".to_string()),
            is_active: true,
            created_at: now,
            updated_at: now,
        })
    }

    /// Get inventory item by ID (async)
    pub async fn get_item_async(&self, id: i64) -> Result<Option<InventoryItem>> {
        let row = sqlx::query_as::<_, InventoryItemRow>("SELECT * FROM inventory_items WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(row.map(Self::row_to_item))
    }

    /// Get inventory item by SKU (async)
    pub async fn get_item_by_sku_async(&self, sku: &str) -> Result<Option<InventoryItem>> {
        let row = sqlx::query_as::<_, InventoryItemRow>("SELECT * FROM inventory_items WHERE sku = $1")
            .bind(sku)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(row.map(Self::row_to_item))
    }

    /// Get stock level for a SKU (async)
    pub async fn get_stock_async(&self, sku: &str) -> Result<Option<StockLevel>> {
        let item_row = sqlx::query_as::<_, InventoryItemRow>("SELECT * FROM inventory_items WHERE sku = $1")
            .bind(sku)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        let item = match item_row {
            Some(r) => r,
            None => return Ok(None),
        };

        let balance_rows = sqlx::query_as::<_, InventoryBalanceRow>(
            "SELECT * FROM inventory_balances WHERE item_id = $1",
        )
        .bind(item.id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        let mut total_on_hand = Decimal::ZERO;
        let mut total_allocated = Decimal::ZERO;
        let mut total_available = Decimal::ZERO;
        let mut locations = Vec::new();

        for b in balance_rows {
            total_on_hand += b.quantity_on_hand;
            total_allocated += b.quantity_allocated;
            total_available += b.quantity_available;
            locations.push(LocationStock {
                location_id: b.location_id,
                location_name: None,
                on_hand: b.quantity_on_hand,
                allocated: b.quantity_allocated,
                available: b.quantity_available,
            });
        }

        Ok(Some(StockLevel {
            sku: item.sku,
            name: item.name,
            total_on_hand,
            total_allocated,
            total_available,
            locations,
        }))
    }

    /// Get balance at specific location (async)
    pub async fn get_balance_async(&self, item_id: i64, location_id: i32) -> Result<Option<InventoryBalance>> {
        let row = sqlx::query_as::<_, InventoryBalanceRow>(
            "SELECT * FROM inventory_balances WHERE item_id = $1 AND location_id = $2",
        )
        .bind(item_id)
        .bind(location_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(row.map(Self::row_to_balance))
    }

    /// Adjust inventory (async)
    pub async fn adjust_async(&self, input: AdjustInventory) -> Result<InventoryTransaction> {
        let now = Utc::now();
        let location_id = input.location_id.unwrap_or(1);

        // Get item ID
        let item: (i64,) = sqlx::query_as("SELECT id FROM inventory_items WHERE sku = $1")
            .bind(&input.sku)
            .fetch_one(&self.pool)
            .await
            .map_err(|_| CommerceError::InventoryItemNotFound(input.sku.clone()))?;

        let item_id = item.0;

        // Get current version for optimistic locking
        let balance: (i32,) = sqlx::query_as(
            "SELECT version FROM inventory_balances WHERE item_id = $1 AND location_id = $2",
        )
        .bind(item_id)
        .bind(location_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        let current_version = balance.0;

        // Update balance with optimistic locking
        let result = sqlx::query(
            r#"
            UPDATE inventory_balances
            SET quantity_on_hand = quantity_on_hand + $1,
                quantity_available = quantity_on_hand + $1 - quantity_allocated,
                version = version + 1,
                updated_at = $2
            WHERE item_id = $3 AND location_id = $4 AND version = $5
            "#,
        )
        .bind(input.quantity)
        .bind(now)
        .bind(item_id)
        .bind(location_id)
        .bind(current_version)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        if result.rows_affected() == 0 {
            return Err(CommerceError::VersionConflict {
                entity: "inventory_balance".to_string(),
                id: format!("{}:{}", item_id, location_id),
                expected_version: current_version,
            });
        }

        // Record transaction
        let tx_type = if input.quantity >= Decimal::ZERO {
            "adjustment"
        } else {
            "adjustment"
        };

        let tx_row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO inventory_transactions (item_id, location_id, transaction_type, quantity,
                                                reference_type, reference_id, reason, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id
            "#,
        )
        .bind(item_id)
        .bind(location_id)
        .bind(tx_type)
        .bind(input.quantity)
        .bind(&input.reference_type)
        .bind(&input.reference_id)
        .bind(&input.reason)
        .bind(now)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(InventoryTransaction {
            id: tx_row.0,
            item_id,
            location_id,
            transaction_type: TransactionType::Adjustment,
            quantity: input.quantity,
            reference_type: input.reference_type,
            reference_id: input.reference_id,
            reason: Some(input.reason),
            created_by: None,
            created_at: now,
        })
    }

    /// Reserve inventory (async)
    pub async fn reserve_async(&self, input: ReserveInventory) -> Result<InventoryReservation> {
        let now = Utc::now();
        let location_id = input.location_id.unwrap_or(1);

        // Get item ID
        let item: (i64,) = sqlx::query_as("SELECT id FROM inventory_items WHERE sku = $1")
            .bind(&input.sku)
            .fetch_one(&self.pool)
            .await
            .map_err(|_| CommerceError::InventoryItemNotFound(input.sku.clone()))?;

        let item_id = item.0;

        // Check availability and get current version for optimistic locking
        let balance: (Decimal, i32) = sqlx::query_as(
            "SELECT quantity_available, version FROM inventory_balances WHERE item_id = $1 AND location_id = $2",
        )
        .bind(item_id)
        .bind(location_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        let (available, current_version) = balance;

        if available < input.quantity {
            return Err(CommerceError::InsufficientStock {
                sku: input.sku,
                requested: input.quantity.to_string(),
                available: available.to_string(),
            });
        }

        // Create reservation
        let id = Uuid::new_v4();
        let expires_at = input
            .expires_in_seconds
            .map(|s| now + chrono::Duration::seconds(s));

        sqlx::query(
            r#"
            INSERT INTO inventory_reservations (id, item_id, location_id, quantity, status,
                                                reference_type, reference_id, expires_at, created_at)
            VALUES ($1, $2, $3, $4, 'pending', $5, $6, $7, $8)
            "#,
        )
        .bind(id)
        .bind(item_id)
        .bind(location_id)
        .bind(input.quantity)
        .bind(&input.reference_type)
        .bind(&input.reference_id)
        .bind(expires_at)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        // Update allocation with optimistic locking
        let result = sqlx::query(
            r#"
            UPDATE inventory_balances
            SET quantity_allocated = quantity_allocated + $1,
                quantity_available = quantity_on_hand - quantity_allocated - $1,
                version = version + 1,
                updated_at = $2
            WHERE item_id = $3 AND location_id = $4 AND version = $5
            "#,
        )
        .bind(input.quantity)
        .bind(now)
        .bind(item_id)
        .bind(location_id)
        .bind(current_version)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        if result.rows_affected() == 0 {
            return Err(CommerceError::VersionConflict {
                entity: "inventory_balance".to_string(),
                id: format!("{}:{}", item_id, location_id),
                expected_version: current_version,
            });
        }

        Ok(InventoryReservation {
            id,
            item_id,
            location_id,
            quantity: input.quantity,
            status: ReservationStatus::Pending,
            reference_type: input.reference_type,
            reference_id: input.reference_id,
            expires_at,
            created_at: now,
        })
    }

    /// Release a reservation (async)
    pub async fn release_reservation_async(&self, reservation_id: Uuid) -> Result<()> {
        let now = Utc::now();

        // Get reservation
        let res = sqlx::query_as::<_, ReservationRow>(
            "SELECT * FROM inventory_reservations WHERE id = $1",
        )
        .bind(reservation_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?
        .ok_or(CommerceError::ReservationNotFound(reservation_id))?;

        if res.status == "released" {
            return Ok(());
        }

        // Get current version for optimistic locking
        let balance: (i32,) = sqlx::query_as(
            "SELECT version FROM inventory_balances WHERE item_id = $1 AND location_id = $2",
        )
        .bind(res.item_id)
        .bind(res.location_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        let current_version = balance.0;

        // Release allocation with optimistic locking
        let result = sqlx::query(
            r#"
            UPDATE inventory_balances
            SET quantity_allocated = quantity_allocated - $1,
                quantity_available = quantity_on_hand - quantity_allocated + $1,
                version = version + 1,
                updated_at = $2
            WHERE item_id = $3 AND location_id = $4 AND version = $5
            "#,
        )
        .bind(res.quantity)
        .bind(now)
        .bind(res.item_id)
        .bind(res.location_id)
        .bind(current_version)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        if result.rows_affected() == 0 {
            return Err(CommerceError::VersionConflict {
                entity: "inventory_balance".to_string(),
                id: format!("{}:{}", res.item_id, res.location_id),
                expected_version: current_version,
            });
        }

        // Update reservation status
        sqlx::query("UPDATE inventory_reservations SET status = 'released' WHERE id = $1")
            .bind(reservation_id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(())
    }

    /// Confirm a reservation (async)
    pub async fn confirm_reservation_async(&self, reservation_id: Uuid) -> Result<()> {
        sqlx::query("UPDATE inventory_reservations SET status = 'confirmed' WHERE id = $1")
            .bind(reservation_id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(())
    }

    /// List inventory items (async)
    pub async fn list_async(&self, filter: InventoryFilter) -> Result<Vec<InventoryItem>> {
        let limit = filter.limit.unwrap_or(100) as i64;
        let offset = filter.offset.unwrap_or(0) as i64;

        let rows = sqlx::query_as::<_, InventoryItemRow>(
            "SELECT * FROM inventory_items WHERE is_active = TRUE ORDER BY created_at DESC LIMIT $1 OFFSET $2",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(rows.into_iter().map(Self::row_to_item).collect())
    }

    /// Get items below reorder point (async)
    pub async fn get_reorder_needed_async(&self) -> Result<Vec<StockLevel>> {
        let rows = sqlx::query_as::<_, InventoryItemRow>(
            r#"
            SELECT i.* FROM inventory_items i
            JOIN inventory_balances b ON i.id = b.item_id
            WHERE b.quantity_available < COALESCE(b.reorder_point, 0)
            AND i.is_active = TRUE
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        let mut result = Vec::new();
        for row in rows {
            if let Some(stock) = self.get_stock_async(&row.sku).await? {
                result.push(stock);
            }
        }

        Ok(result)
    }

    /// Get transaction history (async)
    pub async fn get_transactions_async(&self, item_id: i64, limit: u32) -> Result<Vec<InventoryTransaction>> {
        let rows = sqlx::query_as::<_, TransactionRow>(
            "SELECT * FROM inventory_transactions WHERE item_id = $1 ORDER BY created_at DESC LIMIT $2",
        )
        .bind(item_id)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(rows.into_iter().map(Self::row_to_transaction).collect())
    }
}

impl InventoryRepository for PgInventoryRepository {
    fn create_item(&self, input: CreateInventoryItem) -> Result<InventoryItem> {
        tokio::runtime::Handle::current().block_on(self.create_item_async(input))
    }

    fn get_item(&self, id: i64) -> Result<Option<InventoryItem>> {
        tokio::runtime::Handle::current().block_on(self.get_item_async(id))
    }

    fn get_item_by_sku(&self, sku: &str) -> Result<Option<InventoryItem>> {
        tokio::runtime::Handle::current().block_on(self.get_item_by_sku_async(sku))
    }

    fn get_stock(&self, sku: &str) -> Result<Option<StockLevel>> {
        tokio::runtime::Handle::current().block_on(self.get_stock_async(sku))
    }

    fn get_balance(&self, item_id: i64, location_id: i32) -> Result<Option<InventoryBalance>> {
        tokio::runtime::Handle::current().block_on(self.get_balance_async(item_id, location_id))
    }

    fn adjust(&self, input: AdjustInventory) -> Result<InventoryTransaction> {
        tokio::runtime::Handle::current().block_on(self.adjust_async(input))
    }

    fn reserve(&self, input: ReserveInventory) -> Result<InventoryReservation> {
        tokio::runtime::Handle::current().block_on(self.reserve_async(input))
    }

    fn release_reservation(&self, reservation_id: Uuid) -> Result<()> {
        tokio::runtime::Handle::current().block_on(self.release_reservation_async(reservation_id))
    }

    fn confirm_reservation(&self, reservation_id: Uuid) -> Result<()> {
        tokio::runtime::Handle::current().block_on(self.confirm_reservation_async(reservation_id))
    }

    fn list(&self, filter: InventoryFilter) -> Result<Vec<InventoryItem>> {
        tokio::runtime::Handle::current().block_on(self.list_async(filter))
    }

    fn get_reorder_needed(&self) -> Result<Vec<StockLevel>> {
        tokio::runtime::Handle::current().block_on(self.get_reorder_needed_async())
    }

    fn record_transaction(&self, _transaction: InventoryTransaction) -> Result<InventoryTransaction> {
        Err(CommerceError::Internal("record_transaction not implemented".to_string()))
    }

    fn get_transactions(&self, item_id: i64, limit: u32) -> Result<Vec<InventoryTransaction>> {
        tokio::runtime::Handle::current().block_on(self.get_transactions_async(item_id, limit))
    }
}

fn parse_reservation_status(s: &str) -> ReservationStatus {
    match s {
        "pending" => ReservationStatus::Pending,
        "confirmed" => ReservationStatus::Confirmed,
        "allocated" => ReservationStatus::Allocated,
        "cancelled" => ReservationStatus::Cancelled,
        "released" => ReservationStatus::Released,
        "expired" => ReservationStatus::Expired,
        _ => ReservationStatus::Pending,
    }
}

fn parse_transaction_type(s: &str) -> TransactionType {
    match s {
        "receipt" => TransactionType::Receipt,
        "shipment" => TransactionType::Shipment,
        "adjustment" => TransactionType::Adjustment,
        "transfer" => TransactionType::Transfer,
        "return" => TransactionType::Return,
        "allocation" => TransactionType::Allocation,
        "deallocation" => TransactionType::Deallocation,
        "cycle_count" => TransactionType::CycleCount,
        _ => TransactionType::Adjustment,
    }
}
