//! PostgreSQL implementation of backorder repository

use super::inventory::{PgInventoryRepository, ReservationConfirmOutcome};
use super::map_db_error;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::FromRow;
use sqlx::postgres::PgPool;
use stateset_core::{
    AllocateBackorder, AllocationStatus, Backorder, BackorderAllocation, BackorderFilter,
    BackorderFulfillment, BackorderPriority, BackorderRepository, BackorderStatus,
    BackorderSummary, CommerceError, CreateBackorder, FulfillBackorder, FulfillmentSourceType,
    ReserveInventory, Result, SkuBackorderSummary, UpdateBackorder, generate_backorder_number,
};
use uuid::Uuid;

/// `reference_type` of the inventory reservation that backs an allocation
/// (same constant as the SQLite backend).
pub(crate) const BACKORDER_RESERVATION_REFERENCE: &str = "backorder";

const ALLOCATION_COLUMNS: &str = "id, backorder_id, sku, quantity, location_id, lot_id, status, allocated_at, expires_at, reservation_id";

type PgTx<'a> = sqlx::Transaction<'a, sqlx::Postgres>;

#[derive(Debug, Clone)]
pub struct PgBackorderRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct BackorderRow {
    id: Uuid,
    backorder_number: String,
    order_id: Uuid,
    order_line_id: Option<Uuid>,
    customer_id: Uuid,
    sku: String,
    quantity_ordered: Decimal,
    quantity_fulfilled: Decimal,
    quantity_remaining: Decimal,
    status: String,
    priority: String,
    expected_date: Option<chrono::DateTime<Utc>>,
    promised_date: Option<chrono::DateTime<Utc>>,
    source_location_id: Option<i32>,
    notes: Option<String>,
    created_at: chrono::DateTime<Utc>,
    updated_at: chrono::DateTime<Utc>,
}

#[derive(FromRow)]
struct FulfillmentRow {
    id: Uuid,
    backorder_id: Uuid,
    quantity: Decimal,
    source_type: String,
    source_id: Option<Uuid>,
    notes: Option<String>,
    fulfilled_at: chrono::DateTime<Utc>,
    fulfilled_by: Option<String>,
}

#[derive(FromRow)]
struct AllocationRow {
    id: Uuid,
    backorder_id: Uuid,
    sku: String,
    quantity: Decimal,
    location_id: Option<i32>,
    lot_id: Option<Uuid>,
    status: String,
    allocated_at: chrono::DateTime<Utc>,
    expires_at: Option<chrono::DateTime<Utc>>,
    reservation_id: Option<Uuid>,
}

impl PgBackorderRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_backorder(row: BackorderRow) -> Result<Backorder> {
        let BackorderRow {
            id,
            backorder_number,
            order_id,
            order_line_id,
            customer_id,
            sku,
            quantity_ordered,
            quantity_fulfilled,
            quantity_remaining,
            status,
            priority,
            expected_date,
            promised_date,
            source_location_id,
            notes,
            created_at,
            updated_at,
        } = row;

        let status: BackorderStatus = status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid backorder.status '{}': {}", status, e))
        })?;
        let priority: BackorderPriority = priority.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid backorder.priority '{}': {}",
                priority, e
            ))
        })?;

        Ok(Backorder {
            id,
            backorder_number,
            order_id,
            order_line_id,
            customer_id,
            sku,
            quantity_ordered,
            quantity_fulfilled,
            quantity_remaining,
            status,
            priority,
            expected_date,
            promised_date,
            source_location_id,
            notes,
            created_at,
            updated_at,
        })
    }

    pub(crate) async fn create_backorder_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        input: &CreateBackorder,
    ) -> Result<Backorder> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let backorder_number = generate_backorder_number();
        let priority = input.priority.unwrap_or_default();

        sqlx::query(
            "INSERT INTO backorders (id, backorder_number, order_id, order_line_id, customer_id, sku,
                quantity_ordered, quantity_fulfilled, quantity_remaining, status, priority,
                expected_date, promised_date, source_location_id, notes, created_at, updated_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7,0,$8,'pending',$9,$10,$11,$12,$13,$14,$15)",
        )
        .bind(id)
        .bind(&backorder_number)
        .bind(input.order_id)
        .bind(input.order_line_id)
        .bind(input.customer_id)
        .bind(&input.sku)
        .bind(input.quantity)
        .bind(input.quantity)
        .bind(priority.to_string())
        .bind(input.expected_date)
        .bind(input.promised_date)
        .bind(input.source_location_id)
        .bind(&input.notes)
        .bind(now)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let row = sqlx::query_as::<_, BackorderRow>("SELECT * FROM backorders WHERE id = $1")
            .bind(id)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(map_db_error)?
            .ok_or(CommerceError::NotFound)?;

        Self::row_to_backorder(row)
    }

    /// Open allocations (`reserved`/`confirmed`) of a backorder, oldest first.
    async fn open_allocations_in_tx(
        tx: &mut PgTx<'_>,
        backorder_id: Uuid,
    ) -> Result<Vec<(Uuid, Decimal, Option<Uuid>)>> {
        sqlx::query_as(
            "SELECT id, quantity, reservation_id FROM backorder_allocations
             WHERE backorder_id = $1 AND status IN ('reserved', 'confirmed')
             ORDER BY allocated_at, id
             FOR UPDATE",
        )
        .bind(backorder_id)
        .fetch_all(tx.as_mut())
        .await
        .map_err(map_db_error)
    }

    /// Release the inventory reservation behind every open allocation of the
    /// given backorders and mark the allocations `released`.
    async fn release_open_allocations_in_tx(
        &self,
        tx: &mut PgTx<'_>,
        backorder_ids: &[Uuid],
    ) -> Result<()> {
        let inventory = PgInventoryRepository::new(self.pool.clone());
        for backorder_id in backorder_ids {
            for (allocation_id, _, reservation_id) in
                Self::open_allocations_in_tx(tx, *backorder_id).await?
            {
                if let Some(reservation_id) = reservation_id {
                    inventory.release_reservation_in_tx(tx, reservation_id).await?;
                }
                sqlx::query("UPDATE backorder_allocations SET status = 'released' WHERE id = $1")
                    .bind(allocation_id)
                    .execute(tx.as_mut())
                    .await
                    .map_err(map_db_error)?;
            }
        }
        Ok(())
    }

    async fn open_backorder_ids_for_order_in_tx(
        tx: &mut PgTx<'_>,
        order_id: Uuid,
        order_line_id: Option<Uuid>,
    ) -> Result<Vec<Uuid>> {
        let rows: Vec<(Uuid,)> = sqlx::query_as(
            "SELECT id FROM backorders
             WHERE order_id = $1 AND ($2::uuid IS NULL OR order_line_id = $2)
               AND status NOT IN ('fulfilled', 'cancelled')
             FOR UPDATE",
        )
        .bind(order_id)
        .bind(order_line_id)
        .fetch_all(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    pub(crate) async fn cancel_backorders_for_order_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        order_id: Uuid,
    ) -> Result<()> {
        let now = Utc::now();

        let ids = Self::open_backorder_ids_for_order_in_tx(tx, order_id, None).await?;
        self.release_open_allocations_in_tx(tx, &ids).await?;

        sqlx::query(
            "UPDATE backorders SET status = 'cancelled', updated_at = $1
             WHERE order_id = $2 AND status NOT IN ('fulfilled', 'cancelled')",
        )
        .bind(now)
        .bind(order_id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        Ok(())
    }

    /// Cancel every open backorder raised for one order line (used when the
    /// line is removed from its order) and release any stock allocated
    /// against it. Mirrors the SQLite helper.
    pub(crate) async fn cancel_backorders_for_order_line_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        order_id: Uuid,
        order_line_id: Uuid,
    ) -> Result<()> {
        let now = Utc::now();

        let ids =
            Self::open_backorder_ids_for_order_in_tx(tx, order_id, Some(order_line_id)).await?;
        self.release_open_allocations_in_tx(tx, &ids).await?;

        sqlx::query(
            "UPDATE backorders SET status = 'cancelled', updated_at = $1
             WHERE order_id = $2 AND order_line_id = $3
               AND status NOT IN ('fulfilled', 'cancelled')",
        )
        .bind(now)
        .bind(order_id)
        .bind(order_line_id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        Ok(())
    }

    async fn get_allocation_in_tx(
        tx: &mut PgTx<'_>,
        allocation_id: Uuid,
    ) -> Result<BackorderAllocation> {
        let row = sqlx::query_as::<_, AllocationRow>(&format!(
            "SELECT {ALLOCATION_COLUMNS} FROM backorder_allocations WHERE id = $1 FOR UPDATE"
        ))
        .bind(allocation_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .ok_or(CommerceError::NotFound)?;
        Self::row_to_allocation(row)
    }

    async fn open_allocated_quantity_in_tx(
        tx: &mut PgTx<'_>,
        backorder_id: Uuid,
    ) -> Result<Decimal> {
        Ok(Self::open_allocations_in_tx(tx, backorder_id).await?.iter().map(|(_, q, _)| *q).sum())
    }

    /// Reserve `quantity` units of `sku` for a backorder and record the
    /// allocation (reservation keyed `backorder:<id>`).
    #[allow(clippy::too_many_arguments)]
    async fn allocate_in_tx(
        &self,
        tx: &mut PgTx<'_>,
        backorder_id: Uuid,
        sku: &str,
        quantity: Decimal,
        location_id: i32,
        lot_id: Option<Uuid>,
        expires_at: Option<DateTime<Utc>>,
        now: DateTime<Utc>,
    ) -> Result<BackorderAllocation> {
        let inventory = PgInventoryRepository::new(self.pool.clone());
        let expires_in_seconds = expires_at.map(|at| (at - now).num_seconds().max(1));
        let (reservation, _) = inventory
            .reserve_in_tx(
                tx,
                &ReserveInventory {
                    sku: sku.to_string(),
                    location_id: Some(location_id),
                    quantity,
                    reference_type: BACKORDER_RESERVATION_REFERENCE.to_string(),
                    reference_id: backorder_id.to_string(),
                    expires_in_seconds,
                },
            )
            .await?;

        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO backorder_allocations (id, backorder_id, sku, quantity, location_id, lot_id, status, allocated_at, expires_at, reservation_id)
             VALUES ($1,$2,$3,$4,$5,$6,'reserved',$7,$8,$9)",
        )
        .bind(id)
        .bind(backorder_id)
        .bind(sku)
        .bind(quantity)
        .bind(location_id)
        .bind(lot_id)
        .bind(now)
        .bind(expires_at)
        .bind(reservation.id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        sqlx::query(
            "UPDATE backorders SET status = 'allocated', updated_at = $1
             WHERE id = $2 AND status = 'pending'",
        )
        .bind(now)
        .bind(backorder_id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        Ok(BackorderAllocation {
            id,
            backorder_id,
            sku: sku.to_string(),
            quantity,
            location_id: Some(location_id),
            lot_id,
            status: AllocationStatus::Reserved,
            allocated_at: now,
            expires_at,
            reservation_id: Some(reservation.id),
        })
    }

    /// If a backorder was flagged `allocated` and no open allocation remains,
    /// drop it back to `pending`.
    async fn settle_backorder_allocation_status_in_tx(
        tx: &mut PgTx<'_>,
        backorder_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<()> {
        if Self::open_allocations_in_tx(tx, backorder_id).await?.is_empty() {
            sqlx::query(
                "UPDATE backorders SET status = 'pending', updated_at = $1
                 WHERE id = $2 AND status = 'allocated'",
            )
            .bind(now)
            .bind(backorder_id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        }
        Ok(())
    }

    /// Take `input.quantity` units out of stock for a fulfilment: open
    /// allocations first (their reservations are fulfilled: on-hand and
    /// allocated both decrement, `shipment` ledger row), then any remainder
    /// straight from available stock when fulfilling from `Inventory` and the
    /// SKU has an inventory master. Other sources (PO, transfer, production)
    /// and SKUs without an inventory item pass through untouched. Mirrors the
    /// SQLite backend.
    async fn consume_stock_for_fulfilment_in_tx(
        tx: &mut PgTx<'_>,
        input: &FulfillBackorder,
        sku: &str,
        source_location_id: Option<i32>,
        now: DateTime<Utc>,
    ) -> Result<()> {
        let reason = format!("Backorder {} fulfilment", input.backorder_id);
        let mut remaining = input.quantity;
        for (allocation_id, allocated, reservation_id) in
            Self::open_allocations_in_tx(tx, input.backorder_id).await?
        {
            if remaining <= Decimal::ZERO {
                break;
            }
            let take = remaining.min(allocated);
            if let Some(reservation_id) = reservation_id {
                PgInventoryRepository::fulfil_reservation_in_tx(
                    tx,
                    reservation_id,
                    take,
                    &reason,
                    now,
                )
                .await?;
            }
            if take == allocated {
                sqlx::query("UPDATE backorder_allocations SET status = 'fulfilled' WHERE id = $1")
                    .bind(allocation_id)
                    .execute(tx.as_mut())
                    .await
                    .map_err(map_db_error)?;
            } else {
                sqlx::query("UPDATE backorder_allocations SET quantity = $1 WHERE id = $2")
                    .bind(allocated - take)
                    .bind(allocation_id)
                    .execute(tx.as_mut())
                    .await
                    .map_err(map_db_error)?;
            }
            remaining -= take;
        }

        if remaining > Decimal::ZERO && input.source_type == FulfillmentSourceType::Inventory {
            let item: Option<(i64,)> =
                sqlx::query_as("SELECT id FROM inventory_items WHERE sku = $1")
                    .bind(sku)
                    .fetch_optional(tx.as_mut())
                    .await
                    .map_err(map_db_error)?;
            if let Some((item_id,)) = item {
                PgInventoryRepository::consume_available_in_tx(
                    tx,
                    item_id,
                    source_location_id.unwrap_or(1),
                    remaining,
                    BACKORDER_RESERVATION_REFERENCE,
                    &input.backorder_id.to_string(),
                    &reason,
                    now,
                )
                .await?;
            }
        }
        Ok(())
    }

    fn row_to_fulfillment(row: FulfillmentRow) -> Result<BackorderFulfillment> {
        let FulfillmentRow {
            id,
            backorder_id,
            quantity,
            source_type,
            source_id,
            notes,
            fulfilled_at,
            fulfilled_by,
        } = row;

        let source_type: FulfillmentSourceType = source_type.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid backorder_fulfillment.source_type '{}': {}",
                source_type, e
            ))
        })?;

        Ok(BackorderFulfillment {
            id,
            backorder_id,
            quantity,
            source_type,
            source_id,
            notes,
            fulfilled_at,
            fulfilled_by,
        })
    }

    fn row_to_allocation(row: AllocationRow) -> Result<BackorderAllocation> {
        let AllocationRow {
            id,
            backorder_id,
            sku,
            quantity,
            location_id,
            lot_id,
            status,
            allocated_at,
            expires_at,
            reservation_id,
        } = row;

        let status: AllocationStatus = status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid backorder_allocation.status '{}': {}",
                status, e
            ))
        })?;

        Ok(BackorderAllocation {
            id,
            backorder_id,
            sku,
            quantity,
            location_id,
            lot_id,
            status,
            allocated_at,
            expires_at,
            reservation_id,
        })
    }

    pub async fn create_backorder_async(&self, input: CreateBackorder) -> Result<Backorder> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let backorder_number = generate_backorder_number();
        let priority = input.priority.unwrap_or_default();

        sqlx::query(
            "INSERT INTO backorders (id, backorder_number, order_id, order_line_id, customer_id, sku,
                quantity_ordered, quantity_fulfilled, quantity_remaining, status, priority,
                expected_date, promised_date, source_location_id, notes, created_at, updated_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7,0,$8,'pending',$9,$10,$11,$12,$13,$14,$15)",
        )
        .bind(id)
        .bind(&backorder_number)
        .bind(input.order_id)
        .bind(input.order_line_id)
        .bind(input.customer_id)
        .bind(&input.sku)
        .bind(input.quantity)
        .bind(input.quantity)
        .bind(priority.to_string())
        .bind(input.expected_date)
        .bind(input.promised_date)
        .bind(input.source_location_id)
        .bind(input.notes)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_backorder_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn get_backorder_async(&self, id: Uuid) -> Result<Option<Backorder>> {
        let row = sqlx::query_as::<_, BackorderRow>(
            "SELECT id, backorder_number, order_id, order_line_id, customer_id, sku,
                    quantity_ordered, quantity_fulfilled, quantity_remaining, status, priority,
                    expected_date, promised_date, source_location_id, notes, created_at, updated_at
             FROM backorders WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_backorder).transpose()
    }

    pub async fn get_backorder_by_number_async(&self, number: &str) -> Result<Option<Backorder>> {
        let row = sqlx::query_as::<_, BackorderRow>(
            "SELECT id, backorder_number, order_id, order_line_id, customer_id, sku,
                    quantity_ordered, quantity_fulfilled, quantity_remaining, status, priority,
                    expected_date, promised_date, source_location_id, notes, created_at, updated_at
             FROM backorders WHERE backorder_number = $1",
        )
        .bind(number)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_backorder).transpose()
    }

    pub async fn update_backorder_async(
        &self,
        id: Uuid,
        input: UpdateBackorder,
    ) -> Result<Backorder> {
        let now = Utc::now();

        sqlx::query(
            "UPDATE backorders SET priority = COALESCE($1, priority), expected_date = COALESCE($2, expected_date),
                promised_date = COALESCE($3, promised_date), source_location_id = COALESCE($4, source_location_id),
                notes = COALESCE($5, notes), updated_at = $6 WHERE id = $7",
        )
        .bind(input.priority.map(|p| p.to_string()))
        .bind(input.expected_date)
        .bind(input.promised_date)
        .bind(input.source_location_id)
        .bind(input.notes)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_backorder_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn list_backorders_async(&self, filter: BackorderFilter) -> Result<Vec<Backorder>> {
        let mut sql = "SELECT id, backorder_number, order_id, order_line_id, customer_id, sku,
                quantity_ordered, quantity_fulfilled, quantity_remaining, status, priority,
                expected_date, promised_date, source_location_id, notes, created_at, updated_at
            FROM backorders WHERE 1=1"
            .to_string();
        let mut param_idx = 1;

        if filter.order_id.is_some() {
            sql.push_str(&format!(" AND order_id = ${}", param_idx));
            param_idx += 1;
        }
        if filter.customer_id.is_some() {
            sql.push_str(&format!(" AND customer_id = ${}", param_idx));
            param_idx += 1;
        }
        if filter.sku.is_some() {
            sql.push_str(&format!(" AND sku = ${}", param_idx));
            param_idx += 1;
        }
        if filter.status.is_some() {
            sql.push_str(&format!(" AND status = ${}", param_idx));
            param_idx += 1;
        }
        if filter.priority.is_some() {
            sql.push_str(&format!(" AND priority = ${}", param_idx));
        }

        sql.push_str(" ORDER BY CASE priority WHEN 'critical' THEN 1 WHEN 'high' THEN 2 WHEN 'normal' THEN 3 ELSE 4 END, created_at ASC");

        sql.push_str(&format!(" LIMIT {}", super::effective_limit(filter.limit)));

        let mut q = sqlx::query_as::<_, BackorderRow>(&sql);

        if let Some(order_id) = filter.order_id {
            q = q.bind(order_id);
        }
        if let Some(customer_id) = filter.customer_id {
            q = q.bind(customer_id);
        }
        if let Some(sku) = filter.sku {
            q = q.bind(sku);
        }
        if let Some(status) = filter.status {
            q = q.bind(status.to_string());
        }
        if let Some(priority) = filter.priority {
            q = q.bind(priority.to_string());
        }

        let rows = q.fetch_all(&self.pool).await.map_err(map_db_error)?;
        rows.into_iter().map(Self::row_to_backorder).collect::<Result<Vec<_>>>()
    }

    pub async fn cancel_backorder_async(&self, id: Uuid) -> Result<Backorder> {
        let now = Utc::now();
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let status_str: String =
            sqlx::query_scalar("SELECT status FROM backorders WHERE id = $1 FOR UPDATE")
                .bind(id)
                .fetch_optional(tx.as_mut())
                .await
                .map_err(map_db_error)?
                .ok_or(CommerceError::NotFound)?;
        let status: BackorderStatus = status_str.parse().map_err(|_| {
            CommerceError::DatabaseError(format!("Invalid backorder status '{status_str}'"))
        })?;
        match status {
            BackorderStatus::Cancelled => {
                return self.get_backorder_async(id).await?.ok_or(CommerceError::NotFound);
            }
            BackorderStatus::Fulfilled => {
                return Err(CommerceError::ValidationError(
                    "A fulfilled backorder cannot be cancelled".into(),
                ));
            }
            _ => {}
        }

        self.release_open_allocations_in_tx(&mut tx, &[id]).await?;
        sqlx::query("UPDATE backorders SET status = 'cancelled', updated_at = $1 WHERE id = $2")
            .bind(now)
            .bind(id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        tx.commit().await.map_err(map_db_error)?;

        self.get_backorder_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn get_backorders_for_order_async(&self, order_id: Uuid) -> Result<Vec<Backorder>> {
        self.list_backorders_async(BackorderFilter {
            order_id: Some(order_id),
            ..Default::default()
        })
        .await
    }

    pub async fn get_backorders_for_customer_async(
        &self,
        customer_id: Uuid,
    ) -> Result<Vec<Backorder>> {
        self.list_backorders_async(BackorderFilter {
            customer_id: Some(customer_id),
            ..Default::default()
        })
        .await
    }

    pub async fn get_backorders_for_sku_async(&self, sku: &str) -> Result<Vec<Backorder>> {
        self.list_backorders_async(BackorderFilter {
            sku: Some(sku.to_string()),
            ..Default::default()
        })
        .await
    }

    pub async fn fulfill_backorder_async(&self, input: FulfillBackorder) -> Result<Backorder> {
        if input.quantity <= Decimal::ZERO {
            return Err(CommerceError::ValidationError(
                "Fulfillment quantity must be greater than zero".into(),
            ));
        }

        let now = Utc::now();

        // Read + guards + UPDATE + fulfillment INSERT all inside one transaction
        // with `SELECT ... FOR UPDATE` locking the backorder row, so concurrent
        // fulfillments serialize instead of both reading the same remaining
        // quantity and over-fulfilling.
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let (status_str, remaining, fulfilled, sku, source_location_id): (
            String,
            Decimal,
            Decimal,
            String,
            Option<i32>,
        ) = sqlx::query_as(
            "SELECT status, quantity_remaining, quantity_fulfilled, sku, source_location_id
             FROM backorders WHERE id = $1 FOR UPDATE",
        )
        .bind(input.backorder_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .ok_or(CommerceError::NotFound)?;
        let status: BackorderStatus = status_str.parse().map_err(|_| {
            CommerceError::DatabaseError(format!("Invalid backorder status '{status_str}'"))
        })?;

        // A cancelled or already-fulfilled backorder cannot be fulfilled.
        if matches!(status, BackorderStatus::Cancelled | BackorderStatus::Fulfilled) {
            return Err(CommerceError::ValidationError("Backorder cannot be fulfilled".into()));
        }
        // Cannot fulfill more units than remain.
        if input.quantity > remaining {
            return Err(CommerceError::ValidationError(format!(
                "Cannot fulfill {} - only {} remaining",
                input.quantity, remaining
            )));
        }

        let new_fulfilled = fulfilled + input.quantity;
        let new_remaining = remaining - input.quantity;
        let new_status = if new_remaining <= Decimal::ZERO {
            BackorderStatus::Fulfilled
        } else {
            BackorderStatus::PartiallyFulfilled
        };

        Self::consume_stock_for_fulfilment_in_tx(&mut tx, &input, &sku, source_location_id, now)
            .await?;

        sqlx::query(
            "UPDATE backorders SET quantity_fulfilled = $1, quantity_remaining = $2, status = $3, updated_at = $4 WHERE id = $5",
        )
        .bind(new_fulfilled)
        .bind(new_remaining)
        .bind(new_status.to_string())
        .bind(now)
        .bind(input.backorder_id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let fulfillment_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO backorder_fulfillments (id, backorder_id, quantity, source_type, source_id, notes, fulfilled_at, fulfilled_by)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8)",
        )
        .bind(fulfillment_id)
        .bind(input.backorder_id)
        .bind(input.quantity)
        .bind(input.source_type.to_string())
        .bind(input.source_id)
        .bind(&input.notes)
        .bind(now)
        .bind(&input.fulfilled_by)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;

        self.get_backorder_async(input.backorder_id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn get_fulfillment_history_async(
        &self,
        backorder_id: Uuid,
    ) -> Result<Vec<BackorderFulfillment>> {
        let rows = sqlx::query_as::<_, FulfillmentRow>(
            "SELECT id, backorder_id, quantity, source_type, source_id, notes, fulfilled_at, fulfilled_by
             FROM backorder_fulfillments WHERE backorder_id = $1 ORDER BY fulfilled_at DESC",
        )
        .bind(backorder_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_fulfillment).collect::<Result<Vec<_>>>()
    }

    pub async fn allocate_backorder_async(
        &self,
        input: AllocateBackorder,
    ) -> Result<BackorderAllocation> {
        if input.quantity <= Decimal::ZERO {
            return Err(CommerceError::ValidationError(
                "Allocation quantity must be greater than zero".into(),
            ));
        }
        if input.location_id.is_some_and(|id| id <= 0) {
            return Err(CommerceError::ValidationError("location_id must be positive".into()));
        }
        let now = Utc::now();
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let (status_str, remaining, sku, source_location_id): (
            String,
            Decimal,
            String,
            Option<i32>,
        ) = sqlx::query_as(
            "SELECT status, quantity_remaining, sku, source_location_id FROM backorders
             WHERE id = $1 FOR UPDATE",
        )
        .bind(input.backorder_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .ok_or(CommerceError::NotFound)?;
        let status: BackorderStatus = status_str.parse().map_err(|_| {
            CommerceError::DatabaseError(format!("Invalid backorder status '{status_str}'"))
        })?;
        if matches!(status, BackorderStatus::Cancelled | BackorderStatus::Fulfilled) {
            return Err(CommerceError::ValidationError(format!(
                "Backorder is {status} and cannot be allocated"
            )));
        }
        let already = Self::open_allocated_quantity_in_tx(&mut tx, input.backorder_id).await?;
        if input.quantity + already > remaining {
            return Err(CommerceError::ValidationError(format!(
                "Cannot allocate {} - only {} of the backorder remains unallocated",
                input.quantity,
                remaining - already
            )));
        }

        let location_id = input.location_id.or(source_location_id).unwrap_or(1);
        let allocation = self
            .allocate_in_tx(
                &mut tx,
                input.backorder_id,
                &sku,
                input.quantity,
                location_id,
                input.lot_id,
                input.expires_at,
                now,
            )
            .await?;
        tx.commit().await.map_err(map_db_error)?;
        Ok(allocation)
    }

    pub async fn get_allocations_async(
        &self,
        backorder_id: Uuid,
    ) -> Result<Vec<BackorderAllocation>> {
        let rows = sqlx::query_as::<_, AllocationRow>(&format!(
            "SELECT {ALLOCATION_COLUMNS} FROM backorder_allocations
             WHERE backorder_id = $1 ORDER BY allocated_at, id"
        ))
        .bind(backorder_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_allocation).collect::<Result<Vec<_>>>()
    }

    pub async fn release_allocation_async(
        &self,
        allocation_id: Uuid,
    ) -> Result<BackorderAllocation> {
        let now = Utc::now();
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let allocation = Self::get_allocation_in_tx(&mut tx, allocation_id).await?;
        if !allocation.status.is_open() {
            tx.commit().await.map_err(map_db_error)?;
            return Ok(allocation);
        }
        if let Some(reservation_id) = allocation.reservation_id {
            PgInventoryRepository::new(self.pool.clone())
                .release_reservation_in_tx(&mut tx, reservation_id)
                .await?;
        }
        sqlx::query("UPDATE backorder_allocations SET status = 'released' WHERE id = $1")
            .bind(allocation_id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        Self::settle_backorder_allocation_status_in_tx(&mut tx, allocation.backorder_id, now)
            .await?;
        let updated = Self::get_allocation_in_tx(&mut tx, allocation_id).await?;
        tx.commit().await.map_err(map_db_error)?;
        Ok(updated)
    }

    pub async fn confirm_allocation_async(
        &self,
        allocation_id: Uuid,
    ) -> Result<BackorderAllocation> {
        let now = Utc::now();
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let allocation = Self::get_allocation_in_tx(&mut tx, allocation_id).await?;
        match allocation.status {
            AllocationStatus::Confirmed => {
                tx.commit().await.map_err(map_db_error)?;
                return Ok(allocation);
            }
            AllocationStatus::Reserved => {}
            other => {
                return Err(CommerceError::Conflict(format!(
                    "Backorder allocation {allocation_id} is {other} and cannot be confirmed"
                )));
            }
        }
        if let Some(reservation_id) = allocation.reservation_id {
            let outcome = PgInventoryRepository::new(self.pool.clone())
                .confirm_reservation_in_tx_with_now(&mut tx, reservation_id, now)
                .await?;
            if outcome == ReservationConfirmOutcome::Expired {
                sqlx::query("UPDATE backorder_allocations SET status = 'expired' WHERE id = $1")
                    .bind(allocation_id)
                    .execute(tx.as_mut())
                    .await
                    .map_err(map_db_error)?;
                Self::settle_backorder_allocation_status_in_tx(
                    &mut tx,
                    allocation.backorder_id,
                    now,
                )
                .await?;
                tx.commit().await.map_err(map_db_error)?;
                return Err(CommerceError::ReservationExpired(reservation_id));
            }
        }
        sqlx::query("UPDATE backorder_allocations SET status = 'confirmed' WHERE id = $1")
            .bind(allocation_id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        let updated = Self::get_allocation_in_tx(&mut tx, allocation_id).await?;
        tx.commit().await.map_err(map_db_error)?;
        Ok(updated)
    }

    pub async fn expire_allocations_async(&self) -> Result<u32> {
        let now = Utc::now();
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let rows: Vec<(Uuid, Uuid, Option<Uuid>)> = sqlx::query_as(
            "SELECT id, backorder_id, reservation_id FROM backorder_allocations
             WHERE status = 'reserved' AND expires_at IS NOT NULL AND expires_at < $1
             ORDER BY expires_at, id
             FOR UPDATE SKIP LOCKED",
        )
        .bind(now)
        .fetch_all(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let inventory = PgInventoryRepository::new(self.pool.clone());
        let mut count = 0u32;
        for (id, backorder_id, reservation_id) in rows {
            if let Some(reservation_id) = reservation_id {
                inventory.release_reservation_in_tx(&mut tx, reservation_id).await?;
            }
            sqlx::query("UPDATE backorder_allocations SET status = 'expired' WHERE id = $1")
                .bind(id)
                .execute(tx.as_mut())
                .await
                .map_err(map_db_error)?;
            Self::settle_backorder_allocation_status_in_tx(&mut tx, backorder_id, now).await?;
            count += 1;
        }
        tx.commit().await.map_err(map_db_error)?;
        Ok(count)
    }

    pub async fn auto_allocate_inventory_async(
        &self,
        sku: &str,
    ) -> Result<Vec<BackorderAllocation>> {
        let now = Utc::now();
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let candidates: Vec<(Uuid, Decimal, Option<i32>)> = sqlx::query_as(
            "SELECT id, quantity_remaining, source_location_id FROM backorders
             WHERE sku = $1 AND status IN ('pending', 'partially_fulfilled', 'allocated')
             ORDER BY CASE priority WHEN 'critical' THEN 1 WHEN 'high' THEN 2 WHEN 'normal' THEN 3 ELSE 4 END,
                      created_at ASC, id ASC
             FOR UPDATE",
        )
        .bind(sku)
        .fetch_all(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let mut created = Vec::new();
        for (backorder_id, remaining, source_location_id) in candidates {
            let need =
                remaining - Self::open_allocated_quantity_in_tx(&mut tx, backorder_id).await?;
            if need <= Decimal::ZERO {
                continue;
            }
            let location_id = source_location_id.unwrap_or(1);
            let available: Option<(Decimal,)> = sqlx::query_as(
                "SELECT b.quantity_available FROM inventory_balances b
                 JOIN inventory_items i ON i.id = b.item_id
                 WHERE i.sku = $1 AND b.location_id = $2",
            )
            .bind(sku)
            .bind(location_id)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(map_db_error)?;
            let Some((available,)) = available else { break };
            let take = need.min(available);
            if take <= Decimal::ZERO {
                continue;
            }
            created.push(
                self.allocate_in_tx(&mut tx, backorder_id, sku, take, location_id, None, None, now)
                    .await?,
            );
        }
        tx.commit().await.map_err(map_db_error)?;
        Ok(created)
    }

    pub async fn get_summary_async(&self) -> Result<BackorderSummary> {
        let now = Utc::now();
        let row = sqlx::query_as::<_, (i64, Decimal, i64, i64, i64)>(
            "SELECT
                COUNT(*),
                COALESCE(SUM(quantity_remaining), 0),
                SUM(CASE WHEN status = 'pending' THEN 1 ELSE 0 END),
                SUM(CASE WHEN status = 'allocated' THEN 1 ELSE 0 END),
                SUM(CASE WHEN priority = 'critical' THEN 1 ELSE 0 END)
             FROM backorders WHERE status NOT IN ('fulfilled', 'cancelled')",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        let overdue: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM backorders WHERE status NOT IN ('fulfilled', 'cancelled') AND expected_date IS NOT NULL AND expected_date < $1",
        )
        .bind(now)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(BackorderSummary {
            total_backorders: row.0 as i32,
            total_quantity: row.1,
            pending_count: row.2 as i32,
            allocated_count: row.3 as i32,
            critical_count: row.4 as i32,
            overdue_count: overdue as i32,
        })
    }

    pub async fn get_sku_summary_async(&self, sku: &str) -> Result<Option<SkuBackorderSummary>> {
        let row = sqlx::query_as::<
            _,
            (String, Decimal, i64, Option<chrono::DateTime<Utc>>, Option<chrono::DateTime<Utc>>),
        >(
            "SELECT
                sku,
                COALESCE(SUM(quantity_remaining), 0),
                COUNT(*),
                MIN(created_at),
                MIN(expected_date)
             FROM backorders
             WHERE sku = $1 AND status NOT IN ('fulfilled', 'cancelled')
             GROUP BY sku",
        )
        .bind(sku)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(row.map(|r| SkuBackorderSummary {
            sku: r.0,
            total_quantity: r.1,
            backorder_count: r.2 as i32,
            oldest_date: r.3,
            earliest_expected: r.4,
        }))
    }

    pub async fn get_overdue_backorders_async(&self) -> Result<Vec<Backorder>> {
        let now = Utc::now();
        let rows = sqlx::query_as::<_, BackorderRow>(
            "SELECT id, backorder_number, order_id, order_line_id, customer_id, sku,
                    quantity_ordered, quantity_fulfilled, quantity_remaining, status, priority,
                    expected_date, promised_date, source_location_id, notes, created_at, updated_at
             FROM backorders WHERE status NOT IN ('fulfilled', 'cancelled') AND expected_date IS NOT NULL AND expected_date < $1",
        )
        .bind(now)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_backorder).collect::<Result<Vec<_>>>()
    }

    pub async fn count_pending_async(&self) -> Result<u64> {
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM backorders WHERE status = 'pending'")
                .fetch_one(&self.pool)
                .await
                .map_err(map_db_error)?;
        Ok(count as u64)
    }
}

impl BackorderRepository for PgBackorderRepository {
    fn create_backorder(&self, input: CreateBackorder) -> Result<Backorder> {
        super::block_on(self.create_backorder_async(input))
    }

    fn get_backorder(&self, id: Uuid) -> Result<Option<Backorder>> {
        super::block_on(self.get_backorder_async(id))
    }

    fn get_backorder_by_number(&self, number: &str) -> Result<Option<Backorder>> {
        super::block_on(self.get_backorder_by_number_async(number))
    }

    fn update_backorder(&self, id: Uuid, input: UpdateBackorder) -> Result<Backorder> {
        super::block_on(self.update_backorder_async(id, input))
    }

    fn list_backorders(&self, filter: BackorderFilter) -> Result<Vec<Backorder>> {
        super::block_on(self.list_backorders_async(filter))
    }

    fn cancel_backorder(&self, id: Uuid) -> Result<Backorder> {
        super::block_on(self.cancel_backorder_async(id))
    }

    fn get_backorders_for_order(&self, order_id: Uuid) -> Result<Vec<Backorder>> {
        super::block_on(self.get_backorders_for_order_async(order_id))
    }

    fn get_backorders_for_customer(&self, customer_id: Uuid) -> Result<Vec<Backorder>> {
        super::block_on(self.get_backorders_for_customer_async(customer_id))
    }

    fn get_backorders_for_sku(&self, sku: &str) -> Result<Vec<Backorder>> {
        super::block_on(self.get_backorders_for_sku_async(sku))
    }

    fn fulfill_backorder(&self, input: FulfillBackorder) -> Result<Backorder> {
        super::block_on(self.fulfill_backorder_async(input))
    }

    fn get_fulfillment_history(&self, backorder_id: Uuid) -> Result<Vec<BackorderFulfillment>> {
        super::block_on(self.get_fulfillment_history_async(backorder_id))
    }

    fn allocate_backorder(&self, input: AllocateBackorder) -> Result<BackorderAllocation> {
        super::block_on(self.allocate_backorder_async(input))
    }

    fn get_allocations(&self, backorder_id: Uuid) -> Result<Vec<BackorderAllocation>> {
        super::block_on(self.get_allocations_async(backorder_id))
    }

    fn release_allocation(&self, allocation_id: Uuid) -> Result<BackorderAllocation> {
        super::block_on(self.release_allocation_async(allocation_id))
    }

    fn confirm_allocation(&self, allocation_id: Uuid) -> Result<BackorderAllocation> {
        super::block_on(self.confirm_allocation_async(allocation_id))
    }

    fn expire_allocations(&self) -> Result<u32> {
        super::block_on(self.expire_allocations_async())
    }

    fn auto_allocate_inventory(&self, sku: &str) -> Result<Vec<BackorderAllocation>> {
        super::block_on(self.auto_allocate_inventory_async(sku))
    }

    fn get_summary(&self) -> Result<BackorderSummary> {
        super::block_on(self.get_summary_async())
    }

    fn get_sku_summary(&self, sku: &str) -> Result<Option<SkuBackorderSummary>> {
        super::block_on(self.get_sku_summary_async(sku))
    }

    fn get_overdue_backorders(&self) -> Result<Vec<Backorder>> {
        super::block_on(self.get_overdue_backorders_async())
    }

    fn count_pending(&self) -> Result<u64> {
        super::block_on(self.count_pending_async())
    }
}
