//! PostgreSQL production batch repository implementation

use super::map_db_error;
use chrono::{DateTime, Utc};
use sqlx::FromRow;
use sqlx::postgres::PgPool;
use stateset_core::{
    CommerceError, CreateProductionBatch, ProductionBatch, ProductionBatchFilter,
    ProductionBatchId, ProductionBatchRepository, ProductionBatchStatus, Result,
    UpdateProductionBatch,
};
use uuid::Uuid;

/// PostgreSQL implementation of `ProductionBatchRepository`
#[derive(Debug, Clone)]
pub struct PgProductionBatchRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct ProductionBatchRow {
    id: Uuid,
    name: String,
    status: String,
    vendor_id: Option<Uuid>,
    work_order_ids: serde_json::Value,
    notes: Option<String>,
    scheduled_start: Option<DateTime<Utc>>,
    scheduled_end: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl PgProductionBatchRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_batch(row: ProductionBatchRow) -> Result<ProductionBatch> {
        let status: ProductionBatchStatus = row.status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid production_batch.status '{}': {}",
                row.status, e
            ))
        })?;
        let work_order_ids: Vec<Uuid> =
            serde_json::from_value(row.work_order_ids).map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid production_batch.work_order_ids: {e}"
                ))
            })?;
        Ok(ProductionBatch {
            id: row.id.into(),
            name: row.name,
            status,
            vendor_id: row.vendor_id,
            work_order_ids,
            notes: row.notes,
            scheduled_start: row.scheduled_start,
            scheduled_end: row.scheduled_end,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    async fn fetch_async(&self, id: Uuid) -> Result<Option<ProductionBatch>> {
        let row = sqlx::query_as::<_, ProductionBatchRow>(
            "SELECT * FROM production_batches WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;
        row.map(Self::row_to_batch).transpose()
    }

    /// Create a production batch (async)
    pub async fn create_async(&self, input: CreateProductionBatch) -> Result<ProductionBatch> {
        let id = ProductionBatchId::new();
        let now = Utc::now();
        let work_order_ids = serde_json::to_value(&input.work_order_ids)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        sqlx::query(
            "INSERT INTO production_batches (id, name, status, vendor_id, work_order_ids, notes, scheduled_start, scheduled_end, created_at, updated_at)
             VALUES ($1, $2, 'planned', $3, $4, $5, $6, $7, $8, $8)",
        )
        .bind(Uuid::from(id))
        .bind(&input.name)
        .bind(input.vendor_id)
        .bind(&work_order_ids)
        .bind(&input.notes)
        .bind(input.scheduled_start)
        .bind(input.scheduled_end)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.fetch_async(id.into()).await?.ok_or(CommerceError::NotFound)
    }

    /// Get a production batch by ID (async)
    pub async fn get_async(&self, id: ProductionBatchId) -> Result<Option<ProductionBatch>> {
        self.fetch_async(id.into()).await
    }

    /// Update a production batch (async, partial)
    pub async fn update_async(
        &self,
        id: ProductionBatchId,
        input: UpdateProductionBatch,
    ) -> Result<ProductionBatch> {
        let existing = self.fetch_async(id.into()).await?.ok_or(CommerceError::NotFound)?;
        let now = Utc::now();

        let name = input.name.unwrap_or(existing.name);
        let vendor_id = input.vendor_id.or(existing.vendor_id);
        let status = input.status.unwrap_or(existing.status);
        let notes = input.notes.or(existing.notes);
        let scheduled_start = input.scheduled_start.or(existing.scheduled_start);
        let scheduled_end = input.scheduled_end.or(existing.scheduled_end);

        sqlx::query(
            "UPDATE production_batches SET name = $1, vendor_id = $2, status = $3, notes = $4, scheduled_start = $5, scheduled_end = $6, updated_at = $7 WHERE id = $8",
        )
        .bind(&name)
        .bind(vendor_id)
        .bind(status.to_string())
        .bind(&notes)
        .bind(scheduled_start)
        .bind(scheduled_end)
        .bind(now)
        .bind(Uuid::from(id))
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.fetch_async(id.into()).await?.ok_or(CommerceError::NotFound)
    }

    /// List production batches (async)
    pub async fn list_async(&self, filter: ProductionBatchFilter) -> Result<Vec<ProductionBatch>> {
        let limit = super::effective_limit(filter.limit);
        let offset = i64::from(filter.offset.unwrap_or(0));

        let mut query = String::from("SELECT * FROM production_batches WHERE 1=1");
        let mut param_idx = 1;
        if filter.status.is_some() {
            query.push_str(&format!(" AND status = ${param_idx}"));
            param_idx += 1;
        }
        if filter.vendor_id.is_some() {
            query.push_str(&format!(" AND vendor_id = ${param_idx}"));
            param_idx += 1;
        }
        query.push_str(&format!(
            " ORDER BY created_at DESC LIMIT ${} OFFSET ${}",
            param_idx,
            param_idx + 1
        ));

        let mut q = sqlx::query_as::<_, ProductionBatchRow>(&query);
        if let Some(status) = filter.status {
            q = q.bind(status.to_string());
        }
        if let Some(vendor_id) = filter.vendor_id {
            q = q.bind(vendor_id);
        }
        let rows = q.bind(limit).bind(offset).fetch_all(&self.pool).await.map_err(map_db_error)?;
        rows.into_iter().map(Self::row_to_batch).collect()
    }

    /// Delete a production batch (async)
    pub async fn delete_async(&self, id: ProductionBatchId) -> Result<()> {
        sqlx::query("DELETE FROM production_batches WHERE id = $1")
            .bind(Uuid::from(id))
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;
        Ok(())
    }

    /// Add work orders to a batch, ignoring duplicates (async)
    pub async fn add_work_orders_async(
        &self,
        id: ProductionBatchId,
        work_order_ids: Vec<Uuid>,
    ) -> Result<ProductionBatch> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let row = sqlx::query_as::<_, ProductionBatchRow>(
            "SELECT * FROM production_batches WHERE id = $1 FOR UPDATE",
        )
        .bind(Uuid::from(id))
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .ok_or(CommerceError::NotFound)?;
        let batch = Self::row_to_batch(row)?;

        let mut ids = batch.work_order_ids;
        for wo in work_order_ids {
            if !ids.contains(&wo) {
                ids.push(wo);
            }
        }
        Self::store_work_orders(&mut tx, id, &ids).await?;
        tx.commit().await.map_err(map_db_error)?;
        self.fetch_async(id.into()).await?.ok_or(CommerceError::NotFound)
    }

    /// Remove a work order from a batch (async)
    pub async fn remove_work_order_async(
        &self,
        id: ProductionBatchId,
        work_order_id: Uuid,
    ) -> Result<ProductionBatch> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let row = sqlx::query_as::<_, ProductionBatchRow>(
            "SELECT * FROM production_batches WHERE id = $1 FOR UPDATE",
        )
        .bind(Uuid::from(id))
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .ok_or(CommerceError::NotFound)?;
        let batch = Self::row_to_batch(row)?;

        let mut ids = batch.work_order_ids;
        ids.retain(|w| *w != work_order_id);
        Self::store_work_orders(&mut tx, id, &ids).await?;
        tx.commit().await.map_err(map_db_error)?;
        self.fetch_async(id.into()).await?.ok_or(CommerceError::NotFound)
    }

    async fn store_work_orders(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: ProductionBatchId,
        ids: &[Uuid],
    ) -> Result<()> {
        let json =
            serde_json::to_value(ids).map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        sqlx::query(
            "UPDATE production_batches SET work_order_ids = $1, updated_at = $2 WHERE id = $3",
        )
        .bind(&json)
        .bind(Utc::now())
        .bind(Uuid::from(id))
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        Ok(())
    }
}

impl ProductionBatchRepository for PgProductionBatchRepository {
    fn create(&self, input: CreateProductionBatch) -> Result<ProductionBatch> {
        super::block_on(self.create_async(input))
    }

    fn get(&self, id: ProductionBatchId) -> Result<Option<ProductionBatch>> {
        super::block_on(self.get_async(id))
    }

    fn update(
        &self,
        id: ProductionBatchId,
        input: UpdateProductionBatch,
    ) -> Result<ProductionBatch> {
        super::block_on(self.update_async(id, input))
    }

    fn list(&self, filter: ProductionBatchFilter) -> Result<Vec<ProductionBatch>> {
        super::block_on(self.list_async(filter))
    }

    fn delete(&self, id: ProductionBatchId) -> Result<()> {
        super::block_on(self.delete_async(id))
    }

    fn add_work_orders(
        &self,
        id: ProductionBatchId,
        work_order_ids: Vec<Uuid>,
    ) -> Result<ProductionBatch> {
        super::block_on(self.add_work_orders_async(id, work_order_ids))
    }

    fn remove_work_order(
        &self,
        id: ProductionBatchId,
        work_order_id: Uuid,
    ) -> Result<ProductionBatch> {
        super::block_on(self.remove_work_order_async(id, work_order_id))
    }
}
