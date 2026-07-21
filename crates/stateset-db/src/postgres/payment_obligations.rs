//! PostgreSQL payment obligation repository implementation

use super::map_db_error;
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use sqlx::FromRow;
use sqlx::postgres::PgPool;
use stateset_core::{
    CommerceError, CreatePaymentObligation, CurrencyCode, PaymentObligation,
    PaymentObligationDashboard, PaymentObligationFilter, PaymentObligationId,
    PaymentObligationRepository, PaymentObligationStatus, Result,
};
use uuid::Uuid;

/// PostgreSQL implementation of `PaymentObligationRepository`
#[derive(Debug, Clone)]
pub struct PgPaymentObligationRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct PaymentObligationRow {
    id: Uuid,
    number: String,
    supplier_id: Uuid,
    purchase_order_id: Option<Uuid>,
    amount: Decimal,
    amount_paid: Decimal,
    currency: String,
    due_date: NaiveDate,
    status: String,
    linked_bill_ids: serde_json::Value,
    notes: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl PgPaymentObligationRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_obligation(row: PaymentObligationRow) -> Result<PaymentObligation> {
        let currency: CurrencyCode = row.currency.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid payment_obligation.currency '{}': {}",
                row.currency, e
            ))
        })?;
        let status: PaymentObligationStatus = row.status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid payment_obligation.status '{}': {}",
                row.status, e
            ))
        })?;
        let linked_bill_ids: Vec<Uuid> =
            serde_json::from_value(row.linked_bill_ids).map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid payment_obligation.linked_bill_ids: {e}"
                ))
            })?;
        Ok(PaymentObligation {
            id: row.id.into(),
            number: row.number,
            supplier_id: row.supplier_id,
            purchase_order_id: row.purchase_order_id,
            amount: row.amount,
            amount_paid: row.amount_paid,
            currency,
            due_date: row.due_date,
            status,
            linked_bill_ids,
            notes: row.notes,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    async fn fetch_async(&self, id: Uuid) -> Result<Option<PaymentObligation>> {
        let row = sqlx::query_as::<_, PaymentObligationRow>(
            "SELECT * FROM payment_obligations WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;
        row.map(Self::row_to_obligation).transpose()
    }

    async fn fetch_in_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
    ) -> Result<PaymentObligation> {
        let row = sqlx::query_as::<_, PaymentObligationRow>(
            "SELECT * FROM payment_obligations WHERE id = $1 FOR UPDATE",
        )
        .bind(id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .ok_or(CommerceError::NotFound)?;
        Self::row_to_obligation(row)
    }

    /// Create a payment obligation (async)
    pub async fn create_async(&self, input: CreatePaymentObligation) -> Result<PaymentObligation> {
        if input.amount <= Decimal::ZERO {
            return Err(CommerceError::ValidationError(
                "payment obligation amount must be positive".into(),
            ));
        }
        let id = PaymentObligationId::new();
        let id_str = id.to_string();
        let number = format!("OBL-{}", &id_str[..8]);
        let now = Utc::now();
        let currency = input.currency.unwrap_or(CurrencyCode::USD);

        sqlx::query(
            "INSERT INTO payment_obligations (id, number, supplier_id, purchase_order_id, amount, amount_paid, currency, due_date, status, linked_bill_ids, notes, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, 0, $6, $7, 'pending', '[]'::jsonb, $8, $9, $9)",
        )
        .bind(Uuid::from(id))
        .bind(&number)
        .bind(input.supplier_id)
        .bind(input.purchase_order_id)
        .bind(input.amount)
        .bind(currency.to_string())
        .bind(input.due_date)
        .bind(&input.notes)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.fetch_async(id.into()).await?.ok_or(CommerceError::NotFound)
    }

    /// Get a payment obligation by ID (async)
    pub async fn get_async(&self, id: PaymentObligationId) -> Result<Option<PaymentObligation>> {
        self.fetch_async(id.into()).await
    }

    /// List payment obligations (async)
    pub async fn list_async(
        &self,
        filter: PaymentObligationFilter,
    ) -> Result<Vec<PaymentObligation>> {
        let limit = super::effective_limit(filter.limit);
        let offset = i64::from(filter.offset.unwrap_or(0));

        let mut query = String::from("SELECT * FROM payment_obligations WHERE 1=1");
        let mut param_idx = 1;
        if filter.supplier_id.is_some() {
            query.push_str(&format!(" AND supplier_id = ${param_idx}"));
            param_idx += 1;
        }
        if filter.status.is_some() {
            query.push_str(&format!(" AND status = ${param_idx}"));
            param_idx += 1;
        }
        if filter.due_before.is_some() {
            query.push_str(&format!(" AND due_date <= ${param_idx}"));
            param_idx += 1;
        }
        query.push_str(&format!(
            " ORDER BY due_date ASC LIMIT ${} OFFSET ${}",
            param_idx,
            param_idx + 1
        ));

        let mut q = sqlx::query_as::<_, PaymentObligationRow>(&query);
        if let Some(supplier_id) = filter.supplier_id {
            q = q.bind(supplier_id);
        }
        if let Some(status) = filter.status {
            q = q.bind(status.to_string());
        }
        if let Some(due_before) = filter.due_before {
            q = q.bind(due_before);
        }
        let rows = q.bind(limit).bind(offset).fetch_all(&self.pool).await.map_err(map_db_error)?;
        rows.into_iter().map(Self::row_to_obligation).collect()
    }

    /// Record a payment against an obligation (async)
    pub async fn record_payment_async(
        &self,
        id: PaymentObligationId,
        amount: Decimal,
    ) -> Result<PaymentObligation> {
        if amount <= Decimal::ZERO {
            return Err(CommerceError::ValidationError("payment amount must be positive".into()));
        }
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let mut obligation = Self::fetch_in_tx(&mut tx, id.into()).await?;
        if obligation.status == PaymentObligationStatus::Cancelled {
            return Err(CommerceError::Conflict("cannot pay a cancelled obligation".into()));
        }
        if amount > obligation.outstanding() {
            return Err(CommerceError::ValidationError(format!(
                "payment {amount} exceeds outstanding balance {}",
                obligation.outstanding()
            )));
        }
        obligation.amount_paid += amount;
        let new_status = obligation.derive_status();
        let now = Utc::now();
        sqlx::query(
            "UPDATE payment_obligations SET amount_paid = $1, status = $2, updated_at = $3 WHERE id = $4",
        )
        .bind(obligation.amount_paid)
        .bind(new_status.to_string())
        .bind(now)
        .bind(Uuid::from(id))
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        tx.commit().await.map_err(map_db_error)?;
        self.fetch_async(id.into()).await?.ok_or(CommerceError::NotFound)
    }

    /// Set the obligation status explicitly (async)
    pub async fn set_status_async(
        &self,
        id: PaymentObligationId,
        status: PaymentObligationStatus,
    ) -> Result<PaymentObligation> {
        let now = Utc::now();
        sqlx::query("UPDATE payment_obligations SET status = $1, updated_at = $2 WHERE id = $3")
            .bind(status.to_string())
            .bind(now)
            .bind(Uuid::from(id))
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;
        self.fetch_async(id.into()).await?.ok_or(CommerceError::NotFound)
    }

    /// Link an AP bill to an obligation, idempotently (async)
    pub async fn link_bill_async(
        &self,
        id: PaymentObligationId,
        bill_id: Uuid,
    ) -> Result<PaymentObligation> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let obligation = Self::fetch_in_tx(&mut tx, id.into()).await?;
        let mut ids = obligation.linked_bill_ids;
        if !ids.contains(&bill_id) {
            ids.push(bill_id);
        }
        let json =
            serde_json::to_value(&ids).map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let now = Utc::now();
        sqlx::query(
            "UPDATE payment_obligations SET linked_bill_ids = $1, updated_at = $2 WHERE id = $3",
        )
        .bind(json)
        .bind(now)
        .bind(Uuid::from(id))
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        tx.commit().await.map_err(map_db_error)?;
        self.fetch_async(id.into()).await?.ok_or(CommerceError::NotFound)
    }

    /// Aggregate dashboard summary as of the given date (async)
    pub async fn dashboard_async(&self, today: NaiveDate) -> Result<PaymentObligationDashboard> {
        // Open (non-terminal) obligations only.
        let mut all = Vec::new();
        for status in [
            PaymentObligationStatus::Pending,
            PaymentObligationStatus::Scheduled,
            PaymentObligationStatus::PartiallyPaid,
        ] {
            let batch = self
                .list_async(PaymentObligationFilter { status: Some(status), ..Default::default() })
                .await?;
            all.extend(batch);
        }
        let mut dash = PaymentObligationDashboard::default();
        for o in &all {
            dash.open_count += 1;
            dash.total_outstanding += o.outstanding();
            if o.is_overdue(today) {
                dash.overdue_count += 1;
                dash.overdue_amount += o.outstanding();
            }
        }
        Ok(dash)
    }
}

impl PaymentObligationRepository for PgPaymentObligationRepository {
    fn create(&self, input: CreatePaymentObligation) -> Result<PaymentObligation> {
        super::block_on(self.create_async(input))
    }

    fn get(&self, id: PaymentObligationId) -> Result<Option<PaymentObligation>> {
        super::block_on(self.get_async(id))
    }

    fn list(&self, filter: PaymentObligationFilter) -> Result<Vec<PaymentObligation>> {
        super::block_on(self.list_async(filter))
    }

    fn record_payment(
        &self,
        id: PaymentObligationId,
        amount: Decimal,
    ) -> Result<PaymentObligation> {
        super::block_on(self.record_payment_async(id, amount))
    }

    fn set_status(
        &self,
        id: PaymentObligationId,
        status: PaymentObligationStatus,
    ) -> Result<PaymentObligation> {
        super::block_on(self.set_status_async(id, status))
    }

    fn link_bill(&self, id: PaymentObligationId, bill_id: Uuid) -> Result<PaymentObligation> {
        super::block_on(self.link_bill_async(id, bill_id))
    }

    fn dashboard(&self, today: NaiveDate) -> Result<PaymentObligationDashboard> {
        super::block_on(self.dashboard_async(today))
    }
}
