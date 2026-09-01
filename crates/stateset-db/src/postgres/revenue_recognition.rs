//! PostgreSQL revenue recognition (ASC 606) repository implementation

use super::map_db_error;
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use sqlx::FromRow;
use sqlx::postgres::PgPool;
use stateset_core::{
    CommerceError, CreateRevenueContract, CurrencyCode, PerformanceObligation, RecognitionMethod,
    Result, RevenueContract, RevenueContractFilter, RevenueContractStatus, RevenueEntryStatus,
    RevenueRecognitionRepository, RevenueSchedule, RevenueScheduleEntry, UpdateRevenueContract,
    Validate, generate_revenue_contract_number, generate_revenue_schedule,
};
use uuid::Uuid;

/// PostgreSQL implementation of `RevenueRecognitionRepository`
#[derive(Debug, Clone)]
pub struct PgRevenueRecognitionRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct RevenueContractRow {
    id: Uuid,
    contract_number: String,
    customer_id: Uuid,
    order_id: Option<Uuid>,
    invoice_id: Option<Uuid>,
    transaction_price: Decimal,
    currency: String,
    status: String,
    effective_date: NaiveDate,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct PerformanceObligationRow {
    id: Uuid,
    contract_id: Uuid,
    description: String,
    standalone_selling_price: Option<Decimal>,
    allocated_amount: Decimal,
    recognition_method: String,
    recognized_amount: Decimal,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct RevenueScheduleEntryRow {
    period: i32,
    period_start: NaiveDate,
    amount: Decimal,
    status: String,
}

impl PgRevenueRecognitionRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn parse_field<T: std::str::FromStr>(raw: &str, field: &str) -> Result<T>
    where
        T::Err: std::fmt::Display,
    {
        raw.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid revenue_contract.{field} '{raw}': {e}"))
        })
    }

    fn row_to_contract(row: RevenueContractRow) -> Result<RevenueContract> {
        Ok(RevenueContract {
            id: row.id,
            contract_number: row.contract_number,
            customer_id: row.customer_id,
            order_id: row.order_id,
            invoice_id: row.invoice_id,
            transaction_price: row.transaction_price,
            currency: Self::parse_field::<CurrencyCode>(&row.currency, "currency")?,
            status: Self::parse_field::<RevenueContractStatus>(&row.status, "status")?,
            effective_date: row.effective_date,
            obligations: Vec::new(),
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    fn row_to_obligation(row: PerformanceObligationRow) -> Result<PerformanceObligation> {
        Ok(PerformanceObligation {
            id: row.id,
            contract_id: row.contract_id,
            description: row.description,
            standalone_selling_price: row.standalone_selling_price,
            allocated_amount: row.allocated_amount,
            recognition_method: serde_json::from_str::<RecognitionMethod>(&row.recognition_method)
                .map_err(|e| {
                    CommerceError::DatabaseError(format!(
                        "Invalid performance_obligation.recognition_method: {e}"
                    ))
                })?,
            recognized_amount: row.recognized_amount,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    fn row_to_entry(row: RevenueScheduleEntryRow) -> Result<RevenueScheduleEntry> {
        Ok(RevenueScheduleEntry {
            period: u32::try_from(row.period).unwrap_or(0),
            period_start: row.period_start,
            amount: row.amount,
            status: Self::parse_field::<RevenueEntryStatus>(&row.status, "entry.status")?,
        })
    }

    async fn load_obligations_async(
        &self,
        contract_id: Uuid,
    ) -> Result<Vec<PerformanceObligation>> {
        let rows = sqlx::query_as::<_, PerformanceObligationRow>(
            "SELECT * FROM performance_obligations WHERE contract_id = $1 ORDER BY created_at, id",
        )
        .bind(contract_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;
        rows.into_iter().map(Self::row_to_obligation).collect()
    }

    async fn load_full_async(&self, id: Uuid) -> Result<Option<RevenueContract>> {
        let row = sqlx::query_as::<_, RevenueContractRow>(
            "SELECT * FROM revenue_contracts WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;
        match row {
            Some(row) => {
                let mut head = Self::row_to_contract(row)?;
                head.obligations = self.load_obligations_async(id).await?;
                Ok(Some(head))
            }
            None => Ok(None),
        }
    }

    async fn load_entries_async(&self, obligation_id: Uuid) -> Result<Vec<RevenueScheduleEntry>> {
        let rows = sqlx::query_as::<_, RevenueScheduleEntryRow>(
            "SELECT * FROM revenue_schedule_entries WHERE obligation_id = $1 ORDER BY period",
        )
        .bind(obligation_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;
        rows.into_iter().map(Self::row_to_entry).collect()
    }

    async fn require_obligation_locked(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
    ) -> Result<PerformanceObligation> {
        let row = sqlx::query_as::<_, PerformanceObligationRow>(
            "SELECT * FROM performance_obligations WHERE id = $1 FOR UPDATE",
        )
        .bind(id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        row.map(Self::row_to_obligation).transpose()?.ok_or(CommerceError::NotFound)
    }

    /// Create a revenue contract with obligations (async)
    pub async fn create_contract_async(
        &self,
        input: CreateRevenueContract,
    ) -> Result<RevenueContract> {
        input.validate()?;
        let id = Uuid::new_v4();
        let now = Utc::now();
        let contract_number =
            input.contract_number.clone().unwrap_or_else(generate_revenue_contract_number);
        let currency = input.currency.unwrap_or_default();

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        sqlx::query(
            "INSERT INTO revenue_contracts (id, contract_number, customer_id, order_id, invoice_id, transaction_price, currency, status, effective_date, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, 'draft', $8, $9, $9)",
        )
        .bind(id)
        .bind(&contract_number)
        .bind(input.customer_id)
        .bind(input.order_id)
        .bind(input.invoice_id)
        .bind(input.transaction_price)
        .bind(currency.to_string())
        .bind(input.effective_date)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        for ob in &input.obligations {
            let method_json = serde_json::to_string(&ob.recognition_method)
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            sqlx::query(
                "INSERT INTO performance_obligations (id, contract_id, description, standalone_selling_price, allocated_amount, recognition_method, recognized_amount, created_at, updated_at)
                 VALUES ($1, $2, $3, $4, $5, $6, 0, $7, $7)",
            )
            .bind(Uuid::new_v4())
            .bind(id)
            .bind(&ob.description)
            .bind(ob.standalone_selling_price)
            .bind(ob.allocated_amount)
            .bind(&method_json)
            .bind(now)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        }
        tx.commit().await.map_err(map_db_error)?;
        self.load_full_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// Get a revenue contract with obligations (async)
    pub async fn get_contract_async(&self, id: Uuid) -> Result<Option<RevenueContract>> {
        self.load_full_async(id).await
    }

    /// List revenue contracts (async)
    pub async fn list_contracts_async(
        &self,
        filter: RevenueContractFilter,
    ) -> Result<Vec<RevenueContract>> {
        let after_cursor = super::parse_after_cursor(filter.after_cursor.as_ref())?;
        let limit = super::effective_limit(filter.limit);
        // Offset pagination applies only in non-cursor mode.
        let offset = if after_cursor.is_none() { i64::from(filter.offset.unwrap_or(0)) } else { 0 };
        let mut query = String::from("SELECT * FROM revenue_contracts WHERE 1=1");
        let mut idx = 1;
        if filter.customer_id.is_some() {
            query.push_str(&format!(" AND customer_id = ${idx}"));
            idx += 1;
        }
        if filter.order_id.is_some() {
            query.push_str(&format!(" AND order_id = ${idx}"));
            idx += 1;
        }
        if filter.invoice_id.is_some() {
            query.push_str(&format!(" AND invoice_id = ${idx}"));
            idx += 1;
        }
        if filter.status.is_some() {
            query.push_str(&format!(" AND status = ${idx}"));
            idx += 1;
        }
        if filter.effective_from.is_some() {
            query.push_str(&format!(" AND effective_date >= ${idx}"));
            idx += 1;
        }
        if filter.effective_to.is_some() {
            query.push_str(&format!(" AND effective_date <= ${idx}"));
            idx += 1;
        }
        if filter.search.is_some() {
            query.push_str(&format!(" AND contract_number ILIKE ${idx}"));
            idx += 1;
        }
        if after_cursor.is_some() {
            // Keyset cursor: (created_at, id) for stable DESC ordering
            query.push_str(&format!(
                " AND (created_at < ${idx} OR (created_at = ${idx} AND id < ${}))",
                idx + 1
            ));
            idx += 2;
        }
        query.push_str(&format!(
            " ORDER BY created_at DESC, id DESC LIMIT ${} OFFSET ${}",
            idx,
            idx + 1
        ));

        let mut q = sqlx::query_as::<_, RevenueContractRow>(&query);
        if let Some(customer) = filter.customer_id {
            q = q.bind(customer);
        }
        if let Some(order) = filter.order_id {
            q = q.bind(order);
        }
        if let Some(invoice) = filter.invoice_id {
            q = q.bind(invoice);
        }
        if let Some(status) = filter.status {
            q = q.bind(status.to_string());
        }
        if let Some(from) = filter.effective_from {
            q = q.bind(from);
        }
        if let Some(to) = filter.effective_to {
            q = q.bind(to);
        }
        if let Some(search) = filter.search {
            q = q.bind(format!("%{search}%"));
        }
        if let Some((cursor_created, cursor_id)) = after_cursor {
            q = q.bind(cursor_created).bind(cursor_id);
        }
        let rows = q.bind(limit).bind(offset).fetch_all(&self.pool).await.map_err(map_db_error)?;
        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let id = row.id;
            let mut head = Self::row_to_contract(row)?;
            head.obligations = self.load_obligations_async(id).await?;
            out.push(head);
        }
        Ok(out)
    }

    /// Update a revenue contract (async)
    pub async fn update_contract_async(
        &self,
        id: Uuid,
        input: UpdateRevenueContract,
    ) -> Result<RevenueContract> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let row = sqlx::query_as::<_, RevenueContractRow>(
            "SELECT * FROM revenue_contracts WHERE id = $1 FOR UPDATE",
        )
        .bind(id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        let contract =
            row.map(Self::row_to_contract).transpose()?.ok_or(CommerceError::NotFound)?;
        let status = match input.status {
            Some(next) if next != contract.status => {
                if !contract.status.can_transition_to(next) {
                    return Err(CommerceError::Conflict(format!(
                        "revenue contract cannot transition from {} to {next}",
                        contract.status
                    )));
                }
                next
            }
            _ => contract.status,
        };
        sqlx::query(
            "UPDATE revenue_contracts SET order_id = $1, invoice_id = $2, status = $3, effective_date = $4, updated_at = $5 WHERE id = $6",
        )
        .bind(input.order_id.or(contract.order_id))
        .bind(input.invoice_id.or(contract.invoice_id))
        .bind(status.to_string())
        .bind(input.effective_date.unwrap_or(contract.effective_date))
        .bind(Utc::now())
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        tx.commit().await.map_err(map_db_error)?;
        self.load_full_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// Generate and persist the recognition schedule (async)
    pub async fn generate_schedule_async(&self, obligation_id: Uuid) -> Result<RevenueSchedule> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let obligation = Self::require_obligation_locked(&mut tx, obligation_id).await?;
        // A cancelled contract is dead: generating (or regenerating) a
        // recognition schedule for it would tee up revenue that must never
        // be recognized.
        let contract_status: String =
            sqlx::query_scalar("SELECT status FROM revenue_contracts WHERE id = $1")
                .bind(obligation.contract_id)
                .fetch_one(tx.as_mut())
                .await
                .map_err(map_db_error)?;
        if contract_status == "cancelled" {
            return Err(CommerceError::Conflict(
                "cannot generate a revenue schedule for a cancelled contract".into(),
            ));
        }
        let recognized: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM revenue_schedule_entries WHERE obligation_id = $1 AND status = 'recognized'",
        )
        .bind(obligation_id)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        if recognized > 0 {
            return Err(CommerceError::Conflict(
                "cannot regenerate a schedule with recognized revenue entries".into(),
            ));
        }
        let recognition_date: NaiveDate =
            sqlx::query_scalar("SELECT effective_date FROM revenue_contracts WHERE id = $1")
                .bind(obligation.contract_id)
                .fetch_one(tx.as_mut())
                .await
                .map_err(map_db_error)?;
        let entries = generate_revenue_schedule(
            obligation.recognition_method,
            obligation.allocated_amount,
            recognition_date,
        );
        if entries.is_empty() {
            return Err(CommerceError::ValidationError(
                "cannot generate a time-based revenue schedule for this obligation".into(),
            ));
        }
        sqlx::query("DELETE FROM revenue_schedule_entries WHERE obligation_id = $1")
            .bind(obligation_id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        for e in &entries {
            sqlx::query(
                "INSERT INTO revenue_schedule_entries (obligation_id, period, period_start, amount, status)
                 VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(obligation_id)
            .bind(i32::try_from(e.period).unwrap_or(i32::MAX))
            .bind(e.period_start)
            .bind(e.amount)
            .bind(e.status.to_string())
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        }
        tx.commit().await.map_err(map_db_error)?;
        Ok(RevenueSchedule {
            obligation_id,
            method: obligation.recognition_method,
            total_amount: obligation.allocated_amount,
            entries,
        })
    }

    /// Get the persisted recognition schedule (async)
    pub async fn get_schedule_async(&self, obligation_id: Uuid) -> Result<Option<RevenueSchedule>> {
        let row = sqlx::query_as::<_, PerformanceObligationRow>(
            "SELECT * FROM performance_obligations WHERE id = $1",
        )
        .bind(obligation_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;
        let Some(obligation) = row.map(Self::row_to_obligation).transpose()? else {
            return Ok(None);
        };
        let entries = self.load_entries_async(obligation_id).await?;
        if entries.is_empty() {
            return Ok(None);
        }
        Ok(Some(RevenueSchedule {
            obligation_id,
            method: obligation.recognition_method,
            total_amount: entries.iter().map(|e| e.amount).sum(),
            entries,
        }))
    }

    /// Recognize deferred entries through a date (async)
    pub async fn recognize_period_async(
        &self,
        obligation_id: Uuid,
        through: NaiveDate,
    ) -> Result<RevenueSchedule> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let obligation = Self::require_obligation_locked(&mut tx, obligation_id).await?;
        // Revenue may only be recognized on a live contract: a draft has not
        // been agreed and a cancelled contract is dead. Completed stays
        // allowed so a retry after a final recognition remains an idempotent
        // no-op. Mirrors the SQLite backend.
        let contract_status: String =
            sqlx::query_scalar("SELECT status FROM revenue_contracts WHERE id = $1")
                .bind(obligation.contract_id)
                .fetch_one(tx.as_mut())
                .await
                .map_err(map_db_error)?;
        if matches!(contract_status.as_str(), "draft" | "cancelled") {
            return Err(CommerceError::Conflict(format!(
                "cannot recognize revenue on a {contract_status} contract"
            )));
        }
        let total_entries: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM revenue_schedule_entries WHERE obligation_id = $1",
        )
        .bind(obligation_id)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        if total_entries == 0 {
            return Err(CommerceError::Conflict(
                "no revenue schedule has been generated for this obligation".into(),
            ));
        }
        let newly_recognized: Option<Decimal> = sqlx::query_scalar(
            "SELECT SUM(amount) FROM revenue_schedule_entries WHERE obligation_id = $1 AND status = 'deferred' AND period_start <= $2",
        )
        .bind(obligation_id)
        .bind(through)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        let newly_recognized = newly_recognized.unwrap_or(Decimal::ZERO);
        // Captured so a failed GL post below can revert exactly these entries
        // (older recognized entries share the same period_start predicate).
        let flipped: Vec<i32> = sqlx::query_scalar(
            "SELECT period FROM revenue_schedule_entries WHERE obligation_id = $1 AND status = 'deferred' AND period_start <= $2",
        )
        .bind(obligation_id)
        .bind(through)
        .fetch_all(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        sqlx::query(
            "UPDATE revenue_schedule_entries SET status = 'recognized' WHERE obligation_id = $1 AND status = 'deferred' AND period_start <= $2",
        )
        .bind(obligation_id)
        .bind(through)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        let now = Utc::now();
        sqlx::query(
            "UPDATE performance_obligations SET recognized_amount = recognized_amount + $1, updated_at = $2 WHERE id = $3",
        )
        .bind(newly_recognized)
        .bind(now)
        .bind(obligation_id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        // Complete the contract when every obligation is fully recognized.
        let completed_now = sqlx::query(
            "UPDATE revenue_contracts SET status = 'completed', updated_at = $1
             WHERE id = $2 AND status = 'active' AND NOT EXISTS (
                 SELECT 1 FROM performance_obligations
                 WHERE contract_id = $2 AND recognized_amount < allocated_amount
             )",
        )
        .bind(now)
        .bind(obligation.contract_id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .rows_affected()
            > 0;
        tx.commit().await.map_err(map_db_error)?;
        // The GL post runs outside the subledger transaction. If it fails,
        // compensate: revert exactly what this call recognized so a retry
        // recognizes and posts the full amount again, instead of leaving
        // revenue recognized in the subledger with no journal entry and no
        // repair path. Mirrors the SQLite backend.
        if let Err(post_err) =
            self.auto_post_recognition_entry(obligation_id, newly_recognized, through).await
        {
            let revert = async {
                let mut tx = self.pool.begin().await.map_err(map_db_error)?;
                sqlx::query(
                    "UPDATE revenue_schedule_entries SET status = 'deferred'
                     WHERE obligation_id = $1 AND status = 'recognized' AND period = ANY($2)",
                )
                .bind(obligation_id)
                .bind(&flipped)
                .execute(tx.as_mut())
                .await
                .map_err(map_db_error)?;
                sqlx::query(
                    "UPDATE performance_obligations SET recognized_amount = recognized_amount - $1, updated_at = $2 WHERE id = $3",
                )
                .bind(newly_recognized)
                .bind(Utc::now())
                .bind(obligation_id)
                .execute(tx.as_mut())
                .await
                .map_err(map_db_error)?;
                if completed_now {
                    sqlx::query(
                        "UPDATE revenue_contracts SET status = 'active', updated_at = $1
                         WHERE id = $2 AND status = 'completed'",
                    )
                    .bind(Utc::now())
                    .bind(obligation.contract_id)
                    .execute(tx.as_mut())
                    .await
                    .map_err(map_db_error)?;
                }
                tx.commit().await.map_err(map_db_error)
            };
            let _: Result<()> = revert.await;
            return Err(post_err);
        }
        let entries = self.load_entries_async(obligation_id).await?;
        Ok(RevenueSchedule {
            obligation_id,
            method: obligation.recognition_method,
            total_amount: entries.iter().map(|e| e.amount).sum(),
            entries,
        })
    }

    /// Create and post a balanced revenue-recognition journal entry
    /// (debit deferred/unearned revenue, credit sales revenue) when the active
    /// GL auto-posting config has `auto_post_revenue_recognition` enabled.
    ///
    /// Mirrors the invoice auto-posting pattern: config-gated, posted via the
    /// existing general-ledger repository. The deferred-revenue account is the
    /// config's `unearned_revenue_account_id`, falling back to the first active
    /// posting account with the `unearned_revenue` sub-type; revenue is credited
    /// to the config's `sales_revenue_account_id`.
    async fn auto_post_recognition_entry(
        &self,
        obligation_id: Uuid,
        amount: Decimal,
        through: NaiveDate,
    ) -> Result<()> {
        let gl = super::general_ledger::PgGeneralLedgerRepository::new(self.pool.clone());
        let Some(config) = gl.get_auto_posting_config_async().await? else { return Ok(()) };
        if !config.auto_post_revenue_recognition || amount <= Decimal::ZERO {
            return Ok(());
        }
        let deferred_account_id = match config.unearned_revenue_account_id {
            Some(id) => id,
            None => gl
                .list_accounts_async(stateset_core::GlAccountFilter {
                    account_sub_type: Some(stateset_core::AccountSubType::UnearnedRevenue),
                    status: Some(stateset_core::AccountStatus::Active),
                    is_posting: Some(true),
                    limit: Some(1),
                    ..Default::default()
                })
                .await?
                .first()
                .map(|a| a.id)
                .ok_or_else(|| {
                    CommerceError::ValidationError(
                        "auto-post revenue recognition requires an unearned_revenue account in the auto-posting config or chart of accounts"
                            .to_string(),
                    )
                })?,
        };
        gl.create_journal_entry_async(stateset_core::CreateJournalEntry {
            entry_date: through,
            entry_type: Some(stateset_core::JournalEntryType::Standard),
            description: format!("Revenue recognition for obligation {obligation_id}"),
            lines: vec![
                stateset_core::CreateJournalEntryLine::debit(
                    deferred_account_id,
                    amount,
                    Some("Deferred Revenue".to_string()),
                ),
                stateset_core::CreateJournalEntryLine::credit(
                    config.sales_revenue_account_id,
                    amount,
                    Some("Sales Revenue".to_string()),
                ),
            ],
            source_document_type: Some("revenue_recognition".to_string()),
            source_document_id: Some(obligation_id),
            auto_post: Some(true),
        })
        .await?;
        Ok(())
    }

    /// List the obligations under a contract (async)
    pub async fn list_obligations_async(
        &self,
        contract_id: Uuid,
    ) -> Result<Vec<PerformanceObligation>> {
        self.load_obligations_async(contract_id).await
    }
}

impl RevenueRecognitionRepository for PgRevenueRecognitionRepository {
    fn create_contract(&self, input: CreateRevenueContract) -> Result<RevenueContract> {
        super::block_on(self.create_contract_async(input))
    }

    fn get_contract(&self, id: Uuid) -> Result<Option<RevenueContract>> {
        super::block_on(self.get_contract_async(id))
    }

    fn list_contracts(&self, filter: RevenueContractFilter) -> Result<Vec<RevenueContract>> {
        super::block_on(self.list_contracts_async(filter))
    }

    fn update_contract(&self, id: Uuid, input: UpdateRevenueContract) -> Result<RevenueContract> {
        super::block_on(self.update_contract_async(id, input))
    }

    fn list_obligations(&self, contract_id: Uuid) -> Result<Vec<PerformanceObligation>> {
        super::block_on(self.list_obligations_async(contract_id))
    }

    fn generate_schedule(&self, obligation_id: Uuid) -> Result<RevenueSchedule> {
        super::block_on(self.generate_schedule_async(obligation_id))
    }

    fn get_schedule(&self, obligation_id: Uuid) -> Result<Option<RevenueSchedule>> {
        super::block_on(self.get_schedule_async(obligation_id))
    }

    fn recognize_period(&self, obligation_id: Uuid, through: NaiveDate) -> Result<RevenueSchedule> {
        super::block_on(self.recognize_period_async(obligation_id, through))
    }
}
