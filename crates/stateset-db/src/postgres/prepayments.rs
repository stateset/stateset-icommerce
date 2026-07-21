//! PostgreSQL prepayment repository implementation

use super::map_db_error;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::FromRow;
use sqlx::postgres::PgPool;
use stateset_core::{
    ApplyPrepayment, CommerceError, CreatePrepayment, CurrencyCode, Prepayment,
    PrepaymentApplication, PrepaymentApplicationId, PrepaymentFilter, PrepaymentId,
    PrepaymentRepository, PrepaymentStatus, PrepaymentTargetType, Result,
};
use uuid::Uuid;

/// PostgreSQL implementation of `PrepaymentRepository`
#[derive(Debug, Clone)]
pub struct PgPrepaymentRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct PrepaymentRow {
    id: Uuid,
    number: String,
    supplier_id: Uuid,
    amount: Decimal,
    remaining: Decimal,
    currency: String,
    status: String,
    method: Option<String>,
    reference: Option<String>,
    memo: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct PrepaymentApplicationRow {
    id: Uuid,
    prepayment_id: Uuid,
    target_type: String,
    target_id: Uuid,
    amount: Decimal,
    reversed: bool,
    created_at: DateTime<Utc>,
}

impl PgPrepaymentRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_prepayment(row: PrepaymentRow) -> Result<Prepayment> {
        let currency: CurrencyCode = row.currency.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid prepayment.currency '{}': {}",
                row.currency, e
            ))
        })?;
        let status: PrepaymentStatus = row.status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid prepayment.status '{}': {}",
                row.status, e
            ))
        })?;
        Ok(Prepayment {
            id: row.id.into(),
            number: row.number,
            supplier_id: row.supplier_id,
            amount: row.amount,
            remaining: row.remaining,
            currency,
            status,
            method: row.method,
            reference: row.reference,
            memo: row.memo,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    fn row_to_app(row: PrepaymentApplicationRow) -> Result<PrepaymentApplication> {
        let target_type: PrepaymentTargetType = row.target_type.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid prepayment_application.target_type '{}': {}",
                row.target_type, e
            ))
        })?;
        Ok(PrepaymentApplication {
            id: row.id.into(),
            prepayment_id: row.prepayment_id.into(),
            target_type,
            target_id: row.target_id,
            amount: row.amount,
            reversed: row.reversed,
            created_at: row.created_at,
        })
    }

    async fn fetch_async(&self, id: Uuid) -> Result<Option<Prepayment>> {
        let row = sqlx::query_as::<_, PrepaymentRow>("SELECT * FROM prepayments WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;
        row.map(Self::row_to_prepayment).transpose()
    }

    async fn fetch_in_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
    ) -> Result<Prepayment> {
        let row = sqlx::query_as::<_, PrepaymentRow>(
            "SELECT * FROM prepayments WHERE id = $1 FOR UPDATE",
        )
        .bind(id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .ok_or(CommerceError::NotFound)?;
        Self::row_to_prepayment(row)
    }

    fn status_for(remaining: Decimal, current: PrepaymentStatus) -> PrepaymentStatus {
        match current {
            PrepaymentStatus::Cancelled | PrepaymentStatus::Refunded => current,
            _ if remaining <= Decimal::ZERO => PrepaymentStatus::Applied,
            _ => PrepaymentStatus::Open,
        }
    }
}

impl PgPrepaymentRepository {
    /// Create a prepayment (async)
    pub async fn create_async(&self, input: CreatePrepayment) -> Result<Prepayment> {
        if input.amount <= Decimal::ZERO {
            return Err(CommerceError::ValidationError(
                "prepayment amount must be positive".into(),
            ));
        }
        let id = PrepaymentId::new();
        let id_str = id.to_string();
        let number = format!("PRE-{}", &id_str[..8]);
        let now = Utc::now();
        let currency = input.currency.unwrap_or(CurrencyCode::USD);

        sqlx::query(
            "INSERT INTO prepayments (id, number, supplier_id, amount, remaining, currency, status, method, reference, memo, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $4, $5, 'open', $6, $7, $8, $9, $9)",
        )
        .bind(Uuid::from(id))
        .bind(&number)
        .bind(input.supplier_id)
        .bind(input.amount)
        .bind(currency.to_string())
        .bind(&input.method)
        .bind(&input.reference)
        .bind(&input.memo)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.fetch_async(id.into()).await?.ok_or(CommerceError::NotFound)
    }

    /// Get a prepayment by ID (async)
    pub async fn get_async(&self, id: PrepaymentId) -> Result<Option<Prepayment>> {
        self.fetch_async(id.into()).await
    }

    /// List prepayments (async)
    pub async fn list_async(&self, filter: PrepaymentFilter) -> Result<Vec<Prepayment>> {
        let limit = super::effective_limit(filter.limit);
        let offset = i64::from(filter.offset.unwrap_or(0));

        let mut query = String::from("SELECT * FROM prepayments WHERE 1=1");
        let mut param_idx = 1;
        if filter.supplier_id.is_some() {
            query.push_str(&format!(" AND supplier_id = ${param_idx}"));
            param_idx += 1;
        }
        if filter.status.is_some() {
            query.push_str(&format!(" AND status = ${param_idx}"));
            param_idx += 1;
        }
        query.push_str(&format!(
            " ORDER BY created_at DESC LIMIT ${} OFFSET ${}",
            param_idx,
            param_idx + 1
        ));

        let mut q = sqlx::query_as::<_, PrepaymentRow>(&query);
        if let Some(supplier_id) = filter.supplier_id {
            q = q.bind(supplier_id);
        }
        if let Some(status) = filter.status {
            q = q.bind(status.to_string());
        }
        let rows = q.bind(limit).bind(offset).fetch_all(&self.pool).await.map_err(map_db_error)?;
        rows.into_iter().map(Self::row_to_prepayment).collect()
    }

    /// Apply a prepayment against a bill or obligation (async)
    pub async fn apply_async(
        &self,
        id: PrepaymentId,
        input: ApplyPrepayment,
    ) -> Result<Prepayment> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let prepayment = Self::fetch_in_tx(&mut tx, id.into()).await?;
        if prepayment.status != PrepaymentStatus::Open {
            return Err(CommerceError::Conflict("prepayment is not open for application".into()));
        }
        if input.amount <= Decimal::ZERO {
            return Err(CommerceError::ValidationError(
                "application amount must be positive".into(),
            ));
        }
        if input.amount > prepayment.remaining {
            return Err(CommerceError::ValidationError(
                "application exceeds remaining balance".into(),
            ));
        }
        let new_remaining = prepayment.remaining - input.amount;
        let new_status = Self::status_for(new_remaining, prepayment.status);
        let now = Utc::now();
        sqlx::query(
            "INSERT INTO prepayment_applications (id, prepayment_id, target_type, target_id, amount, reversed, created_at)
             VALUES ($1, $2, $3, $4, $5, FALSE, $6)",
        )
        .bind(Uuid::from(PrepaymentApplicationId::new()))
        .bind(Uuid::from(id))
        .bind(input.target_type.to_string())
        .bind(input.target_id)
        .bind(input.amount)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        sqlx::query(
            "UPDATE prepayments SET remaining = $1, status = $2, updated_at = $3 WHERE id = $4",
        )
        .bind(new_remaining)
        .bind(new_status.to_string())
        .bind(now)
        .bind(Uuid::from(id))
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        tx.commit().await.map_err(map_db_error)?;
        self.fetch_async(id.into()).await?.ok_or(CommerceError::NotFound)
    }

    /// List applications for a prepayment (async)
    pub async fn list_applications_async(
        &self,
        id: PrepaymentId,
    ) -> Result<Vec<PrepaymentApplication>> {
        let rows = sqlx::query_as::<_, PrepaymentApplicationRow>(
            "SELECT * FROM prepayment_applications WHERE prepayment_id = $1 ORDER BY created_at",
        )
        .bind(Uuid::from(id))
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;
        rows.into_iter().map(Self::row_to_app).collect()
    }

    /// Reverse a previously-recorded application (async)
    pub async fn reverse_application_async(
        &self,
        id: PrepaymentId,
        application_id: PrepaymentApplicationId,
    ) -> Result<Prepayment> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let app_row: Option<(Decimal, bool)> = sqlx::query_as(
            "SELECT amount, reversed FROM prepayment_applications WHERE id = $1 AND prepayment_id = $2 FOR UPDATE",
        )
        .bind(Uuid::from(application_id))
        .bind(Uuid::from(id))
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        let (amount, reversed) = app_row.ok_or(CommerceError::NotFound)?;
        if reversed {
            return Err(CommerceError::Conflict("application already reversed".into()));
        }
        let prepayment = Self::fetch_in_tx(&mut tx, id.into()).await?;
        if prepayment.status == PrepaymentStatus::Refunded {
            return Err(CommerceError::Conflict(
                "cannot reverse against a refunded prepayment".into(),
            ));
        }
        let new_remaining = prepayment.remaining + amount;
        let new_status = Self::status_for(new_remaining, prepayment.status);
        let now = Utc::now();
        sqlx::query("UPDATE prepayment_applications SET reversed = TRUE WHERE id = $1")
            .bind(Uuid::from(application_id))
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        sqlx::query(
            "UPDATE prepayments SET remaining = $1, status = $2, updated_at = $3 WHERE id = $4",
        )
        .bind(new_remaining)
        .bind(new_status.to_string())
        .bind(now)
        .bind(Uuid::from(id))
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        tx.commit().await.map_err(map_db_error)?;
        self.fetch_async(id.into()).await?.ok_or(CommerceError::NotFound)
    }

    /// Refund the remaining balance, closing the prepayment (async)
    pub async fn refund_async(&self, id: PrepaymentId) -> Result<Prepayment> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let prepayment = Self::fetch_in_tx(&mut tx, id.into()).await?;
        if prepayment.status == PrepaymentStatus::Cancelled {
            return Err(CommerceError::Conflict("cannot refund a cancelled prepayment".into()));
        }
        // Refund zeroes the remaining balance and closes the prepayment.
        let now = Utc::now();
        sqlx::query(
            "UPDATE prepayments SET remaining = 0, status = 'refunded', updated_at = $1 WHERE id = $2",
        )
        .bind(now)
        .bind(Uuid::from(id))
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        tx.commit().await.map_err(map_db_error)?;
        self.fetch_async(id.into()).await?.ok_or(CommerceError::NotFound)
    }
}

impl PrepaymentRepository for PgPrepaymentRepository {
    fn create(&self, input: CreatePrepayment) -> Result<Prepayment> {
        super::block_on(self.create_async(input))
    }

    fn get(&self, id: PrepaymentId) -> Result<Option<Prepayment>> {
        super::block_on(self.get_async(id))
    }

    fn list(&self, filter: PrepaymentFilter) -> Result<Vec<Prepayment>> {
        super::block_on(self.list_async(filter))
    }

    fn apply(&self, id: PrepaymentId, input: ApplyPrepayment) -> Result<Prepayment> {
        super::block_on(self.apply_async(id, input))
    }

    fn list_applications(&self, id: PrepaymentId) -> Result<Vec<PrepaymentApplication>> {
        super::block_on(self.list_applications_async(id))
    }

    fn reverse_application(
        &self,
        id: PrepaymentId,
        application_id: PrepaymentApplicationId,
    ) -> Result<Prepayment> {
        super::block_on(self.reverse_application_async(id, application_id))
    }

    fn refund(&self, id: PrepaymentId) -> Result<Prepayment> {
        super::block_on(self.refund_async(id))
    }
}
