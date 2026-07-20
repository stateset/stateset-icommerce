//! PostgreSQL supplier SKU repository implementation

use super::map_db_error;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::FromRow;
use sqlx::postgres::PgPool;
use stateset_core::{
    BulkSupplierSkuItem, CommerceError, CreateSupplierSku, CurrencyCode, Result, SupplierSku,
    SupplierSkuFilter, SupplierSkuId, SupplierSkuRepository, UpdateSupplierSku,
};
use uuid::Uuid;

/// PostgreSQL implementation of `SupplierSkuRepository`
#[derive(Debug, Clone)]
pub struct PgSupplierSkuRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct SupplierSkuRow {
    id: Uuid,
    product_id: Uuid,
    supplier_id: Uuid,
    sku: String,
    unit_cost: Option<Decimal>,
    currency: String,
    min_order_qty: Option<Decimal>,
    lead_time_days: Option<i32>,
    is_preferred: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl PgSupplierSkuRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_sku(row: SupplierSkuRow) -> Result<SupplierSku> {
        let currency: CurrencyCode = row.currency.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid supplier_sku.currency '{}': {}",
                row.currency, e
            ))
        })?;
        Ok(SupplierSku {
            id: row.id.into(),
            product_id: row.product_id.into(),
            supplier_id: row.supplier_id,
            sku: row.sku,
            unit_cost: row.unit_cost,
            currency,
            min_order_qty: row.min_order_qty,
            lead_time_days: row.lead_time_days,
            is_preferred: row.is_preferred,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    async fn fetch_async(&self, id: Uuid) -> Result<Option<SupplierSku>> {
        let row = sqlx::query_as::<_, SupplierSkuRow>("SELECT * FROM supplier_skus WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;
        row.map(Self::row_to_sku).transpose()
    }

    /// Create a supplier SKU (async)
    pub async fn create_async(&self, input: CreateSupplierSku) -> Result<SupplierSku> {
        let id = SupplierSkuId::new();
        let now = Utc::now();
        let currency = input.currency.unwrap_or(CurrencyCode::USD);

        sqlx::query(
            "INSERT INTO supplier_skus (id, product_id, supplier_id, sku, unit_cost, currency, min_order_qty, lead_time_days, is_preferred, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, FALSE, $9, $9)",
        )
        .bind(Uuid::from(id))
        .bind(Uuid::from(input.product_id))
        .bind(input.supplier_id)
        .bind(&input.sku)
        .bind(input.unit_cost)
        .bind(currency.to_string())
        .bind(input.min_order_qty)
        .bind(input.lead_time_days)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.fetch_async(id.into()).await?.ok_or(CommerceError::NotFound)
    }

    /// Get a supplier SKU by ID (async)
    pub async fn get_async(&self, id: SupplierSkuId) -> Result<Option<SupplierSku>> {
        self.fetch_async(id.into()).await
    }

    /// Update a supplier SKU (async, partial)
    pub async fn update_async(
        &self,
        id: SupplierSkuId,
        input: UpdateSupplierSku,
    ) -> Result<SupplierSku> {
        let existing = self.fetch_async(id.into()).await?.ok_or(CommerceError::NotFound)?;
        let now = Utc::now();

        let sku = input.sku.unwrap_or(existing.sku);
        let unit_cost = input.unit_cost.or(existing.unit_cost);
        let currency = input.currency.unwrap_or(existing.currency);
        let min_order_qty = input.min_order_qty.or(existing.min_order_qty);
        let lead_time_days = input.lead_time_days.or(existing.lead_time_days);
        let is_preferred = input.is_preferred.unwrap_or(existing.is_preferred);

        sqlx::query(
            "UPDATE supplier_skus SET sku = $1, unit_cost = $2, currency = $3, min_order_qty = $4, lead_time_days = $5, is_preferred = $6, updated_at = $7 WHERE id = $8",
        )
        .bind(&sku)
        .bind(unit_cost)
        .bind(currency.to_string())
        .bind(min_order_qty)
        .bind(lead_time_days)
        .bind(is_preferred)
        .bind(now)
        .bind(Uuid::from(id))
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.fetch_async(id.into()).await?.ok_or(CommerceError::NotFound)
    }

    /// List supplier SKUs (async)
    pub async fn list_async(&self, filter: SupplierSkuFilter) -> Result<Vec<SupplierSku>> {
        let limit = i64::from(filter.limit.unwrap_or(100));
        let offset = i64::from(filter.offset.unwrap_or(0));

        let mut query = String::from("SELECT * FROM supplier_skus WHERE 1=1");
        let mut param_idx = 1;
        if filter.supplier_id.is_some() {
            query.push_str(&format!(" AND supplier_id = ${param_idx}"));
            param_idx += 1;
        }
        if filter.product_id.is_some() {
            query.push_str(&format!(" AND product_id = ${param_idx}"));
            param_idx += 1;
        }
        query.push_str(&format!(
            " ORDER BY is_preferred DESC, sku ASC LIMIT ${} OFFSET ${}",
            param_idx,
            param_idx + 1
        ));

        let mut q = sqlx::query_as::<_, SupplierSkuRow>(&query);
        if let Some(supplier_id) = filter.supplier_id {
            q = q.bind(supplier_id);
        }
        if let Some(product_id) = filter.product_id {
            q = q.bind(Uuid::from(product_id));
        }
        let rows = q.bind(limit).bind(offset).fetch_all(&self.pool).await.map_err(map_db_error)?;
        rows.into_iter().map(Self::row_to_sku).collect()
    }

    /// Delete a supplier SKU (async)
    pub async fn delete_async(&self, id: SupplierSkuId) -> Result<()> {
        sqlx::query("DELETE FROM supplier_skus WHERE id = $1")
            .bind(Uuid::from(id))
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;
        Ok(())
    }

    /// Bulk upsert supplier SKUs for one supplier (async)
    pub async fn bulk_upsert_async(
        &self,
        supplier_id: Uuid,
        items: Vec<BulkSupplierSkuItem>,
    ) -> Result<u64> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let now = Utc::now();
        let mut affected: u64 = 0;
        for item in &items {
            let result = sqlx::query(
                "INSERT INTO supplier_skus (id, product_id, supplier_id, sku, unit_cost, currency, is_preferred, created_at, updated_at)
                 VALUES ($1, $2, $3, $4, $5, 'USD', FALSE, $6, $6)
                 ON CONFLICT (product_id, supplier_id, sku) DO UPDATE SET
                    unit_cost = EXCLUDED.unit_cost,
                    updated_at = EXCLUDED.updated_at",
            )
            .bind(Uuid::from(SupplierSkuId::new()))
            .bind(Uuid::from(item.product_id))
            .bind(supplier_id)
            .bind(&item.sku)
            .bind(item.unit_cost)
            .bind(now)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
            affected += result.rows_affected();
        }
        tx.commit().await.map_err(map_db_error)?;
        Ok(affected)
    }
}

impl SupplierSkuRepository for PgSupplierSkuRepository {
    fn create(&self, input: CreateSupplierSku) -> Result<SupplierSku> {
        super::block_on(self.create_async(input))
    }

    fn get(&self, id: SupplierSkuId) -> Result<Option<SupplierSku>> {
        super::block_on(self.get_async(id))
    }

    fn update(&self, id: SupplierSkuId, input: UpdateSupplierSku) -> Result<SupplierSku> {
        super::block_on(self.update_async(id, input))
    }

    fn list(&self, filter: SupplierSkuFilter) -> Result<Vec<SupplierSku>> {
        super::block_on(self.list_async(filter))
    }

    fn delete(&self, id: SupplierSkuId) -> Result<()> {
        super::block_on(self.delete_async(id))
    }

    fn bulk_upsert(&self, supplier_id: Uuid, items: Vec<BulkSupplierSkuItem>) -> Result<u64> {
        super::block_on(self.bulk_upsert_async(supplier_id, items))
    }
}
