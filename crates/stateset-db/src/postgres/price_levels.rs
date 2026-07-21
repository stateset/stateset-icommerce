//! PostgreSQL price level repository implementation

use super::map_db_error;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::FromRow;
use sqlx::postgres::PgPool;
use stateset_core::{
    CommerceError, CreatePriceLevel, CurrencyCode, PriceAdjustmentType, PriceLevel,
    PriceLevelEntry, PriceLevelFilter, PriceLevelId, PriceLevelRepository, ProductId, Result,
    UpdatePriceLevel,
};
use uuid::Uuid;

/// PostgreSQL implementation of `PriceLevelRepository`
#[derive(Debug, Clone)]
pub struct PgPriceLevelRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct PriceLevelRow {
    id: Uuid,
    name: String,
    code: String,
    description: Option<String>,
    adjustment_type: String,
    adjustment_value: Decimal,
    currency: String,
    is_active: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct PriceLevelEntryRow {
    price_level_id: Uuid,
    product_id: Uuid,
    price: Decimal,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl PgPriceLevelRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_level(row: PriceLevelRow) -> Result<PriceLevel> {
        let adjustment_type: PriceAdjustmentType = row.adjustment_type.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid price_level.adjustment_type '{}': {}",
                row.adjustment_type, e
            ))
        })?;
        let currency: CurrencyCode = row.currency.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid price_level.currency '{}': {}",
                row.currency, e
            ))
        })?;
        Ok(PriceLevel {
            id: row.id.into(),
            name: row.name,
            code: row.code,
            description: row.description,
            adjustment_type,
            adjustment_value: row.adjustment_value,
            currency,
            is_active: row.is_active,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    const fn row_to_entry(row: PriceLevelEntryRow) -> PriceLevelEntry {
        PriceLevelEntry {
            price_level_id: PriceLevelId::from_uuid(row.price_level_id),
            product_id: ProductId::from_uuid(row.product_id),
            price: row.price,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }

    async fn fetch_async(&self, id: Uuid) -> Result<Option<PriceLevel>> {
        let row = sqlx::query_as::<_, PriceLevelRow>("SELECT * FROM price_levels WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;
        row.map(Self::row_to_level).transpose()
    }

    /// Create a price level (async)
    pub async fn create_async(&self, input: CreatePriceLevel) -> Result<PriceLevel> {
        let id = PriceLevelId::new();
        let now = Utc::now();
        let currency = input.currency.unwrap_or(CurrencyCode::USD);

        sqlx::query(
            "INSERT INTO price_levels (id, name, code, description, adjustment_type, adjustment_value, currency, is_active, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, TRUE, $8, $8)",
        )
        .bind(Uuid::from(id))
        .bind(&input.name)
        .bind(&input.code)
        .bind(&input.description)
        .bind(input.adjustment_type.to_string())
        .bind(input.adjustment_value)
        .bind(currency.to_string())
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.fetch_async(id.into()).await?.ok_or(CommerceError::NotFound)
    }

    /// Get a price level by ID (async)
    pub async fn get_async(&self, id: PriceLevelId) -> Result<Option<PriceLevel>> {
        self.fetch_async(id.into()).await
    }

    /// Update a price level (async, partial)
    pub async fn update_async(
        &self,
        id: PriceLevelId,
        input: UpdatePriceLevel,
    ) -> Result<PriceLevel> {
        let existing = self.fetch_async(id.into()).await?.ok_or(CommerceError::NotFound)?;
        let now = Utc::now();

        let name = input.name.unwrap_or(existing.name);
        let description = input.description.or(existing.description);
        let adjustment_type = input.adjustment_type.unwrap_or(existing.adjustment_type);
        let adjustment_value = input.adjustment_value.unwrap_or(existing.adjustment_value);
        let is_active = input.is_active.unwrap_or(existing.is_active);

        sqlx::query(
            "UPDATE price_levels SET name = $1, description = $2, adjustment_type = $3, adjustment_value = $4, is_active = $5, updated_at = $6 WHERE id = $7",
        )
        .bind(&name)
        .bind(&description)
        .bind(adjustment_type.to_string())
        .bind(adjustment_value)
        .bind(is_active)
        .bind(now)
        .bind(Uuid::from(id))
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.fetch_async(id.into()).await?.ok_or(CommerceError::NotFound)
    }

    /// List price levels (async)
    pub async fn list_async(&self, filter: PriceLevelFilter) -> Result<Vec<PriceLevel>> {
        let limit = super::effective_limit(filter.limit);
        let offset = i64::from(filter.offset.unwrap_or(0));

        let mut query = String::from("SELECT * FROM price_levels WHERE 1=1");
        let mut param_idx = 1;
        if filter.is_active.is_some() {
            query.push_str(&format!(" AND is_active = ${param_idx}"));
            param_idx += 1;
        }
        query.push_str(&format!(
            " ORDER BY name ASC LIMIT ${} OFFSET ${}",
            param_idx,
            param_idx + 1
        ));

        let mut q = sqlx::query_as::<_, PriceLevelRow>(&query);
        if let Some(active) = filter.is_active {
            q = q.bind(active);
        }
        let rows = q.bind(limit).bind(offset).fetch_all(&self.pool).await.map_err(map_db_error)?;
        rows.into_iter().map(Self::row_to_level).collect()
    }

    /// Delete a price level and its entries (async)
    pub async fn delete_async(&self, id: PriceLevelId) -> Result<()> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        sqlx::query("DELETE FROM price_level_entries WHERE price_level_id = $1")
            .bind(Uuid::from(id))
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        sqlx::query("DELETE FROM price_levels WHERE id = $1")
            .bind(Uuid::from(id))
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        tx.commit().await.map_err(map_db_error)?;
        Ok(())
    }

    /// Upsert a per-product fixed price entry (async)
    pub async fn set_entry_async(
        &self,
        id: PriceLevelId,
        product_id: ProductId,
        price: Decimal,
    ) -> Result<PriceLevelEntry> {
        let now = Utc::now();
        sqlx::query(
            "INSERT INTO price_level_entries (price_level_id, product_id, price, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $4)
             ON CONFLICT (price_level_id, product_id) DO UPDATE SET
                price = EXCLUDED.price,
                updated_at = EXCLUDED.updated_at",
        )
        .bind(Uuid::from(id))
        .bind(Uuid::from(product_id))
        .bind(price)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        let row = sqlx::query_as::<_, PriceLevelEntryRow>(
            "SELECT * FROM price_level_entries WHERE price_level_id = $1 AND product_id = $2",
        )
        .bind(Uuid::from(id))
        .bind(Uuid::from(product_id))
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?
        .ok_or(CommerceError::NotFound)?;
        Ok(Self::row_to_entry(row))
    }

    /// Remove a per-product entry from a level (async)
    pub async fn delete_entry_async(&self, id: PriceLevelId, product_id: ProductId) -> Result<()> {
        sqlx::query(
            "DELETE FROM price_level_entries WHERE price_level_id = $1 AND product_id = $2",
        )
        .bind(Uuid::from(id))
        .bind(Uuid::from(product_id))
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;
        Ok(())
    }

    /// List the per-product entries for a level (async)
    pub async fn list_entries_async(&self, id: PriceLevelId) -> Result<Vec<PriceLevelEntry>> {
        let rows = sqlx::query_as::<_, PriceLevelEntryRow>(
            "SELECT * FROM price_level_entries WHERE price_level_id = $1 ORDER BY product_id",
        )
        .bind(Uuid::from(id))
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;
        Ok(rows.into_iter().map(Self::row_to_entry).collect())
    }
}

impl PriceLevelRepository for PgPriceLevelRepository {
    fn create(&self, input: CreatePriceLevel) -> Result<PriceLevel> {
        super::block_on(self.create_async(input))
    }

    fn get(&self, id: PriceLevelId) -> Result<Option<PriceLevel>> {
        super::block_on(self.get_async(id))
    }

    fn update(&self, id: PriceLevelId, input: UpdatePriceLevel) -> Result<PriceLevel> {
        super::block_on(self.update_async(id, input))
    }

    fn list(&self, filter: PriceLevelFilter) -> Result<Vec<PriceLevel>> {
        super::block_on(self.list_async(filter))
    }

    fn delete(&self, id: PriceLevelId) -> Result<()> {
        super::block_on(self.delete_async(id))
    }

    fn set_entry(
        &self,
        id: PriceLevelId,
        product_id: ProductId,
        price: Decimal,
    ) -> Result<PriceLevelEntry> {
        super::block_on(self.set_entry_async(id, product_id, price))
    }

    fn delete_entry(&self, id: PriceLevelId, product_id: ProductId) -> Result<()> {
        super::block_on(self.delete_entry_async(id, product_id))
    }

    fn list_entries(&self, id: PriceLevelId) -> Result<Vec<PriceLevelEntry>> {
        super::block_on(self.list_entries_async(id))
    }
}
