//! PostgreSQL price schedule repository implementation

use super::map_db_error;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::FromRow;
use sqlx::postgres::PgPool;
use stateset_core::{
    CommerceError, CreatePriceSchedule, CurrencyCode, PriceSchedule, PriceScheduleEntry,
    PriceScheduleFilter, PriceScheduleId, PriceScheduleRepository, ProductId, Result,
    UpdatePriceSchedule,
};
use uuid::Uuid;

/// PostgreSQL implementation of `PriceScheduleRepository`
#[derive(Debug, Clone)]
pub struct PgPriceScheduleRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct PriceScheduleRow {
    id: Uuid,
    name: String,
    code: Option<String>,
    currency: String,
    starts_at: Option<DateTime<Utc>>,
    ends_at: Option<DateTime<Utc>>,
    is_active: bool,
    priority: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct PriceScheduleEntryRow {
    price_schedule_id: Uuid,
    product_id: Uuid,
    price: Decimal,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl PgPriceScheduleRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_schedule(row: PriceScheduleRow) -> Result<PriceSchedule> {
        let currency: CurrencyCode = row.currency.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid price_schedule.currency '{}': {}",
                row.currency, e
            ))
        })?;
        Ok(PriceSchedule {
            id: row.id.into(),
            name: row.name,
            code: row.code,
            currency,
            starts_at: row.starts_at,
            ends_at: row.ends_at,
            is_active: row.is_active,
            priority: row.priority,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    fn row_to_entry(row: PriceScheduleEntryRow) -> PriceScheduleEntry {
        PriceScheduleEntry {
            price_schedule_id: row.price_schedule_id.into(),
            product_id: row.product_id.into(),
            price: row.price,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }

    async fn fetch_async(&self, id: Uuid) -> Result<Option<PriceSchedule>> {
        let row =
            sqlx::query_as::<_, PriceScheduleRow>("SELECT * FROM price_schedules WHERE id = $1")
                .bind(id)
                .fetch_optional(&self.pool)
                .await
                .map_err(map_db_error)?;
        row.map(Self::row_to_schedule).transpose()
    }

    /// Create a price schedule (async)
    pub async fn create_async(&self, input: CreatePriceSchedule) -> Result<PriceSchedule> {
        let id = PriceScheduleId::new();
        let now = Utc::now();
        let currency = input.currency.unwrap_or(CurrencyCode::USD);

        sqlx::query(
            "INSERT INTO price_schedules (id, name, code, currency, starts_at, ends_at, is_active, priority, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, TRUE, $7, $8, $8)",
        )
        .bind(Uuid::from(id))
        .bind(&input.name)
        .bind(&input.code)
        .bind(currency.to_string())
        .bind(input.starts_at)
        .bind(input.ends_at)
        .bind(input.priority)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.fetch_async(id.into()).await?.ok_or(CommerceError::NotFound)
    }

    /// Get a price schedule by ID (async)
    pub async fn get_async(&self, id: PriceScheduleId) -> Result<Option<PriceSchedule>> {
        self.fetch_async(id.into()).await
    }

    /// Update a price schedule (async, partial)
    pub async fn update_async(
        &self,
        id: PriceScheduleId,
        input: UpdatePriceSchedule,
    ) -> Result<PriceSchedule> {
        let existing = self.fetch_async(id.into()).await?.ok_or(CommerceError::NotFound)?;
        let now = Utc::now();

        // Matches SQLite semantics: only `Some` fields overwrite; `None`
        // leaves the stored value untouched (fields cannot be cleared).
        let name = input.name.unwrap_or(existing.name);
        let code = input.code.or(existing.code);
        let starts_at = input.starts_at.or(existing.starts_at);
        let ends_at = input.ends_at.or(existing.ends_at);
        let is_active = input.is_active.unwrap_or(existing.is_active);
        let priority = input.priority.unwrap_or(existing.priority);

        sqlx::query(
            "UPDATE price_schedules SET name = $1, code = $2, starts_at = $3, ends_at = $4, is_active = $5, priority = $6, updated_at = $7 WHERE id = $8",
        )
        .bind(&name)
        .bind(&code)
        .bind(starts_at)
        .bind(ends_at)
        .bind(is_active)
        .bind(priority)
        .bind(now)
        .bind(Uuid::from(id))
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.fetch_async(id.into()).await?.ok_or(CommerceError::NotFound)
    }

    /// List price schedules (async)
    pub async fn list_async(&self, filter: PriceScheduleFilter) -> Result<Vec<PriceSchedule>> {
        let limit = super::effective_limit(filter.limit);
        let offset = i64::from(filter.offset.unwrap_or(0));

        let mut query = String::from("SELECT * FROM price_schedules WHERE 1=1");
        let mut param_idx = 1;
        if filter.is_active.is_some() {
            query.push_str(&format!(" AND is_active = ${param_idx}"));
            param_idx += 1;
        }
        query.push_str(&format!(
            " ORDER BY priority DESC, created_at DESC LIMIT ${} OFFSET ${}",
            param_idx,
            param_idx + 1
        ));

        let mut q = sqlx::query_as::<_, PriceScheduleRow>(&query);
        if let Some(active) = filter.is_active {
            q = q.bind(active);
        }
        let rows = q.bind(limit).bind(offset).fetch_all(&self.pool).await.map_err(map_db_error)?;
        rows.into_iter().map(Self::row_to_schedule).collect()
    }

    /// Delete a price schedule and its entries (async)
    pub async fn delete_async(&self, id: PriceScheduleId) -> Result<()> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        sqlx::query("DELETE FROM price_schedule_entries WHERE price_schedule_id = $1")
            .bind(Uuid::from(id))
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        sqlx::query("DELETE FROM price_schedules WHERE id = $1")
            .bind(Uuid::from(id))
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        tx.commit().await.map_err(map_db_error)?;
        Ok(())
    }

    /// Upsert a per-product entry (async)
    pub async fn set_entry_async(
        &self,
        id: PriceScheduleId,
        product_id: ProductId,
        price: Decimal,
    ) -> Result<PriceScheduleEntry> {
        let now = Utc::now();
        sqlx::query(
            "INSERT INTO price_schedule_entries (price_schedule_id, product_id, price, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $4)
             ON CONFLICT (price_schedule_id, product_id) DO UPDATE SET
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

        let row = sqlx::query_as::<_, PriceScheduleEntryRow>(
            "SELECT * FROM price_schedule_entries WHERE price_schedule_id = $1 AND product_id = $2",
        )
        .bind(Uuid::from(id))
        .bind(Uuid::from(product_id))
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?
        .ok_or(CommerceError::NotFound)?;
        Ok(Self::row_to_entry(row))
    }

    /// Remove a per-product entry (async)
    pub async fn delete_entry_async(
        &self,
        id: PriceScheduleId,
        product_id: ProductId,
    ) -> Result<()> {
        sqlx::query(
            "DELETE FROM price_schedule_entries WHERE price_schedule_id = $1 AND product_id = $2",
        )
        .bind(Uuid::from(id))
        .bind(Uuid::from(product_id))
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;
        Ok(())
    }

    /// List per-product entries for a schedule (async)
    pub async fn list_entries_async(&self, id: PriceScheduleId) -> Result<Vec<PriceScheduleEntry>> {
        let rows = sqlx::query_as::<_, PriceScheduleEntryRow>(
            "SELECT * FROM price_schedule_entries WHERE price_schedule_id = $1 ORDER BY product_id",
        )
        .bind(Uuid::from(id))
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;
        Ok(rows.into_iter().map(Self::row_to_entry).collect())
    }

    /// Resolve the effective scheduled price for a product at an instant (async)
    pub async fn resolve_price_async(
        &self,
        product_id: ProductId,
        at: DateTime<Utc>,
    ) -> Result<Option<Decimal>> {
        // Active schedules ordered by priority then recency; first whose
        // window contains `at` and that has an entry for the product wins.
        let schedules = self
            .list_async(PriceScheduleFilter { is_active: Some(true), ..Default::default() })
            .await?;
        for schedule in schedules {
            if !schedule.is_effective_at(at) {
                continue;
            }
            let price: Option<Decimal> = sqlx::query_scalar(
                "SELECT price FROM price_schedule_entries WHERE price_schedule_id = $1 AND product_id = $2",
            )
            .bind(Uuid::from(schedule.id))
            .bind(Uuid::from(product_id))
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;
            if let Some(price) = price {
                return Ok(Some(price));
            }
        }
        Ok(None)
    }
}

impl PriceScheduleRepository for PgPriceScheduleRepository {
    fn create(&self, input: CreatePriceSchedule) -> Result<PriceSchedule> {
        super::block_on(self.create_async(input))
    }

    fn get(&self, id: PriceScheduleId) -> Result<Option<PriceSchedule>> {
        super::block_on(self.get_async(id))
    }

    fn update(&self, id: PriceScheduleId, input: UpdatePriceSchedule) -> Result<PriceSchedule> {
        super::block_on(self.update_async(id, input))
    }

    fn list(&self, filter: PriceScheduleFilter) -> Result<Vec<PriceSchedule>> {
        super::block_on(self.list_async(filter))
    }

    fn delete(&self, id: PriceScheduleId) -> Result<()> {
        super::block_on(self.delete_async(id))
    }

    fn set_entry(
        &self,
        id: PriceScheduleId,
        product_id: ProductId,
        price: Decimal,
    ) -> Result<PriceScheduleEntry> {
        super::block_on(self.set_entry_async(id, product_id, price))
    }

    fn delete_entry(&self, id: PriceScheduleId, product_id: ProductId) -> Result<()> {
        super::block_on(self.delete_entry_async(id, product_id))
    }

    fn list_entries(&self, id: PriceScheduleId) -> Result<Vec<PriceScheduleEntry>> {
        super::block_on(self.list_entries_async(id))
    }

    fn resolve_price(&self, product_id: ProductId, at: DateTime<Utc>) -> Result<Option<Decimal>> {
        super::block_on(self.resolve_price_async(product_id, at))
    }
}
