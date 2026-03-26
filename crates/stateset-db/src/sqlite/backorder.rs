//! SQLite implementation of backorder repository

use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rust_decimal::Decimal;
use stateset_core::{
    AllocateBackorder, AllocationStatus, Backorder, BackorderAllocation, BackorderFilter,
    BackorderFulfillment, BackorderRepository, BackorderStatus, BackorderSummary, CommerceError,
    CreateBackorder, FulfillBackorder, Result, SkuBackorderSummary, UpdateBackorder,
    generate_backorder_number,
};
use uuid::Uuid;

use super::{
    map_db_error, parse_datetime_opt, parse_datetime_opt_row, parse_datetime_row,
    parse_decimal_row, parse_enum_row, parse_uuid_opt_row, parse_uuid_row, sum_decimal_query,
};

fn row_to_backorder_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Backorder> {
    Ok(Backorder {
        id: parse_uuid_row(&row.get::<_, String>(0)?, "backorder", "id")?,
        backorder_number: row.get(1)?,
        order_id: parse_uuid_row(&row.get::<_, String>(2)?, "backorder", "order_id")?,
        order_line_id: parse_uuid_opt_row(
            row.get::<_, Option<String>>(3)?,
            "backorder",
            "order_line_id",
        )?,
        customer_id: parse_uuid_row(&row.get::<_, String>(4)?, "backorder", "customer_id")?,
        sku: row.get(5)?,
        quantity_ordered: parse_decimal_row(
            &row.get::<_, String>(6)?,
            "backorder",
            "quantity_ordered",
        )?,
        quantity_fulfilled: parse_decimal_row(
            &row.get::<_, String>(7)?,
            "backorder",
            "quantity_fulfilled",
        )?,
        quantity_remaining: parse_decimal_row(
            &row.get::<_, String>(8)?,
            "backorder",
            "quantity_remaining",
        )?,
        status: parse_enum_row(&row.get::<_, String>(9)?, "backorder", "status")?,
        priority: parse_enum_row(&row.get::<_, String>(10)?, "backorder", "priority")?,
        expected_date: parse_datetime_opt_row(
            row.get::<_, Option<String>>(11)?,
            "backorder",
            "expected_date",
        )?,
        promised_date: parse_datetime_opt_row(
            row.get::<_, Option<String>>(12)?,
            "backorder",
            "promised_date",
        )?,
        source_location_id: row.get(13)?,
        notes: row.get(14)?,
        created_at: parse_datetime_row(&row.get::<_, String>(15)?, "backorder", "created_at")?,
        updated_at: parse_datetime_row(&row.get::<_, String>(16)?, "backorder", "updated_at")?,
    })
}

#[derive(Debug)]
pub struct SqliteBackorderRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteBackorderRepository {
    #[must_use] 
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn row_to_backorder(&self, row: &rusqlite::Row<'_>) -> rusqlite::Result<Backorder> {
        row_to_backorder_row(row)
    }

    fn row_to_fulfillment(
        &self,
        row: &rusqlite::Row<'_>,
    ) -> rusqlite::Result<BackorderFulfillment> {
        Ok(BackorderFulfillment {
            id: parse_uuid_row(&row.get::<_, String>(0)?, "backorder_fulfillment", "id")?,
            backorder_id: parse_uuid_row(
                &row.get::<_, String>(1)?,
                "backorder_fulfillment",
                "backorder_id",
            )?,
            quantity: parse_decimal_row(
                &row.get::<_, String>(2)?,
                "backorder_fulfillment",
                "quantity",
            )?,
            source_type: parse_enum_row(
                &row.get::<_, String>(3)?,
                "backorder_fulfillment",
                "source_type",
            )?,
            source_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>(4)?,
                "backorder_fulfillment",
                "source_id",
            )?,
            notes: row.get(5)?,
            fulfilled_at: parse_datetime_row(
                &row.get::<_, String>(6)?,
                "backorder_fulfillment",
                "fulfilled_at",
            )?,
            fulfilled_by: row.get(7)?,
        })
    }

    fn row_to_allocation(&self, row: &rusqlite::Row<'_>) -> rusqlite::Result<BackorderAllocation> {
        Ok(BackorderAllocation {
            id: parse_uuid_row(&row.get::<_, String>(0)?, "backorder_allocation", "id")?,
            backorder_id: parse_uuid_row(
                &row.get::<_, String>(1)?,
                "backorder_allocation",
                "backorder_id",
            )?,
            sku: row.get(2)?,
            quantity: parse_decimal_row(
                &row.get::<_, String>(3)?,
                "backorder_allocation",
                "quantity",
            )?,
            location_id: row.get(4)?,
            lot_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>(5)?,
                "backorder_allocation",
                "lot_id",
            )?,
            status: parse_enum_row(&row.get::<_, String>(6)?, "backorder_allocation", "status")?,
            allocated_at: parse_datetime_row(
                &row.get::<_, String>(7)?,
                "backorder_allocation",
                "allocated_at",
            )?,
            expires_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>(8)?,
                "backorder_allocation",
                "expires_at",
            )?,
        })
    }
}

pub(crate) fn create_backorder_in_tx(
    tx: &rusqlite::Transaction<'_>,
    input: &CreateBackorder,
) -> std::result::Result<Backorder, rusqlite::Error> {
    let id = Uuid::new_v4();
    let now = Utc::now();
    let backorder_number = generate_backorder_number();
    let priority = input.priority.unwrap_or_default();

    tx.execute(
        "INSERT INTO backorders (id, backorder_number, order_id, order_line_id, customer_id,
            sku, quantity_ordered, quantity_fulfilled, quantity_remaining, status, priority,
            expected_date, promised_date, source_location_id, notes, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, '0', ?, 'pending', ?, ?, ?, ?, ?, ?, ?)",
        rusqlite::params![
            id.to_string(),
            &backorder_number,
            input.order_id.to_string(),
            input.order_line_id.map(|id| id.to_string()),
            input.customer_id.to_string(),
            &input.sku,
            input.quantity.to_string(),
            input.quantity.to_string(),
            priority.to_string(),
            input.expected_date.map(|d| d.to_rfc3339()),
            input.promised_date.map(|d| d.to_rfc3339()),
            input.source_location_id,
            input.notes,
            now.to_rfc3339(),
            now.to_rfc3339(),
        ],
    )?;

    let row = tx.query_row(
        "SELECT id, backorder_number, order_id, order_line_id, customer_id, sku,
                quantity_ordered, quantity_fulfilled, quantity_remaining, status, priority,
                expected_date, promised_date, source_location_id, notes, created_at, updated_at
         FROM backorders WHERE id = ?",
        [id.to_string()],
        row_to_backorder_row,
    );

    match row {
        Ok(bo) => Ok(bo),
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            Err(rusqlite::Error::ToSqlConversionFailure(Box::new(CommerceError::NotFound)))
        }
        Err(e) => Err(e),
    }
}

pub(crate) fn cancel_backorders_for_order_in_tx(
    tx: &rusqlite::Transaction<'_>,
    order_id: Uuid,
) -> std::result::Result<(), rusqlite::Error> {
    let now = Utc::now();

    tx.execute(
        "UPDATE backorders SET status = 'cancelled', updated_at = ? WHERE order_id = ?",
        [now.to_rfc3339(), order_id.to_string()],
    )?;

    tx.execute(
        "UPDATE backorder_allocations SET status = 'released'
         WHERE backorder_id IN (SELECT id FROM backorders WHERE order_id = ?)
           AND status = 'reserved'",
        [order_id.to_string()],
    )?;

    Ok(())
}

impl BackorderRepository for SqliteBackorderRepository {
    fn create_backorder(&self, input: CreateBackorder) -> Result<Backorder> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let backorder_number = generate_backorder_number();
        let priority = input.priority.unwrap_or_default();

        {
            let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            conn.execute(
                "INSERT INTO backorders (id, backorder_number, order_id, order_line_id, customer_id,
                    sku, quantity_ordered, quantity_fulfilled, quantity_remaining, status, priority,
                    expected_date, promised_date, source_location_id, notes, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, '0', ?, 'pending', ?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    id.to_string(),
                    &backorder_number,
                    input.order_id.to_string(),
                    input.order_line_id.map(|id| id.to_string()),
                    input.customer_id.to_string(),
                    &input.sku,
                    input.quantity.to_string(),
                    input.quantity.to_string(), // quantity_remaining starts same as ordered
                    priority.to_string(),
                    input.expected_date.map(|d| d.to_rfc3339()),
                    input.promised_date.map(|d| d.to_rfc3339()),
                    input.source_location_id,
                    input.notes,
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                ],
            ).map_err(map_db_error)?;
        }

        self.get_backorder(id)?.ok_or(CommerceError::NotFound)
    }

    fn get_backorder(&self, id: Uuid) -> Result<Option<Backorder>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let result = conn.query_row(
            "SELECT id, backorder_number, order_id, order_line_id, customer_id, sku,
                    quantity_ordered, quantity_fulfilled, quantity_remaining, status, priority,
                    expected_date, promised_date, source_location_id, notes, created_at, updated_at
             FROM backorders WHERE id = ?",
            [id.to_string()],
            |row| self.row_to_backorder(row),
        );

        match result {
            Ok(bo) => Ok(Some(bo)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn get_backorder_by_number(&self, number: &str) -> Result<Option<Backorder>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let result = conn.query_row(
            "SELECT id, backorder_number, order_id, order_line_id, customer_id, sku,
                    quantity_ordered, quantity_fulfilled, quantity_remaining, status, priority,
                    expected_date, promised_date, source_location_id, notes, created_at, updated_at
             FROM backorders WHERE backorder_number = ?",
            [number],
            |row| self.row_to_backorder(row),
        );

        match result {
            Ok(bo) => Ok(Some(bo)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn update_backorder(&self, id: Uuid, input: UpdateBackorder) -> Result<Backorder> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let now = Utc::now();

        conn.execute(
            "UPDATE backorders SET
                priority = COALESCE(?, priority),
                expected_date = COALESCE(?, expected_date),
                promised_date = COALESCE(?, promised_date),
                source_location_id = COALESCE(?, source_location_id),
                notes = COALESCE(?, notes),
                updated_at = ?
             WHERE id = ?",
            rusqlite::params![
                input.priority.map(|p| p.to_string()),
                input.expected_date.map(|d| d.to_rfc3339()),
                input.promised_date.map(|d| d.to_rfc3339()),
                input.source_location_id,
                input.notes,
                now.to_rfc3339(),
                id.to_string(),
            ],
        )
        .map_err(map_db_error)?;

        self.get_backorder(id)?.ok_or(CommerceError::NotFound)
    }

    fn list_backorders(&self, filter: BackorderFilter) -> Result<Vec<Backorder>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut sql = String::from(
            "SELECT id, backorder_number, order_id, order_line_id, customer_id, sku,
                    quantity_ordered, quantity_fulfilled, quantity_remaining, status, priority,
                    expected_date, promised_date, source_location_id, notes, created_at, updated_at
             FROM backorders WHERE 1=1",
        );
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(ref order_id) = filter.order_id {
            sql.push_str(" AND order_id = ?");
            params.push(Box::new(order_id.to_string()));
        }
        if let Some(ref customer_id) = filter.customer_id {
            sql.push_str(" AND customer_id = ?");
            params.push(Box::new(customer_id.to_string()));
        }
        if let Some(ref sku) = filter.sku {
            sql.push_str(" AND sku = ?");
            params.push(Box::new(sku.clone()));
        }
        if let Some(ref status) = filter.status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(status.to_string()));
        }
        if let Some(ref priority) = filter.priority {
            sql.push_str(" AND priority = ?");
            params.push(Box::new(priority.to_string()));
        }

        // Order by priority (critical first) then by created_at
        sql.push_str(" ORDER BY CASE priority WHEN 'critical' THEN 1 WHEN 'high' THEN 2 WHEN 'normal' THEN 3 ELSE 4 END, created_at ASC");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {limit}"));
        }

        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(std::convert::AsRef::as_ref).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt
            .query_map(param_refs.as_slice(), |row| self.row_to_backorder(row))
            .map_err(map_db_error)?;

        let mut backorders = Vec::new();
        for row in rows {
            backorders.push(row.map_err(map_db_error)?);
        }
        Ok(backorders)
    }

    fn cancel_backorder(&self, id: Uuid) -> Result<Backorder> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let now = Utc::now();

        conn.execute(
            "UPDATE backorders SET status = 'cancelled', updated_at = ? WHERE id = ?",
            [now.to_rfc3339(), id.to_string()],
        )
        .map_err(map_db_error)?;

        // Release any allocations
        conn.execute(
            "UPDATE backorder_allocations SET status = 'released' WHERE backorder_id = ? AND status = 'reserved'",
            [id.to_string()],
        ).map_err(map_db_error)?;

        self.get_backorder(id)?.ok_or(CommerceError::NotFound)
    }

    fn get_backorders_for_order(&self, order_id: Uuid) -> Result<Vec<Backorder>> {
        self.list_backorders(BackorderFilter { order_id: Some(order_id), ..Default::default() })
    }

    fn get_backorders_for_customer(&self, customer_id: Uuid) -> Result<Vec<Backorder>> {
        self.list_backorders(BackorderFilter {
            customer_id: Some(customer_id),
            ..Default::default()
        })
    }

    fn get_backorders_for_sku(&self, sku: &str) -> Result<Vec<Backorder>> {
        self.list_backorders(BackorderFilter { sku: Some(sku.to_string()), ..Default::default() })
    }

    fn fulfill_backorder(&self, input: FulfillBackorder) -> Result<Backorder> {
        let now = Utc::now();
        let id = Uuid::new_v4();

        let backorder = self.get_backorder(input.backorder_id)?.ok_or(CommerceError::NotFound)?;

        if input.quantity > backorder.quantity_remaining {
            return Err(CommerceError::ValidationError(format!(
                "Cannot fulfill {} - only {} remaining",
                input.quantity, backorder.quantity_remaining
            )));
        }

        {
            let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            // Record fulfillment
            conn.execute(
                "INSERT INTO backorder_fulfillments (id, backorder_id, quantity, source_type,
                    source_id, notes, fulfilled_at, fulfilled_by)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    id.to_string(),
                    input.backorder_id.to_string(),
                    input.quantity.to_string(),
                    input.source_type.to_string(),
                    input.source_id.map(|id| id.to_string()),
                    input.notes,
                    now.to_rfc3339(),
                    input.fulfilled_by,
                ],
            )
            .map_err(map_db_error)?;

            // Update backorder quantities and status
            let new_fulfilled = backorder.quantity_fulfilled + input.quantity;
            let new_remaining = backorder.quantity_remaining - input.quantity;
            let new_status = if new_remaining <= Decimal::ZERO {
                BackorderStatus::Fulfilled
            } else {
                BackorderStatus::PartiallyFulfilled
            };

            conn.execute(
                "UPDATE backorders SET quantity_fulfilled = ?, quantity_remaining = ?, status = ?, updated_at = ?
                 WHERE id = ?",
                rusqlite::params![
                    new_fulfilled.to_string(),
                    new_remaining.to_string(),
                    new_status.to_string(),
                    now.to_rfc3339(),
                    input.backorder_id.to_string(),
                ],
            ).map_err(map_db_error)?;
        }

        self.get_backorder(input.backorder_id)?.ok_or(CommerceError::NotFound)
    }

    fn get_fulfillment_history(&self, backorder_id: Uuid) -> Result<Vec<BackorderFulfillment>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, backorder_id, quantity, source_type, source_id, notes, fulfilled_at, fulfilled_by
             FROM backorder_fulfillments WHERE backorder_id = ? ORDER BY fulfilled_at"
        ).map_err(map_db_error)?;

        let rows = stmt
            .query_map([backorder_id.to_string()], |row| self.row_to_fulfillment(row))
            .map_err(map_db_error)?;

        let mut fulfillments = Vec::new();
        for row in rows {
            fulfillments.push(row.map_err(map_db_error)?);
        }
        Ok(fulfillments)
    }

    fn allocate_backorder(&self, input: AllocateBackorder) -> Result<BackorderAllocation> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let id = Uuid::new_v4();
        let now = Utc::now();

        let backorder = self.get_backorder(input.backorder_id)?.ok_or(CommerceError::NotFound)?;

        conn.execute(
            "INSERT INTO backorder_allocations (id, backorder_id, sku, quantity, location_id,
                lot_id, status, allocated_at, expires_at)
             VALUES (?, ?, ?, ?, ?, ?, 'reserved', ?, ?)",
            rusqlite::params![
                id.to_string(),
                input.backorder_id.to_string(),
                &backorder.sku,
                input.quantity.to_string(),
                input.location_id,
                input.lot_id.map(|id| id.to_string()),
                now.to_rfc3339(),
                input.expires_at.map(|d| d.to_rfc3339()),
            ],
        )
        .map_err(map_db_error)?;

        // Update backorder status to allocated
        conn.execute(
            "UPDATE backorders SET status = 'allocated', updated_at = ?
             WHERE id = ? AND status = 'pending'",
            [now.to_rfc3339(), input.backorder_id.to_string()],
        )
        .map_err(map_db_error)?;

        Ok(BackorderAllocation {
            id,
            backorder_id: input.backorder_id,
            sku: backorder.sku,
            quantity: input.quantity,
            location_id: input.location_id,
            lot_id: input.lot_id,
            status: AllocationStatus::Reserved,
            allocated_at: now,
            expires_at: input.expires_at,
        })
    }

    fn get_allocations(&self, backorder_id: Uuid) -> Result<Vec<BackorderAllocation>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, backorder_id, sku, quantity, location_id, lot_id, status, allocated_at, expires_at
             FROM backorder_allocations WHERE backorder_id = ? AND status = 'reserved'"
        ).map_err(map_db_error)?;

        let rows = stmt
            .query_map([backorder_id.to_string()], |row| self.row_to_allocation(row))
            .map_err(map_db_error)?;

        let mut allocations = Vec::new();
        for row in rows {
            allocations.push(row.map_err(map_db_error)?);
        }
        Ok(allocations)
    }

    fn release_allocation(&self, allocation_id: Uuid) -> Result<BackorderAllocation> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        conn.execute(
            "UPDATE backorder_allocations SET status = 'released' WHERE id = ?",
            [allocation_id.to_string()],
        )
        .map_err(map_db_error)?;

        let result = conn.query_row(
            "SELECT id, backorder_id, sku, quantity, location_id, lot_id, status, allocated_at, expires_at
             FROM backorder_allocations WHERE id = ?",
            [allocation_id.to_string()],
            |row| self.row_to_allocation(row),
        ).map_err(map_db_error)?;

        Ok(result)
    }

    fn confirm_allocation(&self, allocation_id: Uuid) -> Result<BackorderAllocation> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        conn.execute(
            "UPDATE backorder_allocations SET status = 'confirmed' WHERE id = ?",
            [allocation_id.to_string()],
        )
        .map_err(map_db_error)?;

        let result = conn.query_row(
            "SELECT id, backorder_id, sku, quantity, location_id, lot_id, status, allocated_at, expires_at
             FROM backorder_allocations WHERE id = ?",
            [allocation_id.to_string()],
            |row| self.row_to_allocation(row),
        ).map_err(map_db_error)?;

        Ok(result)
    }

    fn expire_allocations(&self) -> Result<u32> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let now = Utc::now();

        let count = conn
            .execute(
                "UPDATE backorder_allocations SET status = 'expired'
             WHERE status = 'reserved' AND expires_at IS NOT NULL AND expires_at < ?",
                [now.to_rfc3339()],
            )
            .map_err(map_db_error)?;

        Ok(count as u32)
    }

    fn auto_allocate_inventory(&self, sku: &str) -> Result<Vec<BackorderAllocation>> {
        // Get pending backorders for this SKU ordered by priority
        let _backorders = self.list_backorders(BackorderFilter {
            sku: Some(sku.to_string()),
            status: Some(BackorderStatus::Pending),
            ..Default::default()
        })?;

        // Note: In a real implementation, we would check inventory availability
        // and create allocations. For now, return empty as this requires integration
        // with the inventory module.
        Ok(Vec::new())
    }

    fn get_summary(&self) -> Result<BackorderSummary> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let now = Utc::now();

        let (total, pending, allocated, critical): (i32, i32, i32, i32) = conn
            .query_row(
                "SELECT
                    COUNT(*),
                    SUM(CASE WHEN status = 'pending' THEN 1 ELSE 0 END),
                    SUM(CASE WHEN status = 'allocated' THEN 1 ELSE 0 END),
                    SUM(CASE WHEN priority = 'critical' THEN 1 ELSE 0 END)
                 FROM backorders WHERE status NOT IN ('fulfilled', 'cancelled')",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .map_err(map_db_error)?;

        let overdue: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM backorders
                 WHERE status NOT IN ('fulfilled', 'cancelled')
                 AND expected_date IS NOT NULL AND expected_date < ?",
                [now.to_rfc3339()],
                |row| row.get(0),
            )
            .map_err(map_db_error)?;

        let total_quantity = sum_decimal_query(
            &conn,
            "SELECT quantity_remaining FROM backorders WHERE status NOT IN ('fulfilled', 'cancelled')",
            &[],
            "backorders",
            "quantity_remaining",
        )?;

        Ok(BackorderSummary {
            total_backorders: total,
            total_quantity,
            pending_count: pending,
            allocated_count: allocated,
            critical_count: critical,
            overdue_count: overdue,
        })
    }

    fn get_sku_summary(&self, sku: &str) -> Result<Option<SkuBackorderSummary>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let (count, oldest_date_raw, earliest_expected_raw): (i32, Option<String>, Option<String>) =
            conn.query_row(
                "SELECT COUNT(*), MIN(created_at), MIN(expected_date)
                 FROM backorders
                 WHERE sku = ? AND status NOT IN ('fulfilled', 'cancelled')",
                [sku],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(map_db_error)?;

        if count == 0 {
            return Ok(None);
        }

        let sku_param = sku.to_string();
        let sku_params: [&dyn rusqlite::ToSql; 1] = [&sku_param];
        let total_quantity = sum_decimal_query(
            &conn,
            "SELECT quantity_remaining FROM backorders WHERE sku = ? AND status NOT IN ('fulfilled', 'cancelled')",
            &sku_params,
            "backorders",
            "quantity_remaining",
        )?;
        let oldest_date =
            parse_datetime_opt(oldest_date_raw, "sku_backorder_summary", "oldest_date")?;
        let earliest_expected = parse_datetime_opt(
            earliest_expected_raw,
            "sku_backorder_summary",
            "earliest_expected",
        )?;

        Ok(Some(SkuBackorderSummary {
            sku: sku.to_string(),
            total_quantity,
            backorder_count: count,
            oldest_date,
            earliest_expected,
        }))
    }

    fn get_overdue_backorders(&self) -> Result<Vec<Backorder>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let now = Utc::now();

        let mut stmt = conn
            .prepare(
                "SELECT id, backorder_number, order_id, order_line_id, customer_id, sku,
                    quantity_ordered, quantity_fulfilled, quantity_remaining, status, priority,
                    expected_date, promised_date, source_location_id, notes, created_at, updated_at
             FROM backorders
             WHERE status NOT IN ('fulfilled', 'cancelled')
             AND expected_date IS NOT NULL AND expected_date < ?
             ORDER BY expected_date",
            )
            .map_err(map_db_error)?;

        let rows = stmt
            .query_map([now.to_rfc3339()], |row| self.row_to_backorder(row))
            .map_err(map_db_error)?;

        let mut backorders = Vec::new();
        for row in rows {
            backorders.push(row.map_err(map_db_error)?);
        }
        Ok(backorders)
    }

    fn count_pending(&self) -> Result<u64> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM backorders WHERE status = 'pending'", [], |row| {
                row.get(0)
            })
            .map_err(map_db_error)?;
        Ok(count as u64)
    }
}
