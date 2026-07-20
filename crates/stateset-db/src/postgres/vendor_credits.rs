//! PostgreSQL vendor credit repository implementation

use super::map_db_error;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::FromRow;
use sqlx::postgres::PgPool;
use stateset_core::{
    ApplyVendorCredit, CommerceError, CreateVendorCredit, CurrencyCode, Result, VendorCredit,
    VendorCreditApplication, VendorCreditApplicationId, VendorCreditFilter, VendorCreditId,
    VendorCreditRepository, VendorCreditStatus, VendorCreditTargetType,
};
use uuid::Uuid;

/// PostgreSQL implementation of `VendorCreditRepository`
#[derive(Debug, Clone)]
pub struct PgVendorCreditRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct VendorCreditRow {
    id: Uuid,
    number: String,
    supplier_id: Uuid,
    vendor_return_id: Option<Uuid>,
    amount: Decimal,
    remaining: Decimal,
    currency: String,
    status: String,
    memo: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct VendorCreditApplicationRow {
    id: Uuid,
    vendor_credit_id: Uuid,
    target_type: String,
    target_id: Uuid,
    amount: Decimal,
    reversed: bool,
    created_at: DateTime<Utc>,
}

impl PgVendorCreditRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn parse_field<T: std::str::FromStr>(raw: &str, field: &str) -> Result<T>
    where
        T::Err: std::fmt::Display,
    {
        raw.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid vendor_credit.{field} '{raw}': {e}"))
        })
    }

    fn row_to_credit(row: VendorCreditRow) -> Result<VendorCredit> {
        Ok(VendorCredit {
            id: row.id.into(),
            number: row.number,
            supplier_id: row.supplier_id,
            vendor_return_id: row.vendor_return_id,
            amount: row.amount,
            remaining: row.remaining,
            currency: Self::parse_field::<CurrencyCode>(&row.currency, "currency")?,
            status: Self::parse_field::<VendorCreditStatus>(&row.status, "status")?,
            memo: row.memo,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    fn row_to_app(row: VendorCreditApplicationRow) -> Result<VendorCreditApplication> {
        Ok(VendorCreditApplication {
            id: row.id.into(),
            vendor_credit_id: row.vendor_credit_id.into(),
            target_type: Self::parse_field::<VendorCreditTargetType>(
                &row.target_type,
                "applications.target_type",
            )?,
            target_id: row.target_id,
            amount: row.amount,
            reversed: row.reversed,
            created_at: row.created_at,
        })
    }

    async fn fetch_async(&self, id: Uuid) -> Result<Option<VendorCredit>> {
        let row =
            sqlx::query_as::<_, VendorCreditRow>("SELECT * FROM vendor_credits WHERE id = $1")
                .bind(id)
                .fetch_optional(&self.pool)
                .await
                .map_err(map_db_error)?;
        row.map(Self::row_to_credit).transpose()
    }

    /// Lock the credit row and return it, or `NotFound`.
    async fn fetch_locked(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
    ) -> Result<VendorCredit> {
        let row = sqlx::query_as::<_, VendorCreditRow>(
            "SELECT * FROM vendor_credits WHERE id = $1 FOR UPDATE",
        )
        .bind(id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .ok_or(CommerceError::NotFound)?;
        Self::row_to_credit(row)
    }

    /// Recompute status from remaining balance (does not touch cancelled).
    fn status_for(remaining: Decimal, current: VendorCreditStatus) -> VendorCreditStatus {
        if current == VendorCreditStatus::Cancelled {
            VendorCreditStatus::Cancelled
        } else if remaining <= Decimal::ZERO {
            VendorCreditStatus::Applied
        } else {
            VendorCreditStatus::Open
        }
    }

    async fn store_balance(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
        remaining: Decimal,
        status: VendorCreditStatus,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE vendor_credits SET remaining = $1, status = $2, updated_at = $3 WHERE id = $4",
        )
        .bind(remaining)
        .bind(status.to_string())
        .bind(Utc::now())
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        Ok(())
    }

    /// Create a vendor credit (async)
    pub async fn create_async(&self, input: CreateVendorCredit) -> Result<VendorCredit> {
        if input.amount <= Decimal::ZERO {
            return Err(CommerceError::ValidationError(
                "vendor credit amount must be positive".into(),
            ));
        }
        let id = VendorCreditId::new();
        let id_uuid = Uuid::from(id);
        let now = Utc::now();
        let number = format!("VC-{}", &id_uuid.to_string()[..8]);
        let currency = input.currency.unwrap_or(CurrencyCode::USD);

        sqlx::query(
            "INSERT INTO vendor_credits (id, number, supplier_id, vendor_return_id, amount, remaining, currency, status, memo, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $5, $6, 'open', $7, $8, $8)",
        )
        .bind(id_uuid)
        .bind(&number)
        .bind(input.supplier_id)
        .bind(input.vendor_return_id)
        .bind(input.amount)
        .bind(currency.to_string())
        .bind(&input.memo)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.fetch_async(id_uuid).await?.ok_or(CommerceError::NotFound)
    }

    /// Get a vendor credit by ID (async)
    pub async fn get_async(&self, id: VendorCreditId) -> Result<Option<VendorCredit>> {
        self.fetch_async(id.into()).await
    }

    /// List vendor credits (async)
    pub async fn list_async(&self, filter: VendorCreditFilter) -> Result<Vec<VendorCredit>> {
        let limit = i64::from(filter.limit.unwrap_or(100));
        let offset = i64::from(filter.offset.unwrap_or(0));

        let mut query = String::from("SELECT * FROM vendor_credits WHERE 1=1");
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

        let mut q = sqlx::query_as::<_, VendorCreditRow>(&query);
        if let Some(supplier_id) = filter.supplier_id {
            q = q.bind(supplier_id);
        }
        if let Some(status) = filter.status {
            q = q.bind(status.to_string());
        }
        let rows = q.bind(limit).bind(offset).fetch_all(&self.pool).await.map_err(map_db_error)?;
        rows.into_iter().map(Self::row_to_credit).collect()
    }

    /// Apply a vendor credit against a target (async)
    pub async fn apply_async(
        &self,
        id: VendorCreditId,
        input: ApplyVendorCredit,
    ) -> Result<VendorCredit> {
        let id_uuid = Uuid::from(id);
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let credit = Self::fetch_locked(&mut tx, id_uuid).await?;
        if credit.status == VendorCreditStatus::Cancelled {
            return Err(CommerceError::Conflict("cannot apply a cancelled vendor credit".into()));
        }
        if input.amount <= Decimal::ZERO {
            return Err(CommerceError::ValidationError(
                "application amount must be positive".into(),
            ));
        }
        if input.amount > credit.remaining {
            return Err(CommerceError::ValidationError(
                "application exceeds remaining balance".into(),
            ));
        }
        let new_remaining = credit.remaining - input.amount;
        let new_status = Self::status_for(new_remaining, credit.status);

        sqlx::query(
            "INSERT INTO vendor_credit_applications (id, vendor_credit_id, target_type, target_id, amount, reversed, created_at)
             VALUES ($1, $2, $3, $4, $5, FALSE, $6)",
        )
        .bind(Uuid::from(VendorCreditApplicationId::new()))
        .bind(id_uuid)
        .bind(input.target_type.to_string())
        .bind(input.target_id)
        .bind(input.amount)
        .bind(Utc::now())
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        Self::store_balance(&mut tx, id_uuid, new_remaining, new_status).await?;
        tx.commit().await.map_err(map_db_error)?;
        self.fetch_async(id_uuid).await?.ok_or(CommerceError::NotFound)
    }

    /// List applications for a vendor credit (async)
    pub async fn list_applications_async(
        &self,
        id: VendorCreditId,
    ) -> Result<Vec<VendorCreditApplication>> {
        let rows = sqlx::query_as::<_, VendorCreditApplicationRow>(
            "SELECT * FROM vendor_credit_applications WHERE vendor_credit_id = $1 ORDER BY created_at",
        )
        .bind(Uuid::from(id))
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;
        rows.into_iter().map(Self::row_to_app).collect()
    }

    /// Reverse an application, restoring the balance (async)
    pub async fn reverse_application_async(
        &self,
        id: VendorCreditId,
        application_id: VendorCreditApplicationId,
    ) -> Result<VendorCredit> {
        let id_uuid = Uuid::from(id);
        let app_uuid = Uuid::from(application_id);
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let credit = Self::fetch_locked(&mut tx, id_uuid).await?;
        let row: Option<(Decimal, bool)> = sqlx::query_as(
            "SELECT amount, reversed FROM vendor_credit_applications WHERE id = $1 AND vendor_credit_id = $2 FOR UPDATE",
        )
        .bind(app_uuid)
        .bind(id_uuid)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        let (amount, reversed) = row.ok_or(CommerceError::NotFound)?;
        if reversed {
            return Err(CommerceError::Conflict("application already reversed".into()));
        }

        let new_remaining = credit.remaining + amount;
        let new_status = Self::status_for(new_remaining, credit.status);

        sqlx::query("UPDATE vendor_credit_applications SET reversed = TRUE WHERE id = $1")
            .bind(app_uuid)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        Self::store_balance(&mut tx, id_uuid, new_remaining, new_status).await?;
        tx.commit().await.map_err(map_db_error)?;
        self.fetch_async(id_uuid).await?.ok_or(CommerceError::NotFound)
    }

    /// Cancel a vendor credit with no active applications (async)
    pub async fn cancel_async(&self, id: VendorCreditId) -> Result<VendorCredit> {
        let id_uuid = Uuid::from(id);
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        // Lock the credit row before counting so a concurrent apply serializes.
        Self::fetch_locked(&mut tx, id_uuid).await?;
        let (active,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM vendor_credit_applications WHERE vendor_credit_id = $1 AND reversed = FALSE",
        )
        .bind(id_uuid)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        if active > 0 {
            return Err(CommerceError::Conflict(
                "cannot cancel a vendor credit with active applications".into(),
            ));
        }
        sqlx::query(
            "UPDATE vendor_credits SET status = 'cancelled', updated_at = $1 WHERE id = $2",
        )
        .bind(Utc::now())
        .bind(id_uuid)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        tx.commit().await.map_err(map_db_error)?;
        self.fetch_async(id_uuid).await?.ok_or(CommerceError::NotFound)
    }
}

impl VendorCreditRepository for PgVendorCreditRepository {
    fn create(&self, input: CreateVendorCredit) -> Result<VendorCredit> {
        super::block_on(self.create_async(input))
    }

    fn get(&self, id: VendorCreditId) -> Result<Option<VendorCredit>> {
        super::block_on(self.get_async(id))
    }

    fn list(&self, filter: VendorCreditFilter) -> Result<Vec<VendorCredit>> {
        super::block_on(self.list_async(filter))
    }

    fn apply(&self, id: VendorCreditId, input: ApplyVendorCredit) -> Result<VendorCredit> {
        super::block_on(self.apply_async(id, input))
    }

    fn list_applications(&self, id: VendorCreditId) -> Result<Vec<VendorCreditApplication>> {
        super::block_on(self.list_applications_async(id))
    }

    fn reverse_application(
        &self,
        id: VendorCreditId,
        application_id: VendorCreditApplicationId,
    ) -> Result<VendorCredit> {
        super::block_on(self.reverse_application_async(id, application_id))
    }

    fn cancel(&self, id: VendorCreditId) -> Result<VendorCredit> {
        super::block_on(self.cancel_async(id))
    }
}
