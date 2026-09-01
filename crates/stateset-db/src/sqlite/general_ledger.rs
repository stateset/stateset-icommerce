//! SQLite implementation of General Ledger repository

use crate::KernelOutboxEvent;
use crate::sqlite::kernel_outbox::append_kernel_event_tx;
use crate::sqlite::{map_db_error, parse_uuid, with_immediate_transaction};
use chrono::{NaiveDate, Utc};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{params, types::Type};
use rust_decimal::Decimal;
use stateset_core::{
    AccountStatus, AccountSubType, AccountType, AutoPostingConfig, BalanceSheet, BalanceSheetLine,
    BalanceSide, BatchResult, CreateAutoPostingConfig, CreateGlAccount, CreateGlPeriod,
    CreateJournalEntry, Currency, FX_REVALUATION_REFERENCE, GeneralLedgerRepository, GlAccount,
    GlAccountFilter, GlPeriod, GlPeriodFilter, IncomeStatement, IncomeStatementLine, InvoiceId,
    JournalEntry, JournalEntryFilter, JournalEntryLine, JournalEntrySource, JournalEntryStatus,
    JournalEntryType, PeriodStatus, Result, RevaluationLine, RevaluationResult, TrialBalance,
    TrialBalanceLine, UpdateGlAccount, create_default_chart_of_accounts,
    generate_journal_entry_number,
};
use uuid::Uuid;

/// Families where one journal entry per source document is an invariant.
/// Recognition and depreciation post many entries per document and are
/// deliberately excluded. The returned key feeds the unique
/// `gl_journal_entries.source_document_key` backstop index.
fn source_document_key(
    source_document_type: Option<&str>,
    source_document_id: Option<Uuid>,
) -> Option<String> {
    const SINGLE_ENTRY_TYPES: [&str; 8] = [
        "invoice",
        "payment",
        "bill",
        "bill_payment",
        "cost_transaction",
        "write_off",
        "period_close",
        "reversal",
    ];
    match (source_document_type, source_document_id) {
        (Some(kind), Some(id)) if SINGLE_ENTRY_TYPES.contains(&kind) => {
            Some(format!("{kind}:{id}"))
        }
        _ => None,
    }
}

/// Reduce a stored RFC3339 timestamp to its calendar date, erroring in
/// `rusqlite` terms for use inside transaction closures.
fn parse_rfc3339_date_with_conn(raw: &str, field: &str) -> rusqlite::Result<NaiveDate> {
    chrono::DateTime::parse_from_rfc3339(raw).map(|dt| dt.date_naive()).map_err(|e| {
        rusqlite::Error::ToSqlConversionFailure(Box::new(
            stateset_core::CommerceError::DatabaseError(format!("invalid {field} {raw:?}: {e}")),
        ))
    })
}

#[derive(Debug)]
pub struct SqliteGeneralLedgerRepository {
    pool: Pool<SqliteConnectionManager>,
}

fn parse_required<T>(value: String, column: usize) -> rusqlite::Result<T>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    value.parse::<T>().map_err(|err: T::Err| {
        rusqlite::Error::FromSqlConversionFailure(
            column,
            Type::Text,
            Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, err.to_string())),
        )
    })
}

fn parse_optional<T>(value: Option<String>, column: usize) -> rusqlite::Result<Option<T>>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    match value {
        Some(value) => parse_required(value, column).map(Some),
        None => Ok(None),
    }
}

fn parse_decimal_required(value: String, column: usize) -> rusqlite::Result<Decimal> {
    parse_required(value, column)
}

impl SqliteGeneralLedgerRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn map_account_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<GlAccount> {
        Ok(GlAccount {
            id: parse_required(row.get::<_, String>(0)?, 0)?,
            account_number: row.get(1)?,
            name: row.get(2)?,
            description: row.get(3)?,
            account_type: parse_required(row.get::<_, String>(4)?, 4)?,
            account_sub_type: parse_optional(row.get::<_, Option<String>>(5)?, 5)?,
            parent_account_id: parse_optional(row.get::<_, Option<String>>(6)?, 6)?,
            is_header: row.get::<_, i32>(7)? != 0,
            is_posting: row.get::<_, i32>(8)? != 0,
            normal_balance: parse_required(row.get::<_, String>(9)?, 9)?,
            currency: row.get(10)?,
            status: parse_required(row.get::<_, String>(11)?, 11)?,
            current_balance: parse_decimal_required(row.get::<_, String>(12)?, 12)?,
            created_at: parse_required(row.get::<_, String>(13)?, 13)?,
            updated_at: parse_required(row.get::<_, String>(14)?, 14)?,
        })
    }

    fn map_period_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<GlPeriod> {
        Ok(GlPeriod {
            id: parse_required(row.get::<_, String>(0)?, 0)?,
            period_name: row.get(1)?,
            fiscal_year: row.get(2)?,
            period_number: row.get(3)?,
            start_date: parse_required(row.get::<_, String>(4)?, 4)?,
            end_date: parse_required(row.get::<_, String>(5)?, 5)?,
            status: parse_required(row.get::<_, String>(6)?, 6)?,
            closed_at: parse_optional(row.get::<_, Option<String>>(7)?, 7)?,
            closed_by: row.get(8)?,
            locked_at: parse_optional(row.get::<_, Option<String>>(9)?, 9)?,
            locked_by: row.get(10)?,
            created_at: parse_required(row.get::<_, String>(11)?, 11)?,
            updated_at: parse_required(row.get::<_, String>(12)?, 12)?,
        })
    }

    fn map_journal_entry_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<JournalEntry> {
        Ok(JournalEntry {
            id: parse_required(row.get::<_, String>(0)?, 0)?,
            entry_number: row.get(1)?,
            entry_date: parse_required(row.get::<_, String>(2)?, 2)?,
            period_id: parse_required(row.get::<_, String>(3)?, 3)?,
            entry_type: parse_required(row.get::<_, String>(4)?, 4)?,
            source: parse_required(row.get::<_, String>(5)?, 5)?,
            source_document_type: row.get(6)?,
            source_document_id: parse_optional(row.get::<_, Option<String>>(7)?, 7)?,
            description: row.get(8)?,
            total_debits: parse_decimal_required(row.get::<_, String>(9)?, 9)?,
            total_credits: parse_decimal_required(row.get::<_, String>(10)?, 10)?,
            is_balanced: row.get::<_, i32>(11)? != 0,
            status: parse_required(row.get::<_, String>(12)?, 12)?,
            posted_at: parse_optional(row.get::<_, Option<String>>(13)?, 13)?,
            posted_by: row.get(14)?,
            reversed_entry_id: parse_optional(row.get::<_, Option<String>>(15)?, 15)?,
            reversing_entry_id: parse_optional(row.get::<_, Option<String>>(16)?, 16)?,
            lines: Vec::new(),
            created_at: parse_required(row.get::<_, String>(17)?, 17)?,
            updated_at: parse_required(row.get::<_, String>(18)?, 18)?,
        })
    }

    fn map_journal_entry_line_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<JournalEntryLine> {
        Ok(JournalEntryLine {
            id: parse_required(row.get::<_, String>(0)?, 0)?,
            journal_entry_id: parse_required(row.get::<_, String>(1)?, 1)?,
            line_number: row.get(2)?,
            account_id: parse_required(row.get::<_, String>(3)?, 3)?,
            account_number: row.get(4)?,
            account_name: row.get(5)?,
            description: row.get(6)?,
            debit_amount: parse_decimal_required(row.get::<_, String>(7)?, 7)?,
            credit_amount: parse_decimal_required(row.get::<_, String>(8)?, 8)?,
            currency: row.get(9)?,
            reference_type: row.get(10)?,
            reference_id: parse_optional(row.get::<_, Option<String>>(11)?, 11)?,
            created_at: parse_required(row.get::<_, String>(12)?, 12)?,
        })
    }

    fn map_auto_posting_config_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AutoPostingConfig> {
        Ok(AutoPostingConfig {
            id: parse_required(row.get::<_, String>(0)?, 0)?,
            config_name: row.get(1)?,
            cash_account_id: parse_required(row.get::<_, String>(2)?, 2)?,
            accounts_receivable_account_id: parse_required(row.get::<_, String>(3)?, 3)?,
            inventory_account_id: parse_required(row.get::<_, String>(4)?, 4)?,
            accounts_payable_account_id: parse_required(row.get::<_, String>(5)?, 5)?,
            unearned_revenue_account_id: parse_optional(row.get::<_, Option<String>>(6)?, 6)?,
            sales_revenue_account_id: parse_required(row.get::<_, String>(7)?, 7)?,
            shipping_revenue_account_id: parse_optional(row.get::<_, Option<String>>(8)?, 8)?,
            cogs_account_id: parse_required(row.get::<_, String>(9)?, 9)?,
            bad_debt_expense_account_id: parse_optional(row.get::<_, Option<String>>(10)?, 10)?,
            fx_gain_loss_account_id: parse_optional(row.get::<_, Option<String>>(11)?, 11)?,
            auto_post_depreciation: row.get::<_, i32>(12)? != 0,
            auto_post_revenue_recognition: row.get::<_, i32>(13)? != 0,
            is_active: row.get::<_, i32>(14)? != 0,
            created_at: parse_required(row.get::<_, String>(15)?, 15)?,
            updated_at: parse_required(row.get::<_, String>(16)?, 16)?,
        })
    }

    pub(crate) fn update_account_balance_with_conn(
        conn: &rusqlite::Connection,
        account_id: Uuid,
        debit: Decimal,
        credit: Decimal,
    ) -> Result<()> {
        // Get account to determine normal balance
        let account: GlAccount = conn
            .query_row(
                "SELECT id, account_number, name, description, account_type, account_sub_type,
                    parent_account_id, is_header, is_posting, normal_balance, currency,
                    status, current_balance, created_at, updated_at
             FROM gl_accounts WHERE id = ?1",
                params![account_id.to_string()],
                Self::map_account_row,
            )
            .map_err(map_db_error)?;

        let balance_change = account.balance_effect(debit, credit);
        let new_balance = account.current_balance + balance_change;

        conn.execute(
            "UPDATE gl_accounts SET current_balance = ?1 WHERE id = ?2",
            params![new_balance.to_string(), account_id.to_string()],
        )
        .map_err(map_db_error)?;

        Ok(())
    }

    fn load_journal_entry_lines_with_conn(
        conn: &rusqlite::Connection,
        journal_entry_id: Uuid,
    ) -> rusqlite::Result<Vec<JournalEntryLine>> {
        let mut stmt = conn.prepare(
            "SELECT id, journal_entry_id, line_number, account_id, account_number,
                    account_name, description, debit_amount, credit_amount, currency,
                    reference_type, reference_id, created_at
             FROM gl_journal_entry_lines WHERE journal_entry_id = ?1 ORDER BY line_number",
        )?;
        let rows = stmt
            .query_map(params![journal_entry_id.to_string()], Self::map_journal_entry_line_row)?;
        rows.collect()
    }

    pub(crate) fn load_journal_entry_with_conn(
        conn: &rusqlite::Connection,
        id: Uuid,
    ) -> rusqlite::Result<JournalEntry> {
        let mut entry = conn.query_row(
            "SELECT id, entry_number, entry_date, period_id, entry_type, source,
                    source_document_type, source_document_id, description, total_debits,
                    total_credits, is_balanced, status, posted_at, posted_by,
                    reversed_entry_id, reversing_entry_id, created_at, updated_at
             FROM gl_journal_entries WHERE id = ?1",
            params![id.to_string()],
            Self::map_journal_entry_row,
        )?;
        entry.lines = Self::load_journal_entry_lines_with_conn(conn, id)?;
        Ok(entry)
    }

    /// The period an entry belongs to must be open for its balances to
    /// change — posting into (or voiding out of) a closed/locked period
    /// would silently diverge the period's reported financials from its
    /// closing entry.
    fn ensure_period_open_with_conn(
        conn: &rusqlite::Connection,
        period_id: Uuid,
        action: &str,
    ) -> rusqlite::Result<()> {
        let status: String = conn.query_row(
            "SELECT status FROM gl_periods WHERE id = ?1",
            params![period_id.to_string()],
            |row| row.get(0),
        )?;
        if status != "open" {
            return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                stateset_core::CommerceError::ValidationError(format!(
                    "Cannot {action} journal entry: its period is {status}, not open"
                )),
            )));
        }
        Ok(())
    }

    /// Non-voided journal entry already recorded for a source document, if
    /// any — the idempotency check for the `auto_post_*` family and for
    /// `run_period_close`.
    fn existing_entry_for_source(
        &self,
        source_document_type: &str,
        source_document_id: Uuid,
    ) -> Result<Option<JournalEntry>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;
        let id: String = match conn.query_row(
            "SELECT id FROM gl_journal_entries
             WHERE source_document_type = ?1 AND source_document_id = ?2
               AND status != 'voided'
             LIMIT 1",
            params![source_document_type, source_document_id.to_string()],
            |row| row.get(0),
        ) {
            Ok(id) => id,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(e) => return Err(map_db_error(e)),
        };
        drop(conn);
        self.get_journal_entry(parse_uuid(&id, "gl_journal_entry", "id")?)
    }

    /// Active auto-posting configuration, read on the caller's connection so
    /// subledger transactions can resolve accounts without leaving their
    /// transaction.
    pub(crate) fn get_auto_posting_config_with_conn(
        conn: &rusqlite::Transaction<'_>,
    ) -> rusqlite::Result<Option<AutoPostingConfig>> {
        match conn.query_row(
            "SELECT id, config_name, cash_account_id, accounts_receivable_account_id,
                    inventory_account_id, accounts_payable_account_id, unearned_revenue_account_id,
                    sales_revenue_account_id, shipping_revenue_account_id, cogs_account_id,
                    bad_debt_expense_account_id, fx_gain_loss_account_id, auto_post_depreciation,
                    auto_post_revenue_recognition, is_active, created_at, updated_at
             FROM gl_auto_posting_config WHERE is_active = 1 LIMIT 1",
            [],
            Self::map_auto_posting_config_row,
        ) {
            Ok(config) => Ok(Some(config)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Active auto-posting configuration or a typed "not configured" error,
    /// for the `auto_post_*` transaction bodies.
    fn require_auto_posting_config_with_conn(
        conn: &rusqlite::Transaction<'_>,
    ) -> rusqlite::Result<AutoPostingConfig> {
        Self::get_auto_posting_config_with_conn(conn)?.ok_or_else(|| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(
                stateset_core::CommerceError::ValidationError(
                    "Auto-posting not configured".to_string(),
                ),
            ))
        })
    }

    /// Create AND post a balanced journal entry entirely on the caller's
    /// connection/transaction: validation, the entry and line inserts, the
    /// account-balance updates, and the outbox event commit (or roll back)
    /// together with whatever else the caller is doing. This is what lets a
    /// subledger mutation and its GL posting be a single atomic fact.
    pub(crate) fn create_posted_entry_with_conn(
        conn: &rusqlite::Transaction<'_>,
        input: &CreateJournalEntry,
        posted_by: &str,
    ) -> rusqlite::Result<JournalEntry> {
        let to_rusqlite =
            |e: stateset_core::CommerceError| rusqlite::Error::ToSqlConversionFailure(Box::new(e));

        let id = Uuid::new_v4();
        let now = Utc::now();
        let entry_number = generate_journal_entry_number();

        let total_debits: Decimal = input.lines.iter().map(|l| l.debit_amount).sum();
        let total_credits: Decimal = input.lines.iter().map(|l| l.credit_amount).sum();
        if total_debits != total_credits {
            return Err(to_rusqlite(stateset_core::CommerceError::ValidationError(format!(
                "Journal entry must balance to post: debits {total_debits} != credits {total_credits}"
            ))));
        }
        if let Some((index, _)) = input.lines.iter().enumerate().find(|(_, l)| {
            !((l.debit_amount > Decimal::ZERO && l.credit_amount == Decimal::ZERO)
                || (l.debit_amount == Decimal::ZERO && l.credit_amount > Decimal::ZERO))
        }) {
            return Err(to_rusqlite(stateset_core::CommerceError::JournalLineNotSingleSided {
                entry_id: id,
                line_number: i32::try_from(index + 1).unwrap_or(i32::MAX),
            }));
        }

        let (period_id, period_status): (String, String) = match conn.query_row(
            "SELECT id, status FROM gl_periods WHERE start_date <= ?1 AND end_date >= ?1",
            params![input.entry_date.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ) {
            Ok(row) => row,
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                return Err(to_rusqlite(stateset_core::CommerceError::ValidationError(format!(
                    "No period found for date {}",
                    input.entry_date
                ))));
            }
            Err(e) => return Err(e),
        };
        if period_status != "open" {
            return Err(to_rusqlite(stateset_core::CommerceError::ValidationError(
                "Period is not open for posting".to_string(),
            )));
        }

        conn.execute(
            "INSERT INTO gl_journal_entries (id, entry_number, entry_date, period_id,
             entry_type, source, source_document_type, source_document_id,
             source_document_key, description,
             total_debits, total_credits, is_balanced, status, posted_at, posted_by,
             created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 1, 'posted', ?13, ?14, ?15, ?15)",
            params![
                id.to_string(),
                entry_number,
                input.entry_date.to_string(),
                period_id,
                input.entry_type.unwrap_or(JournalEntryType::Standard).to_string(),
                JournalEntrySource::Manual.to_string(),
                input.source_document_type,
                input.source_document_id.map(|id| id.to_string()),
                source_document_key(
                    input.source_document_type.as_deref(),
                    input.source_document_id,
                ),
                input.description,
                total_debits.to_string(),
                total_credits.to_string(),
                now.to_rfc3339(),
                posted_by,
                now.to_rfc3339(),
            ],
        )?;

        for (line_num, line) in input.lines.iter().enumerate() {
            let account: GlAccount = conn.query_row(
                "SELECT id, account_number, name, description, account_type, account_sub_type,
                        parent_account_id, is_header, is_posting, normal_balance, currency,
                        status, current_balance, created_at, updated_at
                 FROM gl_accounts WHERE id = ?1",
                params![line.account_id.to_string()],
                Self::map_account_row,
            )?;
            conn.execute(
                "INSERT INTO gl_journal_entry_lines (id, journal_entry_id, line_number,
                 account_id, account_number, account_name, description, debit_amount,
                 credit_amount, currency, reference_type, reference_id, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![
                    Uuid::new_v4().to_string(),
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
            )?;
            Self::update_account_balance_with_conn(
                conn,
                line.account_id,
                line.debit_amount,
                line.credit_amount,
            )
            .map_err(to_rusqlite)?;
        }

        append_kernel_event_tx(
            conn,
            &KernelOutboxEvent::domain(
                "ledger.journal_entry_posted.v1",
                "journal_entry",
                id.to_string(),
                serde_json::json!({
                    "journal_entry_id": id.to_string(),
                    "entry_number": entry_number,
                    "source": JournalEntrySource::Manual.to_string(),
                    "total_debits": total_debits.to_string(),
                    "total_credits": total_credits.to_string(),
                    "line_count": input.lines.len(),
                    "posted_by": posted_by,
                    "status": JournalEntryStatus::Posted.to_string(),
                }),
                None,
            ),
        )?;

        Self::load_journal_entry_with_conn(conn, id)
    }

    /// Non-voided journal entry for a source document, read on the caller's
    /// connection: inside an IMMEDIATE transaction this is a race-free
    /// idempotency check (SQLite serializes writers).
    pub(crate) fn existing_entry_for_source_with_conn(
        conn: &rusqlite::Transaction<'_>,
        source_document_type: &str,
        source_document_id: Uuid,
    ) -> rusqlite::Result<Option<JournalEntry>> {
        let id: String = match conn.query_row(
            "SELECT id FROM gl_journal_entries
             WHERE source_document_type = ?1 AND source_document_id = ?2
               AND status != 'voided'
             LIMIT 1",
            params![source_document_type, source_document_id.to_string()],
            |row| row.get(0),
        ) {
            Ok(id) => id,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(e) => return Err(e),
        };
        let id = parse_uuid(&id, "gl_journal_entry", "id")
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        Self::load_journal_entry_with_conn(conn, id).map(Some)
    }

    /// Look up the exchange rate converting one `from` unit into `to` units,
    /// falling back to the inverse of the reverse pair when only that is set.
    fn lookup_rate_with_conn(
        conn: &rusqlite::Connection,
        from: &str,
        to: &str,
    ) -> Result<Option<Decimal>> {
        let direct = conn.query_row(
            "SELECT rate FROM exchange_rates WHERE base_currency = ?1 AND quote_currency = ?2",
            params![from, to],
            |row| row.get::<_, String>(0),
        );
        match direct {
            Ok(rate) => Ok(Some(parse_decimal_required(rate, 0).map_err(map_db_error)?)),
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                let inverse = conn.query_row(
                    "SELECT rate FROM exchange_rates
                     WHERE base_currency = ?1 AND quote_currency = ?2",
                    params![to, from],
                    |row| row.get::<_, String>(0),
                );
                match inverse {
                    Ok(rate) => {
                        let rate = parse_decimal_required(rate, 0).map_err(map_db_error)?;
                        if rate.is_zero() { Ok(None) } else { Ok(Some(Decimal::ONE / rate)) }
                    }
                    Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                    Err(e) => Err(map_db_error(e)),
                }
            }
            Err(e) => Err(map_db_error(e)),
        }
    }

    /// Resolve the account receiving unrealized FX gains/losses: the
    /// configured `fx_gain_loss_account_id`, else the first active posting
    /// account with an Other Expense / Other Revenue sub-type.
    fn resolve_fx_gain_loss_account(&self) -> Result<Uuid> {
        if let Some(id) =
            self.get_auto_posting_config()?.and_then(|config| config.fx_gain_loss_account_id)
        {
            return Ok(id);
        }
        for sub_type in [AccountSubType::OtherExpense, AccountSubType::OtherRevenue] {
            let fallback = self
                .list_accounts(GlAccountFilter {
                    account_sub_type: Some(sub_type),
                    status: Some(AccountStatus::Active),
                    is_posting: Some(true),
                    limit: Some(1),
                    ..Default::default()
                })?
                .into_iter()
                .next();
            if let Some(account) = fallback {
                return Ok(account.id);
            }
        }
        Err(stateset_core::CommerceError::ValidationError(
            "No FX gain/loss account configured for revaluation".to_string(),
        ))
    }
}

impl GeneralLedgerRepository for SqliteGeneralLedgerRepository {
    // ========================================================================
    // Chart of Accounts
    // ========================================================================

    fn create_account(&self, input: CreateGlAccount) -> Result<GlAccount> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let normal_balance = input.account_type.normal_balance();

        {
            let conn = self
                .pool
                .get()
                .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;
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
                    i32::from(input.is_header.unwrap_or(false)),
                    i32::from(input.is_posting.unwrap_or(true)),
                    normal_balance.to_string(),
                    input.currency.unwrap_or_default(),
                    AccountStatus::Active.to_string(),
                    "0",
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                ],
            )
            .map_err(map_db_error)?;
        }

        self.get_account(id)?.ok_or(stateset_core::CommerceError::NotFound)
    }

    fn get_account(&self, id: Uuid) -> Result<Option<GlAccount>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

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
        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

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
        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

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
            let sql = format!("UPDATE gl_accounts SET {} WHERE id = ?", updates.join(", "));
            let params: Vec<&dyn rusqlite::ToSql> =
                values.iter().map(std::convert::AsRef::as_ref).collect();
            conn.execute(&sql, params.as_slice()).map_err(map_db_error)?;
        }

        self.get_account(id)?.ok_or(stateset_core::CommerceError::NotFound)
    }

    fn list_accounts(&self, filter: GlAccountFilter) -> Result<Vec<GlAccount>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let mut sql = String::from(
            "SELECT id, account_number, name, description, account_type, account_sub_type,
                    parent_account_id, is_header, is_posting, normal_balance, currency,
                    status, current_balance, created_at, updated_at
             FROM gl_accounts WHERE 1=1",
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
            params.push(Box::new(i32::from(is_posting)));
        }
        if let Some(is_header) = filter.is_header {
            sql.push_str(" AND is_header = ?");
            params.push(Box::new(i32::from(is_header)));
        }
        if let Some(search) = filter.search {
            sql.push_str(" AND (name LIKE ? OR account_number LIKE ?)");
            let search_term = format!("%{search}%");
            params.push(Box::new(search_term.clone()));
            params.push(Box::new(search_term));
        }

        sql.push_str(" ORDER BY account_number");

        crate::sqlite::append_limit_offset(&mut sql, filter.limit, filter.offset);

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows =
            stmt.query_map(params_refs.as_slice(), Self::map_account_row).map_err(map_db_error)?;

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
        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        // Check if account has transactions
        let count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM gl_journal_entry_lines WHERE account_id = ?1",
                params![id.to_string()],
                |row| row.get(0),
            )
            .map_err(map_db_error)?;

        if count > 0 {
            return Err(stateset_core::CommerceError::ValidationError(
                "Cannot delete account with existing transactions".to_string(),
            ));
        }

        conn.execute("DELETE FROM gl_accounts WHERE id = ?1", params![id.to_string()])
            .map_err(map_db_error)?;

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
        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;
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
        )
        .map_err(map_db_error)?;

        self.get_period(id)?.ok_or(stateset_core::CommerceError::NotFound)
    }

    fn get_period(&self, id: Uuid) -> Result<Option<GlPeriod>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

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
        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

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
        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

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
        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let mut sql = String::from(
            "SELECT id, period_name, fiscal_year, period_number, start_date, end_date,
                    status, closed_at, closed_by, locked_at, locked_by, created_at, updated_at
             FROM gl_periods WHERE 1=1",
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

        crate::sqlite::append_limit_offset(&mut sql, filter.limit, filter.offset);

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows =
            stmt.query_map(params_refs.as_slice(), Self::map_period_row).map_err(map_db_error)?;

        let mut periods = Vec::new();
        for row in rows {
            periods.push(row.map_err(map_db_error)?);
        }
        Ok(periods)
    }

    fn open_period(&self, id: Uuid) -> Result<GlPeriod> {
        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        conn.execute(
            "UPDATE gl_periods SET status = 'open' WHERE id = ?1 AND status = 'future'",
            params![id.to_string()],
        )
        .map_err(map_db_error)?;

        self.get_period(id)?.ok_or(stateset_core::CommerceError::NotFound)
    }

    fn close_period(&self, id: Uuid, closed_by: &str) -> Result<GlPeriod> {
        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;
        let now = Utc::now();

        conn.execute(
            "UPDATE gl_periods SET status = 'closed', closed_at = ?1, closed_by = ?2
             WHERE id = ?3 AND status = 'open'",
            params![now.to_rfc3339(), closed_by, id.to_string()],
        )
        .map_err(map_db_error)?;

        self.get_period(id)?.ok_or(stateset_core::CommerceError::NotFound)
    }

    fn lock_period(&self, id: Uuid, locked_by: &str) -> Result<GlPeriod> {
        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;
        let now = Utc::now();

        conn.execute(
            "UPDATE gl_periods SET status = 'locked', locked_at = ?1, locked_by = ?2
             WHERE id = ?3 AND status = 'closed'",
            params![now.to_rfc3339(), locked_by, id.to_string()],
        )
        .map_err(map_db_error)?;

        self.get_period(id)?.ok_or(stateset_core::CommerceError::NotFound)
    }

    fn reopen_period(&self, id: Uuid) -> Result<GlPeriod> {
        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        conn.execute(
            "UPDATE gl_periods SET status = 'open', closed_at = NULL, closed_by = NULL
             WHERE id = ?1 AND status = 'closed'",
            params![id.to_string()],
        )
        .map_err(map_db_error)?;

        self.get_period(id)?.ok_or(stateset_core::CommerceError::NotFound)
    }

    // ========================================================================
    // Journal Entries
    // ========================================================================

    fn create_journal_entry(&self, input: CreateJournalEntry) -> Result<JournalEntry> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;
        let id = Uuid::new_v4();
        let now = Utc::now();
        let entry_number = generate_journal_entry_number();

        // Get period for entry date
        let period = self.get_period_for_date(input.entry_date)?.ok_or_else(|| {
            stateset_core::CommerceError::ValidationError(format!(
                "No period found for date {}",
                input.entry_date
            ))
        })?;

        if !period.can_post() {
            return Err(stateset_core::CommerceError::ValidationError(
                "Period is not open for posting".to_string(),
            ));
        }

        // Calculate totals
        let total_debits: Decimal = input.lines.iter().map(|l| l.debit_amount).sum();
        let total_credits: Decimal = input.lines.iter().map(|l| l.credit_amount).sum();
        let is_balanced = total_debits == total_credits;
        // Invariant `commerce.ledger.line_not_single_sided`: a line is a pure
        // debit or a pure credit, never both and never neither.
        if let Some((index, _)) = input.lines.iter().enumerate().find(|(_, l)| {
            !((l.debit_amount > Decimal::ZERO && l.credit_amount == Decimal::ZERO)
                || (l.debit_amount == Decimal::ZERO && l.credit_amount > Decimal::ZERO))
        }) {
            return Err(stateset_core::CommerceError::JournalLineNotSingleSided {
                entry_id: id,
                line_number: i32::try_from(index + 1).unwrap_or(i32::MAX),
            });
        }

        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;

        tx.execute(
            "INSERT INTO gl_journal_entries (id, entry_number, entry_date, period_id,
             entry_type, source, source_document_type, source_document_id,
             source_document_key, description,
             total_debits, total_credits, is_balanced, status, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![
                id.to_string(),
                entry_number,
                input.entry_date.to_string(),
                period.id.to_string(),
                input.entry_type.unwrap_or(JournalEntryType::Standard).to_string(),
                JournalEntrySource::Manual.to_string(),
                input.source_document_type,
                input.source_document_id.map(|id| id.to_string()),
                source_document_key(
                    input.source_document_type.as_deref(),
                    input.source_document_id,
                ),
                input.description,
                total_debits.to_string(),
                total_credits.to_string(),
                i32::from(is_balanced),
                JournalEntryStatus::Draft.to_string(),
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;

        // Insert lines
        for (line_num, line) in input.lines.iter().enumerate() {
            let line_id = Uuid::new_v4();

            // Get account info in the same transaction
            let account: GlAccount = tx
                .query_row(
                    "SELECT id, account_number, name, description, account_type, account_sub_type,
                            parent_account_id, is_header, is_posting, normal_balance, currency,
                            status, current_balance, created_at, updated_at
                     FROM gl_accounts WHERE id = ?1",
                    params![line.account_id.to_string()],
                    Self::map_account_row,
                )
                .map_err(map_db_error)?;

            tx.execute(
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
            )
            .map_err(map_db_error)?;
        }

        tx.commit().map_err(map_db_error)?;

        // Auto-post if requested
        if input.auto_post.unwrap_or(false) && is_balanced {
            return self.post_journal_entry(id, "system");
        }

        self.get_journal_entry(id)?.ok_or(stateset_core::CommerceError::NotFound)
    }

    fn get_journal_entry(&self, id: Uuid) -> Result<Option<JournalEntry>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

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
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(e) => return Err(map_db_error(e)),
        };

        Ok(Some(entry))
    }

    fn get_journal_entry_by_number(&self, number: &str) -> Result<Option<JournalEntry>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let id: String = match conn.query_row(
            "SELECT id FROM gl_journal_entries WHERE entry_number = ?1",
            params![number],
            |row| row.get(0),
        ) {
            Ok(id) => id,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(e) => return Err(map_db_error(e)),
        };

        let entry_id = parse_uuid(&id, "gl_journal_entry", "id")?;
        self.get_journal_entry(entry_id)
    }

    fn list_journal_entries(&self, filter: JournalEntryFilter) -> Result<Vec<JournalEntry>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let mut sql = String::from(
            "SELECT id, entry_number, entry_date, period_id, entry_type, source,
                    source_document_type, source_document_id, description, total_debits,
                    total_credits, is_balanced, status, posted_at, posted_by,
                    reversed_entry_id, reversing_entry_id, created_at, updated_at
             FROM gl_journal_entries WHERE 1=1",
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
        // `account_id` selects entries with a line posting to that account. Postgres
        // joins `gl_journal_entry_lines` with `SELECT DISTINCT`; the `id IN (…)`
        // subquery is the equivalent here and cannot produce duplicate rows.
        if let Some(account_id) = filter.account_id {
            sql.push_str(
                " AND id IN (SELECT journal_entry_id FROM gl_journal_entry_lines \
                 WHERE account_id = ?)",
            );
            params.push(Box::new(account_id.to_string()));
        }
        // Free-text search over entry number / description (Postgres uses `ILIKE`;
        // SQLite `LIKE` is case-insensitive for ASCII).
        if let Some(search) = filter.search {
            sql.push_str(" AND (entry_number LIKE ? OR description LIKE ?)");
            let term = format!("%{search}%");
            params.push(Box::new(term.clone()));
            params.push(Box::new(term));
        }

        sql.push_str(" ORDER BY entry_date DESC, entry_number DESC");

        crate::sqlite::append_limit_offset(&mut sql, filter.limit, filter.offset);

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt
            .query_map(params_refs.as_slice(), Self::map_journal_entry_row)
            .map_err(map_db_error)?;

        let mut entries = Vec::new();
        for row in rows {
            let mut entry = row.map_err(map_db_error)?;
            entry.lines = self.get_journal_entry_lines(entry.id)?;
            entries.push(entry);
        }
        Ok(entries)
    }

    fn post_journal_entry(&self, id: Uuid, posted_by: &str) -> Result<JournalEntry> {
        let now = Utc::now();

        with_immediate_transaction(&self.pool, |tx| {
            // Re-read inside the write transaction so a concurrent poster
            // cannot pass validation against stale state.
            let entry = Self::load_journal_entry_with_conn(tx, id)?;

            // Reports which condition failed: `commerce.ledger.entry_unbalanced`
            // / `commerce.ledger.line_not_single_sided` are typed; "not a
            // draft" and "no lines" keep the historical untyped message.
            if let Err(e) = entry.ensure_postable() {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(e)));
            }

            Self::ensure_period_open_with_conn(tx, entry.period_id, "post")?;

            for line in &entry.lines {
                Self::update_account_balance_with_conn(
                    tx,
                    line.account_id,
                    line.debit_amount,
                    line.credit_amount,
                )
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            }

            // Guard on status so the balance updates above can only commit
            // together with exactly one draft -> posted transition.
            let rows_affected = tx.execute(
                "UPDATE gl_journal_entries SET status = 'posted', posted_at = ?1, posted_by = ?2
                 WHERE id = ?3 AND status = 'draft'",
                params![now.to_rfc3339(), posted_by, id.to_string()],
            )?;
            if rows_affected == 0 {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    stateset_core::CommerceError::Conflict(
                        "Journal entry was modified concurrently".to_string(),
                    ),
                )));
            }

            append_kernel_event_tx(
                tx,
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
            )?;

            Ok(())
        })?;

        self.get_journal_entry(id)?.ok_or(stateset_core::CommerceError::NotFound)
    }

    fn void_journal_entry(&self, id: Uuid) -> Result<JournalEntry> {
        with_immediate_transaction(&self.pool, |tx| {
            // Re-read inside the write transaction so a concurrent voider
            // cannot pass validation against stale state.
            let entry = Self::load_journal_entry_with_conn(tx, id)?;

            if !entry.can_void() {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    stateset_core::CommerceError::ValidationError(
                        "Entry cannot be voided - must be posted".to_string(),
                    ),
                )));
            }

            Self::ensure_period_open_with_conn(tx, entry.period_id, "void")?;

            // Reverse account balances
            for line in &entry.lines {
                Self::update_account_balance_with_conn(
                    tx,
                    line.account_id,
                    line.credit_amount,
                    line.debit_amount,
                )
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            }

            // Guard on status so the balance reversal above can only commit
            // together with exactly one posted -> voided transition.
            let rows_affected = tx.execute(
                "UPDATE gl_journal_entries SET status = 'voided', source_document_key = NULL
                 WHERE id = ?1 AND status = 'posted'",
                params![id.to_string()],
            )?;
            if rows_affected == 0 {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    stateset_core::CommerceError::Conflict(
                        "Journal entry was modified concurrently".to_string(),
                    ),
                )));
            }

            Ok(())
        })?;

        self.get_journal_entry(id)?.ok_or(stateset_core::CommerceError::NotFound)
    }

    fn reverse_journal_entry(&self, id: Uuid, reversal_date: NaiveDate) -> Result<JournalEntry> {
        let entry = self.get_journal_entry(id)?.ok_or(stateset_core::CommerceError::NotFound)?;

        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;
        match entry.status {
            JournalEntryStatus::Posted => {
                // Claim the entry (posted -> reversed) before creating the
                // reversing entry, which commits its own transactions; the
                // status guard ensures concurrent reversals cannot both
                // create (and auto-post) a reversal.
                let claimed = conn
                    .execute(
                        "UPDATE gl_journal_entries SET status = 'reversed' WHERE id = ?1 AND status = 'posted'",
                        params![id.to_string()],
                    )
                    .map_err(map_db_error)?;
                if claimed == 0 {
                    return Err(stateset_core::CommerceError::Conflict(
                        "Journal entry was modified concurrently".to_string(),
                    ));
                }
            }
            JournalEntryStatus::Reversed => {
                // A claim with no reversing entry is the stranded state left
                // by a crash between the claim and the reversing entry's
                // creation (the best-effort un-claim below can itself be
                // lost). Resume it; an entry whose reversal exists stays a
                // completed reversal and rejects a second one (repairing the
                // cross-links first if a crash lost them).
                if let Some(reversal) = self.existing_entry_for_source("reversal", id)? {
                    conn.execute(
                        "UPDATE gl_journal_entries SET reversing_entry_id = ?1
                         WHERE id = ?2 AND reversing_entry_id IS NULL",
                        params![reversal.id.to_string(), id.to_string()],
                    )
                    .map_err(map_db_error)?;
                    conn.execute(
                        "UPDATE gl_journal_entries SET reversed_entry_id = ?1
                         WHERE id = ?2 AND reversed_entry_id IS NULL",
                        params![id.to_string(), reversal.id.to_string()],
                    )
                    .map_err(map_db_error)?;
                    return Err(stateset_core::CommerceError::Conflict(
                        "Journal entry is already reversed".to_string(),
                    ));
                }
            }
            _ => {
                return Err(stateset_core::CommerceError::ValidationError(
                    "Can only reverse posted entries".to_string(),
                ));
            }
        }

        // Create reversing entry with swapped debits/credits
        let reversing_lines: Vec<_> = entry
            .lines
            .iter()
            .map(|l| stateset_core::CreateJournalEntryLine {
                account_id: l.account_id,
                description: Some(format!("Reversal of {}", entry.entry_number)),
                debit_amount: l.credit_amount,
                credit_amount: l.debit_amount,
                reference_type: l.reference_type.clone(),
                reference_id: l.reference_id,
            })
            .collect();

        let reversing_entry = match self.create_journal_entry(stateset_core::CreateJournalEntry {
            entry_date: reversal_date,
            entry_type: Some(JournalEntryType::Reversing),
            description: format!("Reversal of {}", entry.entry_number),
            lines: reversing_lines,
            source_document_type: Some("reversal".to_string()),
            source_document_id: Some(entry.id),
            auto_post: Some(true),
        }) {
            Ok(reversing_entry) => reversing_entry,
            Err(e) => {
                // Best-effort release of the claim so the entry is not left
                // marked reversed without a reversing entry.
                let _ = conn.execute(
                    "UPDATE gl_journal_entries SET status = 'posted' WHERE id = ?1 AND status = 'reversed'",
                    params![id.to_string()],
                );
                return Err(e);
            }
        };

        // Link entries
        conn.execute(
            "UPDATE gl_journal_entries SET reversing_entry_id = ?1 WHERE id = ?2",
            params![reversing_entry.id.to_string(), id.to_string()],
        )
        .map_err(map_db_error)?;

        conn.execute(
            "UPDATE gl_journal_entries SET reversed_entry_id = ?1 WHERE id = ?2",
            params![id.to_string(), reversing_entry.id.to_string()],
        )
        .map_err(map_db_error)?;

        self.get_journal_entry(reversing_entry.id)?.ok_or(stateset_core::CommerceError::NotFound)
    }

    fn get_journal_entry_lines(&self, journal_entry_id: Uuid) -> Result<Vec<JournalEntryLine>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let mut stmt = conn
            .prepare(
                "SELECT id, journal_entry_id, line_number, account_id, account_number,
                    account_name, description, debit_amount, credit_amount, currency,
                    reference_type, reference_id, created_at
             FROM gl_journal_entry_lines WHERE journal_entry_id = ?1 ORDER BY line_number",
            )
            .map_err(map_db_error)?;

        let rows = stmt
            .query_map(params![journal_entry_id.to_string()], Self::map_journal_entry_line_row)
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
        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        match conn.query_row(
            "SELECT id, config_name, cash_account_id, accounts_receivable_account_id,
                    inventory_account_id, accounts_payable_account_id, unearned_revenue_account_id,
                    sales_revenue_account_id, shipping_revenue_account_id, cogs_account_id,
                    bad_debt_expense_account_id, fx_gain_loss_account_id, auto_post_depreciation,
                    auto_post_revenue_recognition, is_active, created_at, updated_at
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
        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;
        let id = Uuid::new_v4();
        let now = Utc::now();

        // Deactivate existing configs
        conn.execute("UPDATE gl_auto_posting_config SET is_active = 0", [])
            .map_err(map_db_error)?;

        conn.execute(
            "INSERT INTO gl_auto_posting_config (id, config_name, cash_account_id,
             accounts_receivable_account_id, inventory_account_id, accounts_payable_account_id,
             unearned_revenue_account_id, sales_revenue_account_id, shipping_revenue_account_id,
             cogs_account_id, bad_debt_expense_account_id, fx_gain_loss_account_id,
             auto_post_depreciation,
             auto_post_revenue_recognition, is_active, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
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
                input.fx_gain_loss_account_id.map(|id| id.to_string()),
                i32::from(input.auto_post_depreciation),
                i32::from(input.auto_post_revenue_recognition),
                1,
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;

        self.get_auto_posting_config()?.ok_or(stateset_core::CommerceError::NotFound)
    }

    fn auto_post_invoice(&self, invoice_id: InvoiceId) -> Result<JournalEntry> {
        // One IMMEDIATE transaction covers the idempotency check, the source
        // document read, and the posted entry: SQLite serializes writers, so
        // the same document can never post twice even under concurrent retry.
        with_immediate_transaction(&self.pool, |tx| {
            if let Some(existing) =
                Self::existing_entry_for_source_with_conn(tx, "invoice", invoice_id.into())?
            {
                return Ok(existing);
            }
            let config = Self::require_auto_posting_config_with_conn(tx)?;
            // The money column is `total` (not `total_amount`), and
            // `invoice_date` is a full RFC3339 timestamp reduced to its date
            // (matching Postgres, which reads a `DateTime<Utc>`).
            let (total, invoice_date): (String, String) = tx.query_row(
                "SELECT total, invoice_date FROM invoices WHERE id = ?1",
                params![invoice_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            let amount = parse_decimal_required(total, 0)?;
            let entry_date = parse_rfc3339_date_with_conn(&invoice_date, "invoice_date")?;
            Self::create_posted_entry_with_conn(
                tx,
                &stateset_core::CreateJournalEntry {
                    entry_date,
                    entry_type: Some(JournalEntryType::Standard),
                    description: format!("Invoice {invoice_id}"),
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
                    source_document_id: Some(invoice_id.into()),
                    auto_post: Some(true),
                },
                "system",
            )
        })
    }

    fn auto_post_payment_received(&self, payment_id: Uuid) -> Result<JournalEntry> {
        with_immediate_transaction(&self.pool, |tx| {
            if let Some(existing) =
                Self::existing_entry_for_source_with_conn(tx, "payment", payment_id)?
            {
                return Ok(existing);
            }
            let config = Self::require_auto_posting_config_with_conn(tx)?;
            // The payment date is `paid_at` (nullable) falling back to
            // `created_at`, stored as full RFC3339 timestamps.
            let (amount_str, payment_date): (String, String) = tx.query_row(
                "SELECT amount, COALESCE(paid_at, created_at) FROM payments WHERE id = ?1",
                params![payment_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            let amount = parse_decimal_required(amount_str, 0)?;
            let entry_date = parse_rfc3339_date_with_conn(&payment_date, "payment date")?;
            Self::create_posted_entry_with_conn(
                tx,
                &stateset_core::CreateJournalEntry {
                    entry_date,
                    entry_type: Some(JournalEntryType::Standard),
                    description: format!("Payment {payment_id}"),
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
                },
                "system",
            )
        })
    }

    fn auto_post_bill(&self, bill_id: Uuid) -> Result<JournalEntry> {
        with_immediate_transaction(&self.pool, |tx| {
            if let Some(existing) = Self::existing_entry_for_source_with_conn(tx, "bill", bill_id)?
            {
                return Ok(existing);
            }
            let config = Self::require_auto_posting_config_with_conn(tx)?;
            // AP bills live in `ap_bills`; `bill_date` is a full RFC3339
            // timestamp reduced to its date.
            let (total, bill_date): (String, String) = tx.query_row(
                "SELECT total_amount, bill_date FROM ap_bills WHERE id = ?1",
                params![bill_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            let amount = parse_decimal_required(total, 0)?;
            let entry_date = parse_rfc3339_date_with_conn(&bill_date, "bill_date")?;
            Self::create_posted_entry_with_conn(
                tx,
                &stateset_core::CreateJournalEntry {
                    entry_date,
                    entry_type: Some(JournalEntryType::Standard),
                    description: format!("Bill {bill_id}"),
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
                },
                "system",
            )
        })
    }

    fn auto_post_bill_payment(&self, payment_id: Uuid) -> Result<JournalEntry> {
        with_immediate_transaction(&self.pool, |tx| {
            if let Some(existing) =
                Self::existing_entry_for_source_with_conn(tx, "bill_payment", payment_id)?
            {
                return Ok(existing);
            }
            let config = Self::require_auto_posting_config_with_conn(tx)?;
            // AP payments live in `ap_payments`; `payment_date` is a full
            // RFC3339 timestamp reduced to its date.
            let (amount_str, payment_date): (String, String) = tx.query_row(
                "SELECT amount, payment_date FROM ap_payments WHERE id = ?1",
                params![payment_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            let amount = parse_decimal_required(amount_str, 0)?;
            let entry_date = parse_rfc3339_date_with_conn(&payment_date, "payment date")?;
            Self::create_posted_entry_with_conn(
                tx,
                &stateset_core::CreateJournalEntry {
                    entry_date,
                    entry_type: Some(JournalEntryType::Standard),
                    description: format!("Bill Payment {payment_id}"),
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
                },
                "system",
            )
        })
    }

    fn auto_post_inventory_cost(&self, cost_transaction_id: Uuid) -> Result<JournalEntry> {
        with_immediate_transaction(&self.pool, |tx| {
            if let Some(existing) = Self::existing_entry_for_source_with_conn(
                tx,
                "cost_transaction",
                cost_transaction_id,
            )? {
                return Ok(existing);
            }
            let config = Self::require_auto_posting_config_with_conn(tx)?;
            // The date column is `created_at`, a full RFC3339 timestamp
            // reduced to its date.
            let (cost_str, created_at, transaction_type): (String, String, String) = tx.query_row(
                "SELECT total_cost, created_at, transaction_type FROM cost_transactions WHERE id = ?1",
                params![cost_transaction_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )?;
            let cost = parse_decimal_required(cost_str, 0)?;
            let entry_date = parse_rfc3339_date_with_conn(&created_at, "created_at")?;
            // An inventory issue/sale moves cost out of Inventory into COGS
            // (debit COGS, credit Inventory); anything else (a receipt) does
            // the reverse. Both `"issue"` and `"sale"` count as issues,
            // matching Postgres.
            let is_issue = transaction_type == "issue" || transaction_type == "sale";
            let (debit_account, credit_account) = if is_issue {
                (config.cogs_account_id, config.inventory_account_id)
            } else {
                (config.inventory_account_id, config.cogs_account_id)
            };
            Self::create_posted_entry_with_conn(
                tx,
                &stateset_core::CreateJournalEntry {
                    entry_date,
                    entry_type: Some(JournalEntryType::Standard),
                    description: format!("Inventory Cost {cost_transaction_id}"),
                    lines: vec![
                        stateset_core::CreateJournalEntryLine::debit(
                            debit_account,
                            cost,
                            Some(if is_issue { "COGS" } else { "Inventory" }.to_string()),
                        ),
                        stateset_core::CreateJournalEntryLine::credit(
                            credit_account,
                            cost,
                            Some(if is_issue { "Inventory" } else { "COGS" }.to_string()),
                        ),
                    ],
                    source_document_type: Some("cost_transaction".to_string()),
                    source_document_id: Some(cost_transaction_id),
                    auto_post: Some(true),
                },
                "system",
            )
        })
    }

    fn auto_post_write_off(&self, write_off_id: Uuid) -> Result<JournalEntry> {
        with_immediate_transaction(&self.pool, |tx| {
            if let Some(existing) =
                Self::existing_entry_for_source_with_conn(tx, "write_off", write_off_id)?
            {
                return Ok(existing);
            }
            let config = Self::require_auto_posting_config_with_conn(tx)?;
            let Some(bad_debt_account) = config.bad_debt_expense_account_id else {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    stateset_core::CommerceError::ValidationError(
                        "Bad debt expense account not configured".to_string(),
                    ),
                )));
            };
            let (amount_str, write_off_date): (String, String) = tx.query_row(
                "SELECT amount, write_off_date FROM ar_write_offs WHERE id = ?1",
                params![write_off_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            let amount = parse_decimal_required(amount_str, 0)?;
            let entry_date: NaiveDate = parse_required(write_off_date, 1)?;
            Self::create_posted_entry_with_conn(
                tx,
                &stateset_core::CreateJournalEntry {
                    entry_date,
                    entry_type: Some(JournalEntryType::Standard),
                    description: format!("Write-off {write_off_id}"),
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
                },
                "system",
            )
        })
    }

    // ========================================================================
    // Financial Reports
    // ========================================================================

    fn get_trial_balance(&self, as_of_date: NaiveDate) -> Result<TrialBalance> {
        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        // Balances are derived from journal lines dated on or before the
        // requested date (posted and reversed entries carry balance effect;
        // draft and voided do not), so the report honors `as_of_date` instead
        // of echoing the live running balance.
        let mut stmt = conn
            .prepare(
                "SELECT a.id, a.account_number, a.name, a.account_type, a.normal_balance,
                    decimal_sum(t.debit_amount) AS debits,
                    decimal_sum(t.credit_amount) AS credits
             FROM gl_accounts a
             LEFT JOIN (
                 SELECT l.account_id, l.debit_amount, l.credit_amount
                 FROM gl_journal_entry_lines l
                 JOIN gl_journal_entries je ON l.journal_entry_id = je.id
                 WHERE je.status IN ('posted', 'reversed') AND je.entry_date <= ?1
             ) t ON t.account_id = a.id
             WHERE a.is_posting = 1 AND a.status = 'active'
             GROUP BY a.id
             ORDER BY a.account_number",
            )
            .map_err(map_db_error)?;

        let rows = stmt
            .query_map(params![as_of_date.to_string()], |row| {
                let debits = parse_decimal_required(row.get::<_, String>(5)?, 5)?;
                let credits = parse_decimal_required(row.get::<_, String>(6)?, 6)?;
                let normal_balance: BalanceSide = parse_required(row.get::<_, String>(4)?, 4)?;
                let balance = match normal_balance {
                    BalanceSide::Credit => credits - debits,
                    _ => debits - credits,
                };

                let (debit_balance, credit_balance) = match normal_balance {
                    BalanceSide::Debit => (balance, Decimal::ZERO),
                    BalanceSide::Credit => (Decimal::ZERO, balance),
                    _ => (balance, Decimal::ZERO),
                };

                Ok(TrialBalanceLine {
                    account_id: parse_required(row.get::<_, String>(0)?, 0)?,
                    account_number: row.get(1)?,
                    account_name: row.get(2)?,
                    account_type: parse_required(row.get::<_, String>(3)?, 3)?,
                    debit_balance,
                    credit_balance,
                })
            })
            .map_err(map_db_error)?;

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
        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let mut assets = Vec::new();
        let mut liabilities = Vec::new();
        let mut equity = Vec::new();
        let mut total_assets = Decimal::ZERO;
        let mut total_liabilities = Decimal::ZERO;
        let mut total_equity = Decimal::ZERO;

        // Same as-of derivation as the trial balance: lines from posted and
        // reversed entries dated on or before the requested date.
        let mut stmt = conn
            .prepare(
                "SELECT a.id, a.account_number, a.name, a.account_type, a.account_sub_type,
                    decimal_sum(t.debit_amount) AS debits,
                    decimal_sum(t.credit_amount) AS credits,
                    a.normal_balance
             FROM gl_accounts a
             LEFT JOIN (
                 SELECT l.account_id, l.debit_amount, l.credit_amount
                 FROM gl_journal_entry_lines l
                 JOIN gl_journal_entries je ON l.journal_entry_id = je.id
                 WHERE je.status IN ('posted', 'reversed') AND je.entry_date <= ?1
             ) t ON t.account_id = a.id
             WHERE a.is_posting = 1 AND a.status = 'active'
               AND a.account_type IN ('asset', 'liability', 'equity')
             GROUP BY a.id
             ORDER BY a.account_number",
            )
            .map_err(map_db_error)?;

        let rows = stmt
            .query_map(params![as_of_date.to_string()], |row| {
                let debits = parse_decimal_required(row.get::<_, String>(5)?, 5)?;
                let credits = parse_decimal_required(row.get::<_, String>(6)?, 6)?;
                let normal_balance: BalanceSide = parse_required(row.get::<_, String>(7)?, 7)?;

                let display_balance = match normal_balance {
                    BalanceSide::Credit => credits - debits,
                    _ => debits - credits,
                };

                Ok((
                    parse_required(row.get::<_, String>(0)?, 0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    parse_required(row.get::<_, String>(3)?, 3)?,
                    parse_optional(row.get::<_, Option<String>>(4)?, 4)?,
                    display_balance,
                ))
            })
            .map_err(map_db_error)?;

        for row in rows {
            let (id, number, name, account_type, sub_type, balance): (
                Uuid,
                String,
                String,
                AccountType,
                Option<AccountSubType>,
                Decimal,
            ) = row.map_err(map_db_error)?;

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

    fn get_income_statement(
        &self,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<IncomeStatement> {
        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let mut revenue_lines = Vec::new();
        let mut expense_lines = Vec::new();
        let mut total_revenue = Decimal::ZERO;
        let mut total_expenses = Decimal::ZERO;

        // Get account activity for the period from posted journal entries
        let mut stmt = conn
            .prepare(
                // `debit_amount` / `credit_amount` are TEXT money columns; the
                // built-in SQL `SUM()` coerces them to float (lossy) and returns
                // a non-TEXT value that the `String` read below rejects. The
                // `decimal_sum` aggregate accumulates exactly and returns TEXT
                // ("0" for accounts with no lines).
                "SELECT a.id, a.account_number, a.name, a.account_type, a.account_sub_type,
                    decimal_sum(l.debit_amount) as total_debits,
                    decimal_sum(l.credit_amount) as total_credits
             FROM gl_accounts a
             LEFT JOIN gl_journal_entry_lines l ON a.id = l.account_id
             LEFT JOIN gl_journal_entries je ON l.journal_entry_id = je.id
             WHERE a.is_posting = 1 AND a.status = 'active'
               AND a.account_type IN ('revenue', 'expense')
               AND (je.status = 'posted' OR je.id IS NULL)
               AND (je.entry_type != 'closing' OR je.id IS NULL)
               AND (je.entry_date >= ?1 AND je.entry_date <= ?2 OR je.id IS NULL)
             GROUP BY a.id
             ORDER BY a.account_number",
            )
            .map_err(map_db_error)?;

        let rows = stmt
            .query_map(params![start_date.to_string(), end_date.to_string()], |row| {
                let total_debits = parse_decimal_required(row.get::<_, String>(5)?, 5)?;
                let total_credits = parse_decimal_required(row.get::<_, String>(6)?, 6)?;
                let account_type: AccountType = parse_required(row.get::<_, String>(3)?, 3)?;

                // Revenue has credit normal balance, expense has debit
                let amount = match account_type {
                    AccountType::Revenue => total_credits - total_debits,
                    AccountType::Expense => total_debits - total_credits,
                    _ => Decimal::ZERO,
                };

                Ok((
                    parse_required(row.get::<_, String>(0)?, 0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    account_type,
                    parse_optional(row.get::<_, Option<String>>(4)?, 4)?,
                    amount,
                ))
            })
            .map_err(map_db_error)?;

        for row in rows {
            let (id, number, name, account_type, sub_type, amount): (
                Uuid,
                String,
                String,
                AccountType,
                Option<AccountSubType>,
                Decimal,
            ) = row.map_err(map_db_error)?;

            if amount == Decimal::ZERO {
                continue;
            }

            let line = IncomeStatementLine {
                account_id: id,
                account_number: number,
                account_name: name,
                account_sub_type: sub_type,
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

    fn get_account_balance(
        &self,
        account_id: Uuid,
        as_of_date: Option<NaiveDate>,
    ) -> Result<Option<Decimal>> {
        let Some(account) = self.get_account(account_id)? else {
            return Ok(None);
        };
        // Without a date the live running balance answers; with one, derive
        // from journal lines dated on or before it (posted and reversed
        // entries carry balance effect; draft and voided do not).
        let Some(as_of) = as_of_date else {
            return Ok(Some(account.current_balance));
        };
        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;
        let (debits, credits): (String, String) = conn
            .query_row(
                "SELECT decimal_sum(l.debit_amount), decimal_sum(l.credit_amount)
                 FROM gl_journal_entry_lines l
                 JOIN gl_journal_entries je ON l.journal_entry_id = je.id
                 WHERE l.account_id = ?1
                   AND je.status IN ('posted', 'reversed') AND je.entry_date <= ?2",
                params![account_id.to_string(), as_of.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(map_db_error)?;
        let debits = parse_decimal_required(debits, 0).map_err(map_db_error)?;
        let credits = parse_decimal_required(credits, 1).map_err(map_db_error)?;
        let balance = match account.normal_balance {
            BalanceSide::Credit => credits - debits,
            _ => debits - credits,
        };
        Ok(Some(balance))
    }

    fn get_account_transactions(
        &self,
        account_id: Uuid,
        filter: JournalEntryFilter,
    ) -> Result<Vec<JournalEntryLine>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

        let mut sql = String::from(
            "SELECT l.id, l.journal_entry_id, l.line_number, l.account_id, l.account_number,
                    l.account_name, l.description, l.debit_amount, l.credit_amount, l.currency,
                    l.reference_type, l.reference_id, l.created_at
             FROM gl_journal_entry_lines l
             JOIN gl_journal_entries je ON l.journal_entry_id = je.id
             WHERE l.account_id = ?1",
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
            sql.push_str(&format!(" LIMIT {limit}"));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt
            .query_map(params_refs.as_slice(), Self::map_journal_entry_line_row)
            .map_err(map_db_error)?;

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
                "Period must be open to close".to_string(),
            ));
        }

        // The income statement excludes closing entries, so a re-close would
        // sweep the same revenue/expense again: while a closing entry stands
        // (not voided), the period cannot be closed a second time.
        if let Some(existing) = self.existing_entry_for_source("period_close", period_id)? {
            return Err(stateset_core::CommerceError::ValidationError(format!(
                "Period already has closing entry {}; void it before re-closing",
                existing.entry_number
            )));
        }

        // Generate income statement for the period
        let income_statement = self.get_income_statement(period.start_date, period.end_date)?;

        // Only create closing entry if there's income-statement activity.
        if income_statement.net_income == Decimal::ZERO
            && income_statement.revenue_lines.iter().all(|l| l.amount == Decimal::ZERO)
            && income_statement.expense_lines.iter().all(|l| l.amount == Decimal::ZERO)
        {
            return Err(stateset_core::CommerceError::ValidationError(
                "No net income to close".to_string(),
            ));
        }

        // Get retained earnings account
        let retained_earnings = self
            .list_accounts(GlAccountFilter {
                account_sub_type: Some(AccountSubType::RetainedEarnings),
                ..Default::default()
            })?
            .into_iter()
            .next()
            .ok_or_else(|| {
                stateset_core::CommerceError::ValidationError(
                    "Retained earnings account not found".to_string(),
                )
            })?;

        // Create closing entry - debit revenue accounts, credit expense accounts
        // and net to retained earnings
        let mut lines = Vec::new();

        for rev in income_statement.revenue_lines {
            // Revenue is credit-normal: a positive balance closes with a
            // debit; a contra-normal (negative) balance closes with a credit.
            let memo = Some(format!("Close {} to Retained Earnings", rev.account_name));
            if rev.amount > Decimal::ZERO {
                lines.push(stateset_core::CreateJournalEntryLine::debit(
                    rev.account_id,
                    rev.amount,
                    memo,
                ));
            } else if rev.amount < Decimal::ZERO {
                lines.push(stateset_core::CreateJournalEntryLine::credit(
                    rev.account_id,
                    rev.amount.abs(),
                    memo,
                ));
            }
        }

        for exp in income_statement.expense_lines {
            // Expenses are debit-normal: a positive balance closes with a
            // credit; a contra-normal balance (e.g. a net FX gain sitting on
            // an expense-type gain/loss account) closes with a debit.
            let memo = Some(format!("Close {} to Retained Earnings", exp.account_name));
            if exp.amount > Decimal::ZERO {
                lines.push(stateset_core::CreateJournalEntryLine::credit(
                    exp.account_id,
                    exp.amount,
                    memo,
                ));
            } else if exp.amount < Decimal::ZERO {
                lines.push(stateset_core::CreateJournalEntryLine::debit(
                    exp.account_id,
                    exp.amount.abs(),
                    memo,
                ));
            }
        }

        // Net to retained earnings (omitted when revenue and expenses offset
        // exactly — the closing lines already balance).
        if income_statement.net_income > Decimal::ZERO {
            lines.push(stateset_core::CreateJournalEntryLine::credit(
                retained_earnings.id,
                income_statement.net_income,
                Some("Net income to Retained Earnings".to_string()),
            ));
        } else if income_statement.net_income < Decimal::ZERO {
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
    // FX Revaluation
    // ========================================================================

    fn revalue(
        &self,
        as_of_date: NaiveDate,
        base_currency: Option<Currency>,
    ) -> Result<RevaluationResult> {
        let base = match base_currency {
            Some(base) => base,
            None => {
                let conn = self
                    .pool
                    .get()
                    .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;
                match conn.query_row(
                    "SELECT base_currency FROM store_currency_settings LIMIT 1",
                    [],
                    |row| row.get::<_, String>(0),
                ) {
                    Ok(code) => code.parse::<Currency>().map_err(|e| {
                        stateset_core::CommerceError::DatabaseError(format!(
                            "Invalid store base currency {code:?}: {e}"
                        ))
                    })?,
                    Err(rusqlite::Error::QueryReturnedNoRows) => Currency::default(),
                    Err(e) => return Err(map_db_error(e)),
                }
            }
        };
        let base_code: stateset_core::CurrencyCode = base.code().parse().map_err(|e| {
            stateset_core::CommerceError::ValidationError(format!(
                "Base currency {} is not a valid ISO code: {e}",
                base.code()
            ))
        })?;
        let base_places = u32::from(base.decimal_places());

        let accounts = self.list_accounts(GlAccountFilter {
            status: Some(AccountStatus::Active),
            is_posting: Some(true),
            ..Default::default()
        })?;

        let mut lines: Vec<RevaluationLine> = Vec::new();
        {
            let conn = self
                .pool
                .get()
                .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))?;

            for account in accounts {
                if account.currency == base_code {
                    continue;
                }

                // Outstanding foreign-currency balance: posted lines excluding
                // prior base-currency FX revaluation adjustments.
                let (debits, credits): (String, String) = conn
                    .query_row(
                        "SELECT decimal_sum(l.debit_amount), decimal_sum(l.credit_amount)
                         FROM gl_journal_entry_lines l
                         JOIN gl_journal_entries je ON l.journal_entry_id = je.id
                         WHERE l.account_id = ?1 AND je.status = 'posted'
                           AND (l.reference_type IS NULL OR l.reference_type != ?2)",
                        params![account.id.to_string(), FX_REVALUATION_REFERENCE],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .map_err(map_db_error)?;
                let debits = parse_decimal_required(debits, 0).map_err(map_db_error)?;
                let credits = parse_decimal_required(credits, 1).map_err(map_db_error)?;
                let foreign_balance = account.balance_effect(debits, credits);

                if foreign_balance.is_zero() && account.current_balance.is_zero() {
                    continue;
                }

                let rate =
                    Self::lookup_rate_with_conn(&conn, account.currency.as_str(), base.code())?
                        .ok_or_else(|| {
                            stateset_core::CommerceError::ValidationError(format!(
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
        }

        let total_unrealized_gain_loss: Decimal =
            lines.iter().map(|l| l.unrealized_gain_loss).sum();

        let journal_entry = if lines.iter().any(|l| !l.adjustment.is_zero()) {
            let fx_account_id = self.resolve_fx_gain_loss_account()?;
            let entry_lines = stateset_core::build_revaluation_journal_lines(&lines, fx_account_id);
            Some(self.create_journal_entry(stateset_core::CreateJournalEntry {
                entry_date: as_of_date,
                entry_type: Some(JournalEntryType::Adjusting),
                description: format!("FX revaluation as of {as_of_date}"),
                lines: entry_lines,
                source_document_type: Some(FX_REVALUATION_REFERENCE.to_string()),
                source_document_id: None,
                auto_post: Some(true),
            })?)
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

    // ========================================================================
    // Batch Operations
    // ========================================================================

    fn create_accounts_batch(
        &self,
        inputs: Vec<CreateGlAccount>,
    ) -> Result<BatchResult<GlAccount>> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SqliteDatabase;
    use chrono::NaiveDate;
    use rust_decimal_macros::dec;
    use stateset_core::{
        AccountType, CommerceError, CreateGlAccount, CreateGlPeriod, CreateJournalEntry,
        CreateJournalEntryLine, GeneralLedgerRepository, GlAccountFilter, GlPeriodFilter,
        JournalEntryFilter, JournalEntryStatus,
    };

    fn fresh_repo() -> SqliteGeneralLedgerRepository {
        SqliteDatabase::in_memory().expect("in-memory").general_ledger()
    }

    fn make_account(repo: &SqliteGeneralLedgerRepository, num: &str, ty: AccountType) -> GlAccount {
        repo.create_account(CreateGlAccount {
            account_number: num.into(),
            name: format!("Account {num}"),
            description: None,
            account_type: ty,
            account_sub_type: None,
            parent_account_id: None,
            is_header: Some(false),
            is_posting: Some(true),
            currency: None,
        })
        .expect("create account")
    }

    fn fy_period(repo: &SqliteGeneralLedgerRepository) -> GlPeriod {
        let p = repo
            .create_period(CreateGlPeriod {
                period_name: "FY2026-01".into(),
                fiscal_year: 2026,
                period_number: 1,
                start_date: NaiveDate::from_ymd_opt(2026, 1, 1).expect("date"),
                end_date: NaiveDate::from_ymd_opt(2026, 1, 31).expect("date"),
            })
            .expect("create period");
        repo.open_period(p.id).expect("open period")
    }

    #[test]
    fn create_account_persists_and_round_trips() {
        let repo = fresh_repo();
        let acct = make_account(&repo, "1000", AccountType::Asset);
        assert_eq!(acct.account_number, "1000");
        let by_id = repo.get_account(acct.id).expect("ok").expect("found");
        assert_eq!(by_id.account_number, "1000");
        let by_num = repo.get_account_by_number("1000").expect("ok").expect("found");
        assert_eq!(by_num.id, acct.id);
        assert!(repo.get_account_by_number("missing").expect("ok").is_none());
    }

    #[test]
    fn list_accounts_filters_by_type() {
        let repo = fresh_repo();
        make_account(&repo, "1000", AccountType::Asset);
        make_account(&repo, "1100", AccountType::Asset);
        make_account(&repo, "2000", AccountType::Liability);

        let assets = repo
            .list_accounts(GlAccountFilter {
                account_type: Some(AccountType::Asset),
                ..Default::default()
            })
            .expect("list");
        assert_eq!(assets.len(), 2);
        assert!(assets.iter().all(|a| a.account_type == AccountType::Asset));
    }

    #[test]
    fn create_period_round_trips() {
        let repo = fresh_repo();
        let period = fy_period(&repo);
        let by_id = repo.get_period(period.id).expect("ok").expect("found");
        assert_eq!(by_id.period_name, "FY2026-01");

        let for_date = repo
            .get_period_for_date(NaiveDate::from_ymd_opt(2026, 1, 15).expect("date"))
            .expect("ok")
            .expect("found");
        assert_eq!(for_date.id, period.id);

        let listed = repo
            .list_periods(GlPeriodFilter { fiscal_year: Some(2026), ..Default::default() })
            .expect("list");
        assert_eq!(listed.len(), 1);
    }

    #[test]
    fn create_journal_entry_starts_in_draft() {
        let repo = fresh_repo();
        let _period = fy_period(&repo);
        let cash = make_account(&repo, "1000", AccountType::Asset);
        let revenue = make_account(&repo, "4000", AccountType::Revenue);

        let entry = repo
            .create_journal_entry(CreateJournalEntry {
                entry_date: NaiveDate::from_ymd_opt(2026, 1, 10).expect("date"),
                entry_type: None,
                description: "Sale".into(),
                lines: vec![
                    CreateJournalEntryLine::debit(cash.id, dec!(100), None),
                    CreateJournalEntryLine::credit(revenue.id, dec!(100), None),
                ],
                source_document_type: None,
                source_document_id: None,
                auto_post: Some(false),
            })
            .expect("create");

        assert_eq!(entry.status, JournalEntryStatus::Draft);
        assert!(entry.is_balanced);
        assert_eq!(entry.total_debits, dec!(100));
        assert_eq!(entry.total_credits, dec!(100));
    }

    #[test]
    fn create_journal_entry_rejects_mixed_debit_credit_line() {
        let repo = fresh_repo();
        let _period = fy_period(&repo);
        let cash = make_account(&repo, "1000", AccountType::Asset);
        let revenue = make_account(&repo, "4000", AccountType::Revenue);

        let err = repo
            .create_journal_entry(CreateJournalEntry {
                entry_date: NaiveDate::from_ymd_opt(2026, 1, 10).expect("date"),
                entry_type: None,
                description: "Bad line".into(),
                lines: vec![CreateJournalEntryLine {
                    account_id: cash.id,
                    description: None,
                    debit_amount: dec!(50),
                    credit_amount: dec!(50),
                    reference_type: None,
                    reference_id: None,
                }],
                source_document_type: None,
                source_document_id: Some(revenue.id),
                auto_post: None,
            })
            .expect_err("err");
        assert!(
            matches!(err, CommerceError::JournalLineNotSingleSided { line_number: 1, .. }),
            "got {err:?}"
        );
        assert_eq!(err.invariant_code(), Some("commerce.ledger.line_not_single_sided"));
    }

    #[test]
    fn post_journal_entry_rejects_unbalanced_entry_and_writes_nothing() {
        let repo = fresh_repo();
        let _period = fy_period(&repo);
        let cash = make_account(&repo, "1000", AccountType::Asset);
        let revenue = make_account(&repo, "4000", AccountType::Revenue);

        // Single-sided lines, but debits != credits: allowed as a draft,
        // refused at posting time.
        let entry = repo
            .create_journal_entry(CreateJournalEntry {
                entry_date: NaiveDate::from_ymd_opt(2026, 1, 10).expect("date"),
                entry_type: None,
                description: "Unbalanced".into(),
                lines: vec![
                    CreateJournalEntryLine::debit(cash.id, dec!(100), None),
                    CreateJournalEntryLine::credit(revenue.id, dec!(50), None),
                ],
                source_document_type: None,
                source_document_id: None,
                auto_post: None,
            })
            .expect("create draft");
        assert!(!entry.is_balanced);

        let err = repo.post_journal_entry(entry.id, "tester").expect_err("must not post");
        assert!(
            matches!(
                &err,
                CommerceError::JournalEntryUnbalanced { entry_id, total_debits, total_credits }
                    if *entry_id == entry.id && total_debits == "100" && total_credits == "50"
            ),
            "got {err:?}"
        );
        assert_eq!(err.invariant_code(), Some("commerce.ledger.entry_unbalanced"));

        // Nothing was written: the entry is still a draft and no account
        // balance moved.
        let reloaded = repo.get_journal_entry(entry.id).expect("ok").expect("found");
        assert_eq!(reloaded.status, JournalEntryStatus::Draft);
        assert!(reloaded.posted_at.is_none());
        assert_eq!(account_balance(&repo, cash.id), dec!(0));
        assert_eq!(account_balance(&repo, revenue.id), dec!(0));
    }

    #[test]
    fn create_journal_entry_without_period_returns_validation_error() {
        let repo = fresh_repo();
        let cash = make_account(&repo, "1000", AccountType::Asset);
        let revenue = make_account(&repo, "4000", AccountType::Revenue);

        let err = repo
            .create_journal_entry(CreateJournalEntry {
                entry_date: NaiveDate::from_ymd_opt(2099, 1, 1).expect("date"),
                entry_type: None,
                description: "no period".into(),
                lines: vec![
                    CreateJournalEntryLine::debit(cash.id, dec!(1), None),
                    CreateJournalEntryLine::credit(revenue.id, dec!(1), None),
                ],
                source_document_type: None,
                source_document_id: None,
                auto_post: None,
            })
            .expect_err("err");
        assert!(matches!(err, CommerceError::ValidationError(_)));
    }

    #[test]
    fn post_journal_entry_marks_posted() {
        let repo = fresh_repo();
        let _period = fy_period(&repo);
        let cash = make_account(&repo, "1000", AccountType::Asset);
        let revenue = make_account(&repo, "4000", AccountType::Revenue);

        let entry = repo
            .create_journal_entry(CreateJournalEntry {
                entry_date: NaiveDate::from_ymd_opt(2026, 1, 10).expect("date"),
                entry_type: None,
                description: "Sale".into(),
                lines: vec![
                    CreateJournalEntryLine::debit(cash.id, dec!(50), None),
                    CreateJournalEntryLine::credit(revenue.id, dec!(50), None),
                ],
                source_document_type: None,
                source_document_id: None,
                auto_post: Some(false),
            })
            .expect("create");

        let posted = repo.post_journal_entry(entry.id, "tester").expect("post");
        assert_eq!(posted.status, JournalEntryStatus::Posted);

        let conn = repo.pool.get().expect("connection");
        let payload: String = conn
            .query_row(
                "SELECT payload FROM kernel_outbox WHERE aggregate_id = ? AND event_type = ?",
                params![entry.id.to_string(), "ledger.journal_entry_posted.v1"],
                |row| row.get(0),
            )
            .expect("posting event");
        let payload: serde_json::Value = serde_json::from_str(&payload).expect("valid payload");
        assert_eq!(payload["total_debits"], "50");
        assert_eq!(payload["total_credits"], "50");
        assert_eq!(payload["posted_by"], "tester");
    }

    #[test]
    fn list_journal_entries_filters_by_status() {
        let repo = fresh_repo();
        let _period = fy_period(&repo);
        let cash = make_account(&repo, "1000", AccountType::Asset);
        let revenue = make_account(&repo, "4000", AccountType::Revenue);

        let draft = repo
            .create_journal_entry(CreateJournalEntry {
                entry_date: NaiveDate::from_ymd_opt(2026, 1, 5).expect("date"),
                entry_type: None,
                description: "Draft entry".into(),
                lines: vec![
                    CreateJournalEntryLine::debit(cash.id, dec!(10), None),
                    CreateJournalEntryLine::credit(revenue.id, dec!(10), None),
                ],
                source_document_type: None,
                source_document_id: None,
                auto_post: Some(false),
            })
            .expect("draft");

        let to_post = repo
            .create_journal_entry(CreateJournalEntry {
                entry_date: NaiveDate::from_ymd_opt(2026, 1, 6).expect("date"),
                entry_type: None,
                description: "Posted entry".into(),
                lines: vec![
                    CreateJournalEntryLine::debit(cash.id, dec!(20), None),
                    CreateJournalEntryLine::credit(revenue.id, dec!(20), None),
                ],
                source_document_type: None,
                source_document_id: None,
                auto_post: Some(false),
            })
            .expect("post");
        repo.post_journal_entry(to_post.id, "tester").expect("post");

        let drafts = repo
            .list_journal_entries(JournalEntryFilter {
                status: Some(JournalEntryStatus::Draft),
                ..Default::default()
            })
            .expect("list");
        let posted = repo
            .list_journal_entries(JournalEntryFilter {
                status: Some(JournalEntryStatus::Posted),
                ..Default::default()
            })
            .expect("list");

        assert!(drafts.iter().any(|e| e.id == draft.id));
        assert!(posted.iter().any(|e| e.id == to_post.id));
    }

    #[test]
    fn close_period_changes_status() {
        let repo = fresh_repo();
        let period = fy_period(&repo); // already open
        assert_eq!(period.status, stateset_core::PeriodStatus::Open);
        let closed = repo.close_period(period.id, "tester").expect("close");
        assert_eq!(closed.status, stateset_core::PeriodStatus::Closed);
    }

    #[test]
    fn open_period_transitions_future_to_open() {
        let repo = fresh_repo();
        let raw = repo
            .create_period(CreateGlPeriod {
                period_name: "FY2026-02".into(),
                fiscal_year: 2026,
                period_number: 2,
                start_date: NaiveDate::from_ymd_opt(2026, 2, 1).expect("date"),
                end_date: NaiveDate::from_ymd_opt(2026, 2, 28).expect("date"),
            })
            .expect("create period");
        assert_eq!(raw.status, stateset_core::PeriodStatus::Future);
        let opened = repo.open_period(raw.id).expect("open");
        assert_eq!(opened.status, stateset_core::PeriodStatus::Open);
    }

    #[test]
    fn create_journal_entry_in_future_period_returns_error() {
        let repo = fresh_repo();
        repo.create_period(CreateGlPeriod {
            period_name: "FY2026-01".into(),
            fiscal_year: 2026,
            period_number: 1,
            start_date: NaiveDate::from_ymd_opt(2026, 1, 1).expect("date"),
            end_date: NaiveDate::from_ymd_opt(2026, 1, 31).expect("date"),
        })
        .expect("create period");
        let cash = make_account(&repo, "1000", AccountType::Asset);
        let revenue = make_account(&repo, "4000", AccountType::Revenue);

        let err = repo
            .create_journal_entry(CreateJournalEntry {
                entry_date: NaiveDate::from_ymd_opt(2026, 1, 5).expect("date"),
                entry_type: None,
                description: "won't post".into(),
                lines: vec![
                    CreateJournalEntryLine::debit(cash.id, dec!(1), None),
                    CreateJournalEntryLine::credit(revenue.id, dec!(1), None),
                ],
                source_document_type: None,
                source_document_id: None,
                auto_post: None,
            })
            .expect_err("err");
        assert!(matches!(err, CommerceError::ValidationError(_)));
    }

    fn make_balanced_entry(
        repo: &SqliteGeneralLedgerRepository,
        debit_account: &GlAccount,
        credit_account: &GlAccount,
        amount: rust_decimal::Decimal,
    ) -> JournalEntry {
        repo.create_journal_entry(CreateJournalEntry {
            entry_date: NaiveDate::from_ymd_opt(2026, 1, 10).expect("date"),
            entry_type: None,
            description: "Concurrency test entry".into(),
            lines: vec![
                CreateJournalEntryLine::debit(debit_account.id, amount, None),
                CreateJournalEntryLine::credit(credit_account.id, amount, None),
            ],
            source_document_type: None,
            source_document_id: None,
            auto_post: Some(false),
        })
        .expect("create entry")
    }

    /// A balanced entry tied to a source document, for the unique-key tests.
    fn make_sourced_entry(
        repo: &SqliteGeneralLedgerRepository,
        debit_account: &GlAccount,
        credit_account: &GlAccount,
        doc_type: &str,
        doc_id: Uuid,
        auto_post: bool,
    ) -> stateset_core::Result<JournalEntry> {
        repo.create_journal_entry(CreateJournalEntry {
            entry_date: NaiveDate::from_ymd_opt(2026, 1, 10).expect("date"),
            entry_type: None,
            description: format!("{doc_type} {doc_id}"),
            lines: vec![
                CreateJournalEntryLine::debit(debit_account.id, dec!(40), None),
                CreateJournalEntryLine::credit(credit_account.id, dec!(40), None),
            ],
            source_document_type: Some(doc_type.to_string()),
            source_document_id: Some(doc_id),
            auto_post: Some(auto_post),
        })
    }

    #[test]
    fn source_document_key_backstop_rejects_duplicates_at_the_database() {
        let repo = fresh_repo();
        let _period = fy_period(&repo);
        let cash = make_account(&repo, "1000", AccountType::Asset);
        let revenue = make_account(&repo, "4000", AccountType::Revenue);
        let doc = Uuid::new_v4();

        // First entry for a single-entry-family document succeeds; a second
        // one is rejected by the unique index even though this path has no
        // application-level idempotency check — the database is the backstop.
        let first =
            make_sourced_entry(&repo, &cash, &revenue, "invoice", doc, true).expect("first");
        let dup = make_sourced_entry(&repo, &cash, &revenue, "invoice", doc, false);
        assert!(dup.is_err(), "duplicate source document must be rejected: {dup:?}");

        // Voiding frees the document for a corrected re-post.
        repo.void_journal_entry(first.id).expect("void");
        make_sourced_entry(&repo, &cash, &revenue, "invoice", doc, false)
            .expect("re-post after void");

        // Multi-entry families (recognition, depreciation) are exempt.
        let ob = Uuid::new_v4();
        make_sourced_entry(&repo, &cash, &revenue, "revenue_recognition", ob, false)
            .expect("first recognition");
        make_sourced_entry(&repo, &cash, &revenue, "revenue_recognition", ob, false)
            .expect("second recognition for the same obligation");
    }

    fn account_balance(repo: &SqliteGeneralLedgerRepository, id: Uuid) -> rust_decimal::Decimal {
        repo.get_account(id).expect("ok").expect("found").current_balance
    }

    #[test]
    fn post_journal_entry_concurrent_posts_apply_balances_once() {
        use std::sync::{Arc, Barrier};
        use std::thread;

        let db = Arc::new(SqliteDatabase::in_memory().expect("in-memory"));
        let repo = db.general_ledger();
        let _period = fy_period(&repo);
        let cash = make_account(&repo, "1000", AccountType::Asset);
        let revenue = make_account(&repo, "4000", AccountType::Revenue);
        let entry = make_balanced_entry(&repo, &cash, &revenue, dec!(100));

        let thread_count = 10;
        let barrier = Arc::new(Barrier::new(thread_count));
        let mut handles = Vec::new();
        for _ in 0..thread_count {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            let entry_id = entry.id;
            handles.push(thread::spawn(move || {
                let repo = db.general_ledger();
                barrier.wait();
                repo.post_journal_entry(entry_id, "racer")
            }));
        }

        let results: Vec<_> = handles.into_iter().map(|h| h.join().expect("thread")).collect();
        let successes = results.iter().filter(|r| r.is_ok()).count();
        // Safety invariant: the entry can be posted AT MOST once, so its lines
        // are applied to the ledger at most once. Under extreme lock contention
        // the sole winner can fail transiently (retryable), so zero successes is
        // acceptable; two or more is the double-post bug this guards against.
        assert!(successes <= 1, "journal entry posted more than once: {results:?}");

        let mult = Decimal::from(successes as u64);
        assert_eq!(
            account_balance(&repo, cash.id),
            dec!(100) * mult,
            "cash must reflect exactly the successful post"
        );
        assert_eq!(
            account_balance(&repo, revenue.id),
            dec!(100) * mult,
            "revenue must reflect exactly the successful post"
        );

        let posted = repo.get_journal_entry(entry.id).expect("ok").expect("found");
        let expected_status =
            if successes == 1 { JournalEntryStatus::Posted } else { JournalEntryStatus::Draft };
        assert_eq!(posted.status, expected_status);
    }

    #[test]
    fn post_journal_entry_rejects_already_posted_and_keeps_balances() {
        let repo = fresh_repo();
        let _period = fy_period(&repo);
        let cash = make_account(&repo, "1000", AccountType::Asset);
        let revenue = make_account(&repo, "4000", AccountType::Revenue);
        let entry = make_balanced_entry(&repo, &cash, &revenue, dec!(40));

        repo.post_journal_entry(entry.id, "tester").expect("first post");
        assert_eq!(account_balance(&repo, cash.id), dec!(40));

        let err = repo.post_journal_entry(entry.id, "again").expect_err("second post must fail");
        assert!(
            matches!(err, CommerceError::ValidationError(_) | CommerceError::Conflict(_)),
            "got {err:?}"
        );

        assert_eq!(account_balance(&repo, cash.id), dec!(40));
        assert_eq!(account_balance(&repo, revenue.id), dec!(40));
    }

    #[test]
    fn void_journal_entry_rejects_already_voided_and_keeps_balances() {
        let repo = fresh_repo();
        let _period = fy_period(&repo);
        let cash = make_account(&repo, "1000", AccountType::Asset);
        let revenue = make_account(&repo, "4000", AccountType::Revenue);
        let entry = make_balanced_entry(&repo, &cash, &revenue, dec!(25));

        repo.post_journal_entry(entry.id, "tester").expect("post");
        repo.void_journal_entry(entry.id).expect("first void");
        assert_eq!(account_balance(&repo, cash.id), dec!(0));

        let err = repo.void_journal_entry(entry.id).expect_err("second void must fail");
        assert!(
            matches!(err, CommerceError::ValidationError(_) | CommerceError::Conflict(_)),
            "got {err:?}"
        );

        assert_eq!(account_balance(&repo, cash.id), dec!(0));
        assert_eq!(account_balance(&repo, revenue.id), dec!(0));
    }

    #[test]
    fn reverse_journal_entry_resumes_a_stranded_claim() {
        let repo = fresh_repo();
        let _period = fy_period(&repo);
        let cash = make_account(&repo, "1000", AccountType::Asset);
        let revenue = make_account(&repo, "4000", AccountType::Revenue);
        let entry = make_balanced_entry(&repo, &cash, &revenue, dec!(60));
        repo.post_journal_entry(entry.id, "tester").expect("post");

        // Simulate a crash between the reversal claim and the reversing
        // entry's creation: status flipped, balances applied, no reversal.
        let conn = repo.pool.get().expect("conn");
        conn.execute(
            "UPDATE gl_journal_entries SET status = 'reversed' WHERE id = ?1",
            params![entry.id.to_string()],
        )
        .expect("simulate stranded claim");
        drop(conn);
        assert_eq!(account_balance(&repo, cash.id), dec!(60));

        // The retry must resume: create the reversing entry and net out.
        let reversal = repo
            .reverse_journal_entry(entry.id, NaiveDate::from_ymd_opt(2026, 1, 16).expect("date"))
            .expect("resume stranded reversal");
        assert_eq!(account_balance(&repo, cash.id), dec!(0));
        assert_eq!(account_balance(&repo, revenue.id), dec!(0));
        let original = repo.get_journal_entry(entry.id).expect("get").expect("entry");
        assert_eq!(original.reversing_entry_id, Some(reversal.id));
        assert_eq!(reversal.reversed_entry_id, Some(entry.id));

        // A further reversal is still rejected.
        let err = repo
            .reverse_journal_entry(entry.id, NaiveDate::from_ymd_opt(2026, 1, 17).expect("date"))
            .expect_err("second reversal must fail");
        assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");
    }

    #[test]
    fn reverse_journal_entry_rejects_second_reversal_and_keeps_balances() {
        let repo = fresh_repo();
        let _period = fy_period(&repo);
        let cash = make_account(&repo, "1000", AccountType::Asset);
        let revenue = make_account(&repo, "4000", AccountType::Revenue);
        let entry = make_balanced_entry(&repo, &cash, &revenue, dec!(60));

        repo.post_journal_entry(entry.id, "tester").expect("post");
        repo.reverse_journal_entry(entry.id, NaiveDate::from_ymd_opt(2026, 1, 15).expect("date"))
            .expect("first reversal");
        assert_eq!(account_balance(&repo, cash.id), dec!(0));

        let err = repo
            .reverse_journal_entry(entry.id, NaiveDate::from_ymd_opt(2026, 1, 16).expect("date"))
            .expect_err("second reversal must fail");
        assert!(
            matches!(err, CommerceError::ValidationError(_) | CommerceError::Conflict(_)),
            "got {err:?}"
        );

        assert_eq!(account_balance(&repo, cash.id), dec!(0));
        assert_eq!(account_balance(&repo, revenue.id), dec!(0));

        let reversals = repo
            .list_journal_entries(JournalEntryFilter {
                source_document_id: Some(entry.id),
                ..Default::default()
            })
            .expect("list");
        assert_eq!(reversals.len(), 1, "only one reversing entry may exist");
    }

    // ========================================================================
    // FX revaluation
    // ========================================================================

    fn make_currency_account(
        repo: &SqliteGeneralLedgerRepository,
        num: &str,
        ty: AccountType,
        currency: stateset_core::CurrencyCode,
        sub_type: Option<AccountSubType>,
    ) -> GlAccount {
        repo.create_account(CreateGlAccount {
            account_number: num.into(),
            name: format!("Account {num}"),
            description: None,
            account_type: ty,
            account_sub_type: sub_type,
            parent_account_id: None,
            is_header: Some(false),
            is_posting: Some(true),
            currency: Some(currency),
        })
        .expect("create account")
    }

    fn set_rate(db: &SqliteDatabase, from: Currency, to: Currency, rate: rust_decimal::Decimal) {
        use stateset_core::CurrencyRepository;
        db.currency()
            .set_rate(stateset_core::SetExchangeRate {
                base_currency: from,
                quote_currency: to,
                rate,
                source: Some("test".into()),
            })
            .expect("set rate");
    }

    #[test]
    fn revalue_posts_balanced_gain_entry_for_foreign_account() {
        let db = SqliteDatabase::in_memory().expect("in-memory");
        let repo = db.general_ledger();
        let _period = fy_period(&repo);
        let eur_cash = make_currency_account(
            &repo,
            "1015",
            AccountType::Asset,
            stateset_core::CurrencyCode::EUR,
            None,
        );
        let revenue = make_account(&repo, "4000", AccountType::Revenue);
        // Fallback FX gain/loss account (no auto-posting config set).
        let fx = make_currency_account(
            &repo,
            "7900",
            AccountType::Expense,
            stateset_core::CurrencyCode::USD,
            Some(AccountSubType::OtherExpense),
        );

        // Book 1000 EUR at parity, then move the rate to 1.10.
        set_rate(&db, Currency::EUR, Currency::USD, dec!(1));
        let entry = make_balanced_entry(&repo, &eur_cash, &revenue, dec!(1000));
        repo.post_journal_entry(entry.id, "tester").expect("post");
        set_rate(&db, Currency::EUR, Currency::USD, dec!(1.10));

        let result = repo
            .revalue(NaiveDate::from_ymd_opt(2026, 1, 31).expect("date"), None)
            .expect("revalue");

        assert_eq!(result.base_currency, stateset_core::CurrencyCode::USD);
        assert_eq!(result.total_unrealized_gain_loss, dec!(100.00));
        assert_eq!(result.lines.len(), 1);
        let line = &result.lines[0];
        assert_eq!(line.account_id, eur_cash.id);
        assert_eq!(line.foreign_balance, dec!(1000));
        assert_eq!(line.revalued_value, dec!(1100.00));
        assert_eq!(line.adjustment, dec!(100.00));

        let je = result.journal_entry.expect("journal entry");
        assert_eq!(je.status, JournalEntryStatus::Posted);
        assert!(je.is_balanced);
        assert_eq!(je.total_debits, dec!(100.00));
        assert_eq!(je.total_credits, dec!(100.00));
        assert!(
            je.lines.iter().any(|l| l.account_id == eur_cash.id && l.debit_amount == dec!(100.00))
        );
        assert!(je.lines.iter().any(|l| l.account_id == fx.id && l.credit_amount == dec!(100.00)));

        // Carrying value now reflects the as-of rate.
        assert_eq!(account_balance(&repo, eur_cash.id), dec!(1100.00));

        // Re-running at the same rate is a no-op (idempotent).
        let again = repo
            .revalue(NaiveDate::from_ymd_opt(2026, 1, 31).expect("date"), None)
            .expect("revalue again");
        assert!(again.journal_entry.is_none());
        assert_eq!(again.total_unrealized_gain_loss, dec!(0.00));
        assert_eq!(account_balance(&repo, eur_cash.id), dec!(1100.00));
    }

    #[test]
    fn revalue_posts_loss_entry_when_rate_drops() {
        let db = SqliteDatabase::in_memory().expect("in-memory");
        let repo = db.general_ledger();
        let _period = fy_period(&repo);
        let eur_cash = make_currency_account(
            &repo,
            "1015",
            AccountType::Asset,
            stateset_core::CurrencyCode::EUR,
            None,
        );
        let revenue = make_account(&repo, "4000", AccountType::Revenue);
        let fx = make_currency_account(
            &repo,
            "7900",
            AccountType::Expense,
            stateset_core::CurrencyCode::USD,
            Some(AccountSubType::OtherExpense),
        );

        set_rate(&db, Currency::EUR, Currency::USD, dec!(1));
        let entry = make_balanced_entry(&repo, &eur_cash, &revenue, dec!(1000));
        repo.post_journal_entry(entry.id, "tester").expect("post");
        set_rate(&db, Currency::EUR, Currency::USD, dec!(0.85));

        let result = repo
            .revalue(NaiveDate::from_ymd_opt(2026, 1, 31).expect("date"), None)
            .expect("revalue");

        assert_eq!(result.total_unrealized_gain_loss, dec!(-150.00));
        let je = result.journal_entry.expect("journal entry");
        assert!(je.is_balanced);
        assert!(
            je.lines.iter().any(|l| l.account_id == eur_cash.id && l.credit_amount == dec!(150.00))
        );
        assert!(je.lines.iter().any(|l| l.account_id == fx.id && l.debit_amount == dec!(150.00)));
        assert_eq!(account_balance(&repo, eur_cash.id), dec!(850.00));
    }

    #[test]
    fn revalue_errors_without_fx_account_or_rate() {
        let db = SqliteDatabase::in_memory().expect("in-memory");
        let repo = db.general_ledger();
        let _period = fy_period(&repo);
        let eur_cash = make_currency_account(
            &repo,
            "1015",
            AccountType::Asset,
            stateset_core::CurrencyCode::EUR,
            None,
        );
        let revenue = make_account(&repo, "4000", AccountType::Revenue);
        let entry = make_balanced_entry(&repo, &eur_cash, &revenue, dec!(100));
        repo.post_journal_entry(entry.id, "tester").expect("post");

        // No exchange rate configured for EUR -> USD.
        let err = repo
            .revalue(NaiveDate::from_ymd_opt(2026, 1, 31).expect("date"), None)
            .expect_err("missing rate");
        assert!(matches!(err, CommerceError::ValidationError(_)));

        // Rate configured, but no FX gain/loss account resolvable.
        set_rate(&db, Currency::EUR, Currency::USD, dec!(1.10));
        let err = repo
            .revalue(NaiveDate::from_ymd_opt(2026, 1, 31).expect("date"), None)
            .expect_err("missing fx account");
        assert!(matches!(err, CommerceError::ValidationError(_)));
    }
}
