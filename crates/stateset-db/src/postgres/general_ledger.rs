//! PostgreSQL implementation of General Ledger repository

use super::kernel_outbox::append_kernel_event_tx;
use super::{block_on, map_db_error};
use crate::KernelOutboxEvent;
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;
use sqlx::{FromRow, Postgres, QueryBuilder};
use stateset_core::{
    AccountStatus, AccountSubType, AccountType, AutoPostingConfig, BalanceSheet, BalanceSheetLine,
    BalanceSide, BatchResult, CommerceError, CreateAutoPostingConfig, CreateGlAccount,
    CreateGlPeriod, CreateJournalEntry, CreateJournalEntryLine, Currency, CurrencyCode,
    FX_REVALUATION_REFERENCE, GeneralLedgerRepository, GlAccount, GlAccountFilter, GlPeriod,
    GlPeriodFilter, IncomeStatement, IncomeStatementLine, InvoiceId, JournalEntry,
    JournalEntryFilter, JournalEntryLine, JournalEntrySource, JournalEntryStatus, JournalEntryType,
    PeriodStatus, Result, RevaluationResult, TrialBalance, TrialBalanceLine,
    create_default_chart_of_accounts, generate_journal_entry_number,
};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct PgGeneralLedgerRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct AccountRow {
    id: Uuid,
    account_number: String,
    name: String,
    description: Option<String>,
    account_type: String,
    account_sub_type: Option<String>,
    parent_account_id: Option<Uuid>,
    is_header: bool,
    is_posting: bool,
    normal_balance: String,
    currency: CurrencyCode,
    status: String,
    current_balance: Decimal,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct PeriodRow {
    id: Uuid,
    period_name: String,
    fiscal_year: i32,
    period_number: i32,
    start_date: NaiveDate,
    end_date: NaiveDate,
    status: String,
    closed_at: Option<DateTime<Utc>>,
    closed_by: Option<String>,
    locked_at: Option<DateTime<Utc>>,
    locked_by: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
pub(crate) struct JournalEntryRow {
    id: Uuid,
    entry_number: String,
    entry_date: NaiveDate,
    period_id: Uuid,
    entry_type: String,
    source: String,
    source_document_type: Option<String>,
    source_document_id: Option<Uuid>,
    description: String,
    total_debits: Decimal,
    total_credits: Decimal,
    is_balanced: bool,
    status: String,
    posted_at: Option<DateTime<Utc>>,
    posted_by: Option<String>,
    reversed_entry_id: Option<Uuid>,
    reversing_entry_id: Option<Uuid>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
pub(crate) struct JournalEntryLineRow {
    id: Uuid,
    journal_entry_id: Uuid,
    line_number: i32,
    account_id: Uuid,
    account_number: Option<String>,
    account_name: Option<String>,
    description: Option<String>,
    debit_amount: Decimal,
    credit_amount: Decimal,
    currency: CurrencyCode,
    reference_type: Option<String>,
    reference_id: Option<Uuid>,
    created_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct AutoPostingConfigRow {
    id: Uuid,
    config_name: String,
    cash_account_id: Uuid,
    accounts_receivable_account_id: Uuid,
    inventory_account_id: Uuid,
    accounts_payable_account_id: Uuid,
    unearned_revenue_account_id: Option<Uuid>,
    sales_revenue_account_id: Uuid,
    shipping_revenue_account_id: Option<Uuid>,
    cogs_account_id: Uuid,
    bad_debt_expense_account_id: Option<Uuid>,
    fx_gain_loss_account_id: Option<Uuid>,
    auto_post_depreciation: bool,
    auto_post_revenue_recognition: bool,
    is_active: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl PgGeneralLedgerRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_account(row: AccountRow) -> Result<GlAccount> {
        let AccountRow {
            id,
            account_number,
            name,
            description,
            account_type,
            account_sub_type,
            parent_account_id,
            is_header,
            is_posting,
            normal_balance,
            currency,
            status,
            current_balance,
            created_at,
            updated_at,
        } = row;

        let account_type: AccountType = account_type.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid gl_account.account_type '{}': {}",
                account_type, e
            ))
        })?;
        let account_sub_type = match account_sub_type {
            Some(value) if !value.trim().is_empty() => Some(value.parse().map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid gl_account.account_sub_type '{}': {}",
                    value, e
                ))
            })?),
            _ => None,
        };
        let normal_balance: BalanceSide = normal_balance.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid gl_account.normal_balance '{}': {}",
                normal_balance, e
            ))
        })?;
        let status: AccountStatus = status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid gl_account.status '{}': {}", status, e))
        })?;

        Ok(GlAccount {
            id,
            account_number,
            name,
            description,
            account_type,
            account_sub_type,
            parent_account_id,
            is_header,
            is_posting,
            normal_balance,
            currency,
            status,
            current_balance,
            created_at,
            updated_at,
        })
    }

    fn row_to_period(row: PeriodRow) -> Result<GlPeriod> {
        let PeriodRow {
            id,
            period_name,
            fiscal_year,
            period_number,
            start_date,
            end_date,
            status,
            closed_at,
            closed_by,
            locked_at,
            locked_by,
            created_at,
            updated_at,
        } = row;

        let status: PeriodStatus = status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid gl_period.status '{}': {}", status, e))
        })?;

        Ok(GlPeriod {
            id,
            period_name,
            fiscal_year,
            period_number,
            start_date,
            end_date,
            status,
            closed_at,
            closed_by,
            locked_at,
            locked_by,
            created_at,
            updated_at,
        })
    }

    pub(crate) fn row_to_journal_entry(row: JournalEntryRow) -> Result<JournalEntry> {
        let JournalEntryRow {
            id,
            entry_number,
            entry_date,
            period_id,
            entry_type,
            source,
            source_document_type,
            source_document_id,
            description,
            total_debits,
            total_credits,
            is_balanced,
            status,
            posted_at,
            posted_by,
            reversed_entry_id,
            reversing_entry_id,
            created_at,
            updated_at,
        } = row;

        let entry_type: JournalEntryType = entry_type.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid gl_journal_entry.entry_type '{}': {}",
                entry_type, e
            ))
        })?;
        let source: JournalEntrySource = source.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid gl_journal_entry.source '{}': {}",
                source, e
            ))
        })?;
        let status: JournalEntryStatus = status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid gl_journal_entry.status '{}': {}",
                status, e
            ))
        })?;

        Ok(JournalEntry {
            id,
            entry_number,
            entry_date,
            period_id,
            entry_type,
            source,
            source_document_type,
            source_document_id,
            description,
            total_debits,
            total_credits,
            is_balanced,
            status,
            posted_at,
            posted_by,
            reversed_entry_id,
            reversing_entry_id,
            lines: Vec::new(),
            created_at,
            updated_at,
        })
    }

    pub(crate) fn row_to_journal_entry_line(row: JournalEntryLineRow) -> JournalEntryLine {
        JournalEntryLine {
            id: row.id,
            journal_entry_id: row.journal_entry_id,
            line_number: row.line_number,
            account_id: row.account_id,
            account_number: row.account_number,
            account_name: row.account_name,
            description: row.description,
            debit_amount: row.debit_amount,
            credit_amount: row.credit_amount,
            currency: row.currency,
            reference_type: row.reference_type,
            reference_id: row.reference_id,
            created_at: row.created_at,
        }
    }

    fn row_to_auto_posting_config(row: AutoPostingConfigRow) -> AutoPostingConfig {
        AutoPostingConfig {
            id: row.id,
            config_name: row.config_name,
            cash_account_id: row.cash_account_id,
            accounts_receivable_account_id: row.accounts_receivable_account_id,
            inventory_account_id: row.inventory_account_id,
            accounts_payable_account_id: row.accounts_payable_account_id,
            unearned_revenue_account_id: row.unearned_revenue_account_id,
            sales_revenue_account_id: row.sales_revenue_account_id,
            shipping_revenue_account_id: row.shipping_revenue_account_id,
            cogs_account_id: row.cogs_account_id,
            bad_debt_expense_account_id: row.bad_debt_expense_account_id,
            fx_gain_loss_account_id: row.fx_gain_loss_account_id,
            auto_post_depreciation: row.auto_post_depreciation,
            auto_post_revenue_recognition: row.auto_post_revenue_recognition,
            is_active: row.is_active,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }

    pub(crate) async fn update_account_balance_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, Postgres>,
        account_id: Uuid,
        debit: Decimal,
        credit: Decimal,
    ) -> Result<()> {
        let updated = sqlx::query(
            "UPDATE gl_accounts
             SET current_balance = current_balance + (
                CASE
                    WHEN normal_balance = 'debit' THEN $1 - $2
                    ELSE $2 - $1
                END
             )
             WHERE id = $3",
        )
        .bind(debit)
        .bind(credit)
        .bind(account_id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        if updated.rows_affected() == 0 {
            return Err(CommerceError::NotFound);
        }

        Ok(())
    }

    pub async fn create_account_async(&self, input: CreateGlAccount) -> Result<GlAccount> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let normal_balance = input.account_type.normal_balance();
        let currency = input.currency.unwrap_or(CurrencyCode::USD);
        let is_header = input.is_header.unwrap_or(false);
        let is_posting = input.is_posting.unwrap_or(true);

        sqlx::query(
            "INSERT INTO gl_accounts (id, account_number, name, description, account_type,
                account_sub_type, parent_account_id, is_header, is_posting, normal_balance,
                currency, status, current_balance, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)",
        )
        .bind(id)
        .bind(&input.account_number)
        .bind(&input.name)
        .bind(input.description)
        .bind(input.account_type.to_string())
        .bind(input.account_sub_type.map(|s| s.to_string()))
        .bind(input.parent_account_id)
        .bind(is_header)
        .bind(is_posting)
        .bind(normal_balance.to_string())
        .bind(currency)
        .bind(AccountStatus::Active.to_string())
        .bind(Decimal::ZERO)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_account_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn get_account_async(&self, id: Uuid) -> Result<Option<GlAccount>> {
        let row = sqlx::query_as::<_, AccountRow>(
            "SELECT id, account_number, name, description, account_type, account_sub_type,
                    parent_account_id, is_header, is_posting, normal_balance, currency, status,
                    current_balance, created_at, updated_at
             FROM gl_accounts WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_account).transpose()
    }

    pub async fn get_account_by_number_async(
        &self,
        account_number: &str,
    ) -> Result<Option<GlAccount>> {
        let row = sqlx::query_as::<_, AccountRow>(
            "SELECT id, account_number, name, description, account_type, account_sub_type,
                    parent_account_id, is_header, is_posting, normal_balance, currency, status,
                    current_balance, created_at, updated_at
             FROM gl_accounts WHERE account_number = $1",
        )
        .bind(account_number)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_account).transpose()
    }

    pub async fn update_account_async(
        &self,
        id: Uuid,
        input: stateset_core::UpdateGlAccount,
    ) -> Result<GlAccount> {
        sqlx::query(
            "UPDATE gl_accounts SET
                name = COALESCE($1, name),
                description = COALESCE($2, description),
                parent_account_id = COALESCE($3, parent_account_id),
                status = COALESCE($4, status),
                updated_at = $5
             WHERE id = $6",
        )
        .bind(input.name)
        .bind(input.description)
        .bind(input.parent_account_id)
        .bind(input.status.map(|s| s.to_string()))
        .bind(Utc::now())
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_account_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn list_accounts_async(&self, filter: GlAccountFilter) -> Result<Vec<GlAccount>> {
        let mut builder = QueryBuilder::<Postgres>::new(
            "SELECT id, account_number, name, description, account_type, account_sub_type,
                    parent_account_id, is_header, is_posting, normal_balance, currency, status,
                    current_balance, created_at, updated_at
             FROM gl_accounts WHERE 1=1",
        );

        if let Some(account_type) = filter.account_type {
            builder.push(" AND account_type = ").push_bind(account_type.to_string());
        }
        if let Some(account_sub_type) = filter.account_sub_type {
            builder.push(" AND account_sub_type = ").push_bind(account_sub_type.to_string());
        }
        if let Some(parent_account_id) = filter.parent_account_id {
            builder.push(" AND parent_account_id = ").push_bind(parent_account_id);
        }
        if let Some(status) = filter.status {
            builder.push(" AND status = ").push_bind(status.to_string());
        }
        if let Some(is_posting) = filter.is_posting {
            builder.push(" AND is_posting = ").push_bind(is_posting);
        }
        if let Some(is_header) = filter.is_header {
            builder.push(" AND is_header = ").push_bind(is_header);
        }
        if let Some(search) = filter.search {
            let term = format!("%{}%", search);
            builder
                .push(" AND (name ILIKE ")
                .push_bind(term.clone())
                .push(" OR account_number ILIKE ")
                .push_bind(term)
                .push(")");
        }

        builder.push(" ORDER BY account_number");

        builder.push(" LIMIT ").push_bind(super::effective_limit(filter.limit));
        if let Some(offset) = filter.offset {
            builder.push(" OFFSET ").push_bind(offset as i64);
        }

        let rows = builder
            .build_query_as::<AccountRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_account).collect::<Result<Vec<_>>>()
    }

    pub async fn get_account_hierarchy_async(&self) -> Result<Vec<GlAccount>> {
        self.list_accounts_async(GlAccountFilter::default()).await
    }

    pub async fn delete_account_async(&self, id: Uuid) -> Result<()> {
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM gl_journal_entry_lines WHERE account_id = $1")
                .bind(id)
                .fetch_one(&self.pool)
                .await
                .map_err(map_db_error)?;

        if count > 0 {
            return Err(CommerceError::ValidationError(
                "Cannot delete account with existing transactions".into(),
            ));
        }

        sqlx::query("DELETE FROM gl_accounts WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(())
    }

    pub async fn initialize_chart_of_accounts_async(&self) -> Result<Vec<GlAccount>> {
        let defaults = create_default_chart_of_accounts();
        let mut accounts = Vec::new();

        for input in defaults {
            if self.get_account_by_number_async(&input.account_number).await?.is_none() {
                accounts.push(self.create_account_async(input).await?);
            }
        }

        Ok(accounts)
    }

    pub async fn create_period_async(&self, input: CreateGlPeriod) -> Result<GlPeriod> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        sqlx::query(
            "INSERT INTO gl_periods (id, period_name, fiscal_year, period_number, start_date,
                end_date, status, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        )
        .bind(id)
        .bind(&input.period_name)
        .bind(input.fiscal_year)
        .bind(input.period_number)
        .bind(input.start_date)
        .bind(input.end_date)
        .bind(PeriodStatus::Future.to_string())
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_period_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn get_period_async(&self, id: Uuid) -> Result<Option<GlPeriod>> {
        let row = sqlx::query_as::<_, PeriodRow>(
            "SELECT id, period_name, fiscal_year, period_number, start_date, end_date,
                    status, closed_at, closed_by, locked_at, locked_by, created_at, updated_at
             FROM gl_periods WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_period).transpose()
    }

    pub async fn get_current_period_async(&self) -> Result<Option<GlPeriod>> {
        let row = sqlx::query_as::<_, PeriodRow>(
            "SELECT id, period_name, fiscal_year, period_number, start_date, end_date,
                    status, closed_at, closed_by, locked_at, locked_by, created_at, updated_at
             FROM gl_periods WHERE status = 'open' ORDER BY start_date DESC LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_period).transpose()
    }

    pub async fn get_period_for_date_async(&self, date: NaiveDate) -> Result<Option<GlPeriod>> {
        let row = sqlx::query_as::<_, PeriodRow>(
            "SELECT id, period_name, fiscal_year, period_number, start_date, end_date,
                    status, closed_at, closed_by, locked_at, locked_by, created_at, updated_at
             FROM gl_periods WHERE start_date <= $1 AND end_date >= $1",
        )
        .bind(date)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_period).transpose()
    }

    pub async fn list_periods_async(&self, filter: GlPeriodFilter) -> Result<Vec<GlPeriod>> {
        let mut builder = QueryBuilder::<Postgres>::new(
            "SELECT id, period_name, fiscal_year, period_number, start_date, end_date,
                    status, closed_at, closed_by, locked_at, locked_by, created_at, updated_at
             FROM gl_periods WHERE 1=1",
        );

        if let Some(fiscal_year) = filter.fiscal_year {
            builder.push(" AND fiscal_year = ").push_bind(fiscal_year);
        }
        if let Some(status) = filter.status {
            builder.push(" AND status = ").push_bind(status.to_string());
        }

        // Order by period identity (unique, deterministic), matching SQLite — not
        // `start_date`, which can tie and disagrees when a lower-numbered period has
        // a later start date.
        builder.push(" ORDER BY fiscal_year DESC, period_number DESC");

        builder.push(" LIMIT ").push_bind(super::effective_limit(filter.limit));
        if let Some(offset) = filter.offset {
            builder.push(" OFFSET ").push_bind(offset as i64);
        }

        let rows = builder
            .build_query_as::<PeriodRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_period).collect::<Result<Vec<_>>>()
    }

    pub async fn open_period_async(&self, id: Uuid) -> Result<GlPeriod> {
        sqlx::query("UPDATE gl_periods SET status = 'open' WHERE id = $1 AND status = 'future'")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_period_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn close_period_async(&self, id: Uuid, closed_by: &str) -> Result<GlPeriod> {
        let now = Utc::now();
        sqlx::query(
            "UPDATE gl_periods SET status = 'closed', closed_at = $1, closed_by = $2
             WHERE id = $3 AND status = 'open'",
        )
        .bind(now)
        .bind(closed_by)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_period_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn lock_period_async(&self, id: Uuid, locked_by: &str) -> Result<GlPeriod> {
        let now = Utc::now();
        sqlx::query(
            "UPDATE gl_periods SET status = 'locked', locked_at = $1, locked_by = $2
             WHERE id = $3 AND status = 'closed'",
        )
        .bind(now)
        .bind(locked_by)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_period_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn reopen_period_async(&self, id: Uuid) -> Result<GlPeriod> {
        sqlx::query(
            "UPDATE gl_periods SET status = 'open', closed_at = NULL, closed_by = NULL
             WHERE id = $1 AND status = 'closed'",
        )
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_period_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn create_journal_entry_async(
        &self,
        input: CreateJournalEntry,
    ) -> Result<JournalEntry> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let entry_number = generate_journal_entry_number();

        let period = self.get_period_for_date_async(input.entry_date).await?.ok_or_else(|| {
            CommerceError::ValidationError(format!("No period found for date {}", input.entry_date))
        })?;

        if !period.can_post() {
            return Err(CommerceError::ValidationError(
                "Period is not open for posting".to_string(),
            ));
        }

        let total_debits: Decimal = input.lines.iter().map(|l| l.debit_amount).sum();
        let total_credits: Decimal = input.lines.iter().map(|l| l.credit_amount).sum();
        let is_balanced = total_debits == total_credits;
        // Invariant `commerce.ledger.line_not_single_sided`: a line is a pure
        // debit or a pure credit, never both and never neither.
        if let Some((index, _)) = input.lines.iter().enumerate().find(|(_, l)| {
            !((l.debit_amount > Decimal::ZERO && l.credit_amount == Decimal::ZERO)
                || (l.debit_amount == Decimal::ZERO && l.credit_amount > Decimal::ZERO))
        }) {
            return Err(CommerceError::JournalLineNotSingleSided {
                entry_id: id,
                line_number: i32::try_from(index + 1).unwrap_or(i32::MAX),
            });
        }

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        sqlx::query(
            "INSERT INTO gl_journal_entries (id, entry_number, entry_date, period_id, entry_type,
                source, source_document_type, source_document_id, description, total_debits,
                total_credits, is_balanced, status, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)",
        )
        .bind(id)
        .bind(&entry_number)
        .bind(input.entry_date)
        .bind(period.id)
        .bind(input.entry_type.unwrap_or(JournalEntryType::Standard).to_string())
        .bind(JournalEntrySource::Manual.to_string())
        .bind(input.source_document_type.clone())
        .bind(input.source_document_id)
        .bind(&input.description)
        .bind(total_debits)
        .bind(total_credits)
        .bind(is_balanced)
        .bind(JournalEntryStatus::Draft.to_string())
        .bind(now)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        for (line_num, line) in input.lines.iter().enumerate() {
            let line_id = Uuid::new_v4();
            let account =
                self.get_account_async(line.account_id).await?.ok_or(CommerceError::NotFound)?;

            sqlx::query(
                "INSERT INTO gl_journal_entry_lines (id, journal_entry_id, line_number, account_id,
                    account_number, account_name, description, debit_amount, credit_amount, currency,
                    reference_type, reference_id, created_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
            )
            .bind(line_id)
            .bind(id)
            .bind((line_num + 1) as i32)
            .bind(line.account_id)
            .bind(account.account_number)
            .bind(account.name)
            .bind(line.description.clone())
            .bind(line.debit_amount)
            .bind(line.credit_amount)
            .bind(account.currency)
            .bind(line.reference_type.clone())
            .bind(line.reference_id)
            .bind(now)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        }

        tx.commit().await.map_err(map_db_error)?;

        if input.auto_post.unwrap_or(false) && is_balanced {
            return self.post_journal_entry_async(id, "system").await;
        }

        self.get_journal_entry_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn get_journal_entry_async(&self, id: Uuid) -> Result<Option<JournalEntry>> {
        let row = sqlx::query_as::<_, JournalEntryRow>(
            "SELECT id, entry_number, entry_date, period_id, entry_type, source,
                    source_document_type, source_document_id, description, total_debits,
                    total_credits, is_balanced, status, posted_at, posted_by,
                    reversed_entry_id, reversing_entry_id, created_at, updated_at
             FROM gl_journal_entries WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        match row {
            Some(row) => {
                let mut entry = Self::row_to_journal_entry(row)?;
                entry.lines = self.get_journal_entry_lines_async(id).await?;
                Ok(Some(entry))
            }
            None => Ok(None),
        }
    }

    pub async fn get_journal_entry_by_number_async(
        &self,
        number: &str,
    ) -> Result<Option<JournalEntry>> {
        let id = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM gl_journal_entries WHERE entry_number = $1",
        )
        .bind(number)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        match id {
            Some(id) => self.get_journal_entry_async(id).await,
            None => Ok(None),
        }
    }

    pub async fn list_journal_entries_async(
        &self,
        filter: JournalEntryFilter,
    ) -> Result<Vec<JournalEntry>> {
        let columns = "je.id, je.entry_number, je.entry_date, je.period_id, je.entry_type, je.source,\
            je.source_document_type, je.source_document_id, je.description, je.total_debits,\
            je.total_credits, je.is_balanced, je.status, je.posted_at, je.posted_by,\
            je.reversed_entry_id, je.reversing_entry_id, je.created_at, je.updated_at";
        let mut builder = QueryBuilder::<Postgres>::new("SELECT DISTINCT ");
        builder.push(columns).push(" FROM gl_journal_entries je");

        if filter.account_id.is_some() {
            builder.push(" JOIN gl_journal_entry_lines l ON je.id = l.journal_entry_id");
        }

        builder.push(" WHERE 1=1");

        if let Some(period_id) = filter.period_id {
            builder.push(" AND je.period_id = ").push_bind(period_id);
        }
        if let Some(entry_type) = filter.entry_type {
            builder.push(" AND je.entry_type = ").push_bind(entry_type.to_string());
        }
        if let Some(source) = filter.source {
            builder.push(" AND je.source = ").push_bind(source.to_string());
        }
        if let Some(status) = filter.status {
            builder.push(" AND je.status = ").push_bind(status.to_string());
        }
        if let Some(account_id) = filter.account_id {
            builder.push(" AND l.account_id = ").push_bind(account_id);
        }
        if let Some(from_date) = filter.from_date {
            builder.push(" AND je.entry_date >= ").push_bind(from_date);
        }
        if let Some(to_date) = filter.to_date {
            builder.push(" AND je.entry_date <= ").push_bind(to_date);
        }
        if let Some(source_doc_type) = filter.source_document_type {
            builder.push(" AND je.source_document_type = ").push_bind(source_doc_type);
        }
        if let Some(source_doc_id) = filter.source_document_id {
            builder.push(" AND je.source_document_id = ").push_bind(source_doc_id);
        }
        if let Some(search) = filter.search {
            let term = format!("%{}%", search);
            builder
                .push(" AND (je.entry_number ILIKE ")
                .push_bind(term.clone())
                .push(" OR je.description ILIKE ")
                .push_bind(term)
                .push(")");
        }

        // Deterministic tiebreak by entry number for same-date entries, matching
        // SQLite.
        builder.push(" ORDER BY je.entry_date DESC, je.entry_number DESC");

        builder.push(" LIMIT ").push_bind(super::effective_limit(filter.limit));
        if let Some(offset) = filter.offset {
            builder.push(" OFFSET ").push_bind(offset as i64);
        }

        let rows = builder
            .build_query_as::<JournalEntryRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        let mut entries = Vec::new();
        for row in rows {
            let mut entry = Self::row_to_journal_entry(row)?;
            entry.lines = self.get_journal_entry_lines_async(entry.id).await?;
            entries.push(entry);
        }

        Ok(entries)
    }

    pub async fn post_journal_entry_async(
        &self,
        id: Uuid,
        posted_by: &str,
    ) -> Result<JournalEntry> {
        let entry = self.get_journal_entry_async(id).await?.ok_or(CommerceError::NotFound)?;

        // Reports which condition failed: `commerce.ledger.entry_unbalanced` /
        // `commerce.ledger.line_not_single_sided` are typed; "not a draft" and
        // "no lines" keep the historical untyped message.
        entry.ensure_postable()?;

        let now = Utc::now();
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        // The status guard takes the row lock; a concurrent poster blocks on
        // it and then matches zero rows, so the balance updates below can
        // only commit together with exactly one draft -> posted transition.
        let updated = sqlx::query(
            "UPDATE gl_journal_entries SET status = 'posted', posted_at = $1, posted_by = $2
             WHERE id = $3 AND status = 'draft'",
        )
        .bind(now)
        .bind(posted_by)
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        if updated.rows_affected() == 0 {
            return Err(CommerceError::Conflict(
                "Journal entry was modified concurrently".to_string(),
            ));
        }

        for line in &entry.lines {
            self.update_account_balance_tx(
                &mut tx,
                line.account_id,
                line.debit_amount,
                line.credit_amount,
            )
            .await?;
        }

        append_kernel_event_tx(
            tx.as_mut(),
            &KernelOutboxEvent::domain(
                "ledger.journal_entry_posted.v1",
                "journal_entry",
                id.to_string(),
                serde_json::json!({
                    "journal_entry_id": id.to_string(),
                    "entry_number": entry.entry_number,
                    "source": entry.source.to_string(),
                    "total_debits": entry.total_debits.to_string(),
                    "total_credits": entry.total_credits.to_string(),
                    "line_count": entry.lines.len(),
                    "posted_by": posted_by,
                    "status": JournalEntryStatus::Posted.to_string(),
                }),
                None,
            ),
        )
        .await?;

        tx.commit().await.map_err(map_db_error)?;

        self.get_journal_entry_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn void_journal_entry_async(&self, id: Uuid) -> Result<JournalEntry> {
        let entry = self.get_journal_entry_async(id).await?.ok_or(CommerceError::NotFound)?;

        if !entry.can_void() {
            return Err(CommerceError::ValidationError(
                "Entry cannot be voided - must be posted".to_string(),
            ));
        }

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        // The status guard takes the row lock; a concurrent voider blocks on
        // it and then matches zero rows, so the balance reversal below can
        // only commit together with exactly one posted -> voided transition.
        let updated = sqlx::query(
            "UPDATE gl_journal_entries SET status = 'voided' WHERE id = $1 AND status = 'posted'",
        )
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        if updated.rows_affected() == 0 {
            return Err(CommerceError::Conflict(
                "Journal entry was modified concurrently".to_string(),
            ));
        }

        for line in &entry.lines {
            self.update_account_balance_tx(
                &mut tx,
                line.account_id,
                line.credit_amount,
                line.debit_amount,
            )
            .await?;
        }

        tx.commit().await.map_err(map_db_error)?;

        self.get_journal_entry_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn reverse_journal_entry_async(
        &self,
        id: Uuid,
        reversal_date: NaiveDate,
    ) -> Result<JournalEntry> {
        let entry = self.get_journal_entry_async(id).await?.ok_or(CommerceError::NotFound)?;

        if entry.status != JournalEntryStatus::Posted {
            return Err(CommerceError::ValidationError(
                "Can only reverse posted entries".to_string(),
            ));
        }

        // Claim the entry (posted -> reversed) before creating the reversing
        // entry, which commits its own transactions; the status guard ensures
        // concurrent reversals cannot both create (and auto-post) a reversal.
        let claimed = sqlx::query(
            "UPDATE gl_journal_entries SET status = 'reversed' WHERE id = $1 AND status = 'posted'",
        )
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        if claimed.rows_affected() == 0 {
            return Err(CommerceError::Conflict(
                "Journal entry was modified concurrently".to_string(),
            ));
        }

        let reversing_lines: Vec<CreateJournalEntryLine> = entry
            .lines
            .iter()
            .map(|l| CreateJournalEntryLine {
                account_id: l.account_id,
                description: Some(format!("Reversal of {}", entry.entry_number)),
                debit_amount: l.credit_amount,
                credit_amount: l.debit_amount,
                reference_type: l.reference_type.clone(),
                reference_id: l.reference_id,
            })
            .collect();

        let reversing_entry = match self
            .create_journal_entry_async(CreateJournalEntry {
                entry_date: reversal_date,
                entry_type: Some(JournalEntryType::Reversing),
                description: format!("Reversal of {}", entry.entry_number),
                lines: reversing_lines,
                source_document_type: Some("reversal".to_string()),
                source_document_id: Some(entry.id),
                auto_post: Some(true),
            })
            .await
        {
            Ok(reversing_entry) => reversing_entry,
            Err(e) => {
                // Best-effort release of the claim so the entry is not left
                // marked reversed without a reversing entry.
                let _ = sqlx::query(
                    "UPDATE gl_journal_entries SET status = 'posted' WHERE id = $1 AND status = 'reversed'",
                )
                .bind(id)
                .execute(&self.pool)
                .await;
                return Err(e);
            }
        };

        sqlx::query("UPDATE gl_journal_entries SET reversing_entry_id = $1 WHERE id = $2")
            .bind(reversing_entry.id)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        sqlx::query("UPDATE gl_journal_entries SET reversed_entry_id = $1 WHERE id = $2")
            .bind(id)
            .bind(reversing_entry.id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_journal_entry_async(reversing_entry.id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn get_journal_entry_lines_async(
        &self,
        journal_entry_id: Uuid,
    ) -> Result<Vec<JournalEntryLine>> {
        let rows = sqlx::query_as::<_, JournalEntryLineRow>(
            "SELECT id, journal_entry_id, line_number, account_id, account_number, account_name,
                    description, debit_amount, credit_amount, currency, reference_type, reference_id,
                    created_at
             FROM gl_journal_entry_lines WHERE journal_entry_id = $1 ORDER BY line_number",
        )
        .bind(journal_entry_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(rows.into_iter().map(Self::row_to_journal_entry_line).collect())
    }

    pub async fn get_auto_posting_config_async(&self) -> Result<Option<AutoPostingConfig>> {
        let row = sqlx::query_as::<_, AutoPostingConfigRow>(
            "SELECT id, config_name, cash_account_id, accounts_receivable_account_id, inventory_account_id,
                    accounts_payable_account_id, unearned_revenue_account_id, sales_revenue_account_id,
                    shipping_revenue_account_id, cogs_account_id, bad_debt_expense_account_id,
                    fx_gain_loss_account_id, auto_post_depreciation, auto_post_revenue_recognition,
                    is_active, created_at, updated_at
             FROM gl_auto_posting_config WHERE is_active = TRUE
             ORDER BY created_at DESC LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(row.map(Self::row_to_auto_posting_config))
    }

    pub async fn set_auto_posting_config_async(
        &self,
        input: CreateAutoPostingConfig,
    ) -> Result<AutoPostingConfig> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        // Deactivate-then-insert in one transaction, matching the SQLite
        // backend. Without the deactivation this accumulated multiple
        // `is_active` rows and the un-ordered getter returned an arbitrary
        // one — so "setting" the config did not reliably change which config
        // governed auto-posting.
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        sqlx::query("UPDATE gl_auto_posting_config SET is_active = FALSE WHERE is_active = TRUE")
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        sqlx::query(
            "INSERT INTO gl_auto_posting_config (id, config_name, cash_account_id, accounts_receivable_account_id,
                inventory_account_id, accounts_payable_account_id, unearned_revenue_account_id,
                sales_revenue_account_id, shipping_revenue_account_id, cogs_account_id,
                bad_debt_expense_account_id, fx_gain_loss_account_id, auto_post_depreciation,
                auto_post_revenue_recognition, is_active, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)",
        )
        .bind(id)
        .bind(&input.config_name)
        .bind(input.cash_account_id)
        .bind(input.accounts_receivable_account_id)
        .bind(input.inventory_account_id)
        .bind(input.accounts_payable_account_id)
        .bind(input.unearned_revenue_account_id)
        .bind(input.sales_revenue_account_id)
        .bind(input.shipping_revenue_account_id)
        .bind(input.cogs_account_id)
        .bind(input.bad_debt_expense_account_id)
        .bind(input.fx_gain_loss_account_id)
        .bind(input.auto_post_depreciation)
        .bind(input.auto_post_revenue_recognition)
        .bind(true)
        .bind(now)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;

        self.get_auto_posting_config_async().await?.ok_or(CommerceError::NotFound)
    }

    pub async fn auto_post_invoice_async(&self, invoice_id: Uuid) -> Result<JournalEntry> {
        let config = self.get_auto_posting_config_async().await?.ok_or_else(|| {
            CommerceError::ValidationError("Auto-posting not configured".to_string())
        })?;

        let (amount, invoice_date): (Decimal, DateTime<Utc>) =
            sqlx::query_as("SELECT total, invoice_date FROM invoices WHERE id = $1")
                .bind(invoice_id)
                .fetch_one(&self.pool)
                .await
                .map_err(map_db_error)?;

        let entry_date = invoice_date.date_naive();

        self.create_journal_entry_async(CreateJournalEntry {
            entry_date,
            entry_type: Some(JournalEntryType::Standard),
            description: format!("Invoice {}", invoice_id),
            lines: vec![
                CreateJournalEntryLine::debit(
                    config.accounts_receivable_account_id,
                    amount,
                    Some("Accounts Receivable".to_string()),
                ),
                CreateJournalEntryLine::credit(
                    config.sales_revenue_account_id,
                    amount,
                    Some("Sales Revenue".to_string()),
                ),
            ],
            source_document_type: Some("invoice".to_string()),
            source_document_id: Some(invoice_id),
            auto_post: Some(true),
        })
        .await
    }

    pub async fn auto_post_payment_received_async(&self, payment_id: Uuid) -> Result<JournalEntry> {
        let config = self.get_auto_posting_config_async().await?.ok_or_else(|| {
            CommerceError::ValidationError("Auto-posting not configured".to_string())
        })?;

        let (amount, paid_at): (Decimal, DateTime<Utc>) = sqlx::query_as(
            "SELECT amount, COALESCE(paid_at, created_at) FROM payments WHERE id = $1",
        )
        .bind(payment_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        let entry_date = paid_at.date_naive();

        self.create_journal_entry_async(CreateJournalEntry {
            entry_date,
            entry_type: Some(JournalEntryType::Standard),
            description: format!("Payment {}", payment_id),
            lines: vec![
                CreateJournalEntryLine::debit(
                    config.cash_account_id,
                    amount,
                    Some("Cash".to_string()),
                ),
                CreateJournalEntryLine::credit(
                    config.accounts_receivable_account_id,
                    amount,
                    Some("Accounts Receivable".to_string()),
                ),
            ],
            source_document_type: Some("payment".to_string()),
            source_document_id: Some(payment_id),
            auto_post: Some(true),
        })
        .await
    }

    pub async fn auto_post_bill_async(&self, bill_id: Uuid) -> Result<JournalEntry> {
        let config = self.get_auto_posting_config_async().await?.ok_or_else(|| {
            CommerceError::ValidationError("Auto-posting not configured".to_string())
        })?;

        let (amount, bill_date): (Decimal, NaiveDate) =
            sqlx::query_as("SELECT total_amount, bill_date FROM ap_bills WHERE id = $1")
                .bind(bill_id)
                .fetch_one(&self.pool)
                .await
                .map_err(map_db_error)?;

        self.create_journal_entry_async(CreateJournalEntry {
            entry_date: bill_date,
            entry_type: Some(JournalEntryType::Standard),
            description: format!("Bill {}", bill_id),
            lines: vec![
                CreateJournalEntryLine::debit(
                    config.inventory_account_id,
                    amount,
                    Some("Inventory/Expense".to_string()),
                ),
                CreateJournalEntryLine::credit(
                    config.accounts_payable_account_id,
                    amount,
                    Some("Accounts Payable".to_string()),
                ),
            ],
            source_document_type: Some("bill".to_string()),
            source_document_id: Some(bill_id),
            auto_post: Some(true),
        })
        .await
    }

    pub async fn auto_post_bill_payment_async(&self, payment_id: Uuid) -> Result<JournalEntry> {
        let config = self.get_auto_posting_config_async().await?.ok_or_else(|| {
            CommerceError::ValidationError("Auto-posting not configured".to_string())
        })?;

        let (amount, payment_date): (Decimal, NaiveDate) =
            sqlx::query_as("SELECT amount, payment_date FROM ap_payments WHERE id = $1")
                .bind(payment_id)
                .fetch_one(&self.pool)
                .await
                .map_err(map_db_error)?;

        self.create_journal_entry_async(CreateJournalEntry {
            entry_date: payment_date,
            entry_type: Some(JournalEntryType::Standard),
            description: format!("Bill Payment {}", payment_id),
            lines: vec![
                CreateJournalEntryLine::debit(
                    config.accounts_payable_account_id,
                    amount,
                    Some("Accounts Payable".to_string()),
                ),
                CreateJournalEntryLine::credit(
                    config.cash_account_id,
                    amount,
                    Some("Cash".to_string()),
                ),
            ],
            source_document_type: Some("bill_payment".to_string()),
            source_document_id: Some(payment_id),
            auto_post: Some(true),
        })
        .await
    }

    pub async fn auto_post_inventory_cost_async(
        &self,
        cost_transaction_id: Uuid,
    ) -> Result<JournalEntry> {
        let config = self.get_auto_posting_config_async().await?.ok_or_else(|| {
            CommerceError::ValidationError("Auto-posting not configured".to_string())
        })?;

        let (cost, created_at, transaction_type): (Decimal, DateTime<Utc>, String) =
            sqlx::query_as(
                "SELECT total_cost, created_at, transaction_type FROM cost_transactions WHERE id = $1",
            )
            .bind(cost_transaction_id)
            .fetch_one(&self.pool)
            .await
            .map_err(map_db_error)?;

        let entry_date = created_at.date_naive();
        let is_issue = transaction_type == "issue" || transaction_type == "sale";
        let (debit_account, credit_account) = if is_issue {
            (config.cogs_account_id, config.inventory_account_id)
        } else {
            (config.inventory_account_id, config.cogs_account_id)
        };

        self.create_journal_entry_async(CreateJournalEntry {
            entry_date,
            entry_type: Some(JournalEntryType::Standard),
            description: format!("Inventory Cost {}", cost_transaction_id),
            lines: vec![
                CreateJournalEntryLine::debit(
                    debit_account,
                    cost,
                    Some(if is_issue { "COGS" } else { "Inventory" }.to_string()),
                ),
                CreateJournalEntryLine::credit(
                    credit_account,
                    cost,
                    Some(if is_issue { "Inventory" } else { "COGS" }.to_string()),
                ),
            ],
            source_document_type: Some("cost_transaction".to_string()),
            source_document_id: Some(cost_transaction_id),
            auto_post: Some(true),
        })
        .await
    }

    pub async fn auto_post_write_off_async(&self, write_off_id: Uuid) -> Result<JournalEntry> {
        let config = self.get_auto_posting_config_async().await?.ok_or_else(|| {
            CommerceError::ValidationError("Auto-posting not configured".to_string())
        })?;

        let bad_debt_account = config.bad_debt_expense_account_id.ok_or_else(|| {
            CommerceError::ValidationError("Bad debt expense account not configured".to_string())
        })?;

        let (amount, write_off_date): (Decimal, NaiveDate) =
            sqlx::query_as("SELECT amount, write_off_date FROM ar_write_offs WHERE id = $1")
                .bind(write_off_id)
                .fetch_one(&self.pool)
                .await
                .map_err(map_db_error)?;

        self.create_journal_entry_async(CreateJournalEntry {
            entry_date: write_off_date,
            entry_type: Some(JournalEntryType::Standard),
            description: format!("Write-off {}", write_off_id),
            lines: vec![
                CreateJournalEntryLine::debit(
                    bad_debt_account,
                    amount,
                    Some("Bad Debt Expense".to_string()),
                ),
                CreateJournalEntryLine::credit(
                    config.accounts_receivable_account_id,
                    amount,
                    Some("Accounts Receivable".to_string()),
                ),
            ],
            source_document_type: Some("write_off".to_string()),
            source_document_id: Some(write_off_id),
            auto_post: Some(true),
        })
        .await
    }

    pub async fn get_trial_balance_async(&self, as_of_date: NaiveDate) -> Result<TrialBalance> {
        let rows = sqlx::query_as::<_, (Uuid, String, String, String, String, Decimal)>(
            "SELECT id, account_number, name, account_type, normal_balance, current_balance
             FROM gl_accounts WHERE is_posting = TRUE AND status = 'active' ORDER BY account_number",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        let mut lines = Vec::new();
        let mut total_debits = Decimal::ZERO;
        let mut total_credits = Decimal::ZERO;

        for (id, number, name, account_type, normal_balance, balance) in rows {
            let normal: BalanceSide = normal_balance.parse().map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid gl_account.normal_balance '{}': {}",
                    normal_balance, e
                ))
            })?;
            let (debit_balance, credit_balance) = match normal {
                BalanceSide::Debit => (balance, Decimal::ZERO),
                BalanceSide::Credit => (Decimal::ZERO, balance),
                _ => (balance, Decimal::ZERO),
            };
            let account_type: AccountType = account_type.parse().map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid gl_account.account_type '{}': {}",
                    account_type, e
                ))
            })?;

            lines.push(TrialBalanceLine {
                account_id: id,
                account_number: number,
                account_name: name,
                account_type,
                debit_balance,
                credit_balance,
            });

            total_debits += debit_balance;
            total_credits += credit_balance;
        }

        Ok(TrialBalance {
            as_of_date,
            period_id: None,
            total_debits,
            total_credits,
            is_balanced: total_debits == total_credits,
            lines,
        })
    }

    pub async fn get_balance_sheet_async(&self, as_of_date: NaiveDate) -> Result<BalanceSheet> {
        let rows = sqlx::query_as::<_, (Uuid, String, String, String, Option<String>, Decimal, String)>(
            "SELECT id, account_number, name, account_type, account_sub_type, current_balance, normal_balance
             FROM gl_accounts
             WHERE is_posting = TRUE AND status = 'active'
               AND account_type IN ('asset', 'liability', 'equity')
             ORDER BY account_number",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        let mut assets = Vec::new();
        let mut liabilities = Vec::new();
        let mut equity = Vec::new();
        let mut total_assets = Decimal::ZERO;
        let mut total_liabilities = Decimal::ZERO;
        let mut total_equity = Decimal::ZERO;

        for (id, number, name, account_type, sub_type, balance, _normal_balance) in rows {
            let account_type: AccountType = account_type.parse().map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid gl_account.account_type '{}': {}",
                    account_type, e
                ))
            })?;
            let sub_type = match sub_type {
                Some(value) if !value.trim().is_empty() => Some(value.parse().map_err(|e| {
                    CommerceError::DatabaseError(format!(
                        "Invalid gl_account.account_sub_type '{}': {}",
                        value, e
                    ))
                })?),
                _ => None,
            };
            let line = BalanceSheetLine {
                account_id: id,
                account_number: number,
                account_name: name,
                account_sub_type: sub_type,
                balance,
                indent_level: 0,
                is_total: false,
            };

            match account_type {
                AccountType::Asset => {
                    total_assets += balance;
                    assets.push(line);
                }
                AccountType::Liability => {
                    total_liabilities += balance;
                    liabilities.push(line);
                }
                AccountType::Equity => {
                    total_equity += balance;
                    equity.push(line);
                }
                _ => {}
            }
        }

        Ok(BalanceSheet {
            as_of_date,
            total_assets,
            total_liabilities,
            total_equity,
            assets,
            liabilities,
            equity,
        })
    }

    pub async fn get_income_statement_async(
        &self,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<IncomeStatement> {
        let rows =
            sqlx::query_as::<_, (Uuid, String, String, String, Option<String>, Decimal, Decimal)>(
                "SELECT a.id, a.account_number, a.name, a.account_type, a.account_sub_type,
                    COALESCE(SUM(l.debit_amount), 0) AS total_debits,
                    COALESCE(SUM(l.credit_amount), 0) AS total_credits
             FROM gl_accounts a
             LEFT JOIN gl_journal_entry_lines l ON a.id = l.account_id
             LEFT JOIN gl_journal_entries je ON l.journal_entry_id = je.id
             WHERE a.is_posting = TRUE AND a.status = 'active'
               AND a.account_type IN ('revenue', 'expense')
               AND (je.status = 'posted' OR je.id IS NULL)
               AND (je.entry_date >= $1 AND je.entry_date <= $2 OR je.id IS NULL)
             GROUP BY a.id
             ORDER BY a.account_number",
            )
            .bind(start_date)
            .bind(end_date)
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        let mut revenue_lines = Vec::new();
        let mut expense_lines = Vec::new();
        let mut total_revenue = Decimal::ZERO;
        let mut total_expenses = Decimal::ZERO;

        for (id, number, name, account_type, sub_type, total_debits, total_credits) in rows {
            let account_type: AccountType = account_type.parse().map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid gl_account.account_type '{}': {}",
                    account_type, e
                ))
            })?;
            let amount = match account_type {
                AccountType::Revenue => total_credits - total_debits,
                AccountType::Expense => total_debits - total_credits,
                _ => Decimal::ZERO,
            };

            if amount == Decimal::ZERO {
                continue;
            }

            let line = IncomeStatementLine {
                account_id: id,
                account_number: number,
                account_name: name,
                account_sub_type: match sub_type {
                    Some(value) if !value.trim().is_empty() => {
                        Some(value.parse().map_err(|e| {
                            CommerceError::DatabaseError(format!(
                                "Invalid gl_account.account_sub_type '{}': {}",
                                value, e
                            ))
                        })?)
                    }
                    _ => None,
                },
                amount,
                indent_level: 0,
                is_total: false,
            };

            match account_type {
                AccountType::Revenue => {
                    total_revenue += amount;
                    revenue_lines.push(line);
                }
                AccountType::Expense => {
                    total_expenses += amount;
                    expense_lines.push(line);
                }
                _ => {}
            }
        }

        Ok(IncomeStatement {
            period_start: start_date,
            period_end: end_date,
            total_revenue,
            total_expenses,
            net_income: total_revenue - total_expenses,
            revenue_lines,
            expense_lines,
        })
    }

    pub async fn get_account_balance_async(
        &self,
        account_id: Uuid,
        _as_of_date: Option<NaiveDate>,
    ) -> Result<Option<Decimal>> {
        Ok(self.get_account_async(account_id).await?.map(|account| account.current_balance))
    }

    pub async fn get_account_transactions_async(
        &self,
        account_id: Uuid,
        filter: JournalEntryFilter,
    ) -> Result<Vec<JournalEntryLine>> {
        let mut builder = QueryBuilder::<Postgres>::new(
            "SELECT l.id, l.journal_entry_id, l.line_number, l.account_id, l.account_number,
                    l.account_name, l.description, l.debit_amount, l.credit_amount, l.currency,
                    l.reference_type, l.reference_id, l.created_at
             FROM gl_journal_entry_lines l
             JOIN gl_journal_entries je ON l.journal_entry_id = je.id
             WHERE l.account_id = ",
        );
        builder.push_bind(account_id);

        if let Some(status) = filter.status {
            builder.push(" AND je.status = ").push_bind(status.to_string());
        }
        if let Some(from_date) = filter.from_date {
            builder.push(" AND je.entry_date >= ").push_bind(from_date);
        }
        if let Some(to_date) = filter.to_date {
            builder.push(" AND je.entry_date <= ").push_bind(to_date);
        }

        builder.push(" ORDER BY je.entry_date DESC, l.line_number");

        builder.push(" LIMIT ").push_bind(super::effective_limit(filter.limit));

        let rows = builder
            .build_query_as::<JournalEntryLineRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(rows.into_iter().map(Self::row_to_journal_entry_line).collect())
    }

    pub async fn run_period_close_async(
        &self,
        period_id: Uuid,
        closed_by: &str,
    ) -> Result<JournalEntry> {
        let period = self.get_period_async(period_id).await?.ok_or(CommerceError::NotFound)?;

        if period.status != PeriodStatus::Open {
            return Err(CommerceError::ValidationError("Period must be open to close".to_string()));
        }

        let income_statement =
            self.get_income_statement_async(period.start_date, period.end_date).await?;

        if income_statement.net_income == Decimal::ZERO
            && income_statement.revenue_lines.iter().all(|l| l.amount == Decimal::ZERO)
            && income_statement.expense_lines.iter().all(|l| l.amount == Decimal::ZERO)
        {
            return Err(CommerceError::ValidationError("No net income to close".to_string()));
        }

        let retained_earnings = self
            .list_accounts_async(GlAccountFilter {
                account_sub_type: Some(AccountSubType::RetainedEarnings),
                ..Default::default()
            })
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| {
                CommerceError::ValidationError("Retained earnings account not found".to_string())
            })?;

        let mut lines = Vec::new();

        for rev in income_statement.revenue_lines {
            // Revenue is credit-normal: positive balances close with a debit,
            // contra-normal (negative) balances with a credit.
            let memo = Some(format!("Close {} to Retained Earnings", rev.account_name));
            if rev.amount > Decimal::ZERO {
                lines.push(CreateJournalEntryLine::debit(rev.account_id, rev.amount, memo));
            } else if rev.amount < Decimal::ZERO {
                lines.push(CreateJournalEntryLine::credit(rev.account_id, rev.amount.abs(), memo));
            }
        }

        for exp in income_statement.expense_lines {
            // Expenses are debit-normal: positive balances close with a
            // credit, contra-normal balances (e.g. a net FX gain on an
            // expense-type gain/loss account) with a debit.
            let memo = Some(format!("Close {} to Retained Earnings", exp.account_name));
            if exp.amount > Decimal::ZERO {
                lines.push(CreateJournalEntryLine::credit(exp.account_id, exp.amount, memo));
            } else if exp.amount < Decimal::ZERO {
                lines.push(CreateJournalEntryLine::debit(exp.account_id, exp.amount.abs(), memo));
            }
        }

        if income_statement.net_income > Decimal::ZERO {
            lines.push(CreateJournalEntryLine::credit(
                retained_earnings.id,
                income_statement.net_income,
                Some("Net income to Retained Earnings".to_string()),
            ));
        } else if income_statement.net_income < Decimal::ZERO {
            lines.push(CreateJournalEntryLine::debit(
                retained_earnings.id,
                income_statement.net_income.abs(),
                Some("Net loss to Retained Earnings".to_string()),
            ));
        }

        let closing_entry = self
            .create_journal_entry_async(CreateJournalEntry {
                entry_date: period.end_date,
                entry_type: Some(JournalEntryType::Closing),
                description: format!("Closing entries for {}", period.period_name),
                lines,
                source_document_type: Some("period_close".to_string()),
                source_document_id: Some(period_id),
                auto_post: Some(true),
            })
            .await?;

        self.close_period_async(period_id, closed_by).await?;

        Ok(closing_entry)
    }

    /// Look up the exchange rate converting one `from` unit into `to` units,
    /// falling back to the inverse of the reverse pair when only that is set.
    async fn lookup_rate_async(&self, from: &str, to: &str) -> Result<Option<Decimal>> {
        let direct: Option<Decimal> = sqlx::query_scalar(
            "SELECT rate FROM exchange_rates WHERE base_currency = $1 AND quote_currency = $2",
        )
        .bind(from)
        .bind(to)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;
        if let Some(rate) = direct {
            return Ok(Some(rate));
        }

        let inverse: Option<Decimal> = sqlx::query_scalar(
            "SELECT rate FROM exchange_rates WHERE base_currency = $1 AND quote_currency = $2",
        )
        .bind(to)
        .bind(from)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;
        Ok(inverse.and_then(|rate| if rate.is_zero() { None } else { Some(Decimal::ONE / rate) }))
    }

    /// Resolve the account receiving unrealized FX gains/losses: the
    /// configured `fx_gain_loss_account_id`, else the first active posting
    /// account with an Other Expense / Other Revenue sub-type.
    async fn resolve_fx_gain_loss_account_async(&self) -> Result<Uuid> {
        if let Some(id) = self
            .get_auto_posting_config_async()
            .await?
            .and_then(|config| config.fx_gain_loss_account_id)
        {
            return Ok(id);
        }
        for sub_type in [AccountSubType::OtherExpense, AccountSubType::OtherRevenue] {
            let fallback = self
                .list_accounts_async(GlAccountFilter {
                    account_sub_type: Some(sub_type),
                    status: Some(AccountStatus::Active),
                    is_posting: Some(true),
                    limit: Some(1),
                    ..Default::default()
                })
                .await?
                .into_iter()
                .next();
            if let Some(account) = fallback {
                return Ok(account.id);
            }
        }
        Err(CommerceError::ValidationError(
            "No FX gain/loss account configured for revaluation".to_string(),
        ))
    }

    pub async fn revalue_async(
        &self,
        as_of_date: NaiveDate,
        base_currency: Option<Currency>,
    ) -> Result<RevaluationResult> {
        let base = match base_currency {
            Some(base) => base,
            None => {
                let code: Option<String> =
                    sqlx::query_scalar("SELECT base_currency FROM store_currency_settings LIMIT 1")
                        .fetch_optional(&self.pool)
                        .await
                        .map_err(map_db_error)?;
                match code {
                    Some(code) => code.parse::<Currency>().map_err(|e| {
                        CommerceError::DatabaseError(format!(
                            "Invalid store base currency {code:?}: {e}"
                        ))
                    })?,
                    None => Currency::default(),
                }
            }
        };
        let base_code: CurrencyCode = base.code().parse().map_err(|e| {
            CommerceError::ValidationError(format!(
                "Base currency {} is not a valid ISO code: {e}",
                base.code()
            ))
        })?;
        let base_places = u32::from(base.decimal_places());

        let accounts = self
            .list_accounts_async(GlAccountFilter {
                status: Some(AccountStatus::Active),
                is_posting: Some(true),
                ..Default::default()
            })
            .await?;

        let mut lines: Vec<stateset_core::RevaluationLine> = Vec::new();
        for account in accounts {
            if account.currency == base_code {
                continue;
            }

            // Outstanding foreign-currency balance: posted lines excluding
            // prior base-currency FX revaluation adjustments.
            let row: (Decimal, Decimal) = sqlx::query_as(
                "SELECT COALESCE(SUM(l.debit_amount), 0), COALESCE(SUM(l.credit_amount), 0)
                 FROM gl_journal_entry_lines l
                 JOIN gl_journal_entries je ON l.journal_entry_id = je.id
                 WHERE l.account_id = $1 AND je.status = 'posted'
                   AND (l.reference_type IS NULL OR l.reference_type != $2)",
            )
            .bind(account.id)
            .bind(FX_REVALUATION_REFERENCE)
            .fetch_one(&self.pool)
            .await
            .map_err(map_db_error)?;
            let foreign_balance = account.balance_effect(row.0, row.1);

            if foreign_balance.is_zero() && account.current_balance.is_zero() {
                continue;
            }

            let rate = self
                .lookup_rate_async(account.currency.as_str(), base.code())
                .await?
                .ok_or_else(|| {
                    CommerceError::ValidationError(format!(
                        "No exchange rate available for {} -> {}",
                        account.currency,
                        base.code()
                    ))
                })?;

            lines.push(stateset_core::compute_revaluation_line(
                &account,
                foreign_balance,
                rate,
                base_places,
            ));
        }

        let total_unrealized_gain_loss: Decimal =
            lines.iter().map(|l| l.unrealized_gain_loss).sum();

        let journal_entry = if lines.iter().any(|l| !l.adjustment.is_zero()) {
            let fx_account_id = self.resolve_fx_gain_loss_account_async().await?;
            let entry_lines = stateset_core::build_revaluation_journal_lines(&lines, fx_account_id);
            Some(
                self.create_journal_entry_async(CreateJournalEntry {
                    entry_date: as_of_date,
                    entry_type: Some(JournalEntryType::Adjusting),
                    description: format!("FX revaluation as of {as_of_date}"),
                    lines: entry_lines,
                    source_document_type: Some(FX_REVALUATION_REFERENCE.to_string()),
                    source_document_id: None,
                    auto_post: Some(true),
                })
                .await?,
            )
        } else {
            None
        };

        Ok(RevaluationResult {
            as_of_date,
            base_currency: base_code,
            total_unrealized_gain_loss,
            lines,
            journal_entry,
        })
    }

    pub async fn create_accounts_batch_async(
        &self,
        inputs: Vec<CreateGlAccount>,
    ) -> Result<BatchResult<GlAccount>> {
        let mut result = BatchResult::new();

        for (index, input) in inputs.into_iter().enumerate() {
            match self.create_account_async(input).await {
                Ok(account) => result.record_success(account),
                Err(e) => result.record_failure(index, None, &e),
            }
        }

        Ok(result)
    }

    pub async fn get_accounts_batch_async(&self, ids: Vec<Uuid>) -> Result<Vec<GlAccount>> {
        let mut accounts = Vec::new();
        for id in ids {
            if let Some(account) = self.get_account_async(id).await? {
                accounts.push(account);
            }
        }
        Ok(accounts)
    }
}

impl GeneralLedgerRepository for PgGeneralLedgerRepository {
    fn create_account(&self, input: CreateGlAccount) -> Result<GlAccount> {
        block_on(self.create_account_async(input))
    }

    fn get_account(&self, id: Uuid) -> Result<Option<GlAccount>> {
        block_on(self.get_account_async(id))
    }

    fn get_account_by_number(&self, account_number: &str) -> Result<Option<GlAccount>> {
        block_on(self.get_account_by_number_async(account_number))
    }

    fn update_account(&self, id: Uuid, input: stateset_core::UpdateGlAccount) -> Result<GlAccount> {
        block_on(self.update_account_async(id, input))
    }

    fn list_accounts(&self, filter: GlAccountFilter) -> Result<Vec<GlAccount>> {
        block_on(self.list_accounts_async(filter))
    }

    fn get_account_hierarchy(&self) -> Result<Vec<GlAccount>> {
        block_on(self.get_account_hierarchy_async())
    }

    fn delete_account(&self, id: Uuid) -> Result<()> {
        block_on(self.delete_account_async(id))
    }

    fn initialize_chart_of_accounts(&self) -> Result<Vec<GlAccount>> {
        block_on(self.initialize_chart_of_accounts_async())
    }

    fn create_period(&self, input: CreateGlPeriod) -> Result<GlPeriod> {
        block_on(self.create_period_async(input))
    }

    fn get_period(&self, id: Uuid) -> Result<Option<GlPeriod>> {
        block_on(self.get_period_async(id))
    }

    fn get_current_period(&self) -> Result<Option<GlPeriod>> {
        block_on(self.get_current_period_async())
    }

    fn get_period_for_date(&self, date: NaiveDate) -> Result<Option<GlPeriod>> {
        block_on(self.get_period_for_date_async(date))
    }

    fn list_periods(&self, filter: GlPeriodFilter) -> Result<Vec<GlPeriod>> {
        block_on(self.list_periods_async(filter))
    }

    fn open_period(&self, id: Uuid) -> Result<GlPeriod> {
        block_on(self.open_period_async(id))
    }

    fn close_period(&self, id: Uuid, closed_by: &str) -> Result<GlPeriod> {
        block_on(self.close_period_async(id, closed_by))
    }

    fn lock_period(&self, id: Uuid, locked_by: &str) -> Result<GlPeriod> {
        block_on(self.lock_period_async(id, locked_by))
    }

    fn reopen_period(&self, id: Uuid) -> Result<GlPeriod> {
        block_on(self.reopen_period_async(id))
    }

    fn create_journal_entry(&self, input: CreateJournalEntry) -> Result<JournalEntry> {
        block_on(self.create_journal_entry_async(input))
    }

    fn get_journal_entry(&self, id: Uuid) -> Result<Option<JournalEntry>> {
        block_on(self.get_journal_entry_async(id))
    }

    fn get_journal_entry_by_number(&self, number: &str) -> Result<Option<JournalEntry>> {
        block_on(self.get_journal_entry_by_number_async(number))
    }

    fn list_journal_entries(&self, filter: JournalEntryFilter) -> Result<Vec<JournalEntry>> {
        block_on(self.list_journal_entries_async(filter))
    }

    fn post_journal_entry(&self, id: Uuid, posted_by: &str) -> Result<JournalEntry> {
        block_on(self.post_journal_entry_async(id, posted_by))
    }

    fn void_journal_entry(&self, id: Uuid) -> Result<JournalEntry> {
        block_on(self.void_journal_entry_async(id))
    }

    fn reverse_journal_entry(&self, id: Uuid, reversal_date: NaiveDate) -> Result<JournalEntry> {
        block_on(self.reverse_journal_entry_async(id, reversal_date))
    }

    fn get_journal_entry_lines(&self, journal_entry_id: Uuid) -> Result<Vec<JournalEntryLine>> {
        block_on(self.get_journal_entry_lines_async(journal_entry_id))
    }

    fn get_auto_posting_config(&self) -> Result<Option<AutoPostingConfig>> {
        block_on(self.get_auto_posting_config_async())
    }

    fn set_auto_posting_config(&self, input: CreateAutoPostingConfig) -> Result<AutoPostingConfig> {
        block_on(self.set_auto_posting_config_async(input))
    }

    fn auto_post_invoice(&self, invoice_id: InvoiceId) -> Result<JournalEntry> {
        block_on(self.auto_post_invoice_async(invoice_id.into_uuid()))
    }

    fn auto_post_payment_received(&self, payment_id: Uuid) -> Result<JournalEntry> {
        block_on(self.auto_post_payment_received_async(payment_id))
    }

    fn auto_post_bill(&self, bill_id: Uuid) -> Result<JournalEntry> {
        block_on(self.auto_post_bill_async(bill_id))
    }

    fn auto_post_bill_payment(&self, payment_id: Uuid) -> Result<JournalEntry> {
        block_on(self.auto_post_bill_payment_async(payment_id))
    }

    fn auto_post_inventory_cost(&self, cost_transaction_id: Uuid) -> Result<JournalEntry> {
        block_on(self.auto_post_inventory_cost_async(cost_transaction_id))
    }

    fn auto_post_write_off(&self, write_off_id: Uuid) -> Result<JournalEntry> {
        block_on(self.auto_post_write_off_async(write_off_id))
    }

    fn get_trial_balance(&self, as_of_date: NaiveDate) -> Result<TrialBalance> {
        block_on(self.get_trial_balance_async(as_of_date))
    }

    fn get_balance_sheet(&self, as_of_date: NaiveDate) -> Result<BalanceSheet> {
        block_on(self.get_balance_sheet_async(as_of_date))
    }

    fn get_income_statement(
        &self,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<IncomeStatement> {
        block_on(self.get_income_statement_async(start_date, end_date))
    }

    fn get_account_balance(
        &self,
        account_id: Uuid,
        as_of_date: Option<NaiveDate>,
    ) -> Result<Option<Decimal>> {
        block_on(self.get_account_balance_async(account_id, as_of_date))
    }

    fn get_account_transactions(
        &self,
        account_id: Uuid,
        filter: JournalEntryFilter,
    ) -> Result<Vec<JournalEntryLine>> {
        block_on(self.get_account_transactions_async(account_id, filter))
    }

    fn run_period_close(&self, period_id: Uuid, closed_by: &str) -> Result<JournalEntry> {
        block_on(self.run_period_close_async(period_id, closed_by))
    }

    fn revalue(
        &self,
        as_of_date: NaiveDate,
        base_currency: Option<Currency>,
    ) -> Result<RevaluationResult> {
        block_on(self.revalue_async(as_of_date, base_currency))
    }

    fn create_accounts_batch(
        &self,
        inputs: Vec<CreateGlAccount>,
    ) -> Result<BatchResult<GlAccount>> {
        block_on(self.create_accounts_batch_async(inputs))
    }

    fn get_accounts_batch(&self, ids: Vec<Uuid>) -> Result<Vec<GlAccount>> {
        block_on(self.get_accounts_batch_async(ids))
    }
}
