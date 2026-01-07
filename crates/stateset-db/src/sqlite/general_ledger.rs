//! SQLite implementation of General Ledger repository

use crate::sqlite::{map_db_error, parse_decimal};
use chrono::{NaiveDate, Utc};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rust_decimal::Decimal;
use rusqlite::params;
use stateset_core::{
    AccountStatus, AccountSubType, AccountType, AutoPostingConfig,
    BalanceSide, BalanceSheet, BalanceSheetLine, BatchResult, CreateAutoPostingConfig,
    CreateGlAccount, CreateGlPeriod, CreateJournalEntry, GeneralLedgerRepository,
    GlAccount, GlAccountFilter, GlPeriod, GlPeriodFilter, IncomeStatement,
    IncomeStatementLine, JournalEntry, JournalEntryFilter, JournalEntryLine,
    JournalEntrySource, JournalEntryStatus, JournalEntryType, PeriodStatus, Result,
    TrialBalance, TrialBalanceLine, UpdateGlAccount, create_default_chart_of_accounts,
    generate_journal_entry_number,
};
use uuid::Uuid;

pub struct SqliteGeneralLedgerRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteGeneralLedgerRepository {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn map_account_row(row: &rusqlite::Row) -> rusqlite::Result<GlAccount> {
        Ok(GlAccount {
            id: row.get::<_, String>(0)?.parse().unwrap_or_default(),
            account_number: row.get(1)?,
            name: row.get(2)?,
            description: row.get(3)?,
            account_type: row.get::<_, String>(4)?.parse().unwrap_or(AccountType::Asset),
            account_sub_type: row.get::<_, Option<String>>(5)?.and_then(|s| s.parse().ok()),
            parent_account_id: row.get::<_, Option<String>>(6)?.and_then(|s| s.parse().ok()),
            is_header: row.get::<_, i32>(7)? != 0,
            is_posting: row.get::<_, i32>(8)? != 0,
            normal_balance: row.get::<_, String>(9)?.parse().unwrap_or(BalanceSide::Debit),
            currency: row.get(10)?,
            status: row.get::<_, String>(11)?.parse().unwrap_or(AccountStatus::Active),
            current_balance: parse_decimal(&row.get::<_, String>(12)?),
            created_at: row.get::<_, String>(13)?.parse().unwrap_or_else(|_| Utc::now()),
            updated_at: row.get::<_, String>(14)?.parse().unwrap_or_else(|_| Utc::now()),
        })
    }

    fn map_period_row(row: &rusqlite::Row) -> rusqlite::Result<GlPeriod> {
        Ok(GlPeriod {
            id: row.get::<_, String>(0)?.parse().unwrap_or_default(),
            period_name: row.get(1)?,
            fiscal_year: row.get(2)?,
            period_number: row.get(3)?,
            start_date: row.get::<_, String>(4)?.parse().unwrap_or_else(|_| NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()),
            end_date: row.get::<_, String>(5)?.parse().unwrap_or_else(|_| NaiveDate::from_ymd_opt(2024, 1, 31).unwrap()),
            status: row.get::<_, String>(6)?.parse().unwrap_or(PeriodStatus::Future),
            closed_at: row.get::<_, Option<String>>(7)?.and_then(|s| s.parse().ok()),
            closed_by: row.get(8)?,
            locked_at: row.get::<_, Option<String>>(9)?.and_then(|s| s.parse().ok()),
            locked_by: row.get(10)?,
            created_at: row.get::<_, String>(11)?.parse().unwrap_or_else(|_| Utc::now()),
            updated_at: row.get::<_, String>(12)?.parse().unwrap_or_else(|_| Utc::now()),
        })
    }

    fn map_journal_entry_row(row: &rusqlite::Row) -> rusqlite::Result<JournalEntry> {
        Ok(JournalEntry {
            id: row.get::<_, String>(0)?.parse().unwrap_or_default(),
            entry_number: row.get(1)?,
            entry_date: row.get::<_, String>(2)?.parse().unwrap_or_else(|_| NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()),
            period_id: row.get::<_, String>(3)?.parse().unwrap_or_default(),
            entry_type: row.get::<_, String>(4)?.parse().unwrap_or(JournalEntryType::Standard),
            source: row.get::<_, String>(5)?.parse().unwrap_or(JournalEntrySource::Manual),
            source_document_type: row.get(6)?,
            source_document_id: row.get::<_, Option<String>>(7)?.and_then(|s| s.parse().ok()),
            description: row.get(8)?,
            total_debits: parse_decimal(&row.get::<_, String>(9)?),
            total_credits: parse_decimal(&row.get::<_, String>(10)?),
            is_balanced: row.get::<_, i32>(11)? != 0,
            status: row.get::<_, String>(12)?.parse().unwrap_or(JournalEntryStatus::Draft),
            posted_at: row.get::<_, Option<String>>(13)?.and_then(|s| s.parse().ok()),
            posted_by: row.get(14)?,
            reversed_entry_id: row.get::<_, Option<String>>(15)?.and_then(|s| s.parse().ok()),
            reversing_entry_id: row.get::<_, Option<String>>(16)?.and_then(|s| s.parse().ok()),
            lines: Vec::new(),
            created_at: row.get::<_, String>(17)?.parse().unwrap_or_else(|_| Utc::now()),
            updated_at: row.get::<_, String>(18)?.parse().unwrap_or_else(|_| Utc::now()),
        })
    }

    fn map_journal_entry_line_row(row: &rusqlite::Row) -> rusqlite::Result<JournalEntryLine> {
        Ok(JournalEntryLine {
            id: row.get::<_, String>(0)?.parse().unwrap_or_default(),
            journal_entry_id: row.get::<_, String>(1)?.parse().unwrap_or_default(),
            line_number: row.get(2)?,
            account_id: row.get::<_, String>(3)?.parse().unwrap_or_default(),
            account_number: row.get(4)?,
            account_name: row.get(5)?,
            description: row.get(6)?,
            debit_amount: parse_decimal(&row.get::<_, String>(7)?),
            credit_amount: parse_decimal(&row.get::<_, String>(8)?),
            currency: row.get(9)?,
            reference_type: row.get(10)?,
            reference_id: row.get::<_, Option<String>>(11)?.and_then(|s| s.parse().ok()),
            created_at: row.get::<_, String>(12)?.parse().unwrap_or_else(|_| Utc::now()),
        })
    }

    fn map_auto_posting_config_row(row: &rusqlite::Row) -> rusqlite::Result<AutoPostingConfig> {
        Ok(AutoPostingConfig {
            id: row.get::<_, String>(0)?.parse().unwrap_or_default(),
            config_name: row.get(1)?,
            cash_account_id: row.get::<_, String>(2)?.parse().unwrap_or_default(),
            accounts_receivable_account_id: row.get::<_, String>(3)?.parse().unwrap_or_default(),
            inventory_account_id: row.get::<_, String>(4)?.parse().unwrap_or_default(),
            accounts_payable_account_id: row.get::<_, String>(5)?.parse().unwrap_or_default(),
            unearned_revenue_account_id: row.get::<_, Option<String>>(6)?.and_then(|s| s.parse().ok()),
            sales_revenue_account_id: row.get::<_, String>(7)?.parse().unwrap_or_default(),
            shipping_revenue_account_id: row.get::<_, Option<String>>(8)?.and_then(|s| s.parse().ok()),
            cogs_account_id: row.get::<_, String>(9)?.parse().unwrap_or_default(),
            bad_debt_expense_account_id: row.get::<_, Option<String>>(10)?.and_then(|s| s.parse().ok()),
            is_active: row.get::<_, i32>(11)? != 0,
            created_at: row.get::<_, String>(12)?.parse().unwrap_or_else(|_| Utc::now()),
            updated_at: row.get::<_, String>(13)?.parse().unwrap_or_else(|_| Utc::now()),
        })
    }

    fn update_account_balance(&self, account_id: Uuid, debit: Decimal, credit: Decimal) -> Result<()> {
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        // Get account to determine normal balance
        let account: GlAccount = conn.query_row(
            "SELECT id, account_number, name, description, account_type, account_sub_type,
                    parent_account_id, is_header, is_posting, normal_balance, currency,
                    status, current_balance, created_at, updated_at
             FROM gl_accounts WHERE id = ?1",
            params![account_id.to_string()],
            Self::map_account_row,
        ).map_err(map_db_error)?;

        let balance_change = account.balance_effect(debit, credit);
        let new_balance = account.current_balance + balance_change;

        conn.execute(
            "UPDATE gl_accounts SET current_balance = ?1 WHERE id = ?2",
            params![new_balance.to_string(), account_id.to_string()],
        ).map_err(map_db_error)?;

        Ok(())
    }
}

impl GeneralLedgerRepository for SqliteGeneralLedgerRepository {
    // ========================================================================
    // Chart of Accounts
    // ========================================================================

    fn create_account(&self, input: CreateGlAccount) -> Result<GlAccount> {
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;
        let id = Uuid::new_v4();
        let now = Utc::now();
        let normal_balance = input.account_type.normal_balance();

        conn.execute(
            "INSERT INTO gl_accounts (id, account_number, name, description, account_type,
             account_sub_type, parent_account_id, is_header, is_posting, normal_balance,
             currency, status, current_balance, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                id.to_string(),
                input.account_number,
                input.name,
                input.description,
                input.account_type.to_string(),
                input.account_sub_type.map(|s| s.to_string()),
                input.parent_account_id.map(|id| id.to_string()),
                input.is_header.unwrap_or(false) as i32,
                input.is_posting.unwrap_or(true) as i32,
                normal_balance.to_string(),
                input.currency.unwrap_or_else(|| "USD".to_string()),
                AccountStatus::Active.to_string(),
                "0",
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        ).map_err(map_db_error)?;

        self.get_account(id)?.ok_or(stateset_core::CommerceError::NotFound)
    }

    fn get_account(&self, id: Uuid) -> Result<Option<GlAccount>> {
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        match conn.query_row(
            "SELECT id, account_number, name, description, account_type, account_sub_type,
                    parent_account_id, is_header, is_posting, normal_balance, currency,
                    status, current_balance, created_at, updated_at
             FROM gl_accounts WHERE id = ?1",
            params![id.to_string()],
            Self::map_account_row,
        ) {
            Ok(account) => Ok(Some(account)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn get_account_by_number(&self, account_number: &str) -> Result<Option<GlAccount>> {
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        match conn.query_row(
            "SELECT id, account_number, name, description, account_type, account_sub_type,
                    parent_account_id, is_header, is_posting, normal_balance, currency,
                    status, current_balance, created_at, updated_at
             FROM gl_accounts WHERE account_number = ?1",
            params![account_number],
            Self::map_account_row,
        ) {
            Ok(account) => Ok(Some(account)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn update_account(&self, id: Uuid, input: UpdateGlAccount) -> Result<GlAccount> {
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let mut updates = Vec::new();
        let mut values: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(name) = input.name {
            updates.push("name = ?");
            values.push(Box::new(name));
        }
        if let Some(description) = input.description {
            updates.push("description = ?");
            values.push(Box::new(description));
        }
        if let Some(parent_id) = input.parent_account_id {
            updates.push("parent_account_id = ?");
            values.push(Box::new(parent_id.to_string()));
        }
        if let Some(status) = input.status {
            updates.push("status = ?");
            values.push(Box::new(status.to_string()));
        }

        if !updates.is_empty() {
            values.push(Box::new(id.to_string()));
            let sql = format!(
                "UPDATE gl_accounts SET {} WHERE id = ?",
                updates.join(", ")
            );
            let params: Vec<&dyn rusqlite::ToSql> = values.iter().map(|v| v.as_ref()).collect();
            conn.execute(&sql, params.as_slice()).map_err(map_db_error)?;
        }

        self.get_account(id)?.ok_or(stateset_core::CommerceError::NotFound)
    }

    fn list_accounts(&self, filter: GlAccountFilter) -> Result<Vec<GlAccount>> {
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let mut sql = String::from(
            "SELECT id, account_number, name, description, account_type, account_sub_type,
                    parent_account_id, is_header, is_posting, normal_balance, currency,
                    status, current_balance, created_at, updated_at
             FROM gl_accounts WHERE 1=1"
        );
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(account_type) = filter.account_type {
            sql.push_str(" AND account_type = ?");
            params.push(Box::new(account_type.to_string()));
        }
        if let Some(sub_type) = filter.account_sub_type {
            sql.push_str(" AND account_sub_type = ?");
            params.push(Box::new(sub_type.to_string()));
        }
        if let Some(parent_id) = filter.parent_account_id {
            sql.push_str(" AND parent_account_id = ?");
            params.push(Box::new(parent_id.to_string()));
        }
        if let Some(status) = filter.status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(status.to_string()));
        }
        if let Some(is_posting) = filter.is_posting {
            sql.push_str(" AND is_posting = ?");
            params.push(Box::new(is_posting as i32));
        }
        if let Some(is_header) = filter.is_header {
            sql.push_str(" AND is_header = ?");
            params.push(Box::new(is_header as i32));
        }
        if let Some(search) = filter.search {
            sql.push_str(" AND (name LIKE ? OR account_number LIKE ?)");
            let search_term = format!("%{}%", search);
            params.push(Box::new(search_term.clone()));
            params.push(Box::new(search_term));
        }

        sql.push_str(" ORDER BY account_number");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }
        if let Some(offset) = filter.offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt.query_map(params_refs.as_slice(), Self::map_account_row).map_err(map_db_error)?;

        let mut accounts = Vec::new();
        for row in rows {
            accounts.push(row.map_err(map_db_error)?);
        }
        Ok(accounts)
    }

    fn get_account_hierarchy(&self) -> Result<Vec<GlAccount>> {
        self.list_accounts(GlAccountFilter::default())
    }

    fn delete_account(&self, id: Uuid) -> Result<()> {
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        // Check if account has transactions
        let count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM gl_journal_entry_lines WHERE account_id = ?1",
            params![id.to_string()],
            |row| row.get(0),
        ).map_err(map_db_error)?;

        if count > 0 {
            return Err(stateset_core::CommerceError::ValidationError(
                "Cannot delete account with existing transactions".to_string()
            ));
        }

        conn.execute(
            "DELETE FROM gl_accounts WHERE id = ?1",
            params![id.to_string()],
        ).map_err(map_db_error)?;

        Ok(())
    }

    fn initialize_chart_of_accounts(&self) -> Result<Vec<GlAccount>> {
        let defaults = create_default_chart_of_accounts();
        let mut accounts = Vec::new();

        for input in defaults {
            // Check if account already exists
            if self.get_account_by_number(&input.account_number)?.is_none() {
                accounts.push(self.create_account(input)?);
            }
        }

        Ok(accounts)
    }

    // ========================================================================
    // GL Periods
    // ========================================================================

    fn create_period(&self, input: CreateGlPeriod) -> Result<GlPeriod> {
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;
        let id = Uuid::new_v4();
        let now = Utc::now();

        conn.execute(
            "INSERT INTO gl_periods (id, period_name, fiscal_year, period_number,
             start_date, end_date, status, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                id.to_string(),
                input.period_name,
                input.fiscal_year,
                input.period_number,
                input.start_date.to_string(),
                input.end_date.to_string(),
                PeriodStatus::Future.to_string(),
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        ).map_err(map_db_error)?;

        self.get_period(id)?.ok_or(stateset_core::CommerceError::NotFound)
    }

    fn get_period(&self, id: Uuid) -> Result<Option<GlPeriod>> {
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        match conn.query_row(
            "SELECT id, period_name, fiscal_year, period_number, start_date, end_date,
                    status, closed_at, closed_by, locked_at, locked_by, created_at, updated_at
             FROM gl_periods WHERE id = ?1",
            params![id.to_string()],
            Self::map_period_row,
        ) {
            Ok(period) => Ok(Some(period)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn get_current_period(&self) -> Result<Option<GlPeriod>> {
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        match conn.query_row(
            "SELECT id, period_name, fiscal_year, period_number, start_date, end_date,
                    status, closed_at, closed_by, locked_at, locked_by, created_at, updated_at
             FROM gl_periods WHERE status = 'open' ORDER BY start_date DESC LIMIT 1",
            [],
            Self::map_period_row,
        ) {
            Ok(period) => Ok(Some(period)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn get_period_for_date(&self, date: NaiveDate) -> Result<Option<GlPeriod>> {
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        match conn.query_row(
            "SELECT id, period_name, fiscal_year, period_number, start_date, end_date,
                    status, closed_at, closed_by, locked_at, locked_by, created_at, updated_at
             FROM gl_periods WHERE start_date <= ?1 AND end_date >= ?1",
            params![date.to_string()],
            Self::map_period_row,
        ) {
            Ok(period) => Ok(Some(period)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn list_periods(&self, filter: GlPeriodFilter) -> Result<Vec<GlPeriod>> {
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let mut sql = String::from(
            "SELECT id, period_name, fiscal_year, period_number, start_date, end_date,
                    status, closed_at, closed_by, locked_at, locked_by, created_at, updated_at
             FROM gl_periods WHERE 1=1"
        );
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(year) = filter.fiscal_year {
            sql.push_str(" AND fiscal_year = ?");
            params.push(Box::new(year));
        }
        if let Some(status) = filter.status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(status.to_string()));
        }

        sql.push_str(" ORDER BY fiscal_year DESC, period_number DESC");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }
        if let Some(offset) = filter.offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt.query_map(params_refs.as_slice(), Self::map_period_row).map_err(map_db_error)?;

        let mut periods = Vec::new();
        for row in rows {
            periods.push(row.map_err(map_db_error)?);
        }
        Ok(periods)
    }

    fn open_period(&self, id: Uuid) -> Result<GlPeriod> {
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        conn.execute(
            "UPDATE gl_periods SET status = 'open' WHERE id = ?1 AND status = 'future'",
            params![id.to_string()],
        ).map_err(map_db_error)?;

        self.get_period(id)?.ok_or(stateset_core::CommerceError::NotFound)
    }

    fn close_period(&self, id: Uuid, closed_by: &str) -> Result<GlPeriod> {
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;
        let now = Utc::now();

        conn.execute(
            "UPDATE gl_periods SET status = 'closed', closed_at = ?1, closed_by = ?2
             WHERE id = ?3 AND status = 'open'",
            params![now.to_rfc3339(), closed_by, id.to_string()],
        ).map_err(map_db_error)?;

        self.get_period(id)?.ok_or(stateset_core::CommerceError::NotFound)
    }

    fn lock_period(&self, id: Uuid, locked_by: &str) -> Result<GlPeriod> {
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;
        let now = Utc::now();

        conn.execute(
            "UPDATE gl_periods SET status = 'locked', locked_at = ?1, locked_by = ?2
             WHERE id = ?3 AND status = 'closed'",
            params![now.to_rfc3339(), locked_by, id.to_string()],
        ).map_err(map_db_error)?;

        self.get_period(id)?.ok_or(stateset_core::CommerceError::NotFound)
    }

    fn reopen_period(&self, id: Uuid) -> Result<GlPeriod> {
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        conn.execute(
            "UPDATE gl_periods SET status = 'open', closed_at = NULL, closed_by = NULL
             WHERE id = ?1 AND status = 'closed'",
            params![id.to_string()],
        ).map_err(map_db_error)?;

        self.get_period(id)?.ok_or(stateset_core::CommerceError::NotFound)
    }

    // ========================================================================
    // Journal Entries
    // ========================================================================

    fn create_journal_entry(&self, input: CreateJournalEntry) -> Result<JournalEntry> {
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;
        let id = Uuid::new_v4();
        let now = Utc::now();
        let entry_number = generate_journal_entry_number();

        // Get period for entry date
        let period = self.get_period_for_date(input.entry_date)?
            .ok_or_else(|| stateset_core::CommerceError::ValidationError(
                format!("No period found for date {}", input.entry_date)
            ))?;

        if !period.can_post() {
            return Err(stateset_core::CommerceError::ValidationError(
                "Period is not open for posting".to_string()
            ));
        }

        // Calculate totals
        let total_debits: Decimal = input.lines.iter().map(|l| l.debit_amount).sum();
        let total_credits: Decimal = input.lines.iter().map(|l| l.credit_amount).sum();
        let is_balanced = total_debits == total_credits;

        conn.execute(
            "INSERT INTO gl_journal_entries (id, entry_number, entry_date, period_id,
             entry_type, source, source_document_type, source_document_id, description,
             total_debits, total_credits, is_balanced, status, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                id.to_string(),
                entry_number,
                input.entry_date.to_string(),
                period.id.to_string(),
                input.entry_type.unwrap_or(JournalEntryType::Standard).to_string(),
                JournalEntrySource::Manual.to_string(),
                input.source_document_type,
                input.source_document_id.map(|id| id.to_string()),
                input.description,
                total_debits.to_string(),
                total_credits.to_string(),
                is_balanced as i32,
                JournalEntryStatus::Draft.to_string(),
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        ).map_err(map_db_error)?;

        // Insert lines
        for (line_num, line) in input.lines.iter().enumerate() {
            let line_id = Uuid::new_v4();

            // Get account info
            let account = self.get_account(line.account_id)?
                .ok_or(stateset_core::CommerceError::NotFound)?;

            conn.execute(
                "INSERT INTO gl_journal_entry_lines (id, journal_entry_id, line_number,
                 account_id, account_number, account_name, description, debit_amount,
                 credit_amount, currency, reference_type, reference_id, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![
                    line_id.to_string(),
                    id.to_string(),
                    (line_num + 1) as i32,
                    line.account_id.to_string(),
                    account.account_number,
                    account.name,
                    line.description,
                    line.debit_amount.to_string(),
                    line.credit_amount.to_string(),
                    account.currency,
                    line.reference_type,
                    line.reference_id.map(|id| id.to_string()),
                    now.to_rfc3339(),
                ],
            ).map_err(map_db_error)?;
        }

        // Auto-post if requested
        if input.auto_post.unwrap_or(false) && is_balanced {
            return self.post_journal_entry(id, "system");
        }

        self.get_journal_entry(id)?.ok_or(stateset_core::CommerceError::NotFound)
    }

    fn get_journal_entry(&self, id: Uuid) -> Result<Option<JournalEntry>> {
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let entry = match conn.query_row(
            "SELECT id, entry_number, entry_date, period_id, entry_type, source,
                    source_document_type, source_document_id, description, total_debits,
                    total_credits, is_balanced, status, posted_at, posted_by,
                    reversed_entry_id, reversing_entry_id, created_at, updated_at
             FROM gl_journal_entries WHERE id = ?1",
            params![id.to_string()],
            Self::map_journal_entry_row,
        ) {
            Ok(mut entry) => {
                entry.lines = self.get_journal_entry_lines(id)?;
                entry
            },
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(e) => return Err(map_db_error(e)),
        };

        Ok(Some(entry))
    }

    fn get_journal_entry_by_number(&self, number: &str) -> Result<Option<JournalEntry>> {
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let id: String = match conn.query_row(
            "SELECT id FROM gl_journal_entries WHERE entry_number = ?1",
            params![number],
            |row| row.get(0),
        ) {
            Ok(id) => id,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(e) => return Err(map_db_error(e)),
        };

        self.get_journal_entry(id.parse().unwrap_or_default())
    }

    fn list_journal_entries(&self, filter: JournalEntryFilter) -> Result<Vec<JournalEntry>> {
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let mut sql = String::from(
            "SELECT id, entry_number, entry_date, period_id, entry_type, source,
                    source_document_type, source_document_id, description, total_debits,
                    total_credits, is_balanced, status, posted_at, posted_by,
                    reversed_entry_id, reversing_entry_id, created_at, updated_at
             FROM gl_journal_entries WHERE 1=1"
        );
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(period_id) = filter.period_id {
            sql.push_str(" AND period_id = ?");
            params.push(Box::new(period_id.to_string()));
        }
        if let Some(entry_type) = filter.entry_type {
            sql.push_str(" AND entry_type = ?");
            params.push(Box::new(entry_type.to_string()));
        }
        if let Some(source) = filter.source {
            sql.push_str(" AND source = ?");
            params.push(Box::new(source.to_string()));
        }
        if let Some(status) = filter.status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(status.to_string()));
        }
        if let Some(from_date) = filter.from_date {
            sql.push_str(" AND entry_date >= ?");
            params.push(Box::new(from_date.to_string()));
        }
        if let Some(to_date) = filter.to_date {
            sql.push_str(" AND entry_date <= ?");
            params.push(Box::new(to_date.to_string()));
        }
        if let Some(doc_type) = filter.source_document_type {
            sql.push_str(" AND source_document_type = ?");
            params.push(Box::new(doc_type));
        }
        if let Some(doc_id) = filter.source_document_id {
            sql.push_str(" AND source_document_id = ?");
            params.push(Box::new(doc_id.to_string()));
        }

        sql.push_str(" ORDER BY entry_date DESC, entry_number DESC");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }
        if let Some(offset) = filter.offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt.query_map(params_refs.as_slice(), Self::map_journal_entry_row).map_err(map_db_error)?;

        let mut entries = Vec::new();
        for row in rows {
            let mut entry = row.map_err(map_db_error)?;
            entry.lines = self.get_journal_entry_lines(entry.id)?;
            entries.push(entry);
        }
        Ok(entries)
    }

    fn post_journal_entry(&self, id: Uuid, posted_by: &str) -> Result<JournalEntry> {
        let entry = self.get_journal_entry(id)?.ok_or(stateset_core::CommerceError::NotFound)?;

        if !entry.can_post() {
            return Err(stateset_core::CommerceError::ValidationError(
                "Entry cannot be posted - must be draft and balanced".to_string()
            ));
        }

        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;
        let now = Utc::now();

        // Update entry status
        conn.execute(
            "UPDATE gl_journal_entries SET status = 'posted', posted_at = ?1, posted_by = ?2 WHERE id = ?3",
            params![now.to_rfc3339(), posted_by, id.to_string()],
        ).map_err(map_db_error)?;

        // Update account balances
        for line in &entry.lines {
            self.update_account_balance(line.account_id, line.debit_amount, line.credit_amount)?;
        }

        self.get_journal_entry(id)?.ok_or(stateset_core::CommerceError::NotFound)
    }

    fn void_journal_entry(&self, id: Uuid) -> Result<JournalEntry> {
        let entry = self.get_journal_entry(id)?.ok_or(stateset_core::CommerceError::NotFound)?;

        if !entry.can_void() {
            return Err(stateset_core::CommerceError::ValidationError(
                "Entry cannot be voided - must be posted".to_string()
            ));
        }

        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        // Reverse account balances
        for line in &entry.lines {
            self.update_account_balance(line.account_id, line.credit_amount, line.debit_amount)?;
        }

        // Update entry status
        conn.execute(
            "UPDATE gl_journal_entries SET status = 'voided' WHERE id = ?1",
            params![id.to_string()],
        ).map_err(map_db_error)?;

        self.get_journal_entry(id)?.ok_or(stateset_core::CommerceError::NotFound)
    }

    fn reverse_journal_entry(&self, id: Uuid, reversal_date: NaiveDate) -> Result<JournalEntry> {
        let entry = self.get_journal_entry(id)?.ok_or(stateset_core::CommerceError::NotFound)?;

        if entry.status != JournalEntryStatus::Posted {
            return Err(stateset_core::CommerceError::ValidationError(
                "Can only reverse posted entries".to_string()
            ));
        }

        // Create reversing entry with swapped debits/credits
        let reversing_lines: Vec<_> = entry.lines.iter().map(|l| {
            stateset_core::CreateJournalEntryLine {
                account_id: l.account_id,
                description: Some(format!("Reversal of {}", entry.entry_number)),
                debit_amount: l.credit_amount,
                credit_amount: l.debit_amount,
                reference_type: l.reference_type.clone(),
                reference_id: l.reference_id,
            }
        }).collect();

        let reversing_entry = self.create_journal_entry(stateset_core::CreateJournalEntry {
            entry_date: reversal_date,
            entry_type: Some(JournalEntryType::Reversing),
            description: format!("Reversal of {}", entry.entry_number),
            lines: reversing_lines,
            source_document_type: Some("reversal".to_string()),
            source_document_id: Some(entry.id),
            auto_post: Some(true),
        })?;

        // Link entries
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;
        conn.execute(
            "UPDATE gl_journal_entries SET reversing_entry_id = ?1, status = 'reversed' WHERE id = ?2",
            params![reversing_entry.id.to_string(), id.to_string()],
        ).map_err(map_db_error)?;

        conn.execute(
            "UPDATE gl_journal_entries SET reversed_entry_id = ?1 WHERE id = ?2",
            params![id.to_string(), reversing_entry.id.to_string()],
        ).map_err(map_db_error)?;

        self.get_journal_entry(reversing_entry.id)?.ok_or(stateset_core::CommerceError::NotFound)
    }

    fn get_journal_entry_lines(&self, journal_entry_id: Uuid) -> Result<Vec<JournalEntryLine>> {
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT id, journal_entry_id, line_number, account_id, account_number,
                    account_name, description, debit_amount, credit_amount, currency,
                    reference_type, reference_id, created_at
             FROM gl_journal_entry_lines WHERE journal_entry_id = ?1 ORDER BY line_number"
        ).map_err(map_db_error)?;

        let rows = stmt.query_map(params![journal_entry_id.to_string()], Self::map_journal_entry_line_row)
            .map_err(map_db_error)?;

        let mut lines = Vec::new();
        for row in rows {
            lines.push(row.map_err(map_db_error)?);
        }
        Ok(lines)
    }

    // ========================================================================
    // Auto-posting
    // ========================================================================

    fn get_auto_posting_config(&self) -> Result<Option<AutoPostingConfig>> {
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        match conn.query_row(
            "SELECT id, config_name, cash_account_id, accounts_receivable_account_id,
                    inventory_account_id, accounts_payable_account_id, unearned_revenue_account_id,
                    sales_revenue_account_id, shipping_revenue_account_id, cogs_account_id,
                    bad_debt_expense_account_id, is_active, created_at, updated_at
             FROM gl_auto_posting_config WHERE is_active = 1 LIMIT 1",
            [],
            Self::map_auto_posting_config_row,
        ) {
            Ok(config) => Ok(Some(config)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn set_auto_posting_config(&self, input: CreateAutoPostingConfig) -> Result<AutoPostingConfig> {
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;
        let id = Uuid::new_v4();
        let now = Utc::now();

        // Deactivate existing configs
        conn.execute("UPDATE gl_auto_posting_config SET is_active = 0", []).map_err(map_db_error)?;

        conn.execute(
            "INSERT INTO gl_auto_posting_config (id, config_name, cash_account_id,
             accounts_receivable_account_id, inventory_account_id, accounts_payable_account_id,
             unearned_revenue_account_id, sales_revenue_account_id, shipping_revenue_account_id,
             cogs_account_id, bad_debt_expense_account_id, is_active, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                id.to_string(),
                input.config_name,
                input.cash_account_id.to_string(),
                input.accounts_receivable_account_id.to_string(),
                input.inventory_account_id.to_string(),
                input.accounts_payable_account_id.to_string(),
                input.unearned_revenue_account_id.map(|id| id.to_string()),
                input.sales_revenue_account_id.to_string(),
                input.shipping_revenue_account_id.map(|id| id.to_string()),
                input.cogs_account_id.to_string(),
                input.bad_debt_expense_account_id.map(|id| id.to_string()),
                1,
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        ).map_err(map_db_error)?;

        self.get_auto_posting_config()?.ok_or(stateset_core::CommerceError::NotFound)
    }

    fn auto_post_invoice(&self, invoice_id: Uuid) -> Result<JournalEntry> {
        let config = self.get_auto_posting_config()?
            .ok_or_else(|| stateset_core::CommerceError::ValidationError(
                "Auto-posting not configured".to_string()
            ))?;

        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        // Get invoice details
        let (total, invoice_date): (String, String) = conn.query_row(
            "SELECT total_amount, invoice_date FROM invoices WHERE id = ?1",
            params![invoice_id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).map_err(map_db_error)?;

        let amount = parse_decimal(&total);
        let entry_date: NaiveDate = invoice_date.parse().unwrap_or_else(|_| Utc::now().date_naive());

        self.create_journal_entry(stateset_core::CreateJournalEntry {
            entry_date,
            entry_type: Some(JournalEntryType::Standard),
            description: format!("Invoice {}", invoice_id),
            lines: vec![
                stateset_core::CreateJournalEntryLine::debit(
                    config.accounts_receivable_account_id,
                    amount,
                    Some("Accounts Receivable".to_string()),
                ),
                stateset_core::CreateJournalEntryLine::credit(
                    config.sales_revenue_account_id,
                    amount,
                    Some("Sales Revenue".to_string()),
                ),
            ],
            source_document_type: Some("invoice".to_string()),
            source_document_id: Some(invoice_id),
            auto_post: Some(true),
        })
    }

    fn auto_post_payment_received(&self, payment_id: Uuid) -> Result<JournalEntry> {
        let config = self.get_auto_posting_config()?
            .ok_or_else(|| stateset_core::CommerceError::ValidationError(
                "Auto-posting not configured".to_string()
            ))?;

        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let (amount_str, payment_date): (String, String) = conn.query_row(
            "SELECT amount, payment_date FROM payments WHERE id = ?1",
            params![payment_id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).map_err(map_db_error)?;

        let amount = parse_decimal(&amount_str);
        let entry_date: NaiveDate = payment_date.parse().unwrap_or_else(|_| Utc::now().date_naive());

        self.create_journal_entry(stateset_core::CreateJournalEntry {
            entry_date,
            entry_type: Some(JournalEntryType::Standard),
            description: format!("Payment {}", payment_id),
            lines: vec![
                stateset_core::CreateJournalEntryLine::debit(
                    config.cash_account_id,
                    amount,
                    Some("Cash".to_string()),
                ),
                stateset_core::CreateJournalEntryLine::credit(
                    config.accounts_receivable_account_id,
                    amount,
                    Some("Accounts Receivable".to_string()),
                ),
            ],
            source_document_type: Some("payment".to_string()),
            source_document_id: Some(payment_id),
            auto_post: Some(true),
        })
    }

    fn auto_post_bill(&self, bill_id: Uuid) -> Result<JournalEntry> {
        let config = self.get_auto_posting_config()?
            .ok_or_else(|| stateset_core::CommerceError::ValidationError(
                "Auto-posting not configured".to_string()
            ))?;

        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let (total, bill_date): (String, String) = conn.query_row(
            "SELECT total_amount, bill_date FROM bills WHERE id = ?1",
            params![bill_id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).map_err(map_db_error)?;

        let amount = parse_decimal(&total);
        let entry_date: NaiveDate = bill_date.parse().unwrap_or_else(|_| Utc::now().date_naive());

        self.create_journal_entry(stateset_core::CreateJournalEntry {
            entry_date,
            entry_type: Some(JournalEntryType::Standard),
            description: format!("Bill {}", bill_id),
            lines: vec![
                stateset_core::CreateJournalEntryLine::debit(
                    config.inventory_account_id,
                    amount,
                    Some("Inventory/Expense".to_string()),
                ),
                stateset_core::CreateJournalEntryLine::credit(
                    config.accounts_payable_account_id,
                    amount,
                    Some("Accounts Payable".to_string()),
                ),
            ],
            source_document_type: Some("bill".to_string()),
            source_document_id: Some(bill_id),
            auto_post: Some(true),
        })
    }

    fn auto_post_bill_payment(&self, payment_id: Uuid) -> Result<JournalEntry> {
        let config = self.get_auto_posting_config()?
            .ok_or_else(|| stateset_core::CommerceError::ValidationError(
                "Auto-posting not configured".to_string()
            ))?;

        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let (amount_str, payment_date): (String, String) = conn.query_row(
            "SELECT amount, payment_date FROM bill_payments WHERE id = ?1",
            params![payment_id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).map_err(map_db_error)?;

        let amount = parse_decimal(&amount_str);
        let entry_date: NaiveDate = payment_date.parse().unwrap_or_else(|_| Utc::now().date_naive());

        self.create_journal_entry(stateset_core::CreateJournalEntry {
            entry_date,
            entry_type: Some(JournalEntryType::Standard),
            description: format!("Bill Payment {}", payment_id),
            lines: vec![
                stateset_core::CreateJournalEntryLine::debit(
                    config.accounts_payable_account_id,
                    amount,
                    Some("Accounts Payable".to_string()),
                ),
                stateset_core::CreateJournalEntryLine::credit(
                    config.cash_account_id,
                    amount,
                    Some("Cash".to_string()),
                ),
            ],
            source_document_type: Some("bill_payment".to_string()),
            source_document_id: Some(payment_id),
            auto_post: Some(true),
        })
    }

    fn auto_post_inventory_cost(&self, cost_transaction_id: Uuid) -> Result<JournalEntry> {
        let config = self.get_auto_posting_config()?
            .ok_or_else(|| stateset_core::CommerceError::ValidationError(
                "Auto-posting not configured".to_string()
            ))?;

        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let (cost_str, transaction_date, transaction_type): (String, String, String) = conn.query_row(
            "SELECT total_cost, transaction_date, transaction_type FROM cost_transactions WHERE id = ?1",
            params![cost_transaction_id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ).map_err(map_db_error)?;

        let cost = parse_decimal(&cost_str);
        let entry_date: NaiveDate = transaction_date.parse().unwrap_or_else(|_| Utc::now().date_naive());

        let (debit_account, credit_account) = if transaction_type == "sale" {
            (config.cogs_account_id, config.inventory_account_id)
        } else {
            (config.inventory_account_id, config.cogs_account_id)
        };

        self.create_journal_entry(stateset_core::CreateJournalEntry {
            entry_date,
            entry_type: Some(JournalEntryType::Standard),
            description: format!("Inventory Cost {}", cost_transaction_id),
            lines: vec![
                stateset_core::CreateJournalEntryLine::debit(
                    debit_account,
                    cost,
                    Some(if transaction_type == "sale" { "COGS" } else { "Inventory" }.to_string()),
                ),
                stateset_core::CreateJournalEntryLine::credit(
                    credit_account,
                    cost,
                    Some(if transaction_type == "sale" { "Inventory" } else { "COGS" }.to_string()),
                ),
            ],
            source_document_type: Some("cost_transaction".to_string()),
            source_document_id: Some(cost_transaction_id),
            auto_post: Some(true),
        })
    }

    fn auto_post_write_off(&self, write_off_id: Uuid) -> Result<JournalEntry> {
        let config = self.get_auto_posting_config()?
            .ok_or_else(|| stateset_core::CommerceError::ValidationError(
                "Auto-posting not configured".to_string()
            ))?;

        let bad_debt_account = config.bad_debt_expense_account_id
            .ok_or_else(|| stateset_core::CommerceError::ValidationError(
                "Bad debt expense account not configured".to_string()
            ))?;

        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let (amount_str, write_off_date): (String, String) = conn.query_row(
            "SELECT amount, write_off_date FROM ar_write_offs WHERE id = ?1",
            params![write_off_id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).map_err(map_db_error)?;

        let amount = parse_decimal(&amount_str);
        let entry_date: NaiveDate = write_off_date.parse().unwrap_or_else(|_| Utc::now().date_naive());

        self.create_journal_entry(stateset_core::CreateJournalEntry {
            entry_date,
            entry_type: Some(JournalEntryType::Standard),
            description: format!("Write-off {}", write_off_id),
            lines: vec![
                stateset_core::CreateJournalEntryLine::debit(
                    bad_debt_account,
                    amount,
                    Some("Bad Debt Expense".to_string()),
                ),
                stateset_core::CreateJournalEntryLine::credit(
                    config.accounts_receivable_account_id,
                    amount,
                    Some("Accounts Receivable".to_string()),
                ),
            ],
            source_document_type: Some("write_off".to_string()),
            source_document_id: Some(write_off_id),
            auto_post: Some(true),
        })
    }

    // ========================================================================
    // Financial Reports
    // ========================================================================

    fn get_trial_balance(&self, as_of_date: NaiveDate) -> Result<TrialBalance> {
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT a.id, a.account_number, a.name, a.account_type, a.normal_balance, a.current_balance
             FROM gl_accounts a
             WHERE a.is_posting = 1 AND a.status = 'active'
             ORDER BY a.account_number"
        ).map_err(map_db_error)?;

        let rows = stmt.query_map([], |row| {
            let balance = parse_decimal(&row.get::<_, String>(5)?);
            let normal_balance: BalanceSide = row.get::<_, String>(4)?.parse().unwrap_or(BalanceSide::Debit);

            let (debit_balance, credit_balance) = match normal_balance {
                BalanceSide::Debit => (balance, Decimal::ZERO),
                BalanceSide::Credit => (Decimal::ZERO, balance),
            };

            Ok(TrialBalanceLine {
                account_id: row.get::<_, String>(0)?.parse().unwrap_or_default(),
                account_number: row.get(1)?,
                account_name: row.get(2)?,
                account_type: row.get::<_, String>(3)?.parse().unwrap_or(AccountType::Asset),
                debit_balance,
                credit_balance,
            })
        }).map_err(map_db_error)?;

        let mut lines = Vec::new();
        let mut total_debits = Decimal::ZERO;
        let mut total_credits = Decimal::ZERO;

        for row in rows {
            let line = row.map_err(map_db_error)?;
            total_debits += line.debit_balance;
            total_credits += line.credit_balance;
            lines.push(line);
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

    fn get_balance_sheet(&self, as_of_date: NaiveDate) -> Result<BalanceSheet> {
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let mut assets = Vec::new();
        let mut liabilities = Vec::new();
        let mut equity = Vec::new();
        let mut total_assets = Decimal::ZERO;
        let mut total_liabilities = Decimal::ZERO;
        let mut total_equity = Decimal::ZERO;

        let mut stmt = conn.prepare(
            "SELECT id, account_number, name, account_type, account_sub_type, current_balance, normal_balance
             FROM gl_accounts
             WHERE is_posting = 1 AND status = 'active'
               AND account_type IN ('asset', 'liability', 'equity')
             ORDER BY account_number"
        ).map_err(map_db_error)?;

        let rows = stmt.query_map([], |row| {
            let balance = parse_decimal(&row.get::<_, String>(5)?);
            let normal_balance: BalanceSide = row.get::<_, String>(6)?.parse().unwrap_or(BalanceSide::Debit);

            let display_balance = match normal_balance {
                BalanceSide::Debit => balance,
                BalanceSide::Credit => balance,
            };

            Ok((
                row.get::<_, String>(0)?.parse().unwrap_or_default(),
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                display_balance,
            ))
        }).map_err(map_db_error)?;

        for row in rows {
            let (id, number, name, account_type, sub_type, balance): (Uuid, String, String, String, Option<String>, Decimal)
                = row.map_err(map_db_error)?;

            let line = BalanceSheetLine {
                account_id: id,
                account_number: number,
                account_name: name,
                account_sub_type: sub_type.and_then(|s| s.parse().ok()),
                balance,
                indent_level: 0,
                is_total: false,
            };

            match account_type.as_str() {
                "asset" => {
                    total_assets += balance;
                    assets.push(line);
                },
                "liability" => {
                    total_liabilities += balance;
                    liabilities.push(line);
                },
                "equity" => {
                    total_equity += balance;
                    equity.push(line);
                },
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

    fn get_income_statement(&self, start_date: NaiveDate, end_date: NaiveDate) -> Result<IncomeStatement> {
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let mut revenue_lines = Vec::new();
        let mut expense_lines = Vec::new();
        let mut total_revenue = Decimal::ZERO;
        let mut total_expenses = Decimal::ZERO;

        // Get account activity for the period from posted journal entries
        let mut stmt = conn.prepare(
            "SELECT a.id, a.account_number, a.name, a.account_type, a.account_sub_type,
                    COALESCE(SUM(l.debit_amount), 0) as total_debits,
                    COALESCE(SUM(l.credit_amount), 0) as total_credits
             FROM gl_accounts a
             LEFT JOIN gl_journal_entry_lines l ON a.id = l.account_id
             LEFT JOIN gl_journal_entries je ON l.journal_entry_id = je.id
             WHERE a.is_posting = 1 AND a.status = 'active'
               AND a.account_type IN ('revenue', 'expense')
               AND (je.status = 'posted' OR je.id IS NULL)
               AND (je.entry_date >= ?1 AND je.entry_date <= ?2 OR je.id IS NULL)
             GROUP BY a.id
             ORDER BY a.account_number"
        ).map_err(map_db_error)?;

        let rows = stmt.query_map(params![start_date.to_string(), end_date.to_string()], |row| {
            let total_debits = parse_decimal(&row.get::<_, String>(5)?);
            let total_credits = parse_decimal(&row.get::<_, String>(6)?);
            let account_type: String = row.get(3)?;

            // Revenue has credit normal balance, expense has debit
            let amount = if account_type == "revenue" {
                total_credits - total_debits
            } else {
                total_debits - total_credits
            };

            Ok((
                row.get::<_, String>(0)?.parse().unwrap_or_default(),
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                account_type,
                row.get::<_, Option<String>>(4)?,
                amount,
            ))
        }).map_err(map_db_error)?;

        for row in rows {
            let (id, number, name, account_type, sub_type, amount): (Uuid, String, String, String, Option<String>, Decimal)
                = row.map_err(map_db_error)?;

            if amount == Decimal::ZERO {
                continue;
            }

            let line = IncomeStatementLine {
                account_id: id,
                account_number: number,
                account_name: name,
                account_sub_type: sub_type.and_then(|s| s.parse().ok()),
                amount,
                indent_level: 0,
                is_total: false,
            };

            match account_type.as_str() {
                "revenue" => {
                    total_revenue += amount;
                    revenue_lines.push(line);
                },
                "expense" => {
                    total_expenses += amount;
                    expense_lines.push(line);
                },
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

    fn get_account_balance(&self, account_id: Uuid, _as_of_date: Option<NaiveDate>) -> Result<Decimal> {
        let account = self.get_account(account_id)?.ok_or(stateset_core::CommerceError::NotFound)?;
        Ok(account.current_balance)
    }

    fn get_account_transactions(&self, account_id: Uuid, filter: JournalEntryFilter) -> Result<Vec<JournalEntryLine>> {
        let conn = self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let mut sql = String::from(
            "SELECT l.id, l.journal_entry_id, l.line_number, l.account_id, l.account_number,
                    l.account_name, l.description, l.debit_amount, l.credit_amount, l.currency,
                    l.reference_type, l.reference_id, l.created_at
             FROM gl_journal_entry_lines l
             JOIN gl_journal_entries je ON l.journal_entry_id = je.id
             WHERE l.account_id = ?1"
        );
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(account_id.to_string())];

        if let Some(status) = filter.status {
            sql.push_str(" AND je.status = ?");
            params.push(Box::new(status.to_string()));
        }
        if let Some(from_date) = filter.from_date {
            sql.push_str(" AND je.entry_date >= ?");
            params.push(Box::new(from_date.to_string()));
        }
        if let Some(to_date) = filter.to_date {
            sql.push_str(" AND je.entry_date <= ?");
            params.push(Box::new(to_date.to_string()));
        }

        sql.push_str(" ORDER BY je.entry_date DESC, l.line_number");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt.query_map(params_refs.as_slice(), Self::map_journal_entry_line_row).map_err(map_db_error)?;

        let mut lines = Vec::new();
        for row in rows {
            lines.push(row.map_err(map_db_error)?);
        }
        Ok(lines)
    }

    fn run_period_close(&self, period_id: Uuid, closed_by: &str) -> Result<JournalEntry> {
        let period = self.get_period(period_id)?.ok_or(stateset_core::CommerceError::NotFound)?;

        if period.status != PeriodStatus::Open {
            return Err(stateset_core::CommerceError::ValidationError(
                "Period must be open to close".to_string()
            ));
        }

        // Generate income statement for the period
        let income_statement = self.get_income_statement(period.start_date, period.end_date)?;

        // Only create closing entry if there's net income to transfer
        if income_statement.net_income == Decimal::ZERO {
            // Just close the period
            return Err(stateset_core::CommerceError::ValidationError(
                "No net income to close".to_string()
            ));
        }

        // Get retained earnings account
        let retained_earnings = self.list_accounts(GlAccountFilter {
            account_sub_type: Some(AccountSubType::RetainedEarnings),
            ..Default::default()
        })?.into_iter().next()
            .ok_or_else(|| stateset_core::CommerceError::ValidationError(
                "Retained earnings account not found".to_string()
            ))?;

        // Create closing entry - debit revenue accounts, credit expense accounts
        // and net to retained earnings
        let mut lines = Vec::new();

        for rev in income_statement.revenue_lines {
            lines.push(stateset_core::CreateJournalEntryLine::debit(
                rev.account_id,
                rev.amount,
                Some(format!("Close {} to Retained Earnings", rev.account_name)),
            ));
        }

        for exp in income_statement.expense_lines {
            lines.push(stateset_core::CreateJournalEntryLine::credit(
                exp.account_id,
                exp.amount,
                Some(format!("Close {} to Retained Earnings", exp.account_name)),
            ));
        }

        // Net to retained earnings
        if income_statement.net_income > Decimal::ZERO {
            lines.push(stateset_core::CreateJournalEntryLine::credit(
                retained_earnings.id,
                income_statement.net_income,
                Some("Net income to Retained Earnings".to_string()),
            ));
        } else {
            lines.push(stateset_core::CreateJournalEntryLine::debit(
                retained_earnings.id,
                income_statement.net_income.abs(),
                Some("Net loss to Retained Earnings".to_string()),
            ));
        }

        let closing_entry = self.create_journal_entry(stateset_core::CreateJournalEntry {
            entry_date: period.end_date,
            entry_type: Some(JournalEntryType::Closing),
            description: format!("Closing entries for {}", period.period_name),
            lines,
            source_document_type: Some("period_close".to_string()),
            source_document_id: Some(period_id),
            auto_post: Some(true),
        })?;

        // Close the period
        self.close_period(period_id, closed_by)?;

        Ok(closing_entry)
    }

    // ========================================================================
    // Batch Operations
    // ========================================================================

    fn create_accounts_batch(&self, inputs: Vec<CreateGlAccount>) -> Result<BatchResult<GlAccount>> {
        let mut result = BatchResult::new();

        for (index, input) in inputs.into_iter().enumerate() {
            match self.create_account(input) {
                Ok(account) => result.record_success(account),
                Err(e) => result.record_failure(index, None, &e),
            }
        }

        Ok(result)
    }

    fn get_accounts_batch(&self, ids: Vec<Uuid>) -> Result<Vec<GlAccount>> {
        let mut accounts = Vec::new();
        for id in ids {
            if let Some(account) = self.get_account(id)? {
                accounts.push(account);
            }
        }
        Ok(accounts)
    }
}
