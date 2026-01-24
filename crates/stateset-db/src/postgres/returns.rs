//! PostgreSQL returns repository implementation

use super::map_db_error;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;
use sqlx::FromRow;
use stateset_core::{
    validate_batch_size, BatchResult, CommerceError, CreateReturn, CreateReturnItem, ItemCondition,
    Result, Return, ReturnFilter, ReturnItem, ReturnReason, ReturnRepository, ReturnStatus,
    UpdateReturn,
};
use uuid::Uuid;

/// PostgreSQL implementation of ReturnRepository
#[derive(Clone)]
pub struct PgReturnRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct ReturnRow {
    id: Uuid,
    order_id: Uuid,
    customer_id: Uuid,
    status: String,
    reason: String,
    reason_details: Option<String>,
    idempotency_key: Option<String>,
    refund_amount: Option<Decimal>,
    refund_method: Option<String>,
    tracking_number: Option<String>,
    notes: Option<String>,
    version: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct ReturnItemRow {
    id: Uuid,
    return_id: Uuid,
    order_item_id: Uuid,
    sku: String,
    name: String,
    quantity: i32,
    condition: String,
    refund_amount: Decimal,
}

impl PgReturnRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_return(row: ReturnRow, items: Vec<ReturnItem>) -> Result<Return> {
        let ReturnRow {
            id,
            order_id,
            customer_id,
            status,
            reason,
            reason_details,
            idempotency_key,
            refund_amount,
            refund_method,
            tracking_number,
            notes,
            version,
            created_at,
            updated_at,
        } = row;

        let status: ReturnStatus = status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid return.status '{}': {}", status, e))
        })?;
        let reason: ReturnReason = reason.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid return.reason '{}': {}", reason, e))
        })?;

        Ok(Return {
            id,
            order_id,
            customer_id,
            status,
            reason,
            reason_details,
            idempotency_key,
            refund_amount,
            refund_method,
            tracking_number,
            items,
            notes,
            version,
            created_at,
            updated_at,
        })
    }

    fn row_to_item(row: ReturnItemRow) -> Result<ReturnItem> {
        let ReturnItemRow {
            id,
            return_id,
            order_item_id,
            sku,
            name,
            quantity,
            condition,
            refund_amount,
        } = row;

        let condition: ItemCondition = condition.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid return_item.condition '{}': {}",
                condition, e
            ))
        })?;

        Ok(ReturnItem {
            id,
            return_id,
            order_item_id,
            sku,
            name,
            quantity,
            condition,
            refund_amount,
        })
    }

    /// Create a return (async)
    pub async fn create_async(&self, input: CreateReturn) -> Result<Return> {
        if let Some(key) = input.idempotency_key.as_deref() {
            if let Some(existing) = self.get_by_idempotency_key_async(key).await? {
                return Ok(existing);
            }
        }

        let id = Uuid::new_v4();
        let now = Utc::now();

        // Get customer_id from order
        let order_info: (Uuid,) =
            sqlx::query_as("SELECT customer_id FROM orders WHERE id = $1")
                .bind(input.order_id)
                .fetch_one(&self.pool)
                .await
                .map_err(|_| CommerceError::OrderNotFound(input.order_id))?;

        let customer_id = order_info.0;

        sqlx::query(
            r#"
            INSERT INTO returns (id, order_id, customer_id, status, reason, reason_details, idempotency_key, notes, created_at, updated_at)
            VALUES ($1, $2, $3, 'requested', $4, $5, $6, $7, $8, $9)
            "#,
        )
        .bind(id)
        .bind(input.order_id)
        .bind(customer_id)
        .bind(input.reason.to_string())
        .bind(&input.reason_details)
        .bind(&input.idempotency_key)
        .bind(&input.notes)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        // Create return items
        let mut items = Vec::new();
        for item_input in input.items {
            let item = self.create_item_internal(id, item_input).await?;
            items.push(item);
        }

        Ok(Return {
            id,
            order_id: input.order_id,
            customer_id,
            status: ReturnStatus::Requested,
            reason: input.reason,
            reason_details: input.reason_details,
            idempotency_key: input.idempotency_key,
            refund_amount: None,
            refund_method: None,
            tracking_number: None,
            items,
            notes: input.notes,
            version: 1,
            created_at: now,
            updated_at: now,
        })
    }

    async fn create_item_internal(
        &self,
        return_id: Uuid,
        input: CreateReturnItem,
    ) -> Result<ReturnItem> {
        let id = Uuid::new_v4();

        // Get order item details
        let item_info: (String, String, Decimal) =
            sqlx::query_as("SELECT sku, name, unit_price FROM order_items WHERE id = $1")
                .bind(input.order_item_id)
                .fetch_one(&self.pool)
                .await
                .map_err(map_db_error)?;

        let refund = item_info.2 * Decimal::from(input.quantity);
        let condition = input.condition.unwrap_or(ItemCondition::New);

        sqlx::query(
            r#"
            INSERT INTO return_items (id, return_id, order_item_id, sku, name, quantity, condition, refund_amount)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
        .bind(id)
        .bind(return_id)
        .bind(input.order_item_id)
        .bind(&item_info.0)
        .bind(&item_info.1)
        .bind(input.quantity)
        .bind(condition.to_string())
        .bind(refund)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(ReturnItem {
            id,
            return_id,
            order_item_id: input.order_item_id,
            sku: item_info.0,
            name: item_info.1,
            quantity: input.quantity,
            condition,
            refund_amount: refund,
        })
    }

    async fn get_by_idempotency_key_async(&self, key: &str) -> Result<Option<Return>> {
        let row = sqlx::query_as::<_, ReturnRow>("SELECT * FROM returns WHERE idempotency_key = $1")
            .bind(key)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        let Some(row) = row else {
            return Ok(None);
        };

        let items = self.get_items_async(row.id).await?;
        Ok(Some(Self::row_to_return(row, items)?))
    }

    /// Get a return by ID (async)
    pub async fn get_async(&self, id: Uuid) -> Result<Option<Return>> {
        let row = sqlx::query_as::<_, ReturnRow>("SELECT * FROM returns WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        match row {
            Some(return_row) => {
                let items = self.get_items_async(id).await?;
                Ok(Some(Self::row_to_return(return_row, items)?))
            }
            None => Ok(None),
        }
    }

    /// Get return items (async)
    pub async fn get_items_async(&self, return_id: Uuid) -> Result<Vec<ReturnItem>> {
        let rows = sqlx::query_as::<_, ReturnItemRow>(
            "SELECT * FROM return_items WHERE return_id = $1",
        )
        .bind(return_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            items.push(Self::row_to_item(row)?);
        }
        Ok(items)
    }

    /// Update a return (async)
    pub async fn update_async(&self, id: Uuid, input: UpdateReturn) -> Result<Return> {
        let now = Utc::now();

        let existing = self.get_async(id).await?.ok_or(CommerceError::ReturnNotFound(id))?;

        let new_status = input.status.unwrap_or(existing.status);
        let new_tracking = input.tracking_number.or(existing.tracking_number);
        let new_refund_amount = input.refund_amount.or(existing.refund_amount);
        let new_refund_method = input.refund_method.or(existing.refund_method);
        let new_notes = input.notes.or(existing.notes);

        sqlx::query(
            r#"
            UPDATE returns
            SET status = $1, tracking_number = $2, refund_amount = $3,
                refund_method = $4, notes = $5, updated_at = $6
            WHERE id = $7
            "#,
        )
        .bind(new_status.to_string())
        .bind(&new_tracking)
        .bind(new_refund_amount)
        .bind(&new_refund_method)
        .bind(&new_notes)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::ReturnNotFound(id))
    }

    /// List returns (async)
    pub async fn list_async(&self, filter: ReturnFilter) -> Result<Vec<Return>> {
        let limit = filter.limit.unwrap_or(100) as i64;
        let offset = filter.offset.unwrap_or(0) as i64;

        let rows = sqlx::query_as::<_, ReturnRow>(
            "SELECT * FROM returns ORDER BY created_at DESC LIMIT $1 OFFSET $2",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        let mut returns = Vec::new();
        for row in rows {
            let items = self.get_items_async(row.id).await?;
            returns.push(Self::row_to_return(row, items)?);
        }

        Ok(returns)
    }

    /// Approve a return (async)
    pub async fn approve_async(&self, id: Uuid) -> Result<Return> {
        sqlx::query("UPDATE returns SET status = 'approved', updated_at = $1 WHERE id = $2")
            .bind(Utc::now())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::ReturnNotFound(id))
    }

    /// Reject a return (async)
    pub async fn reject_async(&self, id: Uuid, reason: &str) -> Result<Return> {
        sqlx::query("UPDATE returns SET status = 'rejected', notes = $1, updated_at = $2 WHERE id = $3")
            .bind(reason)
            .bind(Utc::now())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::ReturnNotFound(id))
    }

    /// Complete a return (async)
    pub async fn complete_async(&self, id: Uuid) -> Result<Return> {
        sqlx::query("UPDATE returns SET status = 'completed', updated_at = $1 WHERE id = $2")
            .bind(Utc::now())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::ReturnNotFound(id))
    }

    /// Cancel a return (async)
    pub async fn cancel_async(&self, id: Uuid) -> Result<Return> {
        sqlx::query("UPDATE returns SET status = 'cancelled', updated_at = $1 WHERE id = $2")
            .bind(Utc::now())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::ReturnNotFound(id))
    }

    /// Count returns (async)
    pub async fn count_async(&self, _filter: ReturnFilter) -> Result<u64> {
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM returns")
            .fetch_one(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(count.0 as u64)
    }

    /// Delete a return (async)
    pub async fn delete_async(&self, id: Uuid) -> Result<()> {
        // First delete return items
        sqlx::query("DELETE FROM return_items WHERE return_id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        // Then delete the return
        sqlx::query("DELETE FROM returns WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(())
    }

    // =========================================================================
    // Batch Operations (async)
    // =========================================================================

    /// Create multiple returns - partial success allowed (async)
    pub async fn create_batch_async(&self, inputs: Vec<CreateReturn>) -> Result<BatchResult<Return>> {
        validate_batch_size(&inputs)?;

        let mut result = BatchResult::with_capacity(inputs.len());

        for (index, input) in inputs.into_iter().enumerate() {
            match self.create_async(input).await {
                Ok(ret) => result.record_success(ret),
                Err(e) => result.record_failure(index, None, &e),
            }
        }

        Ok(result)
    }

    /// Create multiple returns - atomic (all-or-nothing) (async)
    pub async fn create_batch_atomic_async(&self, inputs: Vec<CreateReturn>) -> Result<Vec<Return>> {
        validate_batch_size(&inputs)?;

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let mut returns = Vec::with_capacity(inputs.len());

        for input in inputs {
            let id = Uuid::new_v4();
            let now = Utc::now();

            // Get customer_id from order
            let order_info: (Uuid,) =
                sqlx::query_as("SELECT customer_id FROM orders WHERE id = $1")
                    .bind(input.order_id)
                    .fetch_one(tx.as_mut())
                    .await
                    .map_err(|_| CommerceError::OrderNotFound(input.order_id))?;

            let customer_id = order_info.0;

            sqlx::query(
                r#"
                INSERT INTO returns (id, order_id, customer_id, status, reason, reason_details, idempotency_key, notes, created_at, updated_at)
                VALUES ($1, $2, $3, 'requested', $4, $5, $6, $7, $8, $9)
                "#,
            )
            .bind(id)
            .bind(input.order_id)
            .bind(customer_id)
            .bind(input.reason.to_string())
            .bind(&input.reason_details)
            .bind(&input.idempotency_key)
            .bind(&input.notes)
            .bind(now)
            .bind(now)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

            // Create return items within transaction
            let mut items = Vec::new();
            for item_input in input.items.clone() {
                let item_id = Uuid::new_v4();

                // Get order item details
                let item_info: (String, String, Decimal) =
                    sqlx::query_as("SELECT sku, name, unit_price FROM order_items WHERE id = $1")
                        .bind(item_input.order_item_id)
                        .fetch_one(tx.as_mut())
                        .await
                        .map_err(map_db_error)?;

                let refund = item_info.2 * Decimal::from(item_input.quantity);
                let condition = item_input.condition.unwrap_or(ItemCondition::New);

                sqlx::query(
                    r#"
                    INSERT INTO return_items (id, return_id, order_item_id, sku, name, quantity, condition, refund_amount)
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                    "#,
                )
                .bind(item_id)
                .bind(id)
                .bind(item_input.order_item_id)
                .bind(&item_info.0)
                .bind(&item_info.1)
                .bind(item_input.quantity)
                .bind(condition.to_string())
                .bind(refund)
                .execute(tx.as_mut())
                .await
                .map_err(map_db_error)?;

                items.push(ReturnItem {
                    id: item_id,
                    return_id: id,
                    order_item_id: item_input.order_item_id,
                    sku: item_info.0,
                    name: item_info.1,
                    quantity: item_input.quantity,
                    condition,
                    refund_amount: refund,
                });
            }

            returns.push(Return {
                id,
                order_id: input.order_id,
                customer_id,
                status: ReturnStatus::Requested,
                reason: input.reason,
                reason_details: input.reason_details,
                idempotency_key: input.idempotency_key,
                refund_amount: None,
                refund_method: None,
                tracking_number: None,
                items,
                notes: input.notes,
                version: 1,
                created_at: now,
                updated_at: now,
            });
        }

        tx.commit().await.map_err(map_db_error)?;
        Ok(returns)
    }

    /// Update multiple returns - partial success allowed (async)
    pub async fn update_batch_async(&self, updates: Vec<(Uuid, UpdateReturn)>) -> Result<BatchResult<Return>> {
        validate_batch_size(&updates)?;

        let mut result = BatchResult::with_capacity(updates.len());

        for (index, (id, input)) in updates.into_iter().enumerate() {
            match self.update_async(id, input).await {
                Ok(ret) => result.record_success(ret),
                Err(e) => result.record_failure(index, Some(id.to_string()), &e),
            }
        }

        Ok(result)
    }

    /// Update multiple returns - atomic (all-or-nothing) (async)
    pub async fn update_batch_atomic_async(&self, updates: Vec<(Uuid, UpdateReturn)>) -> Result<Vec<Return>> {
        validate_batch_size(&updates)?;

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let mut returns = Vec::with_capacity(updates.len());

        for (id, input) in updates {
            let now = Utc::now();

            let existing_row = sqlx::query_as::<_, ReturnRow>("SELECT * FROM returns WHERE id = $1")
                .bind(id)
                .fetch_optional(tx.as_mut())
                .await
                .map_err(map_db_error)?
                .ok_or(CommerceError::ReturnNotFound(id))?;

            let items = sqlx::query_as::<_, ReturnItemRow>(
                "SELECT * FROM return_items WHERE return_id = $1",
            )
            .bind(id)
            .fetch_all(tx.as_mut())
            .await
            .map_err(map_db_error)?;

            let mut existing_items = Vec::with_capacity(items.len());
            for item in items {
                existing_items.push(Self::row_to_item(item)?);
            }
            let existing = Self::row_to_return(existing_row, existing_items.clone())?;

            let new_status = input.status.unwrap_or(existing.status);
            let new_tracking = input.tracking_number.or(existing.tracking_number);
            let new_refund_amount = input.refund_amount.or(existing.refund_amount);
            let new_refund_method = input.refund_method.or(existing.refund_method);
            let new_notes = input.notes.or(existing.notes);

            sqlx::query(
                r#"
                UPDATE returns
                SET status = $1, tracking_number = $2, refund_amount = $3,
                    refund_method = $4, notes = $5, updated_at = $6
                WHERE id = $7
                "#,
            )
            .bind(new_status.to_string())
            .bind(&new_tracking)
            .bind(new_refund_amount)
            .bind(&new_refund_method)
            .bind(&new_notes)
            .bind(now)
            .bind(id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

            // Fetch the updated return
            let updated_row = sqlx::query_as::<_, ReturnRow>("SELECT * FROM returns WHERE id = $1")
                .bind(id)
                .fetch_one(tx.as_mut())
                .await
                .map_err(map_db_error)?;

            returns.push(Self::row_to_return(updated_row, existing_items)?);
        }

        tx.commit().await.map_err(map_db_error)?;
        Ok(returns)
    }

    /// Delete multiple returns - partial success allowed (async)
    pub async fn delete_batch_async(&self, ids: Vec<Uuid>) -> Result<BatchResult<Uuid>> {
        validate_batch_size(&ids)?;

        let mut result = BatchResult::with_capacity(ids.len());

        for (index, id) in ids.into_iter().enumerate() {
            match self.delete_async(id).await {
                Ok(()) => result.record_success(id),
                Err(e) => result.record_failure(index, Some(id.to_string()), &e),
            }
        }

        Ok(result)
    }

    /// Delete multiple returns - atomic (all-or-nothing) (async)
    pub async fn delete_batch_atomic_async(&self, ids: Vec<Uuid>) -> Result<()> {
        validate_batch_size(&ids)?;

        if ids.is_empty() {
            return Ok(());
        }

        // First delete return items for all returns
        sqlx::query("DELETE FROM return_items WHERE return_id = ANY($1)")
            .bind(&ids)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        // Then delete the returns
        sqlx::query("DELETE FROM returns WHERE id = ANY($1)")
            .bind(&ids)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(())
    }

    /// Get multiple returns by ID (async)
    pub async fn get_batch_async(&self, ids: Vec<Uuid>) -> Result<Vec<Return>> {
        validate_batch_size(&ids)?;

        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let rows = sqlx::query_as::<_, ReturnRow>("SELECT * FROM returns WHERE id = ANY($1)")
            .bind(&ids)
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        let mut returns = Vec::new();
        for row in rows {
            let items = self.get_items_async(row.id).await?;
            returns.push(Self::row_to_return(row, items)?);
        }

        Ok(returns)
    }
}

impl ReturnRepository for PgReturnRepository {
    fn create(&self, input: CreateReturn) -> Result<Return> {
        super::block_on(self.create_async(input))
    }

    fn get(&self, id: Uuid) -> Result<Option<Return>> {
        super::block_on(self.get_async(id))
    }

    fn update(&self, id: Uuid, input: UpdateReturn) -> Result<Return> {
        super::block_on(self.update_async(id, input))
    }

    fn list(&self, filter: ReturnFilter) -> Result<Vec<Return>> {
        super::block_on(self.list_async(filter))
    }

    fn approve(&self, id: Uuid) -> Result<Return> {
        super::block_on(self.approve_async(id))
    }

    fn reject(&self, id: Uuid, reason: &str) -> Result<Return> {
        super::block_on(self.reject_async(id, reason))
    }

    fn complete(&self, id: Uuid) -> Result<Return> {
        super::block_on(self.complete_async(id))
    }

    fn cancel(&self, id: Uuid) -> Result<Return> {
        super::block_on(self.cancel_async(id))
    }

    fn count(&self, filter: ReturnFilter) -> Result<u64> {
        super::block_on(self.count_async(filter))
    }

    // =========================================================================
    // Batch Operations
    // =========================================================================

    fn create_batch(&self, inputs: Vec<CreateReturn>) -> Result<BatchResult<Return>> {
        super::block_on(self.create_batch_async(inputs))
    }

    fn create_batch_atomic(&self, inputs: Vec<CreateReturn>) -> Result<Vec<Return>> {
        super::block_on(self.create_batch_atomic_async(inputs))
    }

    fn update_batch(&self, updates: Vec<(Uuid, UpdateReturn)>) -> Result<BatchResult<Return>> {
        super::block_on(self.update_batch_async(updates))
    }

    fn update_batch_atomic(&self, updates: Vec<(Uuid, UpdateReturn)>) -> Result<Vec<Return>> {
        super::block_on(self.update_batch_atomic_async(updates))
    }

    fn delete_batch(&self, ids: Vec<Uuid>) -> Result<BatchResult<Uuid>> {
        super::block_on(self.delete_batch_async(ids))
    }

    fn delete_batch_atomic(&self, ids: Vec<Uuid>) -> Result<()> {
        super::block_on(self.delete_batch_atomic_async(ids))
    }

    fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<Return>> {
        super::block_on(self.get_batch_async(ids))
    }
}
