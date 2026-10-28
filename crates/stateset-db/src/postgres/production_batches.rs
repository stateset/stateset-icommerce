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
    unresolved_links: bool,
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
        if row.unresolved_links {
            return Err(CommerceError::ValidationError(
                "Legacy production batch has missing work-order links; reconcile before reading"
                    .into(),
            ));
        }
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
            "SELECT * FROM production_batch_state WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;
        row.map(Self::row_to_batch).transpose()
    }

    /// Create a production batch (async)
    pub async fn create_async(&self, input: CreateProductionBatch) -> Result<ProductionBatch> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
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
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        super::production_batch_members::add(&mut tx, id.into(), &input.work_order_ids, now)
            .await?;
        tx.commit().await.map_err(map_db_error)?;
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
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let now = Utc::now();
        super::production_batch_members::update_status(&mut tx, id.into(), input.status, now)
            .await?;
        sqlx::query("UPDATE production_batches SET name = COALESCE($1, name), vendor_id = COALESCE($2, vendor_id), notes = COALESCE($3, notes), scheduled_start = COALESCE($4, scheduled_start), scheduled_end = COALESCE($5, scheduled_end), updated_at = $6 WHERE id = $7")
            .bind(input.name).bind(input.vendor_id).bind(input.notes).bind(input.scheduled_start).bind(input.scheduled_end).bind(now).bind(Uuid::from(id)).execute(tx.as_mut()).await.map_err(map_db_error)?;
        tx.commit().await.map_err(map_db_error)?;
        self.fetch_async(id.into()).await?.ok_or(CommerceError::NotFound)
    }

    /// List production batches (async)
    pub async fn list_async(&self, filter: ProductionBatchFilter) -> Result<Vec<ProductionBatch>> {
        let limit = super::effective_limit(filter.limit);
        let offset = i64::from(filter.offset.unwrap_or(0));

        let mut query = String::from("SELECT * FROM production_batch_state WHERE 1=1");
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
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        super::production_batch_members::guard(&mut tx, id.into(), true, Utc::now()).await?;
        sqlx::query("DELETE FROM production_batches WHERE id = $1")
            .bind(Uuid::from(id))
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        tx.commit().await.map_err(map_db_error)?;
        Ok(())
    }

    /// Add work orders to a batch, ignoring duplicates (async)
    pub async fn add_work_orders_async(
        &self,
        id: ProductionBatchId,
        work_order_ids: Vec<Uuid>,
    ) -> Result<ProductionBatch> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let now = Utc::now();
        super::production_batch_members::guard(&mut tx, id.into(), true, now).await?;
        super::production_batch_members::add(&mut tx, id.into(), &work_order_ids, now).await?;
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
        let now = Utc::now();
        super::production_batch_members::guard(&mut tx, id.into(), true, now).await?;
        sqlx::query(
            "DELETE FROM production_batch_work_orders WHERE batch_id = $1 AND work_order_id = $2",
        )
        .bind(Uuid::from(id))
        .bind(work_order_id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        super::production_batch_members::sync(&mut tx, id.into(), now).await?;
        tx.commit().await.map_err(map_db_error)?;
        self.fetch_async(id.into()).await?.ok_or(CommerceError::NotFound)
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
