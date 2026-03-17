//! PostgreSQL implementation of backorder repository

use super::map_db_error;
use chrono::Utc;
use rust_decimal::Decimal;
use sqlx::FromRow;
use sqlx::postgres::PgPool;
use stateset_core::{
    AllocateBackorder, AllocationStatus, Backorder, BackorderAllocation, BackorderFilter,
    BackorderFulfillment, BackorderPriority, BackorderRepository, BackorderStatus,
    BackorderSummary, CommerceError, CreateBackorder, FulfillBackorder, FulfillmentSourceType,
    Result, SkuBackorderSummary, UpdateBackorder, generate_backorder_number,
};
use uuid::Uuid;

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

    pub(crate) async fn cancel_backorders_for_order_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        order_id: Uuid,
    ) -> Result<()> {
        let now = Utc::now();

        sqlx::query(
            "UPDATE backorders SET status = 'cancelled', updated_at = $1 WHERE order_id = $2",
        )
        .bind(now)
        .bind(order_id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        sqlx::query(
            "UPDATE backorder_allocations SET status = 'released'
             WHERE backorder_id IN (SELECT id FROM backorders WHERE order_id = $1)
               AND status = 'reserved'",
        )
        .bind(order_id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

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

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

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
        sqlx::query("UPDATE backorders SET status = 'cancelled', updated_at = $1 WHERE id = $2")
            .bind(now)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;
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
        let backorder =
            self.get_backorder_async(input.backorder_id).await?.ok_or(CommerceError::NotFound)?;

        if backorder.status == BackorderStatus::Cancelled
            || backorder.status == BackorderStatus::Fulfilled
        {
            return Err(CommerceError::ValidationError("Backorder cannot be fulfilled".into()));
        }

        let new_fulfilled = backorder.quantity_fulfilled + input.quantity;
        let remaining = (backorder.quantity_ordered - new_fulfilled).max(Decimal::ZERO);
        let new_status = if remaining.is_zero() {
            BackorderStatus::Fulfilled
        } else {
            BackorderStatus::PartiallyFulfilled
        };

        let now = Utc::now();
        sqlx::query(
            "UPDATE backorders SET quantity_fulfilled = $1, quantity_remaining = $2, status = $3, updated_at = $4 WHERE id = $5",
        )
        .bind(new_fulfilled)
        .bind(remaining)
        .bind(new_status.to_string())
        .bind(now)
        .bind(input.backorder_id)
        .execute(&self.pool)
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
        .bind(input.notes)
        .bind(now)
        .bind(input.fulfilled_by)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

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
        let id = Uuid::new_v4();
        let now = Utc::now();
        let sku = sqlx::query_scalar::<_, String>("SELECT sku FROM backorders WHERE id = $1")
            .bind(input.backorder_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?
            .ok_or(CommerceError::NotFound)?;

        sqlx::query(
            "INSERT INTO backorder_allocations (id, backorder_id, sku, quantity, location_id, lot_id, status, allocated_at, expires_at)
             VALUES ($1,$2,$3,$4,$5,$6,'reserved',$7,$8)",
        )
        .bind(id)
        .bind(input.backorder_id)
        .bind(&sku)
        .bind(input.quantity)
        .bind(input.location_id)
        .bind(input.lot_id)
        .bind(now)
        .bind(input.expires_at)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        let row = sqlx::query_as::<_, AllocationRow>(
            "SELECT id, backorder_id, sku, quantity, location_id, lot_id, status, allocated_at, expires_at
             FROM backorder_allocations WHERE id = $1",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        Self::row_to_allocation(row)
    }

    pub async fn get_allocations_async(
        &self,
        backorder_id: Uuid,
    ) -> Result<Vec<BackorderAllocation>> {
        let rows = sqlx::query_as::<_, AllocationRow>(
            "SELECT id, backorder_id, sku, quantity, location_id, lot_id, status, allocated_at, expires_at
             FROM backorder_allocations WHERE backorder_id = $1",
        )
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
        sqlx::query("UPDATE backorder_allocations SET status = 'released' WHERE id = $1")
            .bind(allocation_id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        let row = sqlx::query_as::<_, AllocationRow>(
            "SELECT id, backorder_id, sku, quantity, location_id, lot_id, status, allocated_at, expires_at
             FROM backorder_allocations WHERE id = $1",
        )
        .bind(allocation_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        Self::row_to_allocation(row)
    }

    pub async fn confirm_allocation_async(
        &self,
        allocation_id: Uuid,
    ) -> Result<BackorderAllocation> {
        sqlx::query("UPDATE backorder_allocations SET status = 'confirmed' WHERE id = $1")
            .bind(allocation_id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        let row = sqlx::query_as::<_, AllocationRow>(
            "SELECT id, backorder_id, sku, quantity, location_id, lot_id, status, allocated_at, expires_at
             FROM backorder_allocations WHERE id = $1",
        )
        .bind(allocation_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        Self::row_to_allocation(row)
    }

    pub async fn expire_allocations_async(&self) -> Result<u32> {
        let now = Utc::now();
        let count = sqlx::query(
            "UPDATE backorder_allocations SET status = 'expired' WHERE status = 'reserved' AND expires_at IS NOT NULL AND expires_at < $1",
        )
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?
        .rows_affected();

        Ok(count as u32)
    }

    pub async fn auto_allocate_inventory_async(
        &self,
        _sku: &str,
    ) -> Result<Vec<BackorderAllocation>> {
        Ok(Vec::new())
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
